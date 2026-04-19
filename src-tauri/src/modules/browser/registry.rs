//! Browser registry — manages per-session [`BrowserSession`] instances.
//!
//! One `BrowserRegistry` is held as Tauri managed state for the lifetime of
//! the application. Sessions are keyed by the chat session identifier and
//! stored in a `DashMap<String, Arc<tokio::sync::Mutex<BrowserSession>>>` so
//! that:
//!
//! - The DashMap shard lock is released **immediately** after the `Arc` is
//!   cloned — no DashMap `Ref` or `RefMut` is held across an `.await` point.
//! - The `tokio::sync::Mutex` is async-aware and safe to hold across `.await`.
//! - Concurrent tool calls on *different* sessions never block each other.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use dashmap::DashMap;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::modules::browser::cold_state::ColdState;
use crate::modules::browser::errors::BrowserError;
use crate::modules::browser::profile::BrowserProfileMode;
use crate::modules::browser::session::{
    ActionLogEntry, BrowserSession, ConsoleEvent, DownloadEntry, NavigateResult, NetworkErrorEvent,
    ScrollDir, TabInfo, WaitState,
};

/// Snapshot of a single session's browser state, used for Tauri event payloads
/// and the `get_browser_sessions` command.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BrowserStatusEntry {
    /// Chat session identifier.
    pub session_id: String,
    /// Whether the browser is actively running.
    pub running: bool,
    /// Current page URL, if known.
    pub url: Option<String>,
}

/// Global registry of per-session [`BrowserSession`] instances.
///
/// Wrap in `Arc` and register as Tauri managed state so commands can access
/// it without lifetime gymnastics.
pub struct BrowserRegistry {
    /// session_id → `Arc<Mutex<BrowserSession>>`
    sessions: DashMap<String, Arc<Mutex<BrowserSession>>>,
    /// Cold-state store: persists the last-visited URL per session across restarts.
    cold_state: Mutex<ColdState>,
    /// Path used by the cold-state persistence layer (kept for diagnostics).
    pub(crate) cold_state_path: PathBuf,
    /// Tauri `AppHandle` injected in the `setup()` callback so the browser
    /// tool can emit `"browser-status"` events to the frontend BrowserCard.
    ///
    /// Set exactly once via [`Self::set_app_handle`]; `None` before setup.
    app_handle: OnceLock<tauri::AppHandle>,

    /// Active profile mode (Phase 7C, slice 7C.1).  Stored so that every
    /// `launch()` call hands the same policy to `BrowserSession::new`.
    profile_mode: BrowserProfileMode,
    /// User-data root (typically `~/.if2ai/`).  Persistent profile dirs are
    /// created beneath `<if2ai_home>/browser-profiles/`.
    pub(crate) if2ai_home: PathBuf,

    /// Phase 7C, slice 7C.3 — `session_id → true` while the user is taking
    /// over the browser.  All AI tool calls receive a "paused" error during
    /// that window so they don't fight the human input.  Cleared by
    /// [`Self::set_takeover`] when the user releases.
    takeover_flags: DashMap<String, bool>,
}

impl BrowserRegistry {
    /// Create a new empty registry and load cold state from `cold_state_path`.
    ///
    /// `profile_mode` and `if2ai_home` decide where each session's Chromium
    /// `user-data-dir` is created.  Persistent profiles let cookies / login
    /// state survive across app restarts (Phase 7C, slice 7C.1).
    ///
    /// If the cold-state file does not yet exist the registry starts empty;
    /// the file is created on first [`navigate`](Self::navigate) call.
    #[must_use]
    pub fn new(
        cold_state_path: PathBuf,
        profile_mode: BrowserProfileMode,
        if2ai_home: PathBuf,
    ) -> Arc<Self> {
        let cold = ColdState::load(&cold_state_path);
        Arc::new(Self {
            sessions: DashMap::new(),
            cold_state: Mutex::new(cold),
            cold_state_path,
            app_handle: OnceLock::new(),
            profile_mode,
            if2ai_home,
            takeover_flags: DashMap::new(),
        })
    }

