//! `PinnedStore` — persistence + scrub coordinator for the
//! pinned-memory subsystem.
//!
//! Per `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-F1 + §0.5 Δ-2 + Δ-6 + Δ-17, every [`PinnedStore::add`]
//! is a 7-step pipeline:
//!
//! 1. **Length guard** — reject inputs over [`MAX_PIN_CONTENT_CHARS`].
//! 2. **PII scrub** — run [`ThreatScanner::scan_and_redact`] when a
//!    scanner is wired so a `pin_memory` tool call from the LLM cannot
//!    persist a secret in plain text; emit `memory_pii_redacted` when
//!    the scanner fires.
//! 3. **Dedup** — case-insensitive trimmed match on `(content, scope,
//!    project_id)`; on hit, return the existing [`PinnedItem`] without
//!    a second INSERT.
//! 4. **Limit guard** — count current rows in the same `(scope,
//!    project_id)` bucket; reject when the count is already at
//!    [`MAX_PINS_PER_SCOPE`].
//! 5. **INSERT** — UPSERT into the `pinned_items` table with a freshly
//!    minted ULID.
//! 6. **Sidecar rewrite** — re-render `<scope_root>/pinned.md` from the
//!    SQL state via `tmp + rename` so a crash mid-write never leaves a
//!    half-written sidecar (v2 §0.5 Δ-6 — sidecar is best-effort but
//!    must never be torn).
//! 7. **Audit emit** — fire `memory_pinned` carrying the post-insert
//!    pin count so the Telemetry Drawer can render "已固定 N 条".
//!
//! ## Scope contract
//!
//! - `PinScope::Project` rows MUST carry a `project_id`; `list()`
//!   filters by `(scope = 'project' AND project_id = ?)`.
//! - `PinScope::Global` rows have `project_id = NULL`; `list()` for
//!   `scope = Global` filters `project_id IS NULL`.
//! - [`PinnedStore::list_all_for_prompt`] returns
//!   `Global ∪ matching-Project` so the system-prompt injector
//!   (8A.11) gets one merged set in a stable order.
//!
//! ## Hard limits (v2 §0.5 Δ-17)
//!
//! - [`MAX_PINS_PER_SCOPE`] = 50 pins per `(scope, project_id)` bucket
//!   (Global counted separately from each project).
//! - [`MAX_PIN_CONTENT_CHARS`] = 500 characters per pin.
//!
//! ## Single scope_root simplification
//!
//! Today every pin renders into one `<scope_root>/pinned.md` file,
//! regardless of project vs global tier.  v2 §0.5 Δ-6 explicitly
//! defers the multi-root layout (Global at `<data_local>/.if2ai/...`
//! vs Project at `<workdir>/.if2ai/...`) to a later slice — see the
//! TODO inside [`SqlitePinnedStore::rewrite_sidecar`].

#![allow(dead_code)] // first production consumer lands in 8A.10 (pin/unpin tools)

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use ulid::Ulid;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::pinned::types::{PinScope, PinSource, PinnedItem};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::security::ThreatScanner;
use crate::modules::memory::MemoryError;

/// Per-scope pin cap (v2 §0.5 Δ-17).
pub const MAX_PINS_PER_SCOPE: usize = 50;
/// Per-pin character cap (v2 §0.5 Δ-17).
pub const MAX_PIN_CONTENT_CHARS: usize = 500;

/// Storage seam for [`PinnedItem`].  Lives behind
/// `Arc<dyn PinnedStore>` on `AppState` so other subsystems can be
/// tested with [`NullPinnedStore`] without touching SQLite.
#[async_trait]
pub trait PinnedStore: Send + Sync {
    /// Returns the pins visible to the given `(scope, project_id)`
    /// bucket, ordered by `created_at` ASC (oldest first — matches the
    /// markdown sidecar order).
    ///
    /// `scope` choice:
    /// - [`PinScope::Project`] + `project_id = Some(p)` → only that
    ///   project's pins.
    /// - [`PinScope::Global`] (`project_id` is ignored) → only global
    ///   pins.
    async fn list(
        &self,
        scope: PinScope,
        project_id: Option<&str>,
    ) -> Result<Vec<PinnedItem>, MemoryError>;

