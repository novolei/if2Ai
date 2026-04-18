// 8A.5 lands the SessionSummaryStore trait + SqliteSessionSummaryStore +
// NullSessionSummaryStore fallback.  First production consumer is 8A.7
// (RollingSummarizer); the in-file `#[cfg(test)] mod tests` exercises
// every public item, so the bin target's "never used" warnings are
// expected until the next slice.
#![allow(dead_code)]

//! `SessionSummaryStore` — persistence for rolling-summary records.
//!
//! Per `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-B1, every `save()` is a dual write:
//!   1. SQLite UPSERT on the `session_summaries` table (source of truth).
//!   2. JSON sidecar at `<scope_root>/summaries/<session_id>.json`
//!      written atomically via `tmp + rename` (best-effort cold backup).
//!
//! The SQLite write is the contract; if the JSON sidecar fails the
//! function still returns `Ok(())` and emits a `tracing::warn!` — the
//! sidecar is purely human-readable defence-in-depth and we MUST NOT
//! roll back the SQLite write if it succeeds (v2 §0.5 Δ-6).
//!
//! `is_dirty` is computed via the SQL expression `summary != snapshot`
//! at query time rather than via a `GENERATED ALWAYS … VIRTUAL` column
//! so we stay portable across SQLite library versions and so the column
//! list stays minimal.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::schema::{SessionSummaryRecord, SummarySource};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::MemoryError;

/// Storage seam for [`SessionSummaryRecord`].  Lives behind
/// `Arc<dyn SessionSummaryStore>` on `AppState` so other subsystems can
/// be tested with [`NullSessionSummaryStore`] without touching SQLite.
#[async_trait]
pub trait SessionSummaryStore: Send + Sync {
    /// Look up a record by session id.  Returns `Ok(None)` for unknown
    /// sessions; only I/O failures propagate as [`MemoryError`].
    async fn get(&self, session_id: &str) -> Result<Option<SessionSummaryRecord>, MemoryError>;

    /// Upsert a record.  Performs a best-effort dual write — SQLite is
    /// authoritative; the JSON sidecar is best-effort.
    async fn save(&self, record: &SessionSummaryRecord) -> Result<(), MemoryError>;

