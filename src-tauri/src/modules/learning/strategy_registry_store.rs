//! Candidate strategy registry persistence (Phase M5-A).
//!
//! Single canonical on-disk format + lookup index for
//! [`super::strategy_registry::CandidateStrategy`] so the
//! M5-A service layer can register / list / load / delete
//! records across IPC calls without holding in-memory state in
//! `AppState`.
//!
//! Honest scope:
//!
//! - **Format**: one JSON file per record at
//!   `<root>/<strategy_id>.json`.  Pretty-printed for human
//!   review.  Stable schema = the record's own
//!   `registry_version`; the persistence layer adds nothing on
//!   top.
//! - **Index**: derived on demand by listing the directory.  No
//!   separate index file — keeps the layer crash-safe and means
//!   external tooling can drop / curate strategy files without
//!   keeping an index in sync.
//! - **Atomic writes**: write to `.tmp` then rename, so partially
//!   written files never appear in `list`.
//! - **Mirrors the M4.6 [`crate::modules::harness::HarnessReportStore`]
//!   pattern** so reviewers / future tooling can reuse mental
//!   model + utilities (e.g. backup scripts can treat both
//!   roots identically).
//!
//! Out of scope for M5-A:
//!
//! - Compaction / GC.
//! - SQLite or any database backing.
//! - Multi-machine sync.
//! - Concurrent writer arbitration beyond `last writer wins`
//!   (registry mutations are typically operator-driven, not
//!   high-concurrency).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs;

use super::strategy_registry::{CandidateStrategy, RolloutState, StrategySource};

/// Stable persistence layer version.  Bumping is breaking; the
/// `StrategyIndexEntry` shape is the M5-A read-side contract.
pub const STRATEGY_REGISTRY_PERSISTENCE_VERSION: &str = "strategy-registry-persistence@m5.a";

/// File extension used for every persisted record.
const RECORD_FILE_EXT: &str = ".json";

/// Tempfile suffix used during atomic writes.
const RECORD_TMP_SUFFIX: &str = ".tmp";

/// Persistence error family.
#[derive(Debug, Error)]
pub enum RegistryPersistenceError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid strategy_id: {0}")]
    InvalidStrategyId(String),
    #[error("strategy_id not found: {0}")]
    NotFound(String),
    #[error("ill-formed candidate record (failed structural check)")]
    IllFormed,
}

/// Lightweight summary used by [`StrategyRegistryStore::list`].
/// Avoids loading the full record (compare/recommendation refs)
/// when the caller only needs a picker / list view.
///
/// Field set is **strictly a subset** of `CandidateStrategy` so
/// the summary always serialises consistently with the
/// underlying record version.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyIndexEntry {
    /// Stable record contract version of the underlying file.
    pub registry_version: String,
    pub strategy_id: String,
    pub label: String,
    pub source_kind: String,
    pub rollout_state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// `true` iff the record carries a `last_compare_ref`.
    pub has_compare_ref: bool,
    /// `true` iff the record carries a `last_recommendation_ref`.
    pub has_recommendation_ref: bool,
    /// Bytes on disk.
    pub size_bytes: u64,
    /// Absolute path to the JSON file.
    pub path: PathBuf,
}

/// Storage handle for candidate strategies.  Cheap to clone (just
/// a `PathBuf`).  Construct once at app start (or per-call via
/// `with_default_root`) and pass into every caller that needs to
/// save / list / load records.
#[derive(Debug, Clone)]
pub struct StrategyRegistryStore {
    root: PathBuf,
}

impl StrategyRegistryStore {
    /// Construct a store rooted at `root`.  Directory is created
    /// on first save if it does not exist.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default root: `<dirs::data_local_dir>/if2ai/learning/strategies`.
    /// Falls back to `./learning/strategies` when the platform data
    /// dir is unavailable (test sandboxes / CI without HOME).
    #[must_use]
    pub fn with_default_root() -> Self {
        let root = dirs::data_local_dir()
            .map(|d| d.join("if2ai").join("learning").join("strategies"))
            .unwrap_or_else(|| PathBuf::from("./learning/strategies"));
        Self::new(root)
    }

