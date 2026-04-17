//! Browser cold-state persistence.
//!
//! Persists a mapping of `session_id → last-visited URL` across app restarts
//! so that the AI's browser can auto-resume where it left off when a session
//! is next accessed.
//!
//! # File location
//!
//! The JSON file path is injected at construction time and lives under
//! `~/.if2ai/` (never inside the app bundle). See `BrowserRegistry::new()`.
//!
//! # Error handling
//!
//! All I/O failures are logged as `warn!` — they never `panic!` and never
//! propagate to callers. A missing or corrupt file silently starts fresh.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;

// ── Data types ────────────────────────────────────────────────────────────────

/// Persistent mapping of `session_id → last visited URL`.
///
/// Only the `sessions` field is serialised; `path` is kept as runtime state
/// so `save()` knows where to write without needing an external path argument.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ColdState {
    /// Maps chat-session identifier to the last-navigated URL.
    sessions: HashMap<String, String>,
    /// File path; **not** serialised (skipped by serde).
    #[serde(skip)]
    path: PathBuf,
}

// ── Public API ────────────────────────────────────────────────────────────────

impl ColdState {
    /// Load the cold state from `path`, or return an empty `ColdState` on any
    /// I/O or parse failure (no panic, no propagation — just `warn!`).
    #[must_use]
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => match serde_json::from_str::<ColdState>(&contents) {
                Ok(mut state) => {
                    state.path = path.to_owned();
                    state
                }
                Err(e) => {
                    warn!(
                        path = %path.display(),
                        "cold-state JSON parse failed ({e}); starting fresh"
                    );
                    Self {
                        path: path.to_owned(),
                        ..Default::default()
                    }
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // First run — no file yet, that's fine.
                Self {
                    path: path.to_owned(),
                    ..Default::default()
                }
            }
            Err(e) => {
                warn!(
                    path = %path.display(),
                    "cold-state read failed ({e}); starting fresh"
                );
                Self {
                    path: path.to_owned(),
                    ..Default::default()
                }
            }
        }
    }

    /// Persist the current state to disk.
    ///
    /// Creates parent directories as needed. All errors are logged as `warn!`
    /// and silently ignored — the app must not crash on a save failure.
    pub fn save(&self) {
        if let Some(parent) = self.path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!(
                    path = %self.path.display(),
                    "cold-state: failed to create parent dir: {e}"
                );
                return;
            }
        }

        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.path, &json) {
                    warn!(
                        path = %self.path.display(),
                        "cold-state write failed: {e}"
                    );
                }
            }
            Err(e) => {
                warn!("cold-state serialize failed: {e}");
            }
        }
    }

    /// Record that `session_id` last visited `url`.
    ///
    /// Persists immediately by calling [`Self::save`].
    pub fn set(&mut self, session_id: &str, url: &str) {
        self.sessions.insert(session_id.to_owned(), url.to_owned());
        self.save();
    }

    /// Remove the cold-state record for `session_id` (called on browser close).
    ///
    /// Persists immediately by calling [`Self::save`].
    pub fn remove(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
        self.save();
    }

    /// Return the last-saved URL for `session_id`, if any.
    #[must_use]
    pub fn get(&self, session_id: &str) -> Option<&str> {
        self.sessions.get(session_id).map(String::as_str)
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// Returns a path to a non-existent file in a temp location.
    /// The `NamedTempFile` is dropped before this function returns, so the
    /// file is already removed — suitable for "missing file" test cases.
    fn tmp_path() -> PathBuf {
        NamedTempFile::new().unwrap().into_temp_path().to_path_buf()
    }

    #[test]
    fn test_load_missing_file_starts_fresh() {
        let path = tmp_path(); // exists but empty after into_temp_path
                               // Use a non-existent sub-path.
        let missing = path.with_extension("nonexistent.json");
        let cs = ColdState::load(&missing);
        assert!(cs.get("any-session").is_none());
    }

    #[test]
    fn test_set_and_get() {
        let path = tmp_path();
        let mut cs = ColdState::load(&path);
        cs.set("session-1", "https://example.com");
        assert_eq!(cs.get("session-1"), Some("https://example.com"));
    }

    #[test]
    fn test_remove() {
        let path = tmp_path();
        let mut cs = ColdState::load(&path);
        cs.set("session-1", "https://example.com");
        cs.remove("session-1");
        assert!(cs.get("session-1").is_none());
    }

    #[test]
    fn test_save_and_reload() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        let mut cs = ColdState::load(&path);
        cs.set("s1", "https://github.com");
        cs.set("s2", "https://rust-lang.org");
        // save() is called implicitly by set()

        // Reload from disk.
        let cs2 = ColdState::load(&path);
        assert_eq!(cs2.get("s1"), Some("https://github.com"));
        assert_eq!(cs2.get("s2"), Some("https://rust-lang.org"));
    }

    #[test]
    fn test_remove_persisted() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();

        let mut cs = ColdState::load(&path);
        cs.set("s1", "https://example.com");
        cs.remove("s1");

        let cs2 = ColdState::load(&path);
        assert!(cs2.get("s1").is_none());
    }

    #[test]
    fn test_corrupt_file_starts_fresh() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();
        std::fs::write(&path, b"{ this is not valid JSON !!!").unwrap();

        let cs = ColdState::load(&path);
        // Should not panic; should return empty state.
        assert!(cs.get("any").is_none());
    }
}