    /// Returns the union of `Global` pins + the active project's pins,
    /// global first then project, both ordered by `created_at` ASC so
    /// the system-prompt injector (8A.11) gets a stable rendering
    /// order.
    async fn list_all_for_prompt(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<PinnedItem>, MemoryError>;

    /// Insert a pin (or return the existing item on dedup).  Enforces
    /// [`MAX_PIN_CONTENT_CHARS`], [`MAX_PINS_PER_SCOPE`], and PII
    /// scrub.  Always emits `memory_pinned`; emits `memory_pii_redacted`
    /// when the scanner fires.
    async fn add(
        &self,
        content: &str,
        scope: PinScope,
        source: PinSource,
        project_id: Option<&str>,
    ) -> Result<PinnedItem, MemoryError>;

    /// Delete one pin by id.  Returns `Ok(true)` when a row was removed,
    /// `Ok(false)` when the id was not present (idempotent).
    async fn delete(&self, id: &str) -> Result<bool, MemoryError>;

    /// Re-stamp `created_at` for each id in `ids` so the natural
    /// `ORDER BY created_at ASC` matches the supplied order.  Ids not
    /// in the store are silently skipped.
    async fn reorder(&self, ids: &[String]) -> Result<(), MemoryError>;
}

/// SQLite-backed [`PinnedStore`] with a markdown sidecar at
/// `<scope_root>/pinned.md`.  Construct via [`Self::open`] from
/// `main.rs::run`.
pub struct SqlitePinnedStore {
    db: Arc<Mutex<Connection>>,
    scope_root: PathBuf,
    scanner: Option<Arc<ThreatScanner>>,
}

const SELECT_COLUMNS: &str =
    "id, content, scope, project_id, created_at, created_by_kind, created_by_meta";

impl SqlitePinnedStore {
    /// Open (or create) the pinned store backed by `db_path`.  Creates
    /// parent directories + the `pinned_items` table + indexes; the
    /// markdown sidecar lives at `scope_root.join("pinned.md")`.
    pub fn open(
        db_path: &Path,
        scope_root: PathBuf,
        scanner: Option<Arc<ThreatScanner>>,
    ) -> Result<Self, MemoryError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                MemoryError::Generic(format!("failed to create pinned db parent {parent:?}: {e}"))
            })?;
        }
        std::fs::create_dir_all(&scope_root).map_err(|e| {
            MemoryError::Generic(format!(
                "failed to create pinned scope_root {scope_root:?}: {e}"
            ))
        })?;

        let conn = Connection::open(db_path).map_err(|e| {
            MemoryError::Generic(format!("failed to open pinned db {db_path:?}: {e}"))
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pinned_items (
                id              TEXT PRIMARY KEY,
                content         TEXT NOT NULL,
                scope           TEXT NOT NULL,
                project_id      TEXT,
                created_at      TEXT NOT NULL,
                created_by_kind TEXT NOT NULL,
                created_by_meta TEXT NOT NULL DEFAULT '{}'
            );
            CREATE INDEX IF NOT EXISTS idx_pinned_scope ON pinned_items(scope);
            CREATE INDEX IF NOT EXISTS idx_pinned_project ON pinned_items(project_id);
            CREATE INDEX IF NOT EXISTS idx_pinned_created ON pinned_items(created_at);",
        )
        .map_err(|e| MemoryError::Generic(format!("failed to init pinned_items schema: {e}")))?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            scope_root,
            scanner,
        })
    }

    fn row_to_item(row: &rusqlite::Row<'_>) -> Result<PinnedItem, rusqlite::Error> {
        let id: String = row.get(0)?;
        let content: String = row.get(1)?;
        let scope_s: String = row.get(2)?;
        let project_id: Option<String> = row.get(3)?;
        let created_at_s: String = row.get(4)?;
        let kind: String = row.get(5)?;
        let meta: String = row.get(6)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    4,
                    "datetime".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;

        let created_by = match kind.as_str() {
            "tool" => {
                #[derive(serde::Deserialize)]
                struct ToolMeta {
                    tool_name: String,
                    session_id: String,
                }
                serde_json::from_str::<ToolMeta>(&meta)
                    .map(|m| PinSource::Tool {
                        tool_name: m.tool_name,
                        session_id: m.session_id,
                    })
                    .unwrap_or(PinSource::User)
            }
            _ => PinSource::User,
        };

        Ok(PinnedItem {
            id,
            content,
            scope: PinScope::from_str_lossy(&scope_s),
            project_id,
            created_at,
            created_by,
        })
    }

    /// Re-render the markdown sidecar from the current SQL state.
    ///
    /// Best-effort: a failure here is propagated as [`MemoryError`] so
    /// the caller can log + decide.  Atomicity is guaranteed via a
    /// `tmp + rename` pair so a crash mid-write never leaves a torn
    /// sidecar.
    ///
    /// TODO(8B): split per-scope into distinct files (Global at
    /// `<data_local>/.if2ai/memory/pinned.md`, Project at
    /// `<workdir>/.if2ai/memory/pinned.md`) once multi-scope-root
    /// resolution lands.  Today both scopes share one sidecar to keep
    /// the slice small.
    async fn rewrite_sidecar(
        &self,
        scope: PinScope,
        project_id: Option<&str>,
    ) -> Result<(), MemoryError> {
        let pins = self.list(scope, project_id).await?;
        let header = match scope {
            PinScope::Global => "# Pinned (Global)\n\n",
            PinScope::Project => "# Pinned (Project)\n\n",
        };
        let body = pins
            .iter()
            .map(|p| format!("- {}", p.content.replace('\n', " ")))
            .collect::<Vec<_>>()
            .join("\n");
        let payload = if body.is_empty() {
            header.to_string()
        } else {
            format!("{header}{body}\n")
        };

        let scope_root = self.scope_root.clone();
        tokio::task::spawn_blocking(move || -> Result<(), MemoryError> {
            let path = scope_root.join("pinned.md");
            let tmp = scope_root.join("pinned.md.tmp");
            std::fs::write(&tmp, payload.as_bytes()).map_err(|e| {
                MemoryError::Generic(format!("write pinned.md.tmp at {tmp:?}: {e}"))
            })?;
            std::fs::rename(&tmp, &path).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                MemoryError::Generic(format!("rename pinned.md at {path:?}: {e}"))
            })?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("pinned sidecar join error: {e}")))?
    }

    fn count_in_bucket(
        conn: &Connection,
        scope: PinScope,
        project_id: Option<&str>,
    ) -> Result<usize, MemoryError> {
        let n: i64 = match (scope, project_id) {
            (PinScope::Project, Some(p)) => conn
                .query_row(
                    "SELECT COUNT(*) FROM pinned_items \
                     WHERE scope = 'project' AND project_id = ?1",
                    params![p],
                    |r| r.get(0),
                )
                .map_err(MemoryError::from)?,
            (PinScope::Project, None) => conn
                .query_row(
                    "SELECT COUNT(*) FROM pinned_items \
                     WHERE scope = 'project' AND project_id IS NULL",
                    [],
                    |r| r.get(0),
                )
                .map_err(MemoryError::from)?,
            (PinScope::Global, _) => conn
                .query_row(
                    "SELECT COUNT(*) FROM pinned_items \
                     WHERE scope = 'global' AND project_id IS NULL",
                    [],
                    |r| r.get(0),
                )
                .map_err(MemoryError::from)?,
        };
        Ok(n.max(0) as usize)
    }
}

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

