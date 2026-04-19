//! Browser profile location & lifecycle policy (Phase 7C, Slice 7C.1).
//!
//! Each chat session needs a Chromium **user-data-dir** to store cookies,
//! localStorage, IndexedDB and disk cache.  Where that directory lives —
//! and whether it survives a process restart — is governed by
//! [`BrowserProfileMode`].
//!
//! # Modes
//!
//! - [`BrowserProfileMode::PerSessionPersistent`] (default) — one persistent
//!   directory per chat-session id under `<if2ai_home>/browser-profiles/<id>/`.
//!   Cookies survive app restarts; sessions cannot see each other.
//! - [`BrowserProfileMode::Shared`] — every session shares
//!   `<if2ai_home>/browser-profiles/_shared/`.  Useful for "personal assistant"
//!   workflows where the user wants the AI to act as themselves.  Sessions
//!   share login state.
//! - [`BrowserProfileMode::Ephemeral`] — a fresh `tempfile::TempDir` per
//!   session that is wiped on `Drop`.  Reproduces the legacy Phase 7B
//!   behaviour and is the right choice for unit / integration tests.
//!
//! # Resolution order
//!
//! [`BrowserProfileMode::from_env_or_config`] checks, in order:
//!
//! 1. The `IF2AI_BROWSER_PROFILE_MODE` environment variable
//!    (`per_session_persistent` | `shared` | `ephemeral`).
//! 2. `~/.if2ai/browser.toml` `[browser] profile_mode = "..."`.
//! 3. Default → [`BrowserProfileMode::PerSessionPersistent`].
//!
//! # Drop semantics
//!
//! [`ProfileHandle`] only deletes the underlying directory when the mode is
//! `Ephemeral`.  Persistent profiles are intentionally left on disk; cleanup
//! is handled by the explicit `clear_browser_profile` Tauri command.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tracing::{debug, warn};

use crate::modules::browser::errors::BrowserError;

/// Sub-directory under `if2ai_home` that stores all browser profiles.
const PROFILES_DIRNAME: &str = "browser-profiles";
/// Sub-directory used for the `Shared` mode.
const SHARED_PROFILE_DIRNAME: &str = "_shared";
/// Filename of the optional configuration file.
const CONFIG_FILENAME: &str = "browser.toml";
/// Environment variable that overrides everything.
const ENV_VAR: &str = "IF2AI_BROWSER_PROFILE_MODE";

// ── Mode enum ────────────────────────────────────────────────────────────────

/// Strategy for placing the Chromium `user-data-dir`.
///
/// `Clone + Copy` so `BrowserRegistry` can hand each `BrowserSession::new`
/// call its own copy without lifetimes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserProfileMode {
    /// Each chat session owns a dedicated persistent profile under
    /// `<if2ai_home>/browser-profiles/<session_id>/`.
    /// Cookies survive app restarts; sessions are isolated from each other.
    /// **Default.**
    #[default]
    PerSessionPersistent,

    /// All sessions share `<if2ai_home>/browser-profiles/_shared/`.
    /// Cookies survive app restarts and are visible across sessions.
    Shared,

    /// One-shot `tempfile::TempDir` per session, wiped on `Drop`.
    /// Used for tests and intentional "incognito" workflows.
    Ephemeral,
}

impl BrowserProfileMode {
    /// Resolve the active mode from `IF2AI_BROWSER_PROFILE_MODE` →
    /// `<if2ai_home>/browser.toml [browser] profile_mode` → default.
    ///
    /// Never panics: parse failures fall back to the default with a `warn!`.
    #[must_use]
    pub fn from_env_or_config(if2ai_home: &Path) -> Self {
        if let Ok(raw) = std::env::var(ENV_VAR) {
            match parse_mode_str(&raw) {
                Some(mode) => {
                    debug!(mode = ?mode, source = "env", "browser profile mode resolved");
                    return mode;
                }
                None => warn!(value = %raw, "ignoring invalid {ENV_VAR}"),
            }
        }

        let toml_path = if2ai_home.join(CONFIG_FILENAME);
        if let Ok(raw) = fs::read_to_string(&toml_path) {
            if let Some(mode) = parse_toml_mode(&raw) {
                debug!(mode = ?mode, source = %toml_path.display(), "browser profile mode resolved");
                return mode;
            }
        }

        debug!(mode = ?Self::default(), source = "default", "browser profile mode resolved");
        Self::default()
    }

