//! MEM-MOD-P0 — versioned SQLite schema migrations.
//!
//! Replaces the historical "CREATE TABLE IF NOT EXISTS + ALTER TABLE +
//! swallow errors" pattern with a tiny migration runner that:
//!
//! 1. Tracks applied versions in a `schema_migrations` table so we can
//!    tell, post-mortem, exactly which migrations a corrupt DB has seen.
//! 2. Runs each migration inside a single SQLite transaction so a
//!    partial failure cannot leave the schema half-applied.
//! 3. Backfills v1 transparently: fresh installs apply v1 from scratch;
//!    upgraded installs that already have `memory_entries` (from the
//!    old `IF NOT EXISTS` path) are recorded as "v1 already applied"
//!    so v2+ migrations roll forward without touching v1.
//!
//! This module is intentionally generic — `Migration` can describe any
//! table, any module.  Subsequent Packs will register their own slices
//! (P3 `memory_links`, P6 `valid_from`/`valid_to`, P7 `learned_traits`)
//! by appending to the slice returned by [`memory_migrations`].
//!
//! # Why a `fn` pointer for `up`?
//!
//! `Migration` lives in a `&'static [Migration]` slice (no allocations
//! at startup) so we use a plain `fn` rather than a boxed closure.
//! Migrations rarely need captured state; when they do, they can do
//! work via free functions defined alongside the slice.

use rusqlite::Connection;
use std::fmt;

/// One ordered, idempotent schema change.
///
/// `version` MUST be globally unique across the slice and monotonically
/// increasing.  `up` MUST be safe to run on an empty database (i.e. it
/// uses `IF NOT EXISTS` clauses or assumes nothing).  The runner wraps
/// the call in a transaction, so `up` should NOT manage transactions
/// itself.
#[derive(Clone, Copy)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub up: fn(&Connection) -> rusqlite::Result<()>,
}

impl fmt::Debug for Migration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Migration")
            .field("version", &self.version)
            .field("name", &self.name)
            .finish()
    }
}

/// Result of a successful [`run_migrations`] call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Versions actually applied during *this* run (in order).
    pub applied: Vec<u32>,
    /// Versions that were already in `schema_migrations` and skipped.
    pub skipped: Vec<u32>,
    /// `true` if a v1 backfill row was inserted for an upgraded DB
    /// (i.e. `memory_entries` already existed but `schema_migrations`
    /// was empty). Useful for boot-time logging.
    pub v1_backfilled: bool,
}

/// Errors from the migration runner.  `String` payload (not a richer
/// enum) because `MemoryError` already swallows everything as `Generic`
/// at the call site and we want to surface the SQLite message verbatim.
#[derive(Debug)]
pub struct MigrationError(pub String);

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "schema migration failed: {}", self.0)
    }
}

impl std::error::Error for MigrationError {}

impl From<rusqlite::Error> for MigrationError {
    fn from(err: rusqlite::Error) -> Self {
        Self(err.to_string())
    }
}

