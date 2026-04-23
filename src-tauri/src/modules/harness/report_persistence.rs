//! Harness Run Report persistence (Phase M4.6).
//!
//! Single canonical on-disk format + lookup index for
//! [`super::run_report::HarnessRunReport`] so the M4.5 compare
//! flow / M4.8 gate / future M4.6 governance UI can re-load
//! historical reports without re-running the trace pipeline.
//!
//! Honest scope of this module:
//!
//! - **Format**: one JSON file per report at
//!   `<root>/<run_id>.json`.  Pretty-printed for human review.
//!   Stable schema = the report's own
//!   `report_version`; the persistence layer adds nothing on top.
//! - **Index**: derived on demand by listing the directory.  No
//!   separate index file — keeps the layer crash-safe and means
//!   external tooling can drop / curate report files without
//!   keeping an index in sync.
//! - **Atomic writes**: write to `.tmp` then rename, so partially
//!   written files never appear in `list_reports`.
//!
//! Out of scope:
//!
//! - Compaction / rotation / GC (M4.7 corpus orchestration may
//!   add this; M4.6 does NOT).
//! - SQLite / database backing — JSONL on disk is sufficient for
//!   M4.6 review surface, and M4.8 gate reads one report at a
//!   time, so per-file JSON wins on simplicity.
//! - Multi-machine sync.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

use super::run_report::{HarnessRunReport, TaskOutcome};

/// Stable persistence layer version.  Bumping is breaking; the
/// `RunIndexEntry` shape is the M4.6 read-side contract.
pub const HARNESS_REPORT_PERSISTENCE_VERSION: &str = "harness-report-persistence@m4.6";

/// File extension used for every persisted report.
const REPORT_FILE_EXT: &str = ".json";

/// Tempfile suffix used during atomic writes.
const REPORT_TMP_SUFFIX: &str = ".tmp";

/// Persistence error family.
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid run_id: {0}")]
    InvalidRunId(String),
}

/// Lightweight summary used by [`list_reports`].  Avoids loading
/// the full `evidence` bundle when the caller only needs a list /
/// picker UI.
///
/// Field set is **strictly a subset** of `HarnessRunReport` so the
/// summary always serialises consistently with the underlying
/// report version.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunIndexEntry {
    /// Stable report contract version of the underlying file.
    /// Pin so consumers that don't recognise it can refuse early.
    pub report_version: String,
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub task_outcome: TaskOutcome,
    pub turn_count: u64,
    pub blocking_failures_count: usize,
    /// Bytes on disk.
    pub size_bytes: u64,
    /// Absolute path to the JSON file.
    pub path: PathBuf,
}

/// Storage handle for harness reports.  Cheap to clone (just a
/// `PathBuf`).  Construct once at app start (e.g. via
/// `HarnessReportStore::with_default_root`) and pass into every
/// caller that needs to save / list / load reports.
#[derive(Debug, Clone)]
pub struct HarnessReportStore {
    root: PathBuf,
}

impl HarnessReportStore {
    /// Construct a store rooted at `root`.  Directory is created
    /// on first save if it does not exist.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default root: `~/.if2ai/harness/runs`.
    /// MEM-MOD-PATH-FIX — single root via if2ai_data_root().
    #[must_use]
    pub fn with_default_root() -> Self {
        let root = crate::modules::config::store::if2ai_data_root()
            .join("harness")
            .join("runs");
        Self::new(root)
    }

    /// Read-only access to the storage root.  Useful for logs.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Save a report atomically.  Returns the absolute file path.
    ///
    /// Atomicity: writes to `<run_id>.json.tmp` first, then
    /// renames.  Concurrent saves of the same `run_id` race on the
    /// rename — last writer wins (acceptable; report finalisation
    /// is per-run and typically not concurrent).
    pub async fn save(&self, report: &HarnessRunReport) -> Result<PathBuf, PersistenceError> {
        validate_run_id(&report.run_id)?;
        fs::create_dir_all(&self.root).await?;
        let final_path = self.path_for(&report.run_id);
        let tmp_path = final_path.with_extension(format!(
            "{}{}",
            REPORT_FILE_EXT.trim_start_matches('.'),
            REPORT_TMP_SUFFIX
        ));
        let bytes = serde_json::to_vec_pretty(report)?;
        fs::write(&tmp_path, &bytes).await?;
        // Reviewer I-1 fix: best-effort tmp cleanup on rename
        // failure (cross-device rename / permission errors leave
        // the tmp file otherwise occupying disk).
        if let Err(e) = fs::rename(&tmp_path, &final_path).await {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(e.into());
        }
        Ok(final_path)
    }