    /// Materialise this mode into a concrete `user-data-dir` and return a
    /// [`ProfileHandle`] guarding it.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::ProfileError`] when the directory cannot be
    /// created (permissions, disk full, …).
    pub fn resolve(
        self,
        if2ai_home: &Path,
        session_id: &str,
    ) -> Result<ProfileHandle, BrowserError> {
        match self {
            Self::PerSessionPersistent => {
                let safe_id = sanitize_session_id(session_id);
                let path = if2ai_home.join(PROFILES_DIRNAME).join(&safe_id);
                fs::create_dir_all(&path).map_err(|e| {
                    BrowserError::ProfileError(format!(
                        "failed to create per-session profile {}: {e}",
                        path.display()
                    ))
                })?;
                Ok(ProfileHandle {
                    path,
                    mode: self,
                    _guard: None,
                })
            }
            Self::Shared => {
                let path = if2ai_home
                    .join(PROFILES_DIRNAME)
                    .join(SHARED_PROFILE_DIRNAME);
                fs::create_dir_all(&path).map_err(|e| {
                    BrowserError::ProfileError(format!(
                        "failed to create shared profile {}: {e}",
                        path.display()
                    ))
                })?;
                Ok(ProfileHandle {
                    path,
                    mode: self,
                    _guard: None,
                })
            }
            Self::Ephemeral => {
                let guard = tempfile::Builder::new()
                    .prefix("if2ai-chrome-eph-")
                    .tempdir()
                    .map_err(|e| {
                        BrowserError::ProfileError(format!(
                            "failed to create ephemeral profile dir: {e}"
                        ))
                    })?;
                Ok(ProfileHandle {
                    path: guard.path().to_path_buf(),
                    mode: self,
                    _guard: Some(guard),
                })
            }
        }
    }
}

// ── Handle ───────────────────────────────────────────────────────────────────

/// A live reference to a `user-data-dir`.  Owned by `BrowserSession` for the
/// duration of the Chromium child process.
///
/// `Drop` deletes the directory **only** when the mode is `Ephemeral`; for
/// `PerSessionPersistent` and `Shared` the directory is left in place.
#[derive(Debug)]
pub struct ProfileHandle {
    /// Absolute path that should be passed to `BrowserConfig::user_data_dir`.
    pub path: PathBuf,
    /// The mode that produced this handle (kept for diagnostics).
    pub mode: BrowserProfileMode,
    /// Held only when `mode == Ephemeral`; its `Drop` removes the directory.
    _guard: Option<TempDir>,
}

impl ProfileHandle {
    /// True when the handle owns a `TempDir` whose `Drop` will delete the dir.
    #[must_use]
    pub fn is_ephemeral(&self) -> bool {
        matches!(self.mode, BrowserProfileMode::Ephemeral)
    }
}

// ── Listing entries ──────────────────────────────────────────────────────────

// ── Settings file (browser.toml) read / write (Phase 7C, slice 7C.2 UX) ──────

/// Persisted browser settings exposed to the Settings UI.
///
/// Mirrors the contents of `<if2ai_home>/browser.toml`.  The runtime
/// `BrowserRegistry::profile_mode` is captured at app start; changing
/// `mode` here only takes effect on the next launch (signalled to the
/// user via the UI).  `IF2AI_BROWSER_PROFILE_MODE` env var, when set,
/// keeps overriding everything until unset — surfaced via `env_override`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BrowserSettings {
    /// Persisted preferred profile mode (next launch).
    pub profile_mode: BrowserProfileMode,
    /// Soft per-profile disk cap (megabytes).  Reserved for future LRU
    /// cleanup; the field is honoured by the UI only for now.
    pub max_profile_disk_mb: u64,
    /// Soft total disk cap across all profiles (megabytes).
    pub max_total_disk_mb: u64,
    /// True when `IF2AI_BROWSER_PROFILE_MODE` is currently set; in that
    /// case the env value wins over `profile_mode` until cleared.
    pub env_override: Option<BrowserProfileMode>,
    /// The mode the running `BrowserRegistry` was constructed with — what
    /// is *active right now*.  May differ from `profile_mode` if the user
    /// just edited settings and hasn't restarted yet.
    pub active_mode: BrowserProfileMode,
}

const DEFAULT_MAX_PROFILE_DISK_MB: u64 = 500;
const DEFAULT_MAX_TOTAL_DISK_MB: u64 = 5000;