/// Apply every migration in `migrations` whose `version` is greater
/// than the highest version already in `schema_migrations`.
///
/// Behaviour:
/// - Creates `schema_migrations` if missing.
/// - If `schema_migrations` is empty AND `legacy_table` exists in the
///   DB, inserts a synthetic row recording v1 as already applied so
///   pre-P0 installs roll forward into v2+ without re-running the v1
///   `CREATE TABLE`.
/// - Iterates `migrations` in declaration order (caller must keep them
///   sorted by `version`).  Each `up` runs inside its own transaction;
///   a failure rolls that migration back and aborts the whole run.
///
/// Pass `None` for `legacy_table` when bootstrapping a brand-new domain
/// that has no pre-P0 schema to backfill.
pub fn run_migrations(
    conn: &Connection,
    migrations: &[Migration],
    legacy_table: Option<&str>,
) -> Result<MigrationReport, MigrationError> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at TEXT NOT NULL
        )",
        [],
    )?;

    let max_existing: Option<u32> = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<u32>>(0)
        })
        .unwrap_or(None);

    let mut report = MigrationReport::default();

    // Backfill v1 for upgraded installs that already have the canonical
    // table from the old `CREATE TABLE IF NOT EXISTS` path.
    if max_existing.is_none() {
        if let Some(table) = legacy_table {
            if table_exists(conn, table)? {
                conn.execute(
                    "INSERT OR IGNORE INTO schema_migrations (version, name, applied_at)
                     VALUES (1, 'backfilled_pre_p0', ?1)",
                    rusqlite::params![chrono::Utc::now().to_rfc3339()],
                )?;
                report.v1_backfilled = true;
            }
        }
    }

    let already_applied = list_applied_versions(conn)?;

    for migration in migrations {
        if already_applied.contains(&migration.version) {
            report.skipped.push(migration.version);
            continue;
        }

        let tx_label = format!("v{} {}", migration.version, migration.name);
        conn.execute_batch("BEGIN")
            .map_err(|err| MigrationError(format!("failed to start tx for {tx_label}: {err}")))?;

        match (migration.up)(conn).and_then(|()| {
            conn.execute(
                "INSERT INTO schema_migrations (version, name, applied_at)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    migration.version,
                    migration.name,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .map(|_| ())
        }) {
            Ok(()) => {
                conn.execute_batch("COMMIT").map_err(|err| {
                    MigrationError(format!("commit failed for {tx_label}: {err}"))
                })?;
                report.applied.push(migration.version);
            }
            Err(err) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(MigrationError(format!(
                    "migration {tx_label} failed: {err}"
                )));
            }
        }
    }

    Ok(report)
}

fn list_applied_versions(conn: &Connection) -> Result<Vec<u32>, MigrationError> {
    let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let rows = stmt.query_map([], |row| row.get::<_, u32>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, MigrationError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        rusqlite::params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

// ────────────────────────────────────────────────────────────────────
// memory_entries migrations
// ────────────────────────────────────────────────────────────────────

/// All known migrations for the `memory_entries` domain.  P0 ships v1
/// only — every later Pack appends to this slice.  Keep the slice
/// `'static` so [`run_migrations`] takes no allocation at boot.
#[must_use]
pub fn memory_migrations() -> &'static [Migration] {
    MEMORY_MIGRATIONS
}

const MEMORY_MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "memory_entries_initial",
        up: memory_v1_initial,
    },
    Migration {
        version: 2,
        name: "memory_links_table",
        up: memory_v2_links,
    },
    Migration {
        version: 3,
        name: "memory_entry_history",
        up: memory_v3_history,
    },
    Migration {
        version: 4,
        name: "learned_traits_table",
        up: memory_v4_learned_traits,
    },
    Migration {
        version: 5,
        name: "conversation_recall_fts",
        up: memory_v5_conversation_recall_fts,
    },
    Migration {
        version: 6,
        name: "conversation_recall_embeddings",
        up: memory_v6_conversation_recall_embeddings,
    },
    Migration {
        version: 7,
        name: "quality_scoring_columns",
        up: memory_v7_quality_scoring,
    },
    Migration {
        version: 8,
        name: "cognitive_layer_columns",
        up: memory_v8_cognitive_layer,
    },
];

/// v1 — the historical schema, captured as a single migration.  Stays
/// idempotent so re-running it on a populated DB is a no-op.
fn memory_v1_initial(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_entries (
            key          TEXT PRIMARY KEY,
            content      TEXT NOT NULL,
            category     TEXT NOT NULL,
            created_at   TEXT NOT NULL,
            updated_at   TEXT NOT NULL,
            importance   REAL DEFAULT 0.5,
            access_count INTEGER DEFAULT 0,
            trust_score  REAL DEFAULT 0.0
        )",
        [],
    )?;

    // Scope columns landed in Memory Control Plane V1 — keep
    // `ALTER TABLE` calls behind `IF NOT EXISTS` semantics by ignoring
    // duplicate-column errors (SQLite has no native idempotent ALTER).
    let _ = conn.execute("ALTER TABLE memory_entries ADD COLUMN session_id TEXT", []);
    let _ = conn.execute("ALTER TABLE memory_entries ADD COLUMN project_id TEXT", []);

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_category ON memory_entries(category)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_created_at ON memory_entries(created_at)",
        [],
    )?;
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
    Ok(())
}