/// No-op [`PinnedStore`] used as a graceful fallback when SQLite open
/// fails at startup.  Every method returns an empty / `false` result;
/// `add()` reports a synthesised [`MemoryError`] so callers can detect
/// the degraded mode without panicking.
pub struct NullPinnedStore;

impl NullPinnedStore {
    /// Construct a new no-op store.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullPinnedStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PinnedStore for NullPinnedStore {
    async fn list(
        &self,
        _scope: PinScope,
        _project_id: Option<&str>,
    ) -> Result<Vec<PinnedItem>, MemoryError> {
        Ok(Vec::new())
    }
    async fn list_all_for_prompt(
        &self,
        _scope: &MemoryExecutionScope,
    ) -> Result<Vec<PinnedItem>, MemoryError> {
        Ok(Vec::new())
    }
    async fn add(
        &self,
        _content: &str,
        _scope: PinScope,
        _source: PinSource,
        _project_id: Option<&str>,
    ) -> Result<PinnedItem, MemoryError> {
        Err(MemoryError::Generic(
            "pinned store is in null fallback mode (SQLite open failed at startup)".to_string(),
        ))
    }
    async fn delete(&self, _id: &str) -> Result<bool, MemoryError> {
        Ok(false)
    }
    async fn reorder(&self, _ids: &[String]) -> Result<(), MemoryError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_store() -> (tempfile::TempDir, SqlitePinnedStore) {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("memory.db");
        let store = SqlitePinnedStore::open(
            &db_path,
            tmp.path().to_path_buf(),
            Some(Arc::new(ThreatScanner::with_builtin_patterns())),
        )
        .expect("open store");
        (tmp, store)
    }