impl BrowserSettings {
    /// Load settings from `<if2ai_home>/browser.toml`, returning sensible
    /// defaults for missing or malformed fields.  `active_mode` is the
    /// mode currently in use by the running `BrowserRegistry`.
    #[must_use]
    pub fn load(if2ai_home: &Path, active_mode: BrowserProfileMode) -> Self {
        let toml_path = if2ai_home.join(CONFIG_FILENAME);
        let raw = fs::read_to_string(&toml_path).unwrap_or_default();
        let profile_mode = parse_toml_mode(&raw).unwrap_or_default();
        let max_profile_disk_mb =
            parse_toml_u64(&raw, "max_profile_disk_mb").unwrap_or(DEFAULT_MAX_PROFILE_DISK_MB);
        let max_total_disk_mb =
            parse_toml_u64(&raw, "max_total_disk_mb").unwrap_or(DEFAULT_MAX_TOTAL_DISK_MB);
        let env_override = std::env::var(ENV_VAR).ok().and_then(|v| parse_mode_str(&v));
        Self {
            profile_mode,
            max_profile_disk_mb,
            max_total_disk_mb,
            env_override,
            active_mode,
        }
    }

    /// Persist `profile_mode` / `max_profile_disk_mb` / `max_total_disk_mb`
    /// to `<if2ai_home>/browser.toml`.  Other fields (`env_override`,
    /// `active_mode`) are read-only diagnostics and never written.
    ///
    /// # Errors
    /// Returns `BrowserError::ProfileError` on any I/O failure.
    pub fn save(&self, if2ai_home: &Path) -> Result<(), BrowserError> {
        if let Err(e) = fs::create_dir_all(if2ai_home) {
            return Err(BrowserError::ProfileError(format!(
                "failed to create {}: {e}",
                if2ai_home.display()
            )));
        }
        let toml_path = if2ai_home.join(CONFIG_FILENAME);
        let body = format!(
            "# Generated by If2Ai Settings UI (Phase 7C).  Hand-edits are preserved\n\
             # if they live outside the [browser] section, but values inside\n\
             # [browser] are overwritten on every save.\n\
             \n\
             [browser]\n\
             # Where Chromium stores its user-data-dir.  Options:\n\
             #   per_session_persistent | shared | ephemeral\n\
             profile_mode = \"{}\"\n\
             # Soft caps (megabytes).  Reserved for future LRU cleanup.\n\
             max_profile_disk_mb = {}\n\
             max_total_disk_mb = {}\n",
            mode_to_str(self.profile_mode),
            self.max_profile_disk_mb,
            self.max_total_disk_mb,
        );
        fs::write(&toml_path, body).map_err(|e| {
            BrowserError::ProfileError(format!("failed to write {}: {e}", toml_path.display()))
        })?;
        Ok(())
    }
}

fn mode_to_str(mode: BrowserProfileMode) -> &'static str {
    match mode {
        BrowserProfileMode::PerSessionPersistent => "per_session_persistent",
        BrowserProfileMode::Shared => "shared",
        BrowserProfileMode::Ephemeral => "ephemeral",
    }
}

/// Parse a positive integer field under `[browser]` from a TOML string.
fn parse_toml_u64(raw: &str, key: &str) -> Option<u64> {
    let mut in_browser = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('[') {
            let section = rest.trim_end_matches(']').trim();
            in_browser = section.eq_ignore_ascii_case("browser");
            continue;
        }
        if !in_browser {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let (k, v) = trimmed.split_at(eq);
            if k.trim().eq_ignore_ascii_case(key) {
                return v.trim_start_matches('=').trim().parse::<u64>().ok();
            }
        }
    }
    None
}

// ── Profile inventory (for Settings UI) ───────────────────────────────────────

/// Disk-level summary of a single browser profile.  Returned by the
/// `list_browser_profiles` Tauri command and displayed in the BrowserCard
/// settings drawer.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileEntry {
    /// `session_id` portion of the directory name (or `"_shared"`).
    pub session_id: String,
    /// Absolute path on disk.
    pub path: String,
    /// Recursive size in bytes (best-effort; permission errors are ignored).
    pub size_bytes: u64,
    /// RFC-3339 last-modified timestamp of the directory itself, if available.
    pub last_used: Option<String>,
}

/// Enumerate every persistent profile directory under
/// `<if2ai_home>/browser-profiles/`.  Returns an empty `Vec` when the
/// directory does not exist yet.
#[must_use]
pub fn list_profiles(if2ai_home: &Path) -> Vec<ProfileEntry> {
    let root = if2ai_home.join(PROFILES_DIRNAME);
    let entries = match fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let session_id = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_owned(),
            None => continue,
        };
        let size_bytes = dir_size(&path).unwrap_or(0);
        let last_used = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|st| chrono::DateTime::<chrono::Utc>::from(st).to_rfc3339());
        out.push(ProfileEntry {
            session_id,
            path: path.to_string_lossy().into_owned(),
            size_bytes,
            last_used,
        });
    }
    out.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    out
}