    /// Load a report by `run_id`.  Returns `Ok(None)` when the
    /// file does not exist (caller must handle missing reports
    /// distinctly from corrupt ones).
    pub async fn load(&self, run_id: &str) -> Result<Option<HarnessRunReport>, PersistenceError> {
        validate_run_id(run_id)?;
        let path = self.path_for(run_id);
        match fs::read(&path).await {
            Ok(bytes) => {
                let report: HarnessRunReport = serde_json::from_slice(&bytes)?;
                Ok(Some(report))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a report.  Returns `Ok(false)` when the file did not
    /// exist (idempotent).
    pub async fn delete(&self, run_id: &str) -> Result<bool, PersistenceError> {
        validate_run_id(run_id)?;
        let path = self.path_for(run_id);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// List all reports in the store as lightweight summaries.
    /// Sorted by `started_at` descending (newest first).  Skips
    /// entries that fail to parse — the caller gets a partial
    /// list rather than an error so a single corrupt file does
    /// not block the picker.
    pub async fn list(&self) -> Result<Vec<RunIndexEntry>, PersistenceError> {
        // Async existence check — `Path::exists()` is a sync
        // syscall and would block the tokio executor on slow
        // filesystems (network mounts).  Reviewer W-2 fix.
        if !fs::try_exists(&self.root).await.unwrap_or(false) {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        let mut rd = fs::read_dir(&self.root).await?;
        while let Some(item) = rd.next_entry().await? {
            let path = item.path();
            let name = match path.file_name().and_then(|s| s.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if !name.ends_with(REPORT_FILE_EXT) || name.ends_with(REPORT_TMP_SUFFIX) {
                continue;
            }
            // Fast path: read the file, parse the report header
            // fields only.  We deserialise the full report and
            // project — the full parse is cheap (< 2 ms for a
            // typical report).
            let Ok(bytes) = fs::read(&path).await else {
                continue;
            };
            let Ok(report): Result<HarnessRunReport, _> = serde_json::from_slice(&bytes) else {
                tracing::warn!(
                    target: "harness.persistence",
                    path = %path.display(),
                    "[report_store] skipping corrupt report file"
                );
                continue;
            };
            let size_bytes = bytes.len() as u64;
            entries.push(RunIndexEntry {
                report_version: report.report_version.clone(),
                run_id: report.run_id.clone(),
                label: report.label.clone(),
                session_id: report.session_id.clone(),
                started_at: report.started_at,
                ended_at: report.ended_at,
                task_outcome: report.task.outcome,
                turn_count: report.task.turn_count,
                blocking_failures_count: report.blocking_failures.len(),
                size_bytes,
                path,
            });
        }
        entries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(entries)
    }

    fn path_for(&self, run_id: &str) -> PathBuf {
        self.root.join(format!("{run_id}{REPORT_FILE_EXT}"))
    }
}

/// Reject run ids that would escape the storage root via path
/// segments.  Stable rule: ids must contain only `[A-Za-z0-9_-]`.
/// UUIDs (the default backend-generated id) are always valid.
fn validate_run_id(run_id: &str) -> Result<(), PersistenceError> {
    if run_id.is_empty() {
        return Err(PersistenceError::InvalidRunId("empty".to_string()));
    }
    let valid = run_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(PersistenceError::InvalidRunId(format!(
            "run_id must match [A-Za-z0-9_-]+, got '{run_id}'"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn fresh_report(run_id: &str) -> HarnessRunReport {
        HarnessRunReport::new_empty(run_id, Utc::now())
    }

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let tmp = TempDir::new().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        let r = fresh_report("run-abc");
        let path = store.save(&r).await.unwrap();
        assert!(path.exists());
        let loaded = store.load("run-abc").await.unwrap().unwrap();
        assert_eq!(loaded.run_id, "run-abc");
        assert_eq!(loaded.report_version, r.report_version);
    }

    #[tokio::test]
    async fn load_missing_returns_none() {
        let tmp = TempDir::new().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        assert!(store.load("never-saved").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        assert!(!store.delete("never-saved").await.unwrap());
        let r = fresh_report("run-x");
        store.save(&r).await.unwrap();
        assert!(store.delete("run-x").await.unwrap());
        assert!(!store.delete("run-x").await.unwrap());
    }

    #[tokio::test]
    async fn list_sorts_newest_first() {
        let tmp = TempDir::new().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        let r1 = HarnessRunReport::new_empty("run-old", Utc::now() - chrono::Duration::seconds(10));
        let r2 = HarnessRunReport::new_empty("run-new", Utc::now());
        store.save(&r1).await.unwrap();
        store.save(&r2).await.unwrap();
        let entries = store.list().await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].run_id, "run-new");
        assert_eq!(entries[1].run_id, "run-old");
        assert!(entries[0].size_bytes > 0);
        assert_eq!(
            entries[0].report_version,
            super::super::run_report::HARNESS_RUN_REPORT_VERSION
        );
    }

    #[tokio::test]
    async fn list_skips_corrupt_files_without_failing() {
        let tmp = TempDir::new().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        let r = fresh_report("run-good");
        store.save(&r).await.unwrap();
        // Inject a corrupt file with the right extension.
        fs::create_dir_all(tmp.path()).await.unwrap();
        fs::write(tmp.path().join("garbage.json"), b"not json")
            .await
            .unwrap();
        let entries = store.list().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].run_id, "run-good");
    }

    #[tokio::test]
    async fn list_ignores_tempfiles() {
        let tmp = TempDir::new().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        fs::create_dir_all(tmp.path()).await.unwrap();
        fs::write(tmp.path().join("inflight.json.tmp"), b"{}")
            .await
            .unwrap();
        assert!(store.list().await.unwrap().is_empty());
    }

    #[test]
    fn invalid_run_ids_are_refused() {
        assert!(validate_run_id("../etc/passwd").is_err());
        assert!(validate_run_id("with spaces").is_err());
        assert!(validate_run_id("").is_err());
        assert!(validate_run_id("abc-123_DEF").is_ok());
    }

    #[tokio::test]
    async fn save_rejects_path_traversal_run_id() {
        let tmp = TempDir::new().unwrap();
        let store = HarnessReportStore::new(tmp.path());
        let mut r = fresh_report("ok");
        r.run_id = "../escape".to_string();
        assert!(store.save(&r).await.is_err());
    }
}