    /// Records whose `updated_at` falls in the half-open range
    /// `[start, end]`, optionally filtered by `scope.project_id`.
    async fn list_in_range(
        &self,
        scope: &MemoryExecutionScope,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SessionSummaryRecord>, MemoryError>;

    /// Records where `summary != snapshot`, optionally filtered by
    /// `scope.project_id`.  Sorted newest-first.
    async fn list_dirty(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<SessionSummaryRecord>, MemoryError>;

    /// Mark a session as fully processed — sets `snapshot = summary`
    /// and `snapshot_at = now()` so [`Self::list_dirty`] no longer
    /// surfaces it until the next `save()`.
    async fn mark_processed(&self, session_id: &str) -> Result<(), MemoryError>;
}

/// Validate that `session_id` is safe to use as a single path component
/// for the JSON sidecar.  Rejects any value containing `/`, `\`, NUL,
/// or `..`, and any empty / pure-whitespace value.  Returns the
/// original string slice on success so callers can compose paths
/// directly.
pub(crate) fn safe_session_filename(session_id: &str) -> Result<&str, MemoryError> {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        return Err(MemoryError::Generic(
            "session_id must not be empty".to_string(),
        ));
    }
    if session_id.contains('/')
        || session_id.contains('\\')
        || session_id.contains('\0')
        || session_id.split(['/', '\\']).any(|seg| seg == "..")
        || session_id == ".."
        || session_id == "."
    {
        return Err(MemoryError::Generic(format!(
            "unsafe session_id rejected for filename use: {session_id:?}"
        )));
    }
    Ok(session_id)
}

/// SQLite-backed [`SessionSummaryStore`] with JSON sidecar dual-write.
///
/// `db` is wrapped in [`std::sync::Mutex`] and only ever locked from
/// inside [`tokio::task::spawn_blocking`] so the lock is never held
/// across an `.await`.  `scope_root` is the prefix under which the
/// `summaries/<session_id>.json` sidecars are written; in production
/// this is `dirs::data_local_dir().join(".if2ai/memory")` (v2 §0.5 Δ-6).
pub struct SqliteSessionSummaryStore {
    db: Arc<Mutex<Connection>>,
    scope_root: PathBuf,
}

impl SqliteSessionSummaryStore {
    /// Open (or create) the store backed by `db_path` and the sidecar
    /// directory at `scope_root.join("summaries")`.
    ///
    /// Creates parent directories as needed, initialises the
    /// `session_summaries` table, and adds two indexes
    /// (`project_id`, `updated_at`).  Existing rows from previous slice
    /// runs are preserved — schema migrations are idempotent.
    pub fn open(db_path: &Path, scope_root: PathBuf) -> Result<Self, MemoryError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                MemoryError::Generic(format!(
                    "failed to create summary db parent {parent:?}: {e}"
                ))
            })?;
        }
        let conn = Connection::open(db_path).map_err(|e| {
            MemoryError::Generic(format!("failed to open summary db {db_path:?}: {e}"))
        })?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_summaries (
                session_id     TEXT PRIMARY KEY,
                project_id     TEXT,
                created_at     TEXT NOT NULL,
                updated_at     TEXT NOT NULL,
                summary        TEXT NOT NULL,
                snapshot       TEXT NOT NULL DEFAULT '',
                snapshot_at    TEXT,
                message_count  INTEGER NOT NULL DEFAULT 0,
                source         TEXT NOT NULL DEFAULT 'rolling'
            )",
            [],
        )
        .map_err(|e| MemoryError::Generic(format!("failed to create session_summaries: {e}")))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_summaries_project ON session_summaries(project_id)",
            [],
        )
        .map_err(|e| {
            MemoryError::Generic(format!("failed to create idx_summaries_project: {e}"))
        })?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_summaries_updated ON session_summaries(updated_at)",
            [],
        )
        .map_err(|e| {
            MemoryError::Generic(format!("failed to create idx_summaries_updated: {e}"))
        })?;

        let sidecar_dir = scope_root.join("summaries");
        std::fs::create_dir_all(&sidecar_dir).map_err(|e| {
            MemoryError::Generic(format!("failed to create sidecar dir {sidecar_dir:?}: {e}"))
        })?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            scope_root,
        })
    }

    /// Convert a SQLite row into a [`SessionSummaryRecord`].
    ///
    /// Column order MUST match `SELECT_COLUMNS`:
    ///   0=session_id, 1=project_id, 2=created_at, 3=updated_at,
    ///   4=summary, 5=snapshot, 6=snapshot_at, 7=message_count, 8=source.
    fn row_to_record(row: &rusqlite::Row<'_>) -> Result<SessionSummaryRecord, rusqlite::Error> {
        let session_id: String = row.get(0)?;
        let project_id: Option<String> = row.get(1)?;
        let created_at_s: String = row.get(2)?;
        let updated_at_s: String = row.get(3)?;
        let summary: String = row.get(4)?;
        let snapshot: String = row.get(5)?;
        let snapshot_at_s: Option<String> = row.get(6)?;
        let message_count: i64 = row.get(7)?;
        let source_s: String = row.get(8)?;

        let parse_dt = |s: &str, col: usize| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| {
                    rusqlite::Error::InvalidColumnType(
                        col,
                        "datetime".to_string(),
                        rusqlite::types::Type::Text,
                    )
                })
        };

        let created_at = parse_dt(&created_at_s, 2)?;
        let updated_at = parse_dt(&updated_at_s, 3)?;
        let snapshot_at = match snapshot_at_s {
            Some(s) => Some(parse_dt(&s, 6)?),
            None => None,
        };

        Ok(SessionSummaryRecord {
            session_id,
            project_id,
            created_at,
            updated_at,
            summary,
            snapshot,
            snapshot_at,
            message_count: message_count.max(0) as usize,
            source: SummarySource::from_str_lossy(&source_s),
        })
    }

    /// Atomically replace `final_path` with `payload` via a sibling
    /// `.tmp` file + rename.  Best-effort: a failure here is logged
    /// and **not** propagated, because SQLite is the source of truth.
    fn write_sidecar(dir: &Path, session_id: &str, payload: &[u8]) {
        let final_path = dir.join(format!("{session_id}.json"));
        let tmp_path = dir.join(format!("{session_id}.json.tmp"));

        if let Err(e) = std::fs::write(&tmp_path, payload) {
            tracing::warn!(
                "[summary-store] sidecar tmp write failed for {session_id}: {e}; SQLite remains authoritative"
            );
            return;
        }
        if let Err(e) = std::fs::rename(&tmp_path, &final_path) {
            tracing::warn!(
                "[summary-store] sidecar rename failed for {session_id}: {e}; cleaning up .tmp"
            );
            let _ = std::fs::remove_file(&tmp_path);
        }
    }
}

