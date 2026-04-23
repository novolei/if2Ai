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

            let mut results: Vec<MemoryEntry> = if query_str.is_empty() {
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

            // MEM-MOD-P1 — Stats Activation: bump `access_count` on every
            // recalled key so the WeibullDecay formula has signal to work
            // with and the Memory Browser's "访问 N 次" badge stops being
            // a permanent zero.  The +1 is applied to the in-memory
            // entries we return as well so the immediate caller (and
            // any audit emitter consuming the same Vec) sees a fresh
            // value without an extra round-trip.
            bump_access_counts_in_results(&c, &mut results)?;

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

            let mut results: Vec<MemoryEntry> = if query_str.is_empty() {
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

            // MEM-MOD-P1 — see `recall_scoped` above for the rationale.
            bump_access_counts_in_results(&c, &mut results)?;

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

    /// MEM-MOD-P3 — O(1) primary-key lookup.
    async fn get_by_key(&self, key: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        let key = key.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let mut stmt = c.prepare(
                "SELECT key, content, category, created_at, updated_at, importance,
                        access_count, trust_score, session_id, project_id
                 FROM memory_entries
                 WHERE key = ?1",
            )?;
            let mut rows = stmt.query_map(params![key], SqliteMemoryProvider::row_to_entry)?;
            match rows.next() {
                Some(Ok(entry)) => Ok(Some(entry)),
                Some(Err(err)) => Err(MemoryError::Generic(err.to_string())),
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// MEM-MOD-P3 — partial UPDATE preserving stats columns.
    /// MEM-MOD-P6 — also snapshots the previous row into
    /// `memory_entry_history` so the temporal API can reconstruct
    /// "what was the value at time T".
    async fn update_content(&self, key: &str, content: &str) -> Result<(), MemoryError> {
        let key = key.to_string();
        let content = content.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let now = chrono::Utc::now().to_rfc3339();
            // Snapshot the current row first.  Best-effort — a
            // missing history table (older DB pre-P6) is ignored.
            let _ = snapshot_into_history(&c, &key, &now, "update");

            let rows = c
                .execute(
                    "UPDATE memory_entries SET content = ?1, updated_at = ?2 WHERE key = ?3",
                    params![content, now, key],
                )
                .map_err(|e| MemoryError::Generic(format!("update_content failed: {e}")))?;
            if rows == 0 {
                Err(MemoryError::KeyNotFound(key))
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// MEM-MOD-P6 — pull every snapshot for `key`, newest first.
    async fn list_history(
        &self,
        key: &str,
    ) -> Result<Vec<crate::modules::memory::MemoryHistoryEntry>, MemoryError> {
        let key = key.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let mut stmt = c.prepare(
                "SELECT key, content, category, importance, trust_score,
                        valid_from, valid_to, source
                 FROM memory_entry_history
                 WHERE key = ?1
                 ORDER BY valid_from DESC",
            )?;
            let rows = stmt.query_map(params![key], |row| {
                let valid_from_str: String = row.get(5)?;
                let valid_to_str: String = row.get(6)?;
                let parse = |s: &str| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now())
                };
                Ok(crate::modules::memory::MemoryHistoryEntry {
                    key: row.get(0)?,
                    content: row.get(1)?,
                    category: row.get(2)?,
                    importance: row.get(3)?,
                    trust_score: row.get(4)?,
                    valid_from: parse(&valid_from_str),
                    valid_to: parse(&valid_to_str),
                    source: row.get(7)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows.flatten() {
                out.push(row);
            }
            Ok(out)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// MEM-MOD-P3 — INSERT OR IGNORE (UNIQUE on (source, target, type)).
    async fn create_link(
        &self,
        source_key: &str,
        target_key: &str,
        link_type: &str,
    ) -> Result<(), MemoryError> {
        let source_key = source_key.to_string();
        let target_key = target_key.to_string();
        let link_type = link_type.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            c.execute(
                "INSERT OR IGNORE INTO memory_links
                    (source_key, target_key, link_type, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    source_key,
                    target_key,
                    link_type,
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .map_err(|e| MemoryError::Generic(format!("create_link failed: {e}")))?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// MEM-MOD-P3 — store the consolidated entry, link each source.
    async fn consolidate(
        &self,
        source_keys: &[String],
        consolidated_key: &str,
        consolidated_content: &str,
        category: MemoryCategory,
    ) -> Result<usize, MemoryError> {
        let source_keys: Vec<String> = source_keys.to_vec();
        let consolidated_key = consolidated_key.to_string();
        let consolidated_content = consolidated_content.to_string();
        let category_str = category.as_str().to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let now = chrono::Utc::now().to_rfc3339();
            // MEM-MOD-P6 — snapshot existing entry (if any) before
            // overwriting via upsert so the temporal API can still
            // surface the pre-consolidation content.
            let _ = snapshot_into_history(&c, &consolidated_key, &now, "consolidate");
            // 1) upsert the consolidated entry itself.
            c.execute(
                "INSERT INTO memory_entries
                    (key, content, category, created_at, updated_at,
                     importance, access_count, trust_score)
                 VALUES (?1, ?2, ?3, ?4, ?4, 0.6, 0, 0.0)
                 ON CONFLICT(key) DO UPDATE SET
                    content = excluded.content,
                    category = excluded.category,
                    updated_at = excluded.updated_at",
                params![consolidated_key, consolidated_content, category_str, now],
            )
            .map_err(|e| MemoryError::Generic(format!("consolidate upsert failed: {e}")))?;

            // 2) link every source → consolidated.
            let mut linked = 0usize;
            for src in &source_keys {
                let n = c
                    .execute(
                        "INSERT OR IGNORE INTO memory_links
                            (source_key, target_key, link_type, created_at)
                         VALUES (?1, ?2, 'consolidated_into', ?3)",
                        params![src, consolidated_key, now],
                    )
                    .map_err(|e| MemoryError::Generic(format!("link failed: {e}")))?;
                linked += n;
            }
            Ok(linked)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// MEM-MOD-P1 — clamp `trust_score + delta` into `[-1.0, 1.0]` and
    /// persist the result.  Single-row UPDATE — cheap enough to be
    /// invoked from a per-turn LLM tool without batching.
    async fn adjust_trust_score(&self, key: &str, delta: f64) -> Result<f64, MemoryError> {
        let key = key.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;

            let current: Option<f64> = c
                .query_row(
                    "SELECT trust_score FROM memory_entries WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .ok();

            let Some(current) = current else {
                return Err(MemoryError::KeyNotFound(key));
            };

            let new_score = (current + delta).clamp(-1.0_f64, 1.0_f64);
            c.execute(
                "UPDATE memory_entries
                 SET trust_score = ?1, updated_at = ?2
                 WHERE key = ?3",
                params![new_score, chrono::Utc::now().to_rfc3339(), key],
            )
            .map_err(|e| MemoryError::Generic(format!("trust_score update failed: {e}")))?;

            Ok(new_score)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }
}

/// MEM-MOD-P6 — best-effort snapshot of the current `memory_entries`
/// row for `key` into `memory_entry_history` with `valid_to = now`.
/// Returns `Ok(())` even when the source row is missing — this is a
/// fire-and-forget audit hook, not a precondition for the caller's
/// UPDATE / UPSERT.  A missing history table (DB pre-v3 migration)
/// is also silently ignored.
fn snapshot_into_history(
    c: &rusqlite::Connection,
    key: &str,
    now_rfc3339: &str,
    source: &str,
) -> Result<(), MemoryError> {
    let row = c.query_row(
        "SELECT content, category, created_at, importance, trust_score
         FROM memory_entries WHERE key = ?1",
        rusqlite::params![key],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, f64>(4)?,
            ))
        },
    );
    let Ok((content, category, created_at, importance, trust_score)) = row else {
        return Ok(()); // nothing to snapshot
    };

    if let Err(err) = c.execute(
        "INSERT INTO memory_entry_history
            (key, content, category, importance, trust_score,
             valid_from, valid_to, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            key,
            content,
            category,
            importance,
            trust_score,
            created_at,
            now_rfc3339,
            source
        ],
    ) {
        tracing::warn!(
            key,
            error = %err,
            "[memory.history] snapshot insert failed (non-fatal)"
        );
    }
    Ok(())
}

/// MEM-MOD-P1 — Increment `access_count` for every key just returned
/// to a recall caller, in a single batched UPDATE.  Errors here are
/// non-fatal: stats are best-effort and we never want to fail a recall
/// because the bookkeeping write hit a transient SQLite lock.  A
/// `tracing::warn!` makes the failure visible without poisoning the
/// audit stream.
///
/// The matching `MemoryEntry::access_count` values in `results` are
/// bumped in lockstep so the immediate caller (and the audit emitter
/// downstream) sees a fresh count without an extra round-trip.
fn bump_access_counts_in_results(
    c: &rusqlite::Connection,
    results: &mut [MemoryEntry],
) -> Result<(), MemoryError> {
    if results.is_empty() {
        return Ok(());
    }

    let placeholders = std::iter::repeat_n("?", results.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "UPDATE memory_entries
         SET access_count = access_count + 1
         WHERE key IN ({placeholders})"
    );

    let key_params: Vec<&dyn rusqlite::ToSql> = results
        .iter()
        .map(|e| &e.key as &dyn rusqlite::ToSql)
        .collect();

    match c.execute(&sql, rusqlite::params_from_iter(key_params)) {
        Ok(_) => {
            for entry in results.iter_mut() {
                entry.access_count = entry.access_count.saturating_add(1);
            }
            Ok(())
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                count = results.len(),
                "[memory.recall] best-effort access_count bump failed; returning unmodified results"
            );
            Ok(())
        }
    }
}
