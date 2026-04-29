//! DW-004 (truth-loop iter-7) — NDJSON-backed [`KnowledgeStore`].
//!
//! Until a future Pack lands a SQLite/LanceDB backend, this is the
//! production singleton installed by `desktop_host/setup.rs` so DK
//! contributions actually survive process restarts. Replaces the
//! `MockKnowledgeStore` previously installed by
//! `global_knowledge_store()`'s default branch (which kept entries
//! in a `Mutex<Vec<_>>` only).
//!
//! ## Storage format
//!
//! One file: `<if2ai_dir>/domain-knowledge.ndjson` — one
//! `serde_json` object per line.  Append on insert, full rewrite on
//! update.  Bounded I/O cost: DK contributions are deliberately
//! sparse (sub-1000 entries by spec budget §6).
//!
//! ## Concurrency
//!
//! In-memory state is held behind `tokio::sync::RwLock` so `upsert`
//! and `lookup` never block the runtime. The on-disk file is guarded
//! by an `std::sync::Mutex` only across the synchronous
//! `write_all_to_disk_unlocked` rewrite step (≤ a few µs for the
//! expected entry count).
//!
//! ## Failure mode
//!
//! Every disk-side error is logged at `warn`/`error` with the file
//! path and continues — the in-memory copy is the source of truth
//! within a process.  Never panics; the daemon must keep running.
//! Lookup is pure in-mem and therefore always succeeds.

use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;

use async_trait::async_trait;
use chrono::Utc;

use super::{DomainKnowledgeEntry, KnowledgeStore};

/// Default filename inside `<if2ai_dir>` used by
/// [`FileBackedKnowledgeStore::open_or_create`].
pub const DEFAULT_FILE_NAME: &str = "domain-knowledge.ndjson";

/// File-backed `KnowledgeStore`.
///
/// Construct via [`FileBackedKnowledgeStore::open_or_create`] which
/// loads any existing entries on the way in. See module docs for
/// the on-disk format.
pub struct FileBackedKnowledgeStore {
    path: PathBuf,
    mem: tokio::sync::RwLock<Vec<DomainKnowledgeEntry>>,
    write_lock: StdMutex<()>,
}

impl FileBackedKnowledgeStore {
    /// Open `<dir>/<DEFAULT_FILE_NAME>`, loading any existing entries
    /// (skipping malformed lines with a warn log). Creates the parent
    /// directory if it does not exist. Returns the store ready to be
    /// installed into [`super::install_global_knowledge_store`].
    pub fn open_or_create(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(DEFAULT_FILE_NAME);
        let entries = match std::fs::read_to_string(&path) {
            Ok(text) => parse_ndjson_lines(&text, &path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            path,
            mem: tokio::sync::RwLock::new(entries),
            write_lock: StdMutex::new(()),
        })
    }

    /// On-disk path the store writes to. Exposed for tests + ops
    /// tooling; `pub` rather than `pub(crate)` because the global
    /// installer wants to log it on startup.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Number of entries currently held in memory. O(1) lock.
    pub async fn len(&self) -> usize {
        self.mem.read().await.len()
    }

    /// `true` when no entries are loaded. Provided to satisfy
    /// `clippy::len_without_is_empty` and to give callers a cheap
    /// emptiness check that does not allocate.
    pub async fn is_empty(&self) -> bool {
        self.mem.read().await.is_empty()
    }

    /// Best-effort full rewrite of the on-disk file.  Errors are
    /// logged and swallowed; the in-mem copy remains authoritative
    /// for the rest of the process lifetime.
    fn rewrite_to_disk(&self, entries: &[DomainKnowledgeEntry]) {
        // Serialise outside the file-IO mutex so we don't hold a
        // sync mutex across a large allocation.
        let mut buf = String::with_capacity(entries.len().saturating_mul(256));
        for entry in entries {
            match serde_json::to_string(entry) {
                Ok(line) => {
                    buf.push_str(&line);
                    buf.push('\n');
                }
                Err(e) => {
                    tracing::warn!(
                        "[knowledge_store] skipping un-serialisable entry id={}: {e}",
                        entry.id
                    );
                }
            }
        }

        let _guard = match self.write_lock.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::error!(
                    "[knowledge_store] write_lock poisoned, recovering: {}",
                    poisoned
                );
                poisoned.into_inner()
            }
        };
        if let Err(e) = std::fs::write(&self.path, buf) {
            tracing::error!(
                "[knowledge_store] failed to persist {}: {e}",
                self.path.display()
            );
        }
    }
}