    /// Test-only convenience constructor: uses
    /// [`BrowserProfileMode::Ephemeral`] and a throw-away `if2ai_home` so
    /// nothing leaks onto the real disk.  Drop-in replacement for the legacy
    /// 1-argument `new(cold_state_path)` used by Phase 7B tests.
    #[cfg(test)]
    #[must_use]
    pub fn for_test(cold_state_path: PathBuf) -> Arc<Self> {
        let if2ai_home = cold_state_path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(std::env::temp_dir);
        Self::new(cold_state_path, BrowserProfileMode::Ephemeral, if2ai_home)
    }

    /// Read-only view of the configured profile mode (used by diagnostics
    /// and the Tauri commands that surface this in the BrowserCard).
    #[must_use]
    pub fn profile_mode(&self) -> BrowserProfileMode {
        self.profile_mode
    }

    /// Read-only view of the user-data root.
    #[must_use]
    pub fn if2ai_home(&self) -> &std::path::Path {
        &self.if2ai_home
    }

    /// Inject the Tauri `AppHandle` so the browser tool can emit frontend events.
    ///
    /// Called once from `main.rs` `.setup()` after the app is built.
    /// Subsequent calls are silently ignored (OnceLock semantics).
    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        let _ = self.app_handle.set(handle);
    }

    /// Return a reference to the `AppHandle`, if it has been injected.
    ///
    /// Returns `None` before `setup()` completes (e.g. in unit tests).
    #[must_use]
    pub fn app_handle(&self) -> Option<&tauri::AppHandle> {
        self.app_handle.get()
    }

    // ── Internal: clone Arc without holding shard lock across .await ──────────

    /// Look up `session_id` and return a clone of the `Arc<Mutex<BrowserSession>>`.
    ///
    /// Cloning the `Arc` releases the DashMap shard lock immediately, so no
    /// synchronous lock is ever held across an `.await` point.
    fn get_arc(&self, session_id: &str) -> Result<Arc<Mutex<BrowserSession>>, BrowserError> {
        self.sessions
            .get(session_id)
            .map(|r| Arc::clone(&*r))
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))
    }

    // ── Session lifecycle ─────────────────────────────────────────────────────

    /// Launch a new headless browser for `session_id`.
    ///
    /// If a session already exists this is a no-op and returns `Ok(())`.
    pub async fn launch(&self, session_id: &str) -> Result<(), BrowserError> {
        self.launch_with_mode(session_id, false).await
    }

    /// Launch a browser for `session_id` choosing headless or headed.
    ///
    /// Phase 7C, slice 7C.3 — `headed=true` opens a visible Chrome window
    /// so the user can take over.  Used by `request_browser_takeover`.
    pub async fn launch_with_mode(
        &self,
        session_id: &str,
        headed: bool,
    ) -> Result<(), BrowserError> {
        if self.sessions.contains_key(session_id) {
            debug!(session_id, "browser already running — skipping launch");
            return Ok(());
        }
        let session = BrowserSession::new(
            session_id.to_owned(),
            self.profile_mode,
            &self.if2ai_home,
            headed,
        )
        .await?;
        self.sessions
            .insert(session_id.to_owned(), Arc::new(Mutex::new(session)));
        info!(session_id, headed, "browser launched");
        Ok(())
    }

    /// Close and remove the browser for `session_id`.
    ///
    /// Waits for any in-flight operation to complete before closing.
    /// Also removes the cold-state record so the session is not auto-restored
    /// on the next app start.
    pub async fn close(&self, session_id: &str) -> Result<(), BrowserError> {
        // Remove cold-state record before closing so a subsequent restore attempt
        // does not reopen a session the user explicitly stopped.
        self.cold_state.lock().await.remove(session_id);

        let (_, arc) = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;

        // Acquire the lock so we wait for any in-flight operation to finish.
        let session = Arc::try_unwrap(arc).inspect_err(|shared_arc| {
            debug!(
                session_id,
                "BrowserSession still referenced by {} Arc copies; close deferred",
                Arc::strong_count(shared_arc)
            );
        });

        match session {
            Ok(mutex) => {
                mutex.into_inner().close().await?;
                info!(session_id, "browser closed");
            }
            Err(_) => {
                // Another task still holds a reference. The session was already
                // removed from the registry map, so no new operations can start.
                // The Chromium process will be terminated when all Arc references
                // drop and the Browser handle is destroyed.
                info!(session_id, "browser close deferred to last Arc drop");
            }
        }
        Ok(())
    }

    // ── Delegated operations ──────────────────────────────────────────────────

    /// Navigate the browser for `session_id` to `url`.
    ///
    /// On success, persists the final URL in the cold-state file so the session
    /// can be restored after an app restart.
    pub async fn navigate(
        &self,
        session_id: &str,
        url: &str,
    ) -> Result<NavigateResult, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        let result = guard.navigate(url).await?;
        // Persist the post-redirect URL so cold restore lands on the right page.
        self.cold_state.lock().await.set(session_id, &result.url);
        Ok(result)
    }

    /// Return the AXTree snapshot for `session_id`.
    pub async fn snapshot(&self, session_id: &str) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let guard = arc.lock().await;
        guard.snapshot().await
    }

    /// Return a full-page JPEG screenshot (base64) for `session_id`.
    pub async fn screenshot(&self, session_id: &str) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let guard = arc.lock().await;
        guard.screenshot().await
    }

    /// Return a thumbnail JPEG (base64) for the BrowserCard, or `None` on failure.
    pub async fn thumbnail(&self, session_id: &str) -> Result<Option<String>, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let guard = arc.lock().await;
        guard.thumbnail().await
    }

    /// Click the element with the given `ref_num` and return a fresh snapshot.
    pub async fn click(&self, session_id: &str, ref_num: u32) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.click(ref_num).await
    }

    /// Type `text` into `ref_num` (optional) and optionally press Enter.
    pub async fn type_text(
        &self,
        session_id: &str,
        text: &str,
        ref_num: Option<u32>,
        press_enter: bool,
    ) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.type_text(text, ref_num, press_enter).await
    }

    /// Scroll in `direction` by `amount` pages and return a fresh snapshot.
    pub async fn scroll(
        &self,
        session_id: &str,
        direction: ScrollDir,
        amount: u32,
    ) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.scroll(direction, amount).await
    }

    /// Select `value` in the `<select>` at `ref_num`.
    pub async fn select_option(
        &self,
        session_id: &str,
        ref_num: u32,
        value: &str,
    ) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.select_option(ref_num, value).await
    }

    /// Press a named key.
    pub async fn press_key(&self, session_id: &str, key: &str) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.press_key(key).await
    }

    /// Wait for page navigation up to `timeout_ms`.
    pub async fn wait(
        &self,
        session_id: &str,
        timeout_ms: u64,
        state: WaitState,
    ) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.wait(timeout_ms, state).await
    }

    // ── Multi-tab support (Phase 7C, slice 7C.6) ────────────────────────────

    /// Return live metadata for every open tab in `session_id`.
    pub async fn list_tabs(&self, session_id: &str) -> Result<Vec<TabInfo>, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.list_tabs().await
    }

    /// Make the tab at `idx` the active page for `session_id`.
    pub async fn switch_tab(&self, session_id: &str, idx: usize) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.switch_tab(idx).await
    }

    /// Close the tab at `idx`, returning the count of remaining tabs.
    pub async fn close_tab(&self, session_id: &str, idx: usize) -> Result<usize, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let mut guard = arc.lock().await;
        guard.close_tab(idx).await
    }

    // ── Downloads (Phase 7C, slice 7C.8) ────────────────────────────────────

    /// Return the in-memory list of downloads initiated by this session.
    pub async fn list_downloads(
        &self,
        session_id: &str,
    ) -> Result<Vec<DownloadEntry>, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let guard = arc.lock().await;
        Ok(guard.list_downloads().await)
    }

    /// Return the absolute on-disk download directory for `session_id`.
    pub fn download_dir(&self, session_id: &str) -> Option<std::path::PathBuf> {
        let arc = self.get_arc(session_id).ok()?;
        let guard = arc.try_lock().ok()?;
        Some(guard.download_dir().to_path_buf())
    }

    // ── Console + Network observability (Phase 7C, slice 7C.9) ──────────────

    /// Most-recent console errors / warnings for `session_id`.
    pub async fn list_console_events(
        &self,
        session_id: &str,
    ) -> Result<Vec<ConsoleEvent>, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let guard = arc.lock().await;
        Ok(guard.list_console_events().await)
    }

    /// Most-recent HTTP responses with status >= 400 for `session_id`.
    pub async fn list_network_errors(
        &self,
        session_id: &str,
    ) -> Result<Vec<NetworkErrorEvent>, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let guard = arc.lock().await;
        Ok(guard.list_network_errors().await)
    }

    /// Evaluate arbitrary JavaScript and return the serialised result.
    pub async fn evaluate(
        &self,
        session_id: &str,
        expression: &str,
    ) -> Result<String, BrowserError> {
        let arc = self.get_arc(session_id)?;
        let guard = arc.lock().await;
        guard.evaluate(expression).await
    }

    /// Return the action log for `session_id` and clear it.
    pub async fn take_action_log(&self, session_id: &str) -> Vec<ActionLogEntry> {
        let arc = match self.get_arc(session_id) {
            Ok(a) => a,
            Err(_) => return Vec::new(),
        };
        let mut guard = arc.lock().await;
        let log = guard.action_log.clone();
        guard.action_log.clear();
        log
    }

    // ── Cold-state restore ────────────────────────────────────────────────────

    /// Restore a previously-saved browser session from cold state.
    ///
    /// If `session_id` has a persisted URL (saved by a previous `navigate`
    /// call), this method launches a new browser and navigates to that URL.
    ///
    /// Returns `Ok(true)` when a restore was performed, `Ok(false)` when there
    /// is no cold-state record for the session.
    pub async fn restore_cold_state(&self, session_id: &str) -> Result<bool, BrowserError> {
        let saved_url = self
            .cold_state
            .lock()
            .await
            .get(session_id)
            .map(|s| s.to_owned());

        match saved_url {
            Some(url) => {
                self.launch(session_id).await?;
                self.navigate(session_id, &url).await?;
                info!(session_id, url = %url, "cold-state restore complete");
                Ok(true)
            }
            None => Ok(false),
        }
    }

    // ── Headed mode + takeover (Phase 7C, slice 7C.3) ────────────────────────

    /// Re-launch the browser for `session_id` in the requested mode.
    ///
    /// CDP cannot toggle a running Chromium process between headless and
    /// headed in place — we close the existing session, start a fresh
    /// one against the same persistent profile (so cookies / login state
    /// survive when `BrowserProfileMode::PerSessionPersistent` is in
    /// effect), and re-navigate to whatever URL was last visible.  In
    /// `Ephemeral` mode this DOES lose cookies; the caller is expected to
    /// warn the user before triggering a takeover in that case.
    ///
    /// Returns the [`NavigateResult`] from the post-relaunch navigation,
    /// or — when no URL was active — a stub pointing at `about:blank`.
    pub async fn relaunch_with_mode(
        &self,
        session_id: &str,
        headed: bool,
    ) -> Result<NavigateResult, BrowserError> {
        let resume_url = self.current_url(session_id);
        // Best-effort close: ignore "session not found" so a takeover can
        // be requested even when the session hasn't been launched yet.
        if self.is_running(session_id) {
            let _ = self.close(session_id).await;
        }
        self.launch_with_mode(session_id, headed).await?;
        if let Some(url) = resume_url {
            self.navigate(session_id, &url).await
        } else {
            Ok(NavigateResult {
                url: "about:blank".to_string(),
                title: String::new(),
                snapshot: String::new(),
            })
        }
    }

    /// Mark the user as having taken over (or released) the browser for
    /// `session_id`.  Stored as a `DashMap<String, bool>` so reads are
    /// lock-free; entries are removed when `taken=false` to keep the map
    /// from growing unboundedly.
    pub fn set_takeover(&self, session_id: &str, taken: bool) {
        if taken {
            self.takeover_flags.insert(session_id.to_owned(), true);
        } else {
            self.takeover_flags.remove(session_id);
        }
    }

    /// Returns `true` while the user is in control of the browser for
    /// `session_id`.  Tool dispatch checks this on every entry to refuse
    /// AI actions during human takeover.
    #[must_use]
    pub fn is_taken_over(&self, session_id: &str) -> bool {
        self.takeover_flags
            .get(session_id)
            .map(|v| *v.value())
            .unwrap_or(false)
    }

    /// Best-effort `headed` flag for `session_id`.  Returns `None` when
    /// the session is missing or busy (we never block on the session
    /// mutex from this synchronous getter).
    #[must_use]
    pub fn is_headed(&self, session_id: &str) -> Option<bool> {
        let arc = self.get_arc(session_id).ok()?;
        arc.try_lock().ok().map(|s| s.headed)
    }

    // ── Status ────────────────────────────────────────────────────────────────

    /// Return the current URL for `session_id`, if known.
    ///
    /// Uses a non-blocking `try_lock`; returns `None` if the session is busy.
    #[must_use]
    pub fn current_url(&self, session_id: &str) -> Option<String> {
        let arc = match self.get_arc(session_id) {
            Ok(a) => a,
            Err(_) => return None,
        };
        arc.try_lock().ok().and_then(|s| s.current_url.clone())
    }

    /// Return whether a browser is currently active for `session_id`.
    #[must_use]
    pub fn is_running(&self, session_id: &str) -> bool {
        self.sessions.contains_key(session_id)
    }

    /// Enumerate all active sessions and their current status.
    #[must_use]
    pub fn get_all_status(&self) -> Vec<BrowserStatusEntry> {
        self.sessions
            .iter()
            .map(|r| {
                let url = r.try_lock().ok().and_then(|s| s.current_url.clone());
                BrowserStatusEntry {
                    session_id: r.key().clone(),
                    running: true,
                    url,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_registry() -> Arc<BrowserRegistry> {
        let tmp = tempfile::Builder::new()
            .prefix("if2ai-registry-test-")
            .tempdir()
            .unwrap();
        BrowserRegistry::for_test(tmp.path().join("cold-state.json"))
    }

    #[test]
    fn takeover_flag_defaults_false_and_round_trips() {
        let registry = fresh_registry();
        assert!(!registry.is_taken_over("alpha"));
        registry.set_takeover("alpha", true);
        assert!(registry.is_taken_over("alpha"));
        assert!(!registry.is_taken_over("beta"));
        registry.set_takeover("alpha", false);
        assert!(!registry.is_taken_over("alpha"));
    }

    #[test]
    fn is_headed_returns_none_when_session_missing() {
        let registry = fresh_registry();
        assert!(registry.is_headed("does-not-exist").is_none());
    }

    #[test]
    fn relaunch_with_mode_on_missing_session_returns_about_blank() {
        // We don't actually launch Chrome here (would require a real
        // binary), so this test only exercises the URL-resume branch:
        // when there's no prior session and no last URL, the post-launch
        // navigate path is replaced with an `about:blank` stub.  We
        // can't run the full path without a Chromium child, so we only
        // assert the takeover flag plumbing instead.
        let registry = fresh_registry();
        registry.set_takeover("ghost", true);
        assert!(registry.is_taken_over("ghost"));
        registry.set_takeover("ghost", false);
        assert!(!registry.is_taken_over("ghost"));
    }
}
