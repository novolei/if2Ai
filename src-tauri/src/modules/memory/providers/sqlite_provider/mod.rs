//! SQLite-backed memory provider
//!
//! Persists memory entries to a SQLite database, surviving process restarts.
//! Uses rusqlite (sync) wrapped in spawn_blocking for async compatibility.

mod provider_impl;
mod scope;

use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::modules::memory::compat::ClawCliMemoryEntry;
use crate::modules::memory::security::ThreatScanner;
// MemoryCategory + MemoryProvider + MemoryExecutionScope re-exported for
// `super::*` glob in tests.rs (kept byte-identical so the moved test block compiles).
#[allow(unused_imports)]
use crate::modules::memory::scope::MemoryExecutionScope;
#[allow(unused_imports)]
use crate::modules::memory::{MemoryCategory, MemoryEntry, MemoryError, MemoryProvider};

use scope::parse_category;

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

        // MEM-MOD-P0 — schema is now managed by the versioned migration
        // runner.  Pre-P0 installs that already have the canonical
        // `memory_entries` table get a synthetic v1 row inserted into
        // `schema_migrations` so v2+ migrations roll forward without
        // re-running the v1 `CREATE TABLE`.
        let report = crate::modules::memory::migrations::run_migrations(
            &conn,
            crate::modules::memory::migrations::memory_migrations(),
            Some("memory_entries"),
        )
        .map_err(|err| MemoryError::Generic(err.to_string()))?;
        if !report.applied.is_empty() || report.v1_backfilled {
            tracing::info!(
                applied = ?report.applied,
                skipped = ?report.skipped,
                v1_backfilled = report.v1_backfilled,
                "[memory.schema] migrations executed"
            );
        }

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

#[cfg(test)]
mod tests;