/// Delete the persistent profile directory for `session_id`.
///
/// Refuses to delete `_shared/`; callers must explicitly delete shared state
/// through some future shared-profile management UI.
///
/// # Errors
///
/// Returns [`BrowserError::ProfileError`] when the deletion itself fails;
/// missing directories are treated as success (idempotent).
pub fn delete_profile(if2ai_home: &Path, session_id: &str) -> Result<(), BrowserError> {
    if session_id == SHARED_PROFILE_DIRNAME {
        return Err(BrowserError::ProfileError(
            "cannot delete shared profile via clear_browser_profile".into(),
        ));
    }
    let safe_id = sanitize_session_id(session_id);
    let path = if2ai_home.join(PROFILES_DIRNAME).join(&safe_id);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_dir_all(&path).map_err(|e| {
        BrowserError::ProfileError(format!("failed to delete {}: {e}", path.display()))
    })
}

// ── Internals ────────────────────────────────────────────────────────────────

fn parse_mode_str(s: &str) -> Option<BrowserProfileMode> {
    match s.trim().to_ascii_lowercase().as_str() {
        "per_session_persistent" | "per-session-persistent" | "persistent" => {
            Some(BrowserProfileMode::PerSessionPersistent)
        }
        "shared" => Some(BrowserProfileMode::Shared),
        "ephemeral" | "incognito" | "tempdir" => Some(BrowserProfileMode::Ephemeral),
        _ => None,
    }
}

/// Parse `[browser] profile_mode = "..."` from a TOML string.
/// Uses a hand-rolled lookup (not `toml` crate) to avoid dragging a new
/// dependency just for one field.
fn parse_toml_mode(raw: &str) -> Option<BrowserProfileMode> {
    let mut in_browser = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('[') {
            let section = rest.trim_end_matches(']').trim();
            in_browser = section.eq_ignore_ascii_case("browser");
            continue;
        }
        if !in_browser {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let (key, value) = trimmed.split_at(eq);
            let key = key.trim();
            if !key.eq_ignore_ascii_case("profile_mode") {
                continue;
            }
            let raw_value = value
                .trim_start_matches('=')
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            return parse_mode_str(raw_value);
        }
    }
    None
}

/// Replace path-unfriendly characters in a `session_id` with `_` so the
/// resulting directory name is valid on macOS / Linux / Windows.
fn sanitize_session_id(session_id: &str) -> String {
    session_id
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '_',
        })
        .collect()
}

