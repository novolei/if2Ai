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

/// Map a [`MemoryCategory`] to its CoALA cognitive layer (1=Reactive,
/// 2=Deliberative (default), 3=Reflective, 4=Meta). Mirrors the v8
/// migration back-fill SQL so freshly-stored rows match upgraded ones.
fn cognitive_layer_from_category(category: &MemoryCategory) -> i32 {
    match category {
        MemoryCategory::Conversation => 1,
        MemoryCategory::Working | MemoryCategory::Daily => 2,
        MemoryCategory::Reflection | MemoryCategory::Procedural => 3,
        MemoryCategory::Core => 4,
        MemoryCategory::Custom(_) => 2,
    }
}

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
        let cognitive_layer_val = cognitive_layer_from_category(&category);

        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().map_err(|e| MemoryError::Generic(e.to_string()))?;
            // session_id/project_id are NULL for unscoped store() — use store_scoped() for isolation.
            c.execute(
                "INSERT INTO memory_entries
                     (key, content, category, created_at, updated_at, importance, access_count, trust_score, session_id, project_id, quality_score, source_reliability, last_validated_at, contradiction_count, cognitive_layer, context_tags)
                 VALUES ($1, $2, $3, $4, $4, 0.5, 0, 0.0, NULL, NULL, 0.5, 0.5, NULL, 0, $5, '[]')
                 ON CONFLICT(key) DO UPDATE SET
                     content = excluded.content,
                     category = excluded.category,
                     updated_at = excluded.updated_at",
                params![
                    key,
                    content,
                    category_str,
                    chrono::Utc::now().to_rfc3339(),
                    cognitive_layer_val,
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
        let cognitive_layer_val = cognitive_layer_from_category(&category);
        let session_id = scope.session_id.clone();
        let project_id = scope.project_id.clone();

        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().map_err(|e| MemoryError::Generic(e.to_string()))?;
            c.execute(
                "INSERT INTO memory_entries
                     (key, content, category, created_at, updated_at, importance, access_count, trust_score, session_id, project_id, quality_score, source_reliability, last_validated_at, contradiction_count, cognitive_layer, context_tags)
                 VALUES ($1, $2, $3, $4, $4, 0.5, 0, 0.0, $5, $6, 0.5, 0.5, NULL, 0, $7, '[]')
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
                    cognitive_layer_val,
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
                            access_count, trust_score, session_id, project_id,
                            quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
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
                            access_count, trust_score, session_id, project_id,
                            quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
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
                            access_count, trust_score, session_id, project_id,
                            quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
                     FROM memory_entries
                     WHERE category = ?1
                     ORDER BY updated_at DESC
                     LIMIT ?2",
                )?,
                None => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id,
                            quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
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
                            access_count, trust_score, session_id, project_id,
                            quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
                     FROM memory_entries WHERE category = ?1 ORDER BY updated_at DESC",
                )?,
                None => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance,
                            access_count, trust_score, session_id, project_id,
                            quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
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
                            access_count, trust_score, session_id, project_id,
                            quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
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
                            access_count, trust_score, session_id, project_id,
                            quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
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
                     importance, access_count, trust_score, session_id, project_id, \
                     quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags \
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
                // `updated_at` 故意不动：importance 是从 created_at /
                // access_count 反推出来的衍生指标，每轮对话末尾跑一次
                // decay 不算"内容被修改"。原版同时把 updated_at 刷成
                // now 会让整库时间戳塌缩到"最近一次衰减时刻"，前端
                // "上次更新时间"完全失去意义（参见 Memory Browser
                // bug 调查 — 所有 entries 的 updated_at 都落在同一
                // 个 ms 桶里）。
                let rows = c
                    .execute(
                        "UPDATE memory_entries SET importance = ?1 WHERE key = ?2",
                        params![new_importance, entry.key],
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
                        access_count, trust_score, session_id, project_id,
                        quality_score, source_reliability, last_validated_at, contradiction_count,
                            cognitive_layer, context_tags
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
                     importance, access_count, trust_score,
                     quality_score, source_reliability, last_validated_at, contradiction_count,
                     cognitive_layer, context_tags)
                 VALUES (?1, ?2, ?3, ?4, ?4, 0.6, 0, 0.0, 0.5, 0.5, NULL, 0, 2, '[]')
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

    /// Persist updated `importance` and `quality_score` for a single entry.
    /// Used by the forgetting-curve sweep after computing decay.
    async fn update_decay_scores(
        &self,
        key: &str,
        importance: f64,
        quality_score: f64,
    ) -> Result<(), MemoryError> {
        let key = key.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let changed = c
                .execute(
                    "UPDATE memory_entries SET importance = ?1, quality_score = ?2 WHERE key = ?3",
                    params![importance, quality_score, key],
                )
                .map_err(|e| MemoryError::Generic(format!("update_decay_scores failed: {e}")))?;
            if changed == 0 {
                return Err(MemoryError::KeyNotFound(key));
            }
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    /// Spec §2.3 Hybrid C — best-effort SQL UPDATE.  Missing key is
    /// not an error: the conflict resolver may race against a delete.
    /// Forward errors only on actual SQL failures so the resolver
    /// can surface them in its tracing.
    async fn increment_contradiction_count(&self, key: &str) -> Result<(), MemoryError> {
        let key = key.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<(), MemoryError> {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            c.execute(
                "UPDATE memory_entries
                   SET contradiction_count = contradiction_count + 1
                 WHERE key = ?1",
                params![key],
            )
            .map_err(|e| {
                MemoryError::Generic(format!("increment_contradiction_count: sql: {e}"))
            })?;
            Ok(())
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
            // `updated_at` 同样不动：trust_score 是用户/agent 反馈
            // 累加出的衍生指标（feedback bumps 一次 +/- delta），跟
            // memory 的内容文本无关。把 updated_at 也刷成 now 会让
            // 任何一次"赞/踩"都伪装成"内容更新"，污染 timeline。
            // see also: apply_importance_decay 同位置修复。
            c.execute(
                "UPDATE memory_entries SET trust_score = ?1 WHERE key = ?2",
                params![new_score, key],
            )
            .map_err(|e| MemoryError::Generic(format!("trust_score update failed: {e}")))?;

            Ok(new_score)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn conversation_recall_ingest(
        &self,
        session_id: &str,
        project_id: Option<&str>,
        turn_id: &str,
        user_message: &str,
        assistant_excerpt: &str,
    ) -> Result<(), MemoryError> {
        let session_id = session_id.to_string();
        let project_id = project_id.map(str::to_string);
        let turn_id = turn_id.to_string();
        let body = format!("User:\n{user_message}\n\nAssistant:\n{assistant_excerpt}");
        // P1-7 — best-effort dense embedding for hybrid recall ranking.
        // `embedder_for_recall` returns None when the env switch is off
        // OR when FastEmbed init failed (we degrade silently to FTS-only).
        let embedding_blob =
            crate::modules::memory::conversation_recall_vector::embedder_for_recall().and_then(
                |embedder| match embedder.embed_one(&body) {
                    Ok(v) => Some(
                        crate::modules::memory::conversation_recall_vector::f32_slice_to_blob(&v),
                    ),
                    Err(e) => {
                        tracing::debug!(error = %e, "[conversation_recall] embed skipped");
                        None
                    }
                },
            );
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            c.execute(
                "INSERT INTO conversation_recall_fts(session_id, project_id, turn_id, body) VALUES (?1, ?2, ?3, ?4)",
                params![session_id, project_id, turn_id, body],
            )
            .map_err(|e| MemoryError::Generic(format!("conversation_recall ingest: {e}")))?;
            if let Some(blob) = embedding_blob {
                // PRIMARY KEY (session_id, turn_id) — REPLACE keeps the row
                // idempotent if a turn is re-ingested.
                let _ = c.execute(
                    "INSERT OR REPLACE INTO conversation_recall_embeddings\
                     (session_id, project_id, turn_id, embedding) VALUES (?1, ?2, ?3, ?4)",
                    params![session_id, project_id, turn_id, blob],
                );
            }
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn conversation_recall_search(
        &self,
        query: &str,
        session_id: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, MemoryError> {
        let query = query.trim().to_string();
        let session_id = session_id.to_string();
        let project_id = project_id.map(str::to_string);
        let limit = limit.clamp(1, 50);
        // P1-7 hybrid: embed the query once outside the spawn_blocking so
        // the FastEmbed call (which itself uses ORT) doesn't sit on the
        // blocking pool's mutex. `None` ⇒ FTS-only path.
        let query_embedding = if !query.is_empty() {
            crate::modules::memory::conversation_recall_vector::embedder_for_recall().and_then(
                |embedder| match embedder.embed_one(&query) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::debug!(error = %e, "[conversation_recall] query embed skipped");
                        None
                    }
                },
            )
        } else {
            None
        };
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let mut out = Vec::new();
            if query.is_empty() {
                let mut stmt = c
                    .prepare(
                        "SELECT body FROM conversation_recall_fts WHERE session_id = ?1 \
                         ORDER BY rowid DESC LIMIT ?2",
                    )
                    .map_err(|e| MemoryError::Generic(e.to_string()))?;
                let rows = stmt
                    .query_map(params![session_id, limit], |row| row.get::<_, String>(0))
                    .map_err(|e| MemoryError::Generic(e.to_string()))?;
                for r in rows {
                    out.push(r.map_err(|e| MemoryError::Generic(e.to_string()))?);
                }
                return Ok(out);
            }

            let tokens: Vec<String> = query
                .split_whitespace()
                .filter(|t| !t.is_empty())
                .map(|t| t.replace('"', ""))
                .filter(|t| !t.is_empty())
                .collect();
            if tokens.is_empty() {
                return Ok(out);
            }
            let match_body = tokens
                .iter()
                .map(|t| format!("body:\"{t}\""))
                .collect::<Vec<_>>()
                .join(" OR ");

            let sid = session_id.replace('"', "");
            let mut match_expr = format!("session_id:\"{sid}\" AND ({match_body})");
            if let Some(pid) = project_id.as_deref() {
                let pid = pid.replace('"', "");
                match_expr.push_str(&format!(" AND project_id:\"{pid}\""));
            }

            // When we have a query embedding, pull a wider candidate pool
            // (≤ 4*limit, capped at 50) and re-rank by FTS bm25 + cosine
            // against the per-turn embedding. Otherwise keep the historical
            // "FTS bm25 only" path byte-equivalent.
            let widen = if query_embedding.is_some() {
                (limit.saturating_mul(4)).clamp(limit, 50)
            } else {
                limit
            };

            let mut stmt = c
                .prepare(
                    "SELECT body, turn_id, bm25(conversation_recall_fts) AS rank \
                     FROM conversation_recall_fts \
                     WHERE conversation_recall_fts MATCH ?1 \
                     ORDER BY rank \
                     LIMIT ?2",
                )
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let rows = stmt
                .query_map(params![match_expr, widen], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                    ))
                })
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let mut candidates: Vec<(String, String, f64)> = Vec::new();
            for r in rows {
                candidates.push(r.map_err(|e| MemoryError::Generic(e.to_string()))?);
            }
            if candidates.is_empty() {
                return Ok(out);
            }

            let Some(query_vec) = query_embedding else {
                // FTS-only: bm25 ascending order is already best-first.
                for (body, _turn, _rank) in candidates.into_iter().take(limit) {
                    out.push(body);
                }
                return Ok(out);
            };

            // bm25 in SQLite FTS5 is "lower is better" — flip + min-max
            // normalize to a 0..1 score where 1.0 is the best FTS hit.
            let min_rank = candidates
                .iter()
                .map(|(_, _, r)| *r)
                .fold(f64::INFINITY, f64::min);
            let max_rank = candidates
                .iter()
                .map(|(_, _, r)| *r)
                .fold(f64::NEG_INFINITY, f64::max);
            let span = (max_rank - min_rank).abs().max(f64::EPSILON);

            let mut embed_stmt = c
                .prepare(
                    "SELECT embedding FROM conversation_recall_embeddings \
                     WHERE session_id = ?1 AND turn_id = ?2",
                )
                .map_err(|e| MemoryError::Generic(e.to_string()))?;

            // Hybrid weight (0..1): higher = lean on FTS, default 0.4 so
            // semantic similarity dominates when both signals exist.
            let alpha: f64 = std::env::var("IF2AI_CONVERSATION_RECALL_HYBRID_ALPHA")
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.4_f64)
                .clamp(0.0_f64, 1.0_f64);

            let mut scored: Vec<(f64, String)> = Vec::with_capacity(candidates.len());
            for (body, turn, rank) in candidates {
                let fts_score = ((max_rank - rank) / span).clamp(0.0, 1.0);
                let cosine = embed_stmt
                    .query_row(params![session_id, turn], |row| row.get::<_, Vec<u8>>(0))
                    .ok()
                    .and_then(|blob| {
                        crate::modules::memory::conversation_recall_vector::blob_to_f32_slice(&blob)
                    })
                    .map(|v| {
                        crate::modules::memory::conversation_recall_vector::cosine_similarity(
                            &query_vec, &v,
                        ) as f64
                    });
                let combined = match cosine {
                    Some(cos) => alpha * fts_score + (1.0 - alpha) * cos.max(0.0),
                    None => fts_score, // no embedding row ⇒ FTS-only weight
                };
                scored.push((combined, body));
            }
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            for (_score, body) in scored.into_iter().take(limit) {
                out.push(body);
            }
            Ok(out)
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
