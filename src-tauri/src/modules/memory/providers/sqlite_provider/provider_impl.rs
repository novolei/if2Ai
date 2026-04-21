//! `MemoryProvider` trait implementation for `SqliteMemoryProvider`.
//!
//! Extracted from `sqlite_provider/mod.rs` in GFR-T1-D-1 (pure
//! structural move; function bodies byte-identical, including all
//! SQL statement strings + audit emission paths).

use async_trait::async_trait;
use rusqlite::params;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryCategory, MemoryEntry, MemoryError, MemoryProvider};
use crate::modules::runtime::episodic_compaction::WeibullDecay;

use super::scope::{scope_priority_order_by, scope_visibility_clause, scope_visibility_params};
use super::SqliteMemoryProvider;

#[async_trait]
impl MemoryProvider for SqliteMemoryProvider {
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError> {
        let key = key.to_string();
        let content = content.to_string();
        let category_str = category.as_str().to_string();

        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().map_err(|e| MemoryError::Generic(e.to_string()))?;
            // session_id/project_id are NULL for unscoped store() — use store_scoped() for isolation.
            c.execute(
                "INSERT INTO memory_entries
                     (key, content, category, created_at, updated_at, importance, access_count, trust_score, session_id, project_id)
                 VALUES ($1, $2, $3, $4, $4, 0.5, 0, 0.0, NULL, NULL)
                 ON CONFLICT(key) DO UPDATE SET
                     content = excluded.content,
                     category = excluded.category,
                     updated_at = excluded.updated_at",
                params![
                    key,
                    content,
                    category_str,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|e| MemoryError::Generic(format!("Failed to store entry: {e}")))?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn store_scoped(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        scope: &MemoryExecutionScope,
    ) -> Result<(), MemoryError> {
        // Phase 8A §0.5 Δ-2 defence-in-depth: scrub PII/secrets one last
        // time at the persistence boundary so even direct provider callers
        // (bypassing the tool / command layer) never leak credentials to
        // disk.  No-op when no scanner is attached.
        let content = if let Some(ref scanner) = self.scanner {
            let result = scanner.scan_and_redact(key, content);
            if result.flagged {
                let ctx = AuditContext::from_scope(scope);
                MemoryAuditEmitter::memory_pii_redacted(&ctx, key, &result.detected);
            }
            result.cleaned
        } else {
            content.to_string()
        };
        let key = key.to_string();
        let category_str = category.as_str().to_string();
        let session_id = scope.session_id.clone();
        let project_id = scope.project_id.clone();

        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().map_err(|e| MemoryError::Generic(e.to_string()))?;
            c.execute(
                "INSERT INTO memory_entries
                     (key, content, category, created_at, updated_at, importance, access_count, trust_score, session_id, project_id)
                 VALUES ($1, $2, $3, $4, $4, 0.5, 0, 0.0, $5, $6)
                 ON CONFLICT(key) DO UPDATE SET
                     content = excluded.content,
                     category = excluded.category,
                     updated_at = excluded.updated_at,
                     session_id = excluded.session_id,
                     project_id = excluded.project_id",
                params![
                    key,
                    content,
                    category_str,
                    chrono::Utc::now().to_rfc3339(),
                    session_id,
                    project_id,
                ],
            )
            .map_err(|e| MemoryError::Generic(format!("Failed to store scoped entry: {e}")))?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn recall_scoped(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let query_str = query.to_string();
        let category_str = category.map(String::from);
        let session_id = scope.session_id.clone();
        let project_id = scope.project_id.clone();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;

            // Three-tier visibility rules — see `MemoryExecutionScope` doc.
            //
            // 1) session + project scope (most common, agent tool execution):
            //      a) entries owned by this exact session, OR
            //      b) project entries (session_id IS NULL AND project_id = $project), OR
            //      c) legacy / global entries (session_id IS NULL AND project_id IS NULL).
            //    NOT visible: other sessions' session entries, other projects' project entries.
            //
            // 2) project-only scope (no active session, e.g. project-level Memory Browser):
            //      a) project entries (session_id IS NULL AND project_id = $project), OR
            //      b) global entries.
            //    NOT visible: any session-scoped entry, other projects' entries.
            //
            // 3) global scope (CLI / harness / un-bound caller):
            //      a) global entries only (session_id IS NULL AND project_id IS NULL).
            //
            // Legacy unscoped entries (session_id IS NULL AND project_id IS NULL) always
            // behave as global entries, preserving backward-compatibility with rows
            // written before the scope columns existed.
            let scope_clause =
                scope_visibility_clause(session_id.as_deref(), project_id.as_deref());
            let scope_params =
                scope_visibility_params(session_id.as_deref(), project_id.as_deref());
            // Project-aware ranking: scope tier (session > project > global)
            // first, then importance / access_count / recency within tier.
            let order_by = scope_priority_order_by(session_id.as_deref(), project_id.as_deref());

            let limit_i64: i64 = limit as i64;
            let mut all_entries: Vec<MemoryEntry> = Vec::new();
            if let Some(ref cat) = category_str {
                let cat_pos = scope_params.len() + 1;
                let limit_pos = scope_params.len() + 2;
                let sql = format!(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id
                     FROM memory_entries
                     WHERE ({scope_clause})
                       AND category = ?{cat_pos}
                     ORDER BY {order_by}
                     LIMIT ?{limit_pos}"
                );
                let mut stmt = c.prepare(&sql)?;
                let mut bound: Vec<&dyn rusqlite::ToSql> =
                    Vec::with_capacity(scope_params.len() + 2);
                for p in &scope_params {
                    bound.push(p);
                }
                bound.push(cat);
                bound.push(&limit_i64);
                for row in stmt
                    .query_map(
                        rusqlite::params_from_iter(bound),
                        SqliteMemoryProvider::row_to_entry,
                    )?
                    .flatten()
                {
                    all_entries.push(row);
                }
            } else {
                let limit_pos = scope_params.len() + 1;
                let sql = format!(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id
                     FROM memory_entries
                     WHERE ({scope_clause})
                     ORDER BY {order_by}
                     LIMIT ?{limit_pos}"
                );
                let mut stmt = c.prepare(&sql)?;
                let mut bound: Vec<&dyn rusqlite::ToSql> =
                    Vec::with_capacity(scope_params.len() + 1);
                for p in &scope_params {
                    bound.push(p);
                }
                bound.push(&limit_i64);
                for row in stmt
                    .query_map(
                        rusqlite::params_from_iter(bound),
                        SqliteMemoryProvider::row_to_entry,
                    )?
                    .flatten()
                {
                    all_entries.push(row);
                }
            }

            let results = if query_str.is_empty() {
                all_entries
            } else {
                let query_lower = query_str.to_lowercase();
                all_entries
                    .into_iter()
                    .filter(|e| {
                        e.key.to_lowercase().contains(&query_lower)
                            || e.content.to_lowercase().contains(&query_lower)
                    })
                    .collect()
            };

            Ok(results)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn recall(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let query_str = query.to_string();
        let category_str = category.map(String::from);
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;

            let mut stmt = match &category_str {
                Some(_) => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id
                     FROM memory_entries
                     WHERE category = ?1
                     ORDER BY updated_at DESC
                     LIMIT ?2",
                )?,
                None => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id
                     FROM memory_entries
                     ORDER BY updated_at DESC
                     LIMIT ?1",
                )?,
            };

            // Eagerly collect all entries via for-loop so the MappedRows borrow on stmt
            // ends before stmt is dropped — avoids E0597 lifetime errors.
            let mut all_entries: Vec<MemoryEntry> = Vec::new();
            if let Some(ref cat) = category_str {
                for row in stmt
                    .query_map(
                        params![cat, limit as i64],
                        SqliteMemoryProvider::row_to_entry,
                    )?
                    .flatten()
                {
                    all_entries.push(row);
                }
            } else {
                for row in stmt
                    .query_map(params![limit as i64], SqliteMemoryProvider::row_to_entry)?
                    .flatten()
                {
                    all_entries.push(row);
                }
            }

            let results = if query_str.is_empty() {
                all_entries
            } else {
                let query_lower = query_str.to_lowercase();
                all_entries
                    .into_iter()
                    .filter(|e| {
                        e.key.to_lowercase().contains(&query_lower)
                            || e.content.to_lowercase().contains(&query_lower)
                    })
                    .collect()
            };

            Ok(results)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn delete(&self, key: &str) -> Result<(), MemoryError> {
        let key = key.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let rows = c
                .execute("DELETE FROM memory_entries WHERE key = ?1", params![key])
                .map_err(|e| MemoryError::Generic(format!("Failed to delete entry: {e}")))?;

            if rows == 0 {
                Err(MemoryError::KeyNotFound(key))
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn purge_category(&self, category: &str) -> Result<(), MemoryError> {
        let category = category.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            c.execute(
                "DELETE FROM memory_entries WHERE category = ?1",
                params![category],
            )
            .map_err(|e| MemoryError::Generic(format!("Failed to purge category: {e}")))?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// Bulk-delete every row in `memory_entries` in a single statement.
    ///
    /// Overrides the trait's row-by-row fallback so the Settings "Clear
    /// all memories" affordance completes in O(1) round-trips even when
    /// the table has thousands of entries.  Returns the number of rows
    /// the SQLite engine reports as removed.
    async fn clear_all(&self) -> Result<usize, MemoryError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let removed = c
                .execute("DELETE FROM memory_entries", [])
                .map_err(|e| MemoryError::Generic(format!("Failed to clear memories: {e}")))?;
            Ok(removed)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError> {
        let category_str = category.map(String::from);
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;

            // Declare stmt outside the if-let block so its borrow ends before the block closes.
            let mut stmt = match &category_str {
                Some(_) => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id
                     FROM memory_entries WHERE category = ?1 ORDER BY updated_at DESC",
                )?,
                None => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id
                     FROM memory_entries ORDER BY updated_at DESC",
                )?,
            };

            // Use a for-loop to eagerly collect so the MappedRows borrow on stmt ends
            // before stmt is dropped — avoids E0597 lifetime errors.
            let mut result: Vec<MemoryEntry> = Vec::new();
            if let Some(ref cat) = category_str {
                for row in stmt
                    .query_map(params![cat], SqliteMemoryProvider::row_to_entry)?
                    .flatten()
                {
                    result.push(row);
                }
            } else {
                for row in stmt
                    .query_map([], SqliteMemoryProvider::row_to_entry)?
                    .flatten()
                {
                    result.push(row);
                }
            }

            Ok(result)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn export_scoped(
        &self,
        category: Option<&str>,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let category_str = category.map(String::from);
        let session_id = scope.session_id.clone();
        let project_id = scope.project_id.clone();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;

            let scope_clause =
                scope_visibility_clause(session_id.as_deref(), project_id.as_deref());
            let scope_params =
                scope_visibility_params(session_id.as_deref(), project_id.as_deref());

            let mut result: Vec<MemoryEntry> = Vec::new();
            if let Some(ref cat) = category_str {
                let cat_pos = scope_params.len() + 1;
                let sql = format!(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id
                     FROM memory_entries
                     WHERE ({scope_clause})
                       AND category = ?{cat_pos}
                     ORDER BY updated_at DESC"
                );
                let mut stmt = c.prepare(&sql)?;
                let mut bound: Vec<&dyn rusqlite::ToSql> =
                    Vec::with_capacity(scope_params.len() + 1);
                for p in &scope_params {
                    bound.push(p);
                }
                bound.push(cat);
                for row in stmt
                    .query_map(
                        rusqlite::params_from_iter(bound),
                        SqliteMemoryProvider::row_to_entry,
                    )?
                    .flatten()
                {
                    result.push(row);
                }
            } else {
                let sql = format!(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id
                     FROM memory_entries
                     WHERE ({scope_clause})
                     ORDER BY updated_at DESC"
                );
                let mut stmt = c.prepare(&sql)?;
                let mut bound: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(scope_params.len());
                for p in &scope_params {
                    bound.push(p);
                }
                for row in stmt
                    .query_map(
                        rusqlite::params_from_iter(bound),
                        SqliteMemoryProvider::row_to_entry,
                    )?
                    .flatten()
                {
                    result.push(row);
                }
            }

            Ok(result)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// Update an entry's scope tags in place.
    ///
    /// Used by [`MemoryPromotionEngine`](crate::modules::memory::promotion) to
    /// promote `session` → `project` (set `session_id = NULL`, fill
    /// `project_id`) or `project` → `global` (clear both).  The `target_scope`
    /// fields are written verbatim, so callers must construct the desired
    /// final scope.
    async fn promote_scope(
        &self,
        key: &str,
        target_scope: &MemoryExecutionScope,
    ) -> Result<(), MemoryError> {
        let key_owned = key.to_string();
        let session_id = target_scope.session_id.clone();
        let project_id = target_scope.project_id.clone();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let rows = c
                .execute(
                    "UPDATE memory_entries
                       SET session_id = ?1,
                           project_id = ?2,
                           updated_at = ?3
                     WHERE key = ?4",
                    params![
                        session_id,
                        project_id,
                        chrono::Utc::now().to_rfc3339(),
                        key_owned,
                    ],
                )
                .map_err(|e| MemoryError::Generic(format!("promote_scope failed: {e}")))?;
            if rows == 0 {
                Err(MemoryError::KeyNotFound(key_owned))
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// Apply Weibull importance decay to all entries in the database.
    ///
    /// Fetches every entry, computes the decayed importance using
    /// `WeibullDecay::compute_importance`, and persists the updated value.
    /// Returns the count of updated rows.
    ///
    /// This method is called post-turn from `agent.rs` to keep importance
    /// scores up-to-date without requiring a separate background task.
    async fn apply_importance_decay(
        &self,
        lambda_hours: f32,
        k: f32,
    ) -> Result<usize, MemoryError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let decay = WeibullDecay::new(lambda_hours, k);

            // Fetch all entries (including new scope columns for row_to_entry compatibility).
            let mut stmt = c
                .prepare(
                    "SELECT key, content, category, created_at, updated_at, \
                     importance, access_count, trust_score, session_id, project_id \
                     FROM memory_entries",
                )
                .map_err(|e| MemoryError::Generic(e.to_string()))?;

            let entries: Vec<MemoryEntry> = stmt
                .query_map([], SqliteMemoryProvider::row_to_entry)
                .map_err(|e| MemoryError::Generic(e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();

            let mut updated = 0usize;
            for entry in &entries {
                let new_importance = decay.compute_importance(entry);
                let rows = c
                    .execute(
                        "UPDATE memory_entries SET importance = ?1, updated_at = ?2 \
                         WHERE key = ?3",
                        params![new_importance, chrono::Utc::now().to_rfc3339(), entry.key,],
                    )
                    .map_err(|e| MemoryError::Generic(format!("decay update failed: {e}")))?;
                updated += rows;
            }

            Ok(updated)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }
}