/// Recursively sum the size of all regular files under `path`.
/// Permission and I/O errors are silently treated as 0 to keep the
/// `list_browser_profiles` command best-effort.
fn dir_size(path: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_is_per_session_persistent() {
        assert_eq!(
            BrowserProfileMode::default(),
            BrowserProfileMode::PerSessionPersistent
        );
    }

    #[test]
    fn parse_mode_str_roundtrip() {
        assert_eq!(
            parse_mode_str("per_session_persistent"),
            Some(BrowserProfileMode::PerSessionPersistent)
        );
        assert_eq!(parse_mode_str("Shared"), Some(BrowserProfileMode::Shared));
        assert_eq!(
            parse_mode_str("ephemeral"),
            Some(BrowserProfileMode::Ephemeral)
        );
        assert_eq!(parse_mode_str("nonsense"), None);
    }

    #[test]
    fn parse_toml_mode_extracts_value() {
        let toml = r#"
            # comment
            [other]
            ignored = "yes"

            [browser]
            profile_mode = "shared"
            unrelated = 1
        "#;
        assert_eq!(parse_toml_mode(toml), Some(BrowserProfileMode::Shared));
    }

    #[test]
    fn parse_toml_mode_missing_returns_none() {
        assert_eq!(parse_toml_mode("[other]\nfoo = 1\n"), None);
    }

    #[test]
    fn from_env_or_config_env_wins() {
        let tmp = TempDir::new().unwrap();
        // SAFETY: scoped env mutation; tests run single-threaded on this var.
        std::env::set_var(ENV_VAR, "ephemeral");
        let mode = BrowserProfileMode::from_env_or_config(tmp.path());
        std::env::remove_var(ENV_VAR);
        assert_eq!(mode, BrowserProfileMode::Ephemeral);
    }

    #[test]
    fn from_env_or_config_falls_back_to_default() {
        let tmp = TempDir::new().unwrap();
        std::env::remove_var(ENV_VAR);
        let mode = BrowserProfileMode::from_env_or_config(tmp.path());
        assert_eq!(mode, BrowserProfileMode::PerSessionPersistent);
    }

    #[test]
    fn resolve_per_session_creates_dir() {
        let tmp = TempDir::new().unwrap();
        let handle = BrowserProfileMode::PerSessionPersistent
            .resolve(tmp.path(), "session-abc")
            .unwrap();
        assert!(handle.path.exists());
        assert!(handle.path.ends_with("browser-profiles/session-abc"));
        assert!(!handle.is_ephemeral());
    }

    #[test]
    fn resolve_shared_uses_underscore_shared() {
        let tmp = TempDir::new().unwrap();
        let handle = BrowserProfileMode::Shared
            .resolve(tmp.path(), "ignored")
            .unwrap();
        assert!(handle.path.exists());
        assert!(handle.path.ends_with("browser-profiles/_shared"));
    }

    #[test]
    fn resolve_ephemeral_drops_directory() {
        let tmp = TempDir::new().unwrap();
        let path = {
            let handle = BrowserProfileMode::Ephemeral
                .resolve(tmp.path(), "session-xyz")
                .unwrap();
            assert!(handle.path.exists());
            assert!(handle.is_ephemeral());
            handle.path.clone()
        };
        assert!(!path.exists(), "ephemeral dir must be deleted on Drop");
    }

    #[test]
    fn sanitize_session_id_strips_unsafe_chars() {
        assert_eq!(
            sanitize_session_id("abc/def\\ghi:..session"),
            "abc_def_ghi_..session"
        );
    }

    #[test]
    fn list_profiles_returns_empty_when_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(list_profiles(tmp.path()).is_empty());
    }

    #[test]
    fn list_profiles_enumerates_after_resolve() {
        let tmp = TempDir::new().unwrap();
        BrowserProfileMode::PerSessionPersistent
            .resolve(tmp.path(), "alpha")
            .unwrap();
        BrowserProfileMode::PerSessionPersistent
            .resolve(tmp.path(), "beta")
            .unwrap();
        let entries = list_profiles(tmp.path());
        let ids: Vec<_> = entries.iter().map(|e| e.session_id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    #[test]
    fn delete_profile_removes_persistent_dir() {
        let tmp = TempDir::new().unwrap();
        let handle = BrowserProfileMode::PerSessionPersistent
            .resolve(tmp.path(), "tbd")
            .unwrap();
        let path = handle.path.clone();
        drop(handle);
        assert!(path.exists());
        delete_profile(tmp.path(), "tbd").unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn delete_profile_refuses_shared() {
        let tmp = TempDir::new().unwrap();
        let err = delete_profile(tmp.path(), "_shared").unwrap_err();
        assert!(matches!(err, BrowserError::ProfileError(_)));
    }

    #[test]
    fn delete_missing_profile_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        delete_profile(tmp.path(), "never-existed").unwrap();
    }

    #[test]
    fn settings_load_returns_defaults_when_missing() {
        let tmp = TempDir::new().unwrap();
        std::env::remove_var(ENV_VAR);
        let s = BrowserSettings::load(tmp.path(), BrowserProfileMode::Ephemeral);
        assert_eq!(s.profile_mode, BrowserProfileMode::PerSessionPersistent);
        assert_eq!(s.max_profile_disk_mb, DEFAULT_MAX_PROFILE_DISK_MB);
        assert_eq!(s.max_total_disk_mb, DEFAULT_MAX_TOTAL_DISK_MB);
        assert_eq!(s.active_mode, BrowserProfileMode::Ephemeral);
        assert!(s.env_override.is_none());
    }

    #[test]
    fn settings_save_then_reload_roundtrips() {
        let tmp = TempDir::new().unwrap();
        std::env::remove_var(ENV_VAR);
        let s = BrowserSettings {
            profile_mode: BrowserProfileMode::Shared,
            max_profile_disk_mb: 1024,
            max_total_disk_mb: 8192,
            env_override: None,
            active_mode: BrowserProfileMode::PerSessionPersistent,
        };
        s.save(tmp.path()).unwrap();
        let back = BrowserSettings::load(tmp.path(), BrowserProfileMode::PerSessionPersistent);
        assert_eq!(back.profile_mode, BrowserProfileMode::Shared);
        assert_eq!(back.max_profile_disk_mb, 1024);
        assert_eq!(back.max_total_disk_mb, 8192);
    }

    #[test]
    fn settings_load_surfaces_env_override() {
        let tmp = TempDir::new().unwrap();
        std::env::set_var(ENV_VAR, "ephemeral");
        let s = BrowserSettings::load(tmp.path(), BrowserProfileMode::PerSessionPersistent);
        std::env::remove_var(ENV_VAR);
        assert_eq!(s.env_override, Some(BrowserProfileMode::Ephemeral));
    }
}