    /// Read-only access to the storage root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Save a record atomically.  Returns the absolute file path.
    ///
    /// Atomicity: writes to `<strategy_id>.json.tmp` first, then
    /// renames.  Concurrent saves of the same `strategy_id` race
    /// on the rename — last writer wins.
    pub async fn save(
        &self,
        record: &CandidateStrategy,
    ) -> Result<PathBuf, RegistryPersistenceError> {
        if !record.is_well_formed() {
            return Err(RegistryPersistenceError::IllFormed);
        }
        validate_strategy_id(&record.identity.strategy_id)?;
        fs::create_dir_all(&self.root).await?;
        let final_path = self.path_for(&record.identity.strategy_id);
        let tmp_path = final_path.with_extension(format!(
            "{}{}",
            RECORD_FILE_EXT.trim_start_matches('.'),
            RECORD_TMP_SUFFIX
        ));
        let bytes = serde_json::to_vec_pretty(record)?;
        fs::write(&tmp_path, &bytes).await?;
        if let Err(e) = fs::rename(&tmp_path, &final_path).await {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(e.into());
        }
        Ok(final_path)
    }

    /// Load one record by `strategy_id`.  Returns `Ok(None)` when
    /// the file does not exist; returns `Err` when the file
    /// exists but cannot be parsed (caller MUST distinguish
    /// missing vs corrupt).
    pub async fn load(
        &self,
        strategy_id: &str,
    ) -> Result<Option<CandidateStrategy>, RegistryPersistenceError> {
        validate_strategy_id(strategy_id)?;
        let path = self.path_for(strategy_id);
        match fs::read(&path).await {
            Ok(bytes) => {
                let record: CandidateStrategy = serde_json::from_slice(&bytes)?;
                Ok(Some(record))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete one record by `strategy_id`.  Returns `Ok(false)`
    /// when the file did not exist (idempotent).
    pub async fn delete(&self, strategy_id: &str) -> Result<bool, RegistryPersistenceError> {
        validate_strategy_id(strategy_id)?;
        let path = self.path_for(strategy_id);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// List every persisted record as a lightweight summary.
    /// Sorted newest-first by `updated_at`.  Skips corrupt /
    /// unrelated files silently (logged at WARN).  Returns
    /// `Ok(vec![])` when the root does not exist yet.
    pub async fn list(&self) -> Result<Vec<StrategyIndexEntry>, RegistryPersistenceError> {
        let mut entries: Vec<StrategyIndexEntry> = Vec::new();
        let mut rd = match fs::read_dir(&self.root).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e.into()),
        };
        while let Some(dirent) = rd.next_entry().await? {
            let path = dirent.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = match fs::read(&path).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(
                        target: "learning.registry",
                        path = %path.display(),
                        error = %e,
                        "[strategy_registry] skip file: read failed"
                    );
                    continue;
                }
            };
            let record: CandidateStrategy = match serde_json::from_slice(&bytes) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        target: "learning.registry",
                        path = %path.display(),
                        error = %e,
                        "[strategy_registry] skip file: parse failed"
                    );
                    continue;
                }
            };
            let size_bytes = bytes.len() as u64;
            entries.push(StrategyIndexEntry {
                registry_version: record.registry_version.clone(),
                strategy_id: record.identity.strategy_id.clone(),
                label: record.identity.label.clone(),
                source_kind: source_kind_label(&record.source).to_string(),
                rollout_state: record.rollout_state.label().to_string(),
                created_at: record.created_at,
                updated_at: record.updated_at,
                has_compare_ref: record.last_compare_ref.is_some(),
                has_recommendation_ref: record.last_recommendation_ref.is_some(),
                size_bytes,
                path,
            });
        }
        entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(entries)
    }

    /// Filesystem path for a given `strategy_id`.  Public for
    /// tests / debugging only.
    #[must_use]
    pub fn path_for(&self, strategy_id: &str) -> PathBuf {
        self.root.join(format!("{strategy_id}{RECORD_FILE_EXT}"))
    }
}