/// MEM-MOD-P3 — v2: lightweight typed adjacency table for the
/// `memory_link` / `memory_consolidate` tools.
///
/// Schema is intentionally minimal — `link_type` is a free-form
/// string so future tools (Mem0-style "supersedes", Letta-style
/// "evidence_for") can co-exist without a schema bump.  ON DELETE
/// CASCADE keeps the table clean when a memory is removed.
fn memory_v2_links(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_links (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            source_key  TEXT NOT NULL,
            target_key  TEXT NOT NULL,
            link_type   TEXT NOT NULL,
            created_at  TEXT NOT NULL,
            FOREIGN KEY (source_key) REFERENCES memory_entries(key) ON DELETE CASCADE,
            FOREIGN KEY (target_key) REFERENCES memory_entries(key) ON DELETE CASCADE,
            UNIQUE (source_key, target_key, link_type)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_links_source ON memory_links(source_key)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_memory_links_target ON memory_links(target_key)",
        [],
    )?;
    Ok(())
}

/// MEM-MOD-P6 — v3: temporal versioning audit table.
///
/// Every `memory_update` / `memory_consolidate` writes a snapshot of
/// the *previous* row here with `valid_from = old.created_at` and
/// `valid_to = now`. The current `memory_entries` row remains the
/// source of truth for "what is the value right now"; this table is
/// the answer to "what *was* the value at time T".
///
/// Indexed on `(key, valid_from)` so `memory_recall_at_time(t)` can
/// run as a covering range scan without a sort.
fn memory_v3_history(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_entry_history (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            key          TEXT NOT NULL,
            content      TEXT NOT NULL,
            category     TEXT NOT NULL,
            importance   REAL NOT NULL,
            trust_score  REAL NOT NULL,
            valid_from   TEXT NOT NULL,
            valid_to     TEXT NOT NULL,
            source       TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_key_from
         ON memory_entry_history(key, valid_from)",
        [],
    )?;
    Ok(())
}

/// MEM-MOD-P7 — v4: cross-session learned traits ("the user is X").
///
/// Each row is one durable observation about the user that survives
/// session boundaries.  The agent's `trait_extractor` writes here
/// after a session ends; the `Learned Traits` prompt block reads from
/// here on every turn.  `disagreed_at` lets the user retire a trait
/// they don't endorse without losing the audit trail.
fn memory_v4_learned_traits(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS learned_traits (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            trait_text      TEXT NOT NULL,
            evidence_count  INTEGER NOT NULL DEFAULT 1,
            confidence      REAL NOT NULL DEFAULT 0.5,
            first_seen_at   TEXT NOT NULL,
            last_updated_at TEXT NOT NULL,
            disagreed_at    TEXT,
            source_session  TEXT
        )",
        [],
    )?;
    // Disagreed traits are excluded from the prompt block — partial
    // index keeps the hot read path small.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_traits_active
         ON learned_traits(last_updated_at DESC)
         WHERE disagreed_at IS NULL",
        [],
    )?;
    Ok(())
}

/// P1-7 — turn-level conversation snippets for `conversation_search` (FTS5).
///
/// Vector hybrid scoring can be layered later; the FTS surface is the
/// durable cross-turn retrieval primitive on SQLite builds with FTS5.
fn memory_v5_conversation_recall_fts(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS conversation_recall_fts USING fts5(
            session_id UNINDEXED,
            project_id UNINDEXED,
            turn_id UNINDEXED,
            body,
            tokenize = 'porter unicode61'
        )",
        [],
    )?;
    Ok(())
}

/// P1-7 — turn-level embedding rows for hybrid recall ranking.
///
/// Stores one embedding per (session, turn). Body text stays in the FTS5
/// virtual table; we only persist the dense vector here as a little-endian
/// f32 BLOB (see [`conversation_recall_vector`]).
fn memory_v6_conversation_recall_embeddings(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS conversation_recall_embeddings (
            session_id  TEXT NOT NULL,
            project_id  TEXT,
            turn_id     TEXT NOT NULL,
            embedding   BLOB NOT NULL,
            PRIMARY KEY (session_id, turn_id)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conv_recall_embed_session
         ON conversation_recall_embeddings(session_id)",
        [],
    )?;
    Ok(())
}