/// Column projection used by every read path so [`Self::row_to_record`]
/// can rely on a fixed positional layout.
const SELECT_COLUMNS: &str =
    "session_id, project_id, created_at, updated_at, summary, snapshot, snapshot_at, message_count, source";

#[async_trait]
impl SessionSummaryStore for SqliteSessionSummaryStore {
    async fn get(&self, session_id: &str) -> Result<Option<SessionSummaryRecord>, MemoryError> {
        let db = self.db.clone();
        let id = session_id.to_string();
        tokio::task::spawn_blocking(
            move || -> Result<Option<SessionSummaryRecord>, MemoryError> {
                let conn = db
                    .lock()
                    .map_err(|e| MemoryError::Generic(format!("summary db mutex poisoned: {e}")))?;
                let sql =
                    format!("SELECT {SELECT_COLUMNS} FROM session_summaries WHERE session_id = ?1");
                conn.query_row(&sql, params![id], SqliteSessionSummaryStore::row_to_record)
                    .optional()
                    .map_err(MemoryError::from)
            },
        )
        .await
        .map_err(|e| MemoryError::Generic(format!("summary get join error: {e}")))?
    }

    async fn save(&self, record: &SessionSummaryRecord) -> Result<(), MemoryError> {
        // Sanitize before doing anything else — both the SQLite write
        // and the sidecar write embed `session_id` in user-visible
        // surfaces, so reject path-traversal-shaped input early.
        safe_session_filename(&record.session_id)?;

        let db = self.db.clone();
        let r = record.clone();
        tokio::task::spawn_blocking(move || -> Result<(), MemoryError> {
            let conn = db
                .lock()
                .map_err(|e| MemoryError::Generic(format!("summary db mutex poisoned: {e}")))?;
            conn.execute(
                "INSERT INTO session_summaries
                    (session_id, project_id, created_at, updated_at, summary,
                     snapshot, snapshot_at, message_count, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(session_id) DO UPDATE SET
                    project_id    = excluded.project_id,
                    updated_at    = excluded.updated_at,
                    summary       = excluded.summary,
                    snapshot      = excluded.snapshot,
                    snapshot_at   = excluded.snapshot_at,
                    message_count = excluded.message_count,
                    source        = excluded.source",
                params![
                    r.session_id,
                    r.project_id,
                    r.created_at.to_rfc3339(),
                    r.updated_at.to_rfc3339(),
                    r.summary,
                    r.snapshot,
                    r.snapshot_at.map(|t| t.to_rfc3339()),
                    r.message_count as i64,
                    r.source.as_str(),
                ],
            )
            .map_err(MemoryError::from)?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("summary save join error: {e}")))??;

        // Phase B — JSON sidecar (best-effort, never rolls back SQLite).
        let dir = self.scope_root.join("summaries");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(
                "[summary-store] sidecar dir create failed {dir:?}: {e}; skipping sidecar"
            );
            return Ok(());
        }
        let payload = match serde_json::to_vec_pretty(record) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(
                    "[summary-store] sidecar serialize failed for {}: {e}",
                    record.session_id
                );
                return Ok(());
            }
        };
        Self::write_sidecar(&dir, &record.session_id, &payload);
        Ok(())
    }

    async fn list_in_range(
        &self,
        scope: &MemoryExecutionScope,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SessionSummaryRecord>, MemoryError> {
        let db = self.db.clone();
        let project = scope.project_id.clone();
        let start_s = start.to_rfc3339();
        let end_s = end.to_rfc3339();

        tokio::task::spawn_blocking(move || -> Result<Vec<SessionSummaryRecord>, MemoryError> {
            let conn = db
                .lock()
                .map_err(|e| MemoryError::Generic(format!("summary db mutex poisoned: {e}")))?;
            let rows: Vec<SessionSummaryRecord> = match project {
                Some(p) => {
                    let sql = format!(
                        "SELECT {SELECT_COLUMNS} FROM session_summaries
                         WHERE updated_at BETWEEN ?1 AND ?2 AND project_id = ?3
                         ORDER BY updated_at DESC"
                    );
                    let mut stmt = conn.prepare(&sql).map_err(MemoryError::from)?;
                    let it = stmt
                        .query_map(
                            params![start_s, end_s, p],
                            SqliteSessionSummaryStore::row_to_record,
                        )
                        .map_err(MemoryError::from)?;
                    it.collect::<Result<Vec<_>, _>>()
                        .map_err(MemoryError::from)?
                }
                None => {
                    let sql = format!(
                        "SELECT {SELECT_COLUMNS} FROM session_summaries
                         WHERE updated_at BETWEEN ?1 AND ?2
                         ORDER BY updated_at DESC"
                    );
                    let mut stmt = conn.prepare(&sql).map_err(MemoryError::from)?;
                    let it = stmt
                        .query_map(
                            params![start_s, end_s],
                            SqliteSessionSummaryStore::row_to_record,
                        )
                        .map_err(MemoryError::from)?;
                    it.collect::<Result<Vec<_>, _>>()
                        .map_err(MemoryError::from)?
                }
            };
            Ok(rows)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("summary list_in_range join error: {e}")))?
    }

    async fn list_dirty(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<SessionSummaryRecord>, MemoryError> {
        let db = self.db.clone();
        let project = scope.project_id.clone();

        tokio::task::spawn_blocking(move || -> Result<Vec<SessionSummaryRecord>, MemoryError> {
            let conn = db
                .lock()
                .map_err(|e| MemoryError::Generic(format!("summary db mutex poisoned: {e}")))?;
            let rows: Vec<SessionSummaryRecord> = match project {
                Some(p) => {
                    let sql = format!(
                        "SELECT {SELECT_COLUMNS} FROM session_summaries
                         WHERE summary != snapshot AND project_id = ?1
                         ORDER BY updated_at DESC"
                    );
                    let mut stmt = conn.prepare(&sql).map_err(MemoryError::from)?;
                    let it = stmt
                        .query_map(params![p], SqliteSessionSummaryStore::row_to_record)
                        .map_err(MemoryError::from)?;
                    it.collect::<Result<Vec<_>, _>>()
                        .map_err(MemoryError::from)?
                }
                None => {
                    let sql = format!(
                        "SELECT {SELECT_COLUMNS} FROM session_summaries
                         WHERE summary != snapshot
                         ORDER BY updated_at DESC"
                    );
                    let mut stmt = conn.prepare(&sql).map_err(MemoryError::from)?;
                    let it = stmt
                        .query_map([], SqliteSessionSummaryStore::row_to_record)
                        .map_err(MemoryError::from)?;
                    it.collect::<Result<Vec<_>, _>>()
                        .map_err(MemoryError::from)?
                }
            };
            Ok(rows)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("summary list_dirty join error: {e}")))?
    }

    async fn mark_processed(&self, session_id: &str) -> Result<(), MemoryError> {
        let db = self.db.clone();
        let id = session_id.to_string();
        let now = Utc::now().to_rfc3339();
        tokio::task::spawn_blocking(move || -> Result<(), MemoryError> {
            let conn = db
                .lock()
                .map_err(|e| MemoryError::Generic(format!("summary db mutex poisoned: {e}")))?;
            conn.execute(
                "UPDATE session_summaries
                 SET snapshot = summary, snapshot_at = ?2
                 WHERE session_id = ?1",
                params![id, now],
            )
            .map_err(MemoryError::from)?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("summary mark_processed join error: {e}")))?
    }
}

/// No-op [`SessionSummaryStore`] used as a graceful fallback when the
/// SQLite open fails at startup.  Every method returns an empty / `None`
/// value and `save()` accepts the record without persisting.  Used so
/// the rest of `AppState` can hold a `Arc<dyn SessionSummaryStore>`
/// unconditionally; the surrounding code logs the degraded mode.
pub struct NullSessionSummaryStore;

impl NullSessionSummaryStore {
    /// Create a new no-op store.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullSessionSummaryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionSummaryStore for NullSessionSummaryStore {
    async fn get(&self, _session_id: &str) -> Result<Option<SessionSummaryRecord>, MemoryError> {
        Ok(None)
    }
    async fn save(&self, _record: &SessionSummaryRecord) -> Result<(), MemoryError> {
        Ok(())
    }
    async fn list_in_range(
        &self,
        _scope: &MemoryExecutionScope,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<SessionSummaryRecord>, MemoryError> {
        Ok(Vec::new())
    }
    async fn list_dirty(
        &self,
        _scope: &MemoryExecutionScope,
    ) -> Result<Vec<SessionSummaryRecord>, MemoryError> {
        Ok(Vec::new())
    }
    async fn mark_processed(&self, _session_id: &str) -> Result<(), MemoryError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use tempfile::tempdir;

    fn make_record(
        session_id: &str,
        project_id: Option<&str>,
        summary: &str,
    ) -> SessionSummaryRecord {
        let now = Utc::now();
        SessionSummaryRecord {
            session_id: session_id.into(),
            project_id: project_id.map(str::to_string),
            created_at: now,
            updated_at: now,
            summary: summary.into(),
            snapshot: String::new(),
            snapshot_at: None,
            message_count: 1,
            source: SummarySource::Rolling,
        }
    }

    fn open_store() -> (tempfile::TempDir, SqliteSessionSummaryStore) {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("memory.db");
        let store = SqliteSessionSummaryStore::open(&db_path, tmp.path().to_path_buf())
            .expect("open store");
        (tmp, store)
    }

    #[test]
    fn safe_filename_accepts_uuid_and_alnum() {
        assert!(safe_session_filename("abc-123_x").is_ok());
        assert!(safe_session_filename("550e8400-e29b-41d4-a716-446655440000").is_ok());
    }

    #[test]
    fn safe_filename_rejects_traversal() {
        assert!(safe_session_filename("../etc/passwd").is_err());
        assert!(safe_session_filename("a/b").is_err());
        assert!(safe_session_filename("a\\b").is_err());
        assert!(safe_session_filename("..").is_err());
        assert!(safe_session_filename(".").is_err());
        assert!(safe_session_filename("").is_err());
        assert!(safe_session_filename("   ").is_err());
        assert!(safe_session_filename("foo\0bar").is_err());
    }

    #[tokio::test]
    async fn save_and_get_round_trip() {
        let (_tmp, store) = open_store();
        let rec = make_record("s1", Some("p1"), "hello");
        store.save(&rec).await.expect("save");
        let got = store.get("s1").await.expect("get").expect("present");
        assert_eq!(got.session_id, "s1");
        assert_eq!(got.summary, "hello");
        assert_eq!(got.project_id.as_deref(), Some("p1"));
        assert!(got.is_dirty());
    }

    #[tokio::test]
    async fn dirty_flag_after_save_and_clear_after_mark() {
        let (_tmp, store) = open_store();
        let rec = make_record("s1", None, "draft");
        store.save(&rec).await.expect("save");
        let scope = MemoryExecutionScope::global();
        let dirty = store.list_dirty(&scope).await.expect("list_dirty");
        assert_eq!(dirty.len(), 1);
        store.mark_processed("s1").await.expect("mark_processed");
        let dirty_after = store.list_dirty(&scope).await.expect("list_dirty");
        assert!(dirty_after.is_empty());
        let got = store.get("s1").await.unwrap().unwrap();
        assert!(!got.is_dirty());
        assert!(got.snapshot_at.is_some());
    }

    #[tokio::test]
    async fn dual_write_creates_json_sidecar() {
        let (tmp, store) = open_store();
        let rec = make_record("s-side", None, "sidecar test");
        store.save(&rec).await.expect("save");
        let sidecar = tmp.path().join("summaries").join("s-side.json");
        assert!(sidecar.exists(), "sidecar missing: {sidecar:?}");
        let bytes = std::fs::read(&sidecar).expect("read sidecar");
        let parsed: SessionSummaryRecord =
            serde_json::from_slice(&bytes).expect("parse sidecar JSON");
        assert_eq!(parsed.summary, "sidecar test");
    }

    #[tokio::test]
    async fn dual_write_leaves_no_partial_tmp_file() {
        let (tmp, store) = open_store();
        let rec = make_record("s-clean", None, "clean tmp test");
        store.save(&rec).await.expect("save");
        let sidecar_dir = tmp.path().join("summaries");
        let mut stray_tmp = false;
        for entry in std::fs::read_dir(&sidecar_dir).expect("read_dir") {
            let entry = entry.expect("dir entry");
            if entry.file_name().to_string_lossy().ends_with(".tmp") {
                stray_tmp = true;
            }
        }
        assert!(!stray_tmp, "stray .tmp file left in {sidecar_dir:?}");
    }

    #[tokio::test]
    async fn unsafe_session_id_is_rejected_by_save() {
        let (_tmp, store) = open_store();
        let rec = make_record("../etc/passwd", None, "evil");
        let err = store.save(&rec).await.expect_err("must reject");
        let msg = err.to_string();
        assert!(msg.contains("unsafe session_id"), "unexpected msg: {msg}");
    }

    #[tokio::test]
    async fn list_in_range_filters_by_window_and_project() {
        let (_tmp, store) = open_store();
        let now = Utc::now();
        let mut old = make_record("s-old", Some("p1"), "old");
        old.updated_at = now - Duration::hours(48);
        let mut recent = make_record("s-recent", Some("p1"), "recent");
        recent.updated_at = now - Duration::hours(1);
        let mut other_project = make_record("s-other", Some("p2"), "other");
        other_project.updated_at = now - Duration::hours(1);
        store.save(&old).await.unwrap();
        store.save(&recent).await.unwrap();
        store.save(&other_project).await.unwrap();

        let scope = MemoryExecutionScope {
            session_id: None,
            project_id: Some("p1".into()),
            workdir: None,
        };
        let win = store
            .list_in_range(&scope, now - Duration::hours(24), now + Duration::hours(1))
            .await
            .expect("list_in_range");
        assert_eq!(win.len(), 1);
        assert_eq!(win[0].session_id, "s-recent");
    }

    #[tokio::test]
    async fn concurrent_save_is_safe_for_same_id() {
        let (_tmp, store) = open_store();
        let store = Arc::new(store);
        let mut handles = Vec::new();
        for i in 0..8 {
            let s = store.clone();
            handles.push(tokio::spawn(async move {
                let mut rec = make_record("s-conc", None, &format!("v{i}"));
                rec.message_count = i;
                s.save(&rec).await
            }));
        }
        for h in handles {
            h.await.expect("join").expect("save");
        }
        let final_rec = store.get("s-conc").await.unwrap().expect("present");
        // Don't assert on which writer won — only that exactly one row
        // exists and the table is queryable.
        assert_eq!(final_rec.session_id, "s-conc");
    }

    #[tokio::test]
    async fn null_store_is_inert_but_succeeds() {
        let store = NullSessionSummaryStore::new();
        assert!(store.get("anything").await.unwrap().is_none());
        let rec = make_record("s1", None, "ignored");
        store.save(&rec).await.unwrap();
        let scope = MemoryExecutionScope::global();
        assert!(store.list_dirty(&scope).await.unwrap().is_empty());
        assert!(store
            .list_in_range(&scope, Utc::now(), Utc::now())
            .await
            .unwrap()
            .is_empty());
        store.mark_processed("s1").await.unwrap();
    }
}
