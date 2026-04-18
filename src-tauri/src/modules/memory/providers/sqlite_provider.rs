//! SQLite-backed memory provider
//!
//! Persists memory entries to a SQLite database, surviving process restarts.
//! Uses rusqlite (sync) wrapped in spawn_blocking for async compatibility.

use async_trait::async_trait;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compat::ClawCliMemoryEntry;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::security::ThreatScanner;
use crate::modules::memory::{MemoryCategory, MemoryEntry, MemoryError, MemoryProvider};
use crate::modules::runtime::episodic_compaction::WeibullDecay;

/// SQLite-backed implementation of [`MemoryProvider`]
///
/// Stores memory entries in a SQLite database with proper indexing
/// for category-based queries and full-text search support.
pub struct SqliteMemoryProvider {
    conn: Arc<Mutex<Connection>>,
    /// Phase 8A — optional shared PII scanner.  When `Some`, every
    /// scope-aware write (`store_scoped`) runs the content through
    /// [`ThreatScanner::scan_and_redact`] before persisting and emits a
    /// `memory_pii_redacted` audit event when hits are found.  Defaults to
    /// `None` so existing tests / call sites that construct the provider
    /// directly remain unchanged (defence-in-depth: the tool layer or
    /// command layer is the primary scrub site; this is the safety net).
    scanner: Option<Arc<ThreatScanner>>,
}