    fn open_store_no_scanner() -> (tempfile::TempDir, SqlitePinnedStore) {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("memory.db");
        let store =
            SqlitePinnedStore::open(&db_path, tmp.path().to_path_buf(), None).expect("open store");
        (tmp, store)
    }

    #[tokio::test]
    async fn add_returns_item_with_ulid_id() {
        let (_tmp, store) = open_store();
        let item = store
            .add("hello", PinScope::Global, PinSource::User, None)
            .await
            .expect("add");
        assert_eq!(item.content, "hello");
        assert_eq!(item.scope, PinScope::Global);
        assert!(item.project_id.is_none());
        // ULIDs are exactly 26 chars in Crockford-base32.
        assert_eq!(item.id.len(), 26, "expected ULID, got {}", item.id);
        assert!(Ulid::from_string(&item.id).is_ok());
    }

    #[tokio::test]
    async fn add_dedup_returns_existing_id() {
        let (_tmp, store) = open_store();
        let first = store
            .add("same content", PinScope::Global, PinSource::User, None)
            .await
            .expect("add first");
        let second = store
            .add("same content", PinScope::Global, PinSource::User, None)
            .await
            .expect("add second");
        assert_eq!(first.id, second.id, "dedup must return same id");
        let all = store
            .list(PinScope::Global, None)
            .await
            .expect("list global");
        assert_eq!(all.len(), 1, "dedup must not insert second row");
    }

    #[tokio::test]
    async fn add_dedup_case_insensitive_and_trim() {
        let (_tmp, store) = open_store();
        let first = store
            .add("Foo Bar", PinScope::Global, PinSource::User, None)
            .await
            .expect("add Foo Bar");
        let second = store
            .add("  foo bar  ", PinScope::Global, PinSource::User, None)
            .await
            .expect("add 'foo bar'");
        assert_eq!(first.id, second.id);
    }

    #[tokio::test]
    async fn add_rejects_too_long_content() {
        let (_tmp, store) = open_store();
        let too_long: String = "x".repeat(MAX_PIN_CONTENT_CHARS + 1);
        let err = store
            .add(&too_long, PinScope::Global, PinSource::User, None)
            .await
            .expect_err("must reject");
        assert!(matches!(err, MemoryError::PinnedContentTooLong { .. }));
    }

    #[tokio::test]
    async fn add_rejects_after_50_pins() {
        let (_tmp, store) = open_store_no_scanner();
        for i in 0..MAX_PINS_PER_SCOPE {
            store
                .add(
                    &format!("pin number {i}"),
                    PinScope::Global,
                    PinSource::User,
                    None,
                )
                .await
                .expect("add within cap");
        }
        let err = store
            .add("one too many", PinScope::Global, PinSource::User, None)
            .await
            .expect_err("must reject");
        assert!(matches!(err, MemoryError::PinnedLimitExceeded(50)));
    }

