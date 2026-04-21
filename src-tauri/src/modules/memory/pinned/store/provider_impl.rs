//! `PinnedStore` trait implementation for `SqlitePinnedStore`.
//!
//! Extracted from `pinned/store/mod.rs` in GFR-T1-H-1 (pure structural
//! move; function bodies + SQL strings byte-identical).

use async_trait::async_trait;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use ulid::Ulid;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::pinned::types::{PinScope, PinSource, PinnedItem};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::MemoryError;

use super::PinnedStore;
use super::{SqlitePinnedStore, MAX_PINS_PER_SCOPE, MAX_PIN_CONTENT_CHARS, SELECT_COLUMNS};

#[async_trait]
impl PinnedStore for SqlitePinnedStore {
    async fn list(
        &self,
        scope: PinScope,
        project_id: Option<&str>,
    ) -> Result<Vec<PinnedItem>, MemoryError> {
        let db = self.db.clone();
        let project = project_id.map(str::to_string);
        tokio::task::spawn_blocking(move || -> Result<Vec<PinnedItem>, MemoryError> {
            let conn = db
                .lock()
                .map_err(|e| MemoryError::Generic(format!("pinned db mutex poisoned: {e}")))?;
            let rows: Vec<PinnedItem> = match (scope, project) {
                (PinScope::Project, Some(p)) => {
                    let sql = format!(
                        "SELECT {SELECT_COLUMNS} FROM pinned_items \
                         WHERE scope = 'project' AND project_id = ?1 \
                         ORDER BY created_at ASC"
                    );
                    let mut stmt = conn.prepare(&sql).map_err(MemoryError::from)?;
                    let it = stmt
                        .query_map(params![p], SqlitePinnedStore::row_to_item)
                        .map_err(MemoryError::from)?;
                    it.collect::<Result<Vec<_>, _>>()
                        .map_err(MemoryError::from)?
                }
                (PinScope::Project, None) => {
                    let sql = format!(
                        "SELECT {SELECT_COLUMNS} FROM pinned_items \
                         WHERE scope = 'project' AND project_id IS NULL \
                         ORDER BY created_at ASC"
                    );
                    let mut stmt = conn.prepare(&sql).map_err(MemoryError::from)?;
                    let it = stmt
                        .query_map([], SqlitePinnedStore::row_to_item)
                        .map_err(MemoryError::from)?;
                    it.collect::<Result<Vec<_>, _>>()
                        .map_err(MemoryError::from)?
                }
                (PinScope::Global, _) => {
                    let sql = format!(
                        "SELECT {SELECT_COLUMNS} FROM pinned_items \
                         WHERE scope = 'global' AND project_id IS NULL \
                         ORDER BY created_at ASC"
                    );
                    let mut stmt = conn.prepare(&sql).map_err(MemoryError::from)?;
                    let it = stmt
                        .query_map([], SqlitePinnedStore::row_to_item)
                        .map_err(MemoryError::from)?;
                    it.collect::<Result<Vec<_>, _>>()
                        .map_err(MemoryError::from)?
                }
            };
            Ok(rows)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("pinned list join error: {e}")))?
    }