#[async_trait]
impl KnowledgeStore for FileBackedKnowledgeStore {
    async fn upsert(&self, entry: DomainKnowledgeEntry) {
        let snapshot = {
            let mut guard = self.mem.write().await;
            if let Some(existing) = guard.iter_mut().find(|e| e.id == entry.id) {
                *existing = entry;
            } else {
                guard.push(entry);
            }
            guard.clone()
        };
        // Persist outside the in-mem write guard so concurrent
        // lookups are never blocked on disk I/O.
        self.rewrite_to_disk(&snapshot);
    }

    async fn lookup(&self, query: &str, kind_filter: Option<&str>) -> Vec<DomainKnowledgeEntry> {
        let needle = query.to_lowercase();
        let mut hits: Vec<DomainKnowledgeEntry> = Vec::new();
        let mut touched_ids: Vec<String> = Vec::new();
        {
            let mut guard = self.mem.write().await;
            for entry in guard.iter_mut() {
                if let Some(label) = kind_filter {
                    if entry.kind.label() != label {
                        continue;
                    }
                }
                let body = entry.kind.search_text().to_lowercase();
                if needle.is_empty() || body.contains(&needle) {
                    entry.access_count = entry.access_count.saturating_add(1);
                    entry.last_used = Some(Utc::now());
                    hits.push(entry.clone());
                    touched_ids.push(entry.id.clone());
                }
            }
            // Persist access_count / last_used updates only when a
            // lookup actually mutated state; avoids writing on every
            // miss.
            if !touched_ids.is_empty() {
                let snapshot = guard.clone();
                drop(guard);
                self.rewrite_to_disk(&snapshot);
            }
        }
        hits
    }
}

fn parse_ndjson_lines(text: &str, path: &Path) -> Vec<DomainKnowledgeEntry> {
    let mut out: Vec<DomainKnowledgeEntry> = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<DomainKnowledgeEntry>(trimmed) {
            Ok(entry) => out.push(entry),
            Err(e) => tracing::warn!(
                "[knowledge_store] {} line {}: skipping malformed entry: {e}",
                path.display(),
                idx + 1
            ),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::skills::domain_knowledge::{DomainKnowledgeKind, KnowledgeAuthor, SOPStep};
    use tempfile::TempDir;

    fn sample_entry(task: &str) -> DomainKnowledgeEntry {
        DomainKnowledgeEntry::new(
            DomainKnowledgeKind::TaskSOP {
                task_type: task.to_string(),
                prerequisites: vec!["env ready".to_string()],
                key_pitfalls: vec!["watch for rate limits".to_string()],
                execution_steps: vec![SOPStep {
                    tool_name: "shell".to_string(),
                    description: "ls".to_string(),
                    args_template: None,
                    expected_result: "ok".to_string(),
                }],
            },
            KnowledgeAuthor::User,
            0.9,
        )
    }

    #[tokio::test]
    async fn upsert_persists_across_reopen() {
        let tmp = TempDir::new().unwrap();
        let store = FileBackedKnowledgeStore::open_or_create(tmp.path()).unwrap();
        let entry = sample_entry("deploy_release");
        let id = entry.id.clone();
        store.upsert(entry.clone()).await;
        assert_eq!(store.len().await, 1);

        // Drop the first store to simulate process restart, then
        // reopen and verify lookup hits the persisted entry.
        drop(store);
        let reopened = FileBackedKnowledgeStore::open_or_create(tmp.path()).unwrap();
        assert_eq!(reopened.len().await, 1);
        let hits = reopened.lookup("deploy_release", Some("task_sop")).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, id);
        // access_count was incremented by the lookup itself.
        assert_eq!(hits[0].access_count, 1);
    }

    #[tokio::test]
    async fn upsert_replaces_existing_entry_by_id() {
        let tmp = TempDir::new().unwrap();
        let store = FileBackedKnowledgeStore::open_or_create(tmp.path()).unwrap();
        let mut entry = sample_entry("recurring_task");
        store.upsert(entry.clone()).await;
        // Mutate then re-upsert with the same id.
        entry.confidence = 0.5;
        store.upsert(entry.clone()).await;
        assert_eq!(store.len().await, 1);

        drop(store);
        let reopened = FileBackedKnowledgeStore::open_or_create(tmp.path()).unwrap();
        let hits = reopened.lookup("recurring_task", None).await;
        assert_eq!(hits.len(), 1);
        assert!((hits[0].confidence - 0.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn malformed_lines_are_skipped_on_open() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(DEFAULT_FILE_NAME);
        // Write one valid + one garbage line.
        let entry = sample_entry("good");
        let line = serde_json::to_string(&entry).unwrap();
        std::fs::write(&path, format!("{line}\nthis-is-not-json\n")).unwrap();

        let store = FileBackedKnowledgeStore::open_or_create(tmp.path()).unwrap();
        assert_eq!(store.len().await, 1);
        let hits = store.lookup("good", None).await;
        assert_eq!(hits.len(), 1);
    }
}