fn memory_v7_quality_scoring(conn: &Connection) -> rusqlite::Result<()> {
    fn add_column_if_not_exists(conn: &Connection, sql: &str) -> rusqlite::Result<()> {
        match conn.execute(sql, []) {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate column name") => Ok(()),
            Err(e) => Err(e),
        }
    }

    add_column_if_not_exists(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN quality_score REAL DEFAULT 0.5",
    )?;
    add_column_if_not_exists(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN source_reliability REAL DEFAULT 0.5",
    )?;
    add_column_if_not_exists(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN last_validated_at TEXT",
    )?;
    add_column_if_not_exists(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN contradiction_count INTEGER DEFAULT 0",
    )?;
    Ok(())
}

fn memory_v8_cognitive_layer(conn: &Connection) -> rusqlite::Result<()> {
    fn add_col(conn: &Connection, sql: &str) -> rusqlite::Result<()> {
        match conn.execute(sql, []) {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate column name") => Ok(()),
            Err(e) => Err(e),
        }
    }

    add_col(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN cognitive_layer INTEGER DEFAULT 2",
    )?;
    add_col(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN context_tags TEXT DEFAULT '[]'",
    )?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_memory_cognitive_layer ON memory_entries(cognitive_layer);",
    )?;

    // Back-fill cognitive_layer from category for existing rows.
    conn.execute_batch(
        "UPDATE memory_entries SET cognitive_layer = 1 WHERE category = 'conversation';
         UPDATE memory_entries SET cognitive_layer = 2 WHERE category IN ('working', 'daily');
         UPDATE memory_entries SET cognitive_layer = 3 WHERE category IN ('reflection', 'procedural');
         UPDATE memory_entries SET cognitive_layer = 4 WHERE category = 'core';",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn fresh_install_applies_all_migrations() {
        let c = open();
        let report = run_migrations(&c, memory_migrations(), Some("memory_entries")).unwrap();
        assert_eq!(report.applied, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(report.skipped.is_empty());
        assert!(!report.v1_backfilled);
        assert!(table_exists(&c, "schema_migrations").unwrap());
        assert!(table_exists(&c, "memory_entries").unwrap());
        assert!(table_exists(&c, "memory_links").unwrap());
        assert!(table_exists(&c, "memory_entry_history").unwrap());
        assert!(table_exists(&c, "learned_traits").unwrap());
        assert!(table_exists(&c, "conversation_recall_fts").unwrap());
        assert!(table_exists(&c, "conversation_recall_embeddings").unwrap());
    }

    #[test]
    fn upgrade_backfills_v1_for_legacy_db_and_runs_v2() {
        let c = open();
        // Simulate a pre-P0 install: the table exists but the
        // schema_migrations bookkeeping does not.
        c.execute(
            "CREATE TABLE memory_entries (key TEXT PRIMARY KEY, content TEXT, category TEXT, \
             created_at TEXT, updated_at TEXT)",
            [],
        )
        .unwrap();

        let report = run_migrations(&c, memory_migrations(), Some("memory_entries")).unwrap();
        assert!(report.v1_backfilled);
        assert_eq!(
            report.applied,
            vec![2, 3, 4, 5, 6, 7, 8],
            "v1 backfilled, v2..=v8 fresh"
        );
        assert_eq!(report.skipped, vec![1]);
        assert!(table_exists(&c, "memory_links").unwrap());
        assert!(table_exists(&c, "memory_entry_history").unwrap());
        assert!(table_exists(&c, "learned_traits").unwrap());
        assert!(table_exists(&c, "conversation_recall_fts").unwrap());
        assert!(table_exists(&c, "conversation_recall_embeddings").unwrap());
    }

    #[test]
    fn rerun_is_a_noop() {
        let c = open();
        run_migrations(&c, memory_migrations(), Some("memory_entries")).unwrap();
        let report = run_migrations(&c, memory_migrations(), Some("memory_entries")).unwrap();
        assert!(report.applied.is_empty());
        assert_eq!(report.skipped, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn failing_migration_rolls_back() {
        let c = open();
        const BAD: &[Migration] = &[Migration {
            version: 99,
            name: "bad",
            up: |conn| {
                conn.execute("CREATE TABLE will_be_rolled_back (id INTEGER)", [])?;
                // Force a failure after a successful sub-statement.
                conn.execute("INVALID SQL", [])?;
                Ok(())
            },
        }];

        let err = run_migrations(&c, BAD, None).unwrap_err();
        assert!(err.to_string().contains("v99 bad"));
        // Table created in the failed migration MUST not survive.
        assert!(!table_exists(&c, "will_be_rolled_back").unwrap());
        // schema_migrations exists but should NOT contain v99.
        let applied = list_applied_versions(&c).unwrap();
        assert!(!applied.contains(&99));
    }

    #[test]
    fn legacy_table_none_does_not_backfill() {
        let c = open();
        let report = run_migrations(&c, memory_migrations(), None).unwrap();
        assert!(!report.v1_backfilled);
        assert_eq!(report.applied, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[tokio::test]
    async fn v7_columns_round_trip_via_sqlite_provider() {
        // Spec 2026-05-08-a1-pr2-pr6a-memory-foundation-design.md §5.1:
        // open fresh SQLite, apply all migrations, store entry, verify all
        // 6 new columns have default values, call update_decay_scores,
        // re-read, assert values land. Confirm KeyNotFound on missing key.
        use crate::modules::memory::{
            MemoryCategory, MemoryError, MemoryProvider, SqliteMemoryProvider,
        };
        use tempfile::tempdir;

        let dir = tempdir().expect("tempdir");
        let provider =
            SqliteMemoryProvider::new(dir.path().join("memory.db")).expect("provider init");

        // Store an entry — exercises INSERT path that includes new fields.
        // Use Custom("episodic") because the v8 back-fill SQL only matches
        // conversation / working|daily / reflection|procedural / core; a
        // custom category leaves the column at its DEFAULT 2 (Deliberative).
        let key = "v7-rt-key";
        provider
            .store(
                key,
                "round-trip content",
                MemoryCategory::Custom("episodic".to_string()),
            )
            .await
            .expect("store ok");

        // Read back — verify defaults.
        let entries = provider.export(None).await.expect("export ok");
        let entry = entries
            .iter()
            .find(|e| e.key == key)
            .expect("entry present");
        assert!(
            (entry.quality_score - 0.5).abs() < f64::EPSILON,
            "default quality_score should be 0.5, got {}",
            entry.quality_score
        );
        assert!(
            (entry.source_reliability - 0.5).abs() < f64::EPSILON,
            "default source_reliability should be 0.5, got {}",
            entry.source_reliability
        );
        assert!(
            entry.last_validated_at.is_none(),
            "default last_validated_at should be None"
        );
        assert_eq!(entry.contradiction_count, 0);
        assert_eq!(entry.cognitive_layer, 2);
        assert_eq!(entry.context_tags.len(), 0);

        // Update decay scores — exercises the new trait method.
        provider
            .update_decay_scores(key, 0.7, 0.85)
            .await
            .expect("update_decay_scores ok");

        // Re-read — verify values landed.
        let entries = provider.export(None).await.expect("export ok 2");
        let entry = entries
            .iter()
            .find(|e| e.key == key)
            .expect("entry present 2");
        assert!(
            (entry.importance - 0.7).abs() < f64::EPSILON,
            "importance should be 0.7 after update, got {}",
            entry.importance
        );
        assert!(
            (entry.quality_score - 0.85).abs() < f64::EPSILON,
            "quality_score should be 0.85 after update, got {}",
            entry.quality_score
        );

        // Missing key — should error with KeyNotFound.
        let result = provider
            .update_decay_scores("does-not-exist", 0.5, 0.5)
            .await;
        assert!(
            matches!(result, Err(MemoryError::KeyNotFound(_))),
            "expected KeyNotFound for missing key, got {result:?}"
        );
    }
}