    async fn list_all_for_prompt(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<PinnedItem>, MemoryError> {
        let mut out = self.list(PinScope::Global, None).await?;
        if let Some(pid) = scope.project_id.as_deref() {
            let proj = self.list(PinScope::Project, Some(pid)).await?;
            out.extend(proj);
        }
        Ok(out)
    }

    async fn add(
        &self,
        content: &str,
        scope: PinScope,
        source: PinSource,
        project_id: Option<&str>,
    ) -> Result<PinnedItem, MemoryError> {
        // Step 1 — length guard (v2 §0.5 Δ-17).
        let char_count = content.chars().count();
        if char_count > MAX_PIN_CONTENT_CHARS {
            return Err(MemoryError::PinnedContentTooLong {
                got: char_count,
                limit: MAX_PIN_CONTENT_CHARS,
            });
        }

        // Step 2 — PII scrub (v2 §0.5 Δ-2).  When no scanner is wired,
        // pass content through verbatim so unit tests / Null fallback
        // still work.
        let cleaned = match self.scanner.as_deref() {
            Some(scanner) => {
                let report = scanner.scan_and_redact("pin", content);
                if report.flagged {
                    let exec_scope = MemoryExecutionScope {
                        session_id: match &source {
                            PinSource::Tool { session_id, .. } => Some(session_id.clone()),
                            PinSource::User => None,
                        },
                        project_id: project_id.map(str::to_string),
                        workdir: None,
                    };
                    let ctx = AuditContext::from_scope(&exec_scope);
                    MemoryAuditEmitter::memory_pii_redacted(&ctx, "pin", &report.detected);
                }
                report.cleaned
            }
            None => content.to_string(),
        };

        let normalized_project = match scope {
            PinScope::Project => project_id.map(str::to_string),
            PinScope::Global => None,
        };

        // Steps 3–5 happen under the SQLite mutex on a blocking thread.
        let db = self.db.clone();
        let cleaned_for_db = cleaned.clone();
        let normalized_project_for_db = normalized_project.clone();
        let source_for_db = source.clone();

        let outcome: AddOutcome =
            tokio::task::spawn_blocking(move || -> Result<AddOutcome, MemoryError> {
                let conn = db
                    .lock()
                    .map_err(|e| MemoryError::Generic(format!("pinned db mutex poisoned: {e}")))?;

                // Step 3 — case-insensitive trimmed dedup.
                let dedup_sql = "SELECT id FROM pinned_items WHERE \
                    TRIM(LOWER(content)) = TRIM(LOWER(?1)) \
                    AND scope = ?2 \
                    AND COALESCE(project_id, '') = COALESCE(?3, '') \
                    LIMIT 1";
                let existing_id: Option<String> = conn
                    .query_row(
                        dedup_sql,
                        params![
                            &cleaned_for_db,
                            scope.as_str(),
                            &normalized_project_for_db
                        ],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(MemoryError::from)?;
                if let Some(id) = existing_id {
                    let sql = format!(
                        "SELECT {SELECT_COLUMNS} FROM pinned_items WHERE id = ?1"
                    );
                    let item = conn
                        .query_row(&sql, params![id], SqlitePinnedStore::row_to_item)
                        .map_err(MemoryError::from)?;
                    let total = SqlitePinnedStore::count_in_bucket(
                        &conn,
                        scope,
                        normalized_project_for_db.as_deref(),
                    )?;
                    return Ok(AddOutcome::Existing { item, total });
                }

                // Step 4 — limit guard (count rows in same bucket).
                let count = SqlitePinnedStore::count_in_bucket(
                    &conn,
                    scope,
                    normalized_project_for_db.as_deref(),
                )?;
                if count >= MAX_PINS_PER_SCOPE {
                    return Err(MemoryError::PinnedLimitExceeded(MAX_PINS_PER_SCOPE));
                }

                // Step 5 — INSERT with fresh ULID + serialised provenance.
                let id = Ulid::new().to_string();
                let created_at = Utc::now();
                let (kind, meta) = match &source_for_db {
                    PinSource::User => ("user".to_string(), "{}".to_string()),
                    PinSource::Tool {
                        tool_name,
                        session_id,
                    } => {
                        let payload = serde_json::json!({
                            "tool_name": tool_name,
                            "session_id": session_id,
                        });
                        ("tool".to_string(), payload.to_string())
                    }
                };
                conn.execute(
                    "INSERT INTO pinned_items \
                        (id, content, scope, project_id, created_at, created_by_kind, created_by_meta) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        id,
                        cleaned_for_db,
                        scope.as_str(),
                        normalized_project_for_db,
                        created_at.to_rfc3339(),
                        kind,
                        meta,
                    ],
                )
                .map_err(MemoryError::from)?;

                let item = PinnedItem {
                    id,
                    content: cleaned_for_db,
                    scope,
                    project_id: normalized_project_for_db.clone(),
                    created_at,
                    created_by: source_for_db,
                };
                let total = count + 1;
                Ok(AddOutcome::Inserted { item, total })
            })
            .await
            .map_err(|e| MemoryError::Generic(format!("pinned add join error: {e}")))??;

        let (item, total, inserted) = match outcome {
            AddOutcome::Existing { item, total } => (item, total, false),
            AddOutcome::Inserted { item, total } => (item, total, true),
        };

        // Step 6 — sidecar rewrite (only when SQL state actually
        // changed; dedup hits leave the file untouched).
        if inserted {
            self.rewrite_sidecar(scope, normalized_project.as_deref())
                .await?;
        }

        // Step 7 — audit emit (always — even on dedup so the UI can
        // surface the user's intent).
        let exec_scope = MemoryExecutionScope {
            session_id: match &item.created_by {
                PinSource::Tool { session_id, .. } => Some(session_id.clone()),
                PinSource::User => None,
            },
            project_id: item.project_id.clone(),
            workdir: None,
        };
        let ctx = AuditContext::from_scope(&exec_scope);
        let excerpt: String = item.content.chars().take(64).collect();
        MemoryAuditEmitter::memory_pinned(&ctx, scope.as_str(), &excerpt, total);

        Ok(item)
    }

    async fn delete(&self, id: &str) -> Result<bool, MemoryError> {
        let db = self.db.clone();
        let id_owned = id.to_string();
        let removed_with_meta: Option<(String, Option<String>)> = tokio::task::spawn_blocking(
            move || -> Result<Option<(String, Option<String>)>, MemoryError> {
                let conn = db
                    .lock()
                    .map_err(|e| MemoryError::Generic(format!("pinned db mutex poisoned: {e}")))?;
                let row: Option<(String, Option<String>)> = conn
                    .query_row(
                        "SELECT scope, project_id FROM pinned_items WHERE id = ?1",
                        params![id_owned],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
                    )
                    .optional()
                    .map_err(MemoryError::from)?;
                if row.is_none() {
                    return Ok(None);
                }
                let n = conn
                    .execute("DELETE FROM pinned_items WHERE id = ?1", params![id_owned])
                    .map_err(MemoryError::from)?;
                if n == 0 {
                    Ok(None)
                } else {
                    Ok(row)
                }
            },
        )
        .await
        .map_err(|e| MemoryError::Generic(format!("pinned delete join error: {e}")))??;

        let Some((scope_s, project_id)) = removed_with_meta else {
            return Ok(false);
        };
        let scope = PinScope::from_str_lossy(&scope_s);
        self.rewrite_sidecar(scope, project_id.as_deref()).await?;

        let exec_scope = MemoryExecutionScope {
            session_id: None,
            project_id: project_id.clone(),
            workdir: None,
        };
        let ctx = AuditContext::from_scope(&exec_scope);
        MemoryAuditEmitter::memory_unpinned(&ctx, scope.as_str(), 1, id);

        Ok(true)
    }

    async fn reorder(&self, ids: &[String]) -> Result<(), MemoryError> {
        if ids.is_empty() {
            return Ok(());
        }
        let db = self.db.clone();
        let ids_owned: Vec<String> = ids.to_vec();
        let touched: Vec<(PinScope, Option<String>)> = tokio::task::spawn_blocking(
            move || -> Result<Vec<(PinScope, Option<String>)>, MemoryError> {
                let conn = db
                    .lock()
                    .map_err(|e| MemoryError::Generic(format!("pinned db mutex poisoned: {e}")))?;
                let base = Utc::now();
                let mut buckets: Vec<(PinScope, Option<String>)> = Vec::new();
                for (i, id) in ids_owned.iter().enumerate() {
                    let row: Option<(String, Option<String>)> = conn
                        .query_row(
                            "SELECT scope, project_id FROM pinned_items WHERE id = ?1",
                            params![id],
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
                        )
                        .optional()
                        .map_err(MemoryError::from)?;
                    let Some((scope_s, project_id)) = row else {
                        continue;
                    };
                    let stamp = base + chrono::Duration::milliseconds(i as i64);
                    conn.execute(
                        "UPDATE pinned_items SET created_at = ?1 WHERE id = ?2",
                        params![stamp.to_rfc3339(), id],
                    )
                    .map_err(MemoryError::from)?;
                    let bucket = (PinScope::from_str_lossy(&scope_s), project_id);
                    if !buckets.contains(&bucket) {
                        buckets.push(bucket);
                    }
                }
                Ok(buckets)
            },
        )
        .await
        .map_err(|e| MemoryError::Generic(format!("pinned reorder join error: {e}")))??;

        for (scope, project_id) in touched {
            self.rewrite_sidecar(scope, project_id.as_deref()).await?;
        }
        Ok(())
    }
}

enum AddOutcome {
    Existing { item: PinnedItem, total: usize },
    Inserted { item: PinnedItem, total: usize },
}