impl SqliteMemoryProvider {
    /// Create a new SQLite provider, initializing the schema if needed.
    pub fn new(db_path: PathBuf) -> Result<Self, MemoryError> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| MemoryError::Generic(format!("Failed to create directory: {e}")))?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| MemoryError::Generic(format!("Failed to open database: {e}")))?;

        // Initialize base schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_entries (
                key TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                category TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                access_count INTEGER DEFAULT 0,
                trust_score REAL DEFAULT 0.0
            )",
            [],
        )
        .map_err(|e| MemoryError::Generic(format!("Failed to create schema: {e}")))?;

        // Scope isolation columns — added in Memory Control Plane V1.
        // ALTER TABLE is idempotent: errors from duplicate-column additions are silently ignored.
        let _ = conn.execute("ALTER TABLE memory_entries ADD COLUMN session_id TEXT", []);
        let _ = conn.execute("ALTER TABLE memory_entries ADD COLUMN project_id TEXT", []);

        // Create indexes (idempotent)
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_category ON memory_entries(category)",
            [],
        )
        .map_err(|e| MemoryError::Generic(format!("Failed to create index: {e}")))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_created_at ON memory_entries(created_at)",
            [],
        )
        .map_err(|e| MemoryError::Generic(format!("Failed to create index: {e}")))?;

        // Indexes for scope-based queries — partial indexes on the rows that
        // actually carry the scope tag, which keeps them small and selective
        // (most rows are session-only or global, so `idx_memory_project_id`
        // typically covers a few percent of the table).
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_session_id ON memory_entries(session_id) \
             WHERE session_id IS NOT NULL",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_project_id ON memory_entries(project_id) \
             WHERE project_id IS NOT NULL",
            [],
        );

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            scanner: None,
        })
    }

    /// Builder: attach a shared [`ThreatScanner`] so every scope-aware write
    /// goes through `scan_and_redact` before INSERT.  See the `scanner`
    /// field doc for the threat model.
    #[must_use]
    pub fn with_scanner(mut self, scanner: Arc<ThreatScanner>) -> Self {
        self.scanner = Some(scanner);
        self
    }

    /// Parse a database row into a MemoryEntry.
    ///
    /// Expected column order:
    ///   0=key, 1=content, 2=category, 3=created_at, 4=updated_at,
    ///   5=importance, 6=access_count, 7=trust_score,
    ///   8=session_id, 9=project_id
    fn row_to_entry(row: &rusqlite::Row<'_>) -> Result<MemoryEntry, rusqlite::Error> {
        let key: String = row.get(0)?;
        let content: String = row.get(1)?;
        let category_str: String = row.get(2)?;
        let created_at_str: String = row.get(3)?;
        let updated_at_str: String = row.get(4)?;
        let importance: f64 = row.get(5).unwrap_or(0.5);
        let access_count: i64 = row.get(6).unwrap_or(0);
        let trust_score: f64 = row.get(7).unwrap_or(0.0);
        // Columns 8 and 9 are optional scope fields added in Memory Control Plane V1.
        let session_id: Option<String> = row.get(8).unwrap_or(None);
        let project_id: Option<String> = row.get(9).unwrap_or(None);

        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    4,
                    "created_at".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;

        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    5,
                    "updated_at".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;

        Ok(MemoryEntry {
            key,
            content,
            category: parse_category(&category_str),
            created_at,
            updated_at,
            importance,
            access_count: access_count as u64,
            trust_score,
            session_id,
            project_id,
        })
    }

    /// Migrate a legacy `memory.json` (claw-cli format) into the SQLite store (M1).
    ///
    /// The legacy file is a JSON array of [`ClawCliMemoryEntry`] objects with
    /// only the base fields (`key`, `content`, `category`, `created_at`,
    /// `updated_at`). All extended fields (importance, access_count, trust_score,
    /// scope ids) default to neutral values: `importance=0.5`, `access_count=0`,
    /// `trust_score=0.0`, and the entry is stored unscoped (NULL session/project).
    ///
    /// The entire migration runs inside a single SQLite transaction. On any
    /// per-row insert failure the whole transaction is rolled back and the
    /// error is propagated. Calling this on a non-existent path returns
    /// `Ok(0)` so it can be used as an idempotent upgrade hook.
    ///
    /// Returns the number of entries successfully imported.
    #[allow(dead_code)] // Invoked from upgrade path; keep the public API stable.
    pub async fn migrate_from_json(&self, path: &Path) -> Result<usize, MemoryError> {
        if !path.exists() {
            tracing::debug!(
                "[SqliteMemoryProvider] migrate_from_json: no legacy file at {path:?}, skipping"
            );
            return Ok(0);
        }

        let raw = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| MemoryError::Generic(format!("read {path:?} failed: {e}")))?;

        let legacy: Vec<ClawCliMemoryEntry> = serde_json::from_str(&raw)
            .map_err(|e| MemoryError::Generic(format!("parse {path:?} failed: {e}")))?;

        if legacy.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.clone();
        let path_for_log = path.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<usize, MemoryError> {
            let mut c = conn.lock().map_err(|e| MemoryError::Generic(e.to_string()))?;
            let tx = c
                .transaction()
                .map_err(|e| MemoryError::Generic(format!("begin transaction failed: {e}")))?;

            let mut inserted = 0usize;
            for entry in &legacy {
                let now = chrono::Utc::now().to_rfc3339();
                let created = if entry.created_at.is_empty() {
                    now.clone()
                } else {
                    entry.created_at.clone()
                };
                let updated = if entry.updated_at.is_empty() {
                    now.clone()
                } else {
                    entry.updated_at.clone()
                };

                // ON CONFLICT(key) DO NOTHING preserves existing entries when migrating
                // a second time — the migration is idempotent and never overwrites
                // newer SQLite-native data.
                let rows = tx
                    .execute(
                        "INSERT INTO memory_entries
                            (key, content, category, created_at, updated_at,
                             importance, access_count, trust_score,
                             session_id, project_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, 0.5, 0, 0.0, NULL, NULL)
                         ON CONFLICT(key) DO NOTHING",
                        params![
                            entry.key,
                            entry.content,
                            entry.category,
                            created,
                            updated,
                        ],
                    )
                    .map_err(|e| {
                        MemoryError::Generic(format!(
                            "insert key={} during migration failed: {e}",
                            entry.key
                        ))
                    })?;
                inserted += rows;
            }

            tx.commit()
                .map_err(|e| MemoryError::Generic(format!("commit failed: {e}")))?;
            tracing::info!(
                "[SqliteMemoryProvider] migrate_from_json: imported {inserted}/{total} entries from {path:?}",
                total = legacy.len(),
                path = path_for_log,
            );
            Ok(inserted)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("migration task panicked: {e}")))?
    }
}