    #[tokio::test]
    async fn scope_isolation_project_vs_global() {
        let (_tmp, store) = open_store();
        store
            .add("g1", PinScope::Global, PinSource::User, None)
            .await
            .expect("add global");
        store
            .add("p1", PinScope::Project, PinSource::User, Some("proj-A"))
            .await
            .expect("add project");
        let global = store.list(PinScope::Global, None).await.expect("list g");
        let project = store
            .list(PinScope::Project, Some("proj-A"))
            .await
            .expect("list p");
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].content, "g1");
        assert_eq!(project.len(), 1);
        assert_eq!(project[0].content, "p1");
    }

    #[tokio::test]
    async fn scope_isolation_two_projects() {
        let (_tmp, store) = open_store();
        store
            .add("a-pin", PinScope::Project, PinSource::User, Some("A"))
            .await
            .expect("A");
        store
            .add("b-pin", PinScope::Project, PinSource::User, Some("B"))
            .await
            .expect("B");
        let a = store
            .list(PinScope::Project, Some("A"))
            .await
            .expect("list A");
        let b = store
            .list(PinScope::Project, Some("B"))
            .await
            .expect("list B");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].content, "a-pin");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].content, "b-pin");
    }

    #[tokio::test]
    async fn list_all_for_prompt_merges_global_and_project() {
        let (_tmp, store) = open_store();
        store
            .add("g", PinScope::Global, PinSource::User, None)
            .await
            .expect("g");
        store
            .add("p", PinScope::Project, PinSource::User, Some("X"))
            .await
            .expect("p");
        let scope = MemoryExecutionScope {
            session_id: None,
            project_id: Some("X".into()),
            workdir: None,
        };
        let merged = store
            .list_all_for_prompt(&scope)
            .await
            .expect("list_all_for_prompt");
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].content, "g", "global must come first");
        assert_eq!(merged[1].content, "p");
    }

    #[tokio::test]
    async fn delete_returns_true_then_false() {
        let (_tmp, store) = open_store();
        let item = store
            .add("ephemeral", PinScope::Global, PinSource::User, None)
            .await
            .expect("add");
        assert!(store.delete(&item.id).await.expect("first delete"));
        assert!(!store.delete(&item.id).await.expect("second delete"));
    }

    #[tokio::test]
    async fn reorder_changes_list_order() {
        let (_tmp, store) = open_store();
        let a = store
            .add("alpha", PinScope::Global, PinSource::User, None)
            .await
            .expect("a");
        let b = store
            .add("beta", PinScope::Global, PinSource::User, None)
            .await
            .expect("b");
        let c = store
            .add("gamma", PinScope::Global, PinSource::User, None)
            .await
            .expect("c");
        store
            .reorder(&[c.id.clone(), a.id.clone(), b.id.clone()])
            .await
            .expect("reorder");
        let listed = store.list(PinScope::Global, None).await.expect("list");
        let ids: Vec<&str> = listed.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec![c.id.as_str(), a.id.as_str(), b.id.as_str()]);
    }

    #[tokio::test]
    async fn pii_in_pin_content_is_scrubbed_and_audited() {
        let (_tmp, store) = open_store();
        let secret = format!("note: sk-{}", "a".repeat(40));
        let item = store
            .add(&secret, PinScope::Global, PinSource::User, None)
            .await
            .expect("add");
        assert!(
            item.content.contains("[REDACTED:ApiKey]"),
            "expected redacted marker, got {:?}",
            item.content
        );
        assert!(!item.content.contains("sk-aaaa"));
    }

    #[tokio::test]
    async fn sidecar_markdown_matches_sqlite_after_add() {
        let (tmp, store) = open_store_no_scanner();
        store
            .add("first pin", PinScope::Global, PinSource::User, None)
            .await
            .expect("add 1");
        store
            .add("second pin", PinScope::Global, PinSource::User, None)
            .await
            .expect("add 2");
        let path = tmp.path().join("pinned.md");
        let body = std::fs::read_to_string(&path).expect("read sidecar");
        assert!(body.contains("- first pin"), "missing first; got: {body}");
        assert!(body.contains("- second pin"), "missing second; got: {body}");
    }

    #[tokio::test]
    async fn sidecar_atomic_no_partial_file() {
        let (tmp, store) = open_store_no_scanner();
        store
            .add("p", PinScope::Global, PinSource::User, None)
            .await
            .expect("add");
        let mut stray = false;
        for entry in std::fs::read_dir(tmp.path()).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(".tmp") {
                stray = true;
            }
        }
        assert!(!stray, "stray .tmp file left in scope_root");
    }

    #[tokio::test]
    async fn null_store_is_inert() {
        let store = NullPinnedStore::new();
        assert!(store.list(PinScope::Global, None).await.unwrap().is_empty());
        let scope = MemoryExecutionScope::global();
        assert!(store.list_all_for_prompt(&scope).await.unwrap().is_empty());
        assert!(store
            .add("ignored", PinScope::Global, PinSource::User, None)
            .await
            .is_err());
        assert!(!store.delete("anything").await.unwrap());
        store.reorder(&[]).await.unwrap();
    }

    #[tokio::test]
    async fn tool_source_round_trips_through_sqlite() {
        let (_tmp, store) = open_store_no_scanner();
        let src = PinSource::Tool {
            tool_name: "pin_memory".into(),
            session_id: "sess-42".into(),
        };
        let item = store
            .add("agent pin", PinScope::Global, src.clone(), None)
            .await
            .expect("add");
        let listed = store.list(PinScope::Global, None).await.expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, item.id);
        assert_eq!(listed[0].created_by, src);
    }
}