/// Stable wire label for a [`StrategySource`].  Used by the
/// index entry; kept in sync with the on-wire `tag = "kind"`
/// rename of [`StrategySource`].
fn source_kind_label(s: &StrategySource) -> &'static str {
    match s {
        StrategySource::Reflection { .. } => "reflection",
        StrategySource::Manual => "manual",
        StrategySource::CuratedRule { .. } => "curated_rule",
        StrategySource::Other { .. } => "other",
    }
}

/// Stable label exporter so callers don't need to import
/// [`RolloutState`].  Currently unused outside this crate but
/// kept here for symmetry with the harness store.
#[must_use]
pub fn rollout_state_label(s: RolloutState) -> &'static str {
    s.label()
}

/// Reject ids that would escape the registry root or collide
/// with the tmpfile suffix.  Mirrors the harness report store's
/// `validate_run_id` to keep operational mental model identical.
fn validate_strategy_id(strategy_id: &str) -> Result<(), RegistryPersistenceError> {
    if strategy_id.is_empty() {
        return Err(RegistryPersistenceError::InvalidStrategyId(
            "(empty)".to_string(),
        ));
    }
    if strategy_id.contains('/')
        || strategy_id.contains('\\')
        || strategy_id.contains("..")
        || strategy_id.contains('\0')
    {
        return Err(RegistryPersistenceError::InvalidStrategyId(
            strategy_id.to_string(),
        ));
    }
    if strategy_id.ends_with(RECORD_TMP_SUFFIX) {
        return Err(RegistryPersistenceError::InvalidStrategyId(
            strategy_id.to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::learning::strategy_registry::{
        CandidateStrategy, StrategyIdentity, StrategySource,
    };
    use tempfile::tempdir;

    fn sample_record(id: &str) -> CandidateStrategy {
        CandidateStrategy::new_draft(
            StrategyIdentity {
                strategy_id: id.to_string(),
                label: format!("label for {id}"),
                policy_version: None,
                definition_ref: None,
            },
            StrategySource::Manual,
            None,
            Utc::now(),
        )
    }

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let tmp = tempdir().unwrap();
        let store = StrategyRegistryStore::new(tmp.path());
        let r = sample_record("strat-load");
        store.save(&r).await.unwrap();
        let back = store.load("strat-load").await.unwrap().unwrap();
        assert_eq!(back, r);
    }

    #[tokio::test]
    async fn missing_load_returns_none() {
        let tmp = tempdir().unwrap();
        let store = StrategyRegistryStore::new(tmp.path());
        let back = store.load("does-not-exist").await.unwrap();
        assert!(back.is_none());
    }

    #[tokio::test]
    async fn list_is_newest_first() {
        let tmp = tempdir().unwrap();
        let store = StrategyRegistryStore::new(tmp.path());
        let mut a = sample_record("strat-a");
        let mut b = sample_record("strat-b");
        // Force ordered timestamps
        a.updated_at = Utc::now() - chrono::Duration::seconds(60);
        b.updated_at = Utc::now();
        store.save(&a).await.unwrap();
        store.save(&b).await.unwrap();
        let listed = store.list().await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].strategy_id, "strat-b");
        assert_eq!(listed[1].strategy_id, "strat-a");
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let tmp = tempdir().unwrap();
        let store = StrategyRegistryStore::new(tmp.path());
        let r = sample_record("strat-del");
        store.save(&r).await.unwrap();
        assert!(store.delete("strat-del").await.unwrap());
        assert!(!store.delete("strat-del").await.unwrap());
    }

    #[tokio::test]
    async fn invalid_id_is_refused() {
        let tmp = tempdir().unwrap();
        let store = StrategyRegistryStore::new(tmp.path());
        assert!(matches!(
            store.load("../etc/passwd").await,
            Err(RegistryPersistenceError::InvalidStrategyId(_))
        ));
    }

    #[tokio::test]
    async fn ill_formed_record_is_refused() {
        let tmp = tempdir().unwrap();
        let store = StrategyRegistryStore::new(tmp.path());
        let mut r = sample_record("strat-bad");
        r.identity.label.clear();
        let res = store.save(&r).await;
        assert!(matches!(res, Err(RegistryPersistenceError::IllFormed)));
    }
}