/// Build the SQL `WHERE` fragment that enforces the three-tier visibility rules
/// described on `recall_scoped`.
///
/// Returns a fragment that uses positional parameters `?1`..`?N`, where `N`
/// matches the length of the slice returned by [`scope_visibility_params`].
/// The returned fragment is intentionally wrapped in parentheses by the caller
/// so it can be combined with additional `AND` clauses (e.g. category filter).
fn scope_visibility_clause(session_id: Option<&str>, project_id: Option<&str>) -> String {
    match (session_id, project_id) {
        // session + project: own session entries OR project-level entries OR global.
        (Some(_), Some(_)) => "session_id = ?1 \
             OR (session_id IS NULL AND project_id = ?2) \
             OR (session_id IS NULL AND project_id IS NULL)"
            .to_string(),
        // session only: own session entries OR global (legacy callers without project).
        (Some(_), None) => "session_id = ?1 \
             OR (session_id IS NULL AND project_id IS NULL)"
            .to_string(),
        // project only: project-level entries OR global. No session entries leak across.
        (None, Some(_)) => "(session_id IS NULL AND project_id = ?1) \
             OR (session_id IS NULL AND project_id IS NULL)"
            .to_string(),
        // global: only truly unscoped entries.
        (None, None) => "session_id IS NULL AND project_id IS NULL".to_string(),
    }
}

/// Build the bound parameter list aligned with [`scope_visibility_clause`].
fn scope_visibility_params(session_id: Option<&str>, project_id: Option<&str>) -> Vec<String> {
    match (session_id, project_id) {
        (Some(s), Some(p)) => vec![s.to_string(), p.to_string()],
        (Some(s), None) => vec![s.to_string()],
        (None, Some(p)) => vec![p.to_string()],
        (None, None) => Vec::new(),
    }
}

/// SQL `ORDER BY` fragment that ranks recall results by **scope tier first**,
/// then by importance / access_count / recency.  Lower tier number wins.
///
/// Tiering (must match `scope_visibility_clause` so every visible row has a
/// well-defined tier):
///   - 0 = session-owned (`session_id` matches the caller)
///   - 1 = project-owned (`session_id IS NULL AND project_id` matches)
///   - 2 = global / legacy (both NULL)
///
/// Within a tier we surface the most "trusted-and-frequently-used" memory
/// first: `importance DESC, access_count DESC, updated_at DESC`.
///
/// Returns a clause without the leading `ORDER BY` keyword so the caller can
/// inline it after `WHERE (...)`.
fn scope_priority_order_by(session_id: Option<&str>, project_id: Option<&str>) -> String {
    // The CASE expression is parameter-free and safe to inline because both
    // operands come from already-bound `?N` slots; we just *reference* them.
    let case_expr = match (session_id, project_id) {
        (Some(_), Some(_)) => {
            // ?1 = session, ?2 = project — same parameter positions used by
            // `scope_visibility_clause`.
            "CASE \
                 WHEN session_id = ?1 THEN 0 \
                 WHEN session_id IS NULL AND project_id = ?2 THEN 1 \
                 ELSE 2 \
             END"
        }
        (Some(_), None) => {
            // ?1 = session.  No project, so tier 1 collapses into tier 2.
            "CASE WHEN session_id = ?1 THEN 0 ELSE 2 END"
        }
        (None, Some(_)) => {
            // ?1 = project.  No session-tier rows are visible; collapse to 1/2.
            "CASE WHEN session_id IS NULL AND project_id = ?1 THEN 1 ELSE 2 END"
        }
        (None, None) => {
            // Only globals are visible; constant tier.
            "2"
        }
    };
    format!(
        "{case_expr} ASC, \
         importance DESC, \
         access_count DESC, \
         updated_at DESC"
    )
}

/// Parse a category string, handling both known and custom categories
fn parse_category(s: &str) -> MemoryCategory {
    match s {
        "core" => MemoryCategory::Core,
        "daily" => MemoryCategory::Daily,
        "conversation" => MemoryCategory::Conversation,
        other => MemoryCategory::Custom(other.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_provider() -> (SqliteMemoryProvider, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_memory.db");
        let provider = SqliteMemoryProvider::new(db_path).unwrap();
        (provider, temp_dir)
    }

    #[tokio::test]
    async fn store_and_recall() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("test_key", "Hello world", MemoryCategory::Core)
            .await
            .unwrap();

        let results = provider.recall("test_key", None, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "test_key");
        assert_eq!(results[0].content, "Hello world");
        assert_eq!(results[0].importance, 0.5);
        assert_eq!(results[0].access_count, 0);
    }

    #[tokio::test]
    async fn store_upserts_existing() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("key", "First value", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("key", "Second value", MemoryCategory::Core)
            .await
            .unwrap();

        let results = provider.recall("key", None, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Second value");
    }

    #[tokio::test]
    async fn delete_entry() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("to_delete", "value", MemoryCategory::Core)
            .await
            .unwrap();
        provider.delete("to_delete").await.unwrap();

        let results = provider.recall("to_delete", None, 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn purge_category() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("a", "1", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("b", "2", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("c", "3", MemoryCategory::Daily)
            .await
            .unwrap();

        provider.purge_category("core").await.unwrap();

        let core = provider.recall("", Some("core"), 10).await.unwrap();
        assert!(core.is_empty());

        let daily = provider.recall("", Some("daily"), 10).await.unwrap();
        assert_eq!(daily.len(), 1);
    }

    #[tokio::test]
    async fn filter_by_category() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("x", "1", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("y", "2", MemoryCategory::Daily)
            .await
            .unwrap();

        let core_results = provider.recall("", Some("core"), 10).await.unwrap();
        assert_eq!(core_results.len(), 1);
        assert_eq!(core_results[0].key, "x");
    }

    #[tokio::test]
    async fn recall_with_limit() {
        let (provider, _temp) = create_test_provider();

        for i in 0..5 {
            provider
                .store(
                    &format!("key_{i}"),
                    &format!("value_{i}"),
                    MemoryCategory::Core,
                )
                .await
                .unwrap();
        }

        let all = provider.recall("", None, 10).await.unwrap();
        assert_eq!(all.len(), 5);
    }

    #[tokio::test]
    async fn export_all() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("a", "1", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("b", "2", MemoryCategory::Daily)
            .await
            .unwrap();

        let exported = provider.export(None).await.unwrap();
        assert_eq!(exported.len(), 2);

        let core_export = provider.export(Some("core")).await.unwrap();
        assert_eq!(core_export.len(), 1);
    }

    #[tokio::test]
    async fn persists_across_reopens() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_memory.db");

        let provider = SqliteMemoryProvider::new(db_path.clone()).unwrap();
        provider
            .store("persist_key", "persist_value", MemoryCategory::Core)
            .await
            .unwrap();

        // Reopen the same database file
        let provider2 = SqliteMemoryProvider::new(db_path).unwrap();
        let results = provider2.recall("persist_key", None, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "persist_value");
    }

    /// Verifies that `apply_importance_decay` writes updated importance values
    /// back to SQLite for all matching entries.
    ///
    /// For brand-new entries, `WeibullDecay::compute_importance` computes:
    ///   `(base + trust_boost) * decay_factor`
    /// where `trust_boost = (0.0 + 1.0) * 0.05 = 0.05` and `decay_factor ≈ 1.0`
    /// for age≈0.  This results in importance ≈ 0.55, not ≤ 0.5.
    /// The test asserts that the value is actually written back (changed from default).
    #[tokio::test]
    async fn apply_importance_decay_updates_entries() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("decay_a", "data a", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("decay_b", "data b", MemoryCategory::Daily)
            .await
            .unwrap();

        // Use export to retrieve all entries and find by key — avoids recall's
        // LIMIT clause filtering out the target when multiple entries exist.
        let all_before = provider.export(None).await.unwrap();
        let before_a = all_before
            .iter()
            .find(|e| e.key == "decay_a")
            .expect("decay_a should exist before decay");
        assert!(
            (before_a.importance - 0.5).abs() < 1e-6,
            "initial importance should be 0.5, got {}",
            before_a.importance
        );

        // Apply decay with default parameters (7-day lambda, k=1.2).
        let updated = provider.apply_importance_decay(168.0, 1.2).await.unwrap();
        assert_eq!(updated, 2, "should have updated 2 entries");

        // For a brand-new entry (age≈0) with neutral trust (score=0), the
        // compute_importance formula gives ≈ 0.55 — different from the
        // stored default 0.5.
        let all_after = provider.export(None).await.unwrap();
        let after_a = all_after
            .iter()
            .find(|e| e.key == "decay_a")
            .expect("decay_a should exist after decay");
        assert!(
            (after_a.importance - before_a.importance).abs() > 1e-9,
            "importance should have changed after decay, before={} after={}",
            before_a.importance,
            after_a.importance
        );
    }

    /// Test that a store_scoped entry is visible to recall_scoped with the same session,
    /// and invisible to recall_scoped with a different session.
    ///
    /// This is the core P0 isolation property of the Memory Control Plane.
    #[tokio::test]
    async fn store_scoped_entry_is_visible_only_within_same_session() {
        use crate::modules::memory::scope::MemoryScopeResolver;

        let (provider, _temp) = create_test_provider();

        let scope_a = MemoryScopeResolver::resolve(Some("session-a"), None, None);
        let scope_b = MemoryScopeResolver::resolve(Some("session-b"), None, None);

        // Store entry scoped to session-a.
        provider
            .store_scoped(
                "secret_key",
                "session-a secret",
                MemoryCategory::Core,
                &scope_a,
            )
            .await
            .unwrap();

        // session-a can recall the entry.
        let results_a = provider
            .recall_scoped("secret_key", None, 10, &scope_a)
            .await
            .unwrap();
        assert_eq!(results_a.len(), 1, "session-a should see its own entry");
        assert_eq!(results_a[0].content, "session-a secret");

        // session-b cannot see session-a's entry.
        let results_b = provider
            .recall_scoped("secret_key", None, 10, &scope_b)
            .await
            .unwrap();
        assert!(
            results_b.is_empty(),
            "session-b must not see session-a's scoped entry; got: {:?}",
            results_b
        );
    }

    #[tokio::test]
    async fn migrate_from_json_imports_entries_idempotently() {
        let (provider, temp) = create_test_provider();
        let json_path = temp.path().join("memory.json");

        let payload = r#"[
            {
                "key": "claw_a",
                "content": "claw content A",
                "category": "core",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-02T00:00:00Z"
            },
            {
                "key": "claw_b",
                "content": "claw content B",
                "category": "daily",
                "created_at": "2024-01-03T00:00:00Z",
                "updated_at": "2024-01-04T00:00:00Z"
            }
        ]"#;
        std::fs::write(&json_path, payload).unwrap();

        let imported = provider.migrate_from_json(&json_path).await.unwrap();
        assert_eq!(imported, 2);

        let exported = provider.export(None).await.unwrap();
        assert_eq!(exported.len(), 2);
        assert!(exported.iter().any(|e| e.key == "claw_a"));
        assert!(exported.iter().any(|e| e.key == "claw_b"));

        // Second migration is a no-op — ON CONFLICT DO NOTHING preserves originals.
        let imported_again = provider.migrate_from_json(&json_path).await.unwrap();
        assert_eq!(imported_again, 0);

        // Missing file returns Ok(0).
        let missing = provider
            .migrate_from_json(&temp.path().join("nope.json"))
            .await
            .unwrap();
        assert_eq!(missing, 0);
    }

    /// Test that an unscoped (global) entry stored via store() is visible to
    /// recall_scoped with any session — backward-compat requirement.
    #[tokio::test]
    async fn global_entry_visible_to_all_sessions() {
        use crate::modules::memory::scope::MemoryScopeResolver;

        let (provider, _temp) = create_test_provider();

        let scope_any = MemoryScopeResolver::resolve(Some("session-x"), None, None);

        // Store a global (unscoped) entry via the legacy store() method.
        provider
            .store("global_key", "global fact", MemoryCategory::Core)
            .await
            .unwrap();

        // Any session can see global entries because recall_scoped includes
        // WHERE (session_id = ?1 OR session_id IS NULL).
        let results = provider
            .recall_scoped("global_key", None, 10, &scope_any)
            .await
            .unwrap();
        assert_eq!(
            results.len(),
            1,
            "global entry should be visible to all sessions"
        );
        assert_eq!(results[0].content, "global fact");
    }

    // -----------------------------------------------------------------------
    // Three-tier scope visibility regression suite — covers the rules in
    // `recall_scoped` / `scope_visibility_clause`.
    //
    // Each test stores entries via `store_scoped` with explicit scopes and
    // asserts the visibility set seen from various caller scopes.  We assert
    // exact key sets (sorted) to make accidental leaks very loud.

    use crate::modules::memory::scope::MemoryScopeResolver;

    /// Helper: store a scoped entry from a tuple `(key, session, project)`.
    async fn store_scope(
        provider: &SqliteMemoryProvider,
        key: &str,
        session_id: Option<&str>,
        project_id: Option<&str>,
    ) {
        let scope = MemoryScopeResolver::resolve(session_id, project_id, None);
        provider
            .store_scoped(key, key, MemoryCategory::Core, &scope)
            .await
            .unwrap();
    }

    /// Helper: collect sorted keys returned by `recall_scoped`.
    async fn recall_keys(
        provider: &SqliteMemoryProvider,
        session_id: Option<&str>,
        project_id: Option<&str>,
    ) -> Vec<String> {
        let scope = MemoryScopeResolver::resolve(session_id, project_id, None);
        let mut out: Vec<String> = provider
            .recall_scoped("", None, 100, &scope)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.key)
            .collect();
        out.sort();
        out
    }

    /// (1) session-scoped entry is visible to its own session — and that
    ///     session also sees its project's project-level entries plus globals.
    /// (2) session-scoped entry is invisible to other sessions in the same
    ///     project (sessions are leaves, not shared).
    #[tokio::test]
    async fn session_entry_visible_to_own_session_only() {
        let (provider, _temp) = create_test_provider();

        // Same-project, two sessions.
        store_scope(&provider, "sess_a_only", Some("sess-a"), Some("proj-1")).await;
        store_scope(&provider, "sess_b_only", Some("sess-b"), Some("proj-1")).await;
        // Project-level (no session) and global entries for cross-checks.
        store_scope(&provider, "proj_1_shared", None, Some("proj-1")).await;
        store_scope(&provider, "global_shared", None, None).await;

        let from_a = recall_keys(&provider, Some("sess-a"), Some("proj-1")).await;
        assert_eq!(
            from_a,
            vec![
                "global_shared".to_string(),
                "proj_1_shared".to_string(),
                "sess_a_only".to_string(),
            ],
            "sess-a should see its own + project-level + global; got {:?}",
            from_a
        );

        let from_b = recall_keys(&provider, Some("sess-b"), Some("proj-1")).await;
        assert!(
            !from_b.contains(&"sess_a_only".to_string()),
            "sess-b must NOT see sess-a's session entry; got {:?}",
            from_b
        );
    }

    /// (3) project-scoped entry is visible across different sessions of the
    ///     SAME project.
    #[tokio::test]
    async fn project_entry_visible_across_sessions_of_same_project() {
        let (provider, _temp) = create_test_provider();

        store_scope(&provider, "proj_1_shared", None, Some("proj-1")).await;

        let from_a = recall_keys(&provider, Some("sess-a"), Some("proj-1")).await;
        let from_b = recall_keys(&provider, Some("sess-b"), Some("proj-1")).await;

        assert!(
            from_a.contains(&"proj_1_shared".to_string()),
            "sess-a should see project entry; got {:?}",
            from_a
        );
        assert!(
            from_b.contains(&"proj_1_shared".to_string()),
            "sess-b should see project entry too; got {:?}",
            from_b
        );
    }

    /// (4) project-scoped entry is invisible to OTHER projects, regardless of
    ///     whether the caller is session-bound.
    #[tokio::test]
    async fn project_entry_isolated_from_other_project() {
        let (provider, _temp) = create_test_provider();

        store_scope(&provider, "proj_1_secret", None, Some("proj-1")).await;
        store_scope(&provider, "proj_2_secret", None, Some("proj-2")).await;

        // Caller in proj-1 must not see proj-2's entry, and vice versa.
        let from_proj1 = recall_keys(&provider, Some("sess-x"), Some("proj-1")).await;
        assert!(
            !from_proj1.contains(&"proj_2_secret".to_string()),
            "proj-1 caller leaked proj-2 entry; got {:?}",
            from_proj1
        );

        let from_proj2 = recall_keys(&provider, Some("sess-y"), Some("proj-2")).await;
        assert!(
            !from_proj2.contains(&"proj_1_secret".to_string()),
            "proj-2 caller leaked proj-1 entry; got {:?}",
            from_proj2
        );

        // Project-only scope (no session) should also enforce isolation.
        let proj1_only = recall_keys(&provider, None, Some("proj-1")).await;
        assert!(
            proj1_only.contains(&"proj_1_secret".to_string())
                && !proj1_only.contains(&"proj_2_secret".to_string()),
            "project-only scope failed isolation; got {:?}",
            proj1_only
        );
    }

    /// (5) global entry is visible from every scope flavour: session+project,
    ///     project-only, and global.
    #[tokio::test]
    async fn global_entry_visible_from_every_scope() {
        let (provider, _temp) = create_test_provider();

        store_scope(&provider, "global_x", None, None).await;

        let from_session = recall_keys(&provider, Some("sess-a"), Some("proj-1")).await;
        let from_project = recall_keys(&provider, None, Some("proj-1")).await;
        let from_global = recall_keys(&provider, None, None).await;

        assert!(from_session.contains(&"global_x".to_string()));
        assert!(from_project.contains(&"global_x".to_string()));
        assert!(from_global.contains(&"global_x".to_string()));
    }

    /// Project-aware ranking: when a session caller can see entries from all
    /// three tiers, the session-tier rows must surface FIRST, then project,
    /// then global — regardless of `updated_at` order.
    #[tokio::test]
    async fn recall_ranks_session_above_project_above_global() {
        let (provider, _temp) = create_test_provider();

        // Insert in reverse-priority order so a naive `ORDER BY updated_at`
        // would put global first; the scope-tier ranking must override that.
        store_scope(&provider, "g_first", None, None).await;
        store_scope(&provider, "p_then", None, Some("proj-1")).await;
        store_scope(&provider, "s_last", Some("sess-a"), Some("proj-1")).await;

        let scope = MemoryScopeResolver::resolve(Some("sess-a"), Some("proj-1"), None);
        let ordered: Vec<String> = provider
            .recall_scoped("", None, 10, &scope)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.key)
            .collect();

        assert_eq!(
            ordered,
            vec![
                "s_last".to_string(),
                "p_then".to_string(),
                "g_first".to_string(),
            ],
            "expected session > project > global ordering, got {:?}",
            ordered
        );
    }

    /// Within the same scope tier, higher `importance` wins over recency.
    /// We use `apply_importance_decay` then a manual UPDATE to set deterministic
    /// importance values, since `store_scoped` always defaults to 0.5.
    #[tokio::test]
    async fn recall_within_tier_orders_by_importance_then_access_count() {
        let (provider, _temp) = create_test_provider();

        // Two same-tier entries.
        store_scope(&provider, "low_imp", None, None).await;
        store_scope(&provider, "high_imp", None, None).await;

        // Bump high_imp's importance and access_count via direct SQL — this is
        // a test-only path that bypasses store_scoped to set up a deterministic
        // ranking signal.
        {
            let c = provider.conn.lock().unwrap();
            c.execute(
                "UPDATE memory_entries SET importance = 0.95, access_count = 10 WHERE key = ?1",
                params!["high_imp"],
            )
            .unwrap();
            c.execute(
                "UPDATE memory_entries SET importance = 0.10, access_count = 0 WHERE key = ?1",
                params!["low_imp"],
            )
            .unwrap();
        }

        let scope = MemoryExecutionScope::global();
        let ordered: Vec<String> = provider
            .recall_scoped("", None, 10, &scope)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.key)
            .collect();

        assert_eq!(
            ordered,
            vec!["high_imp".to_string(), "low_imp".to_string()],
            "high-importance entry must surface first within the same tier; got {:?}",
            ordered
        );
    }

    /// Belt-and-braces: global scope must NOT see any session- or project-
    /// scoped entries, only true globals.
    #[tokio::test]
    async fn global_scope_sees_only_global_entries() {
        let (provider, _temp) = create_test_provider();

        store_scope(&provider, "sess_only", Some("sess-a"), Some("proj-1")).await;
        store_scope(&provider, "proj_only", None, Some("proj-1")).await;
        store_scope(&provider, "true_global", None, None).await;

        let from_global = recall_keys(&provider, None, None).await;
        assert_eq!(
            from_global,
            vec!["true_global".to_string()],
            "global scope must only see truly unscoped entries; got {:?}",
            from_global
        );
    }

    /// `demote_scope` is wired through the trait default to `promote_scope`,
    /// so a successful demote must persist the narrower scope tags exactly
    /// like a promote does.  Round-trip: `global → project → session`.
    #[tokio::test]
    async fn demote_scope_round_trip_global_to_project_to_session() {
        let (provider, _t) = create_test_provider();

        // Seed: a global entry (no session/project tags).
        let global_scope = MemoryExecutionScope {
            session_id: None,
            project_id: None,
            workdir: None,
        };
        provider
            .store_scoped(
                "demoteable",
                "secret",
                MemoryCategory::Conversation,
                &global_scope,
            )
            .await
            .unwrap();

        // Demote 1: global → project P1.
        let project_scope = MemoryExecutionScope {
            session_id: None,
            project_id: Some("P1".to_string()),
            workdir: None,
        };
        provider
            .demote_scope("demoteable", &project_scope)
            .await
            .unwrap();
        let after_p = provider.export(None).await.unwrap();
        let entry_p = after_p.iter().find(|e| e.key == "demoteable").unwrap();
        assert_eq!(entry_p.session_id, None);
        assert_eq!(entry_p.project_id.as_deref(), Some("P1"));

        // Demote 2: project → session S1 (still owned by P1).
        let session_scope = MemoryExecutionScope {
            session_id: Some("S1".to_string()),
            project_id: Some("P1".to_string()),
            workdir: None,
        };
        provider
            .demote_scope("demoteable", &session_scope)
            .await
            .unwrap();
        let after_s = provider.export(None).await.unwrap();
        let entry_s = after_s.iter().find(|e| e.key == "demoteable").unwrap();
        assert_eq!(entry_s.session_id.as_deref(), Some("S1"));
        assert_eq!(entry_s.project_id.as_deref(), Some("P1"));

        // Sanity: a non-existent key surfaces KeyNotFound, not silent success.
        let err = provider
            .demote_scope("missing", &session_scope)
            .await
            .unwrap_err();
        assert!(
            matches!(err, MemoryError::KeyNotFound(ref k) if k == "missing"),
            "expected KeyNotFound for missing key, got {err:?}"
        );
    }
}
