//! Browser registry — manages per-session [`BrowserSession`] instances.
//!
//! One `BrowserRegistry` is held as Tauri managed state for the lifetime of
//! the application. Sessions are keyed by the chat session identifier and
//! stored in a [`DashMap`] so that concurrent tool invocations from
//! different sessions can proceed without blocking each other.

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, info};

use crate::modules::browser::errors::BrowserError;
use crate::modules::browser::session::{ActionLogEntry, BrowserSession, NavigateResult, ScrollDir};

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
    /// session_id → BrowserSession
    sessions: DashMap<String, BrowserSession>,
    /// Path used by the cold-state persistence layer (injected; used in 7B.7).
    pub cold_state_path: PathBuf,
}

impl BrowserRegistry {
    /// Create a new empty registry.
    ///
    /// `cold_state_path` is the JSON file used to persist session URLs across
    /// app restarts; it is consumed by the cold-state module added in 7B.7.
    #[must_use]
    pub fn new(cold_state_path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            sessions: DashMap::new(),
            cold_state_path,
        })
    }

    // ── Session lifecycle ─────────────────────────────────────────────────────

    /// Launch a new headless browser for `session_id`.
    ///
    /// If a session already exists this is a no-op and returns `Ok(())`.
    pub async fn launch(&self, session_id: &str) -> Result<(), BrowserError> {
        if self.sessions.contains_key(session_id) {
            debug!(session_id, "browser already running — skipping launch");
            return Ok(());
        }
        let session = BrowserSession::new(session_id.to_owned()).await?;
        self.sessions.insert(session_id.to_owned(), session);
        info!(session_id, "browser launched");
        Ok(())
    }

    /// Close and remove the browser for `session_id`.
    pub async fn close(&self, session_id: &str) -> Result<(), BrowserError> {
        let (_, session) = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        session.close().await?;
        info!(session_id, "browser closed");
        Ok(())
    }

    // ── Delegated operations ──────────────────────────────────────────────────

    /// Navigate the browser for `session_id` to `url`.
    pub async fn navigate(
        &self,
        session_id: &str,
        url: &str,
    ) -> Result<NavigateResult, BrowserError> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.navigate(url).await
    }

    /// Return the AXTree snapshot for `session_id`.
    pub async fn snapshot(&self, session_id: &str) -> Result<String, BrowserError> {
        let entry = self
            .sessions
            .get(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.snapshot().await
    }

    /// Return a full-page JPEG screenshot (base64) for `session_id`.
    pub async fn screenshot(&self, session_id: &str) -> Result<String, BrowserError> {
        let entry = self
            .sessions
            .get(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.screenshot().await
    }

    /// Return a thumbnail JPEG (base64) for the BrowserCard, or `None` on failure.
    pub async fn thumbnail(&self, session_id: &str) -> Result<Option<String>, BrowserError> {
        let entry = self
            .sessions
            .get(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.thumbnail().await
    }

    /// Click the element with the given `ref_num` and return a fresh snapshot.
    pub async fn click(&self, session_id: &str, ref_num: u32) -> Result<String, BrowserError> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.click(ref_num).await
    }

    /// Type `text` into `ref_num` (optional) and optionally press Enter.
    pub async fn type_text(
        &self,
        session_id: &str,
        text: &str,
        ref_num: Option<u32>,
        press_enter: bool,
    ) -> Result<String, BrowserError> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.type_text(text, ref_num, press_enter).await
    }

    /// Scroll in `direction` by `amount` pages and return a fresh snapshot.
    pub async fn scroll(
        &self,
        session_id: &str,
        direction: ScrollDir,
        amount: u32,
    ) -> Result<String, BrowserError> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.scroll(direction, amount).await
    }

    /// Select `value` in the `<select>` at `ref_num`.
    pub async fn select_option(
        &self,
        session_id: &str,
        ref_num: u32,
        value: &str,
    ) -> Result<String, BrowserError> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.select_option(ref_num, value).await
    }

    /// Press a named key.
    pub async fn press_key(&self, session_id: &str, key: &str) -> Result<String, BrowserError> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.press_key(key).await
    }

    /// Wait for page navigation up to `timeout_ms`.
    pub async fn wait(
        &self,
        session_id: &str,
        timeout_ms: u64,
        state: &str,
    ) -> Result<String, BrowserError> {
        let mut entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.wait(timeout_ms, state).await
    }

    /// Evaluate arbitrary JavaScript and return the serialised result.
    pub async fn evaluate(
        &self,
        session_id: &str,
        expression: &str,
    ) -> Result<String, BrowserError> {
        let entry = self
            .sessions
            .get(session_id)
            .ok_or_else(|| BrowserError::SessionNotFound(session_id.to_owned()))?;
        entry.evaluate(expression).await
    }

    /// Return the action log for `session_id` and clear it.
    pub fn take_action_log(&self, session_id: &str) -> Vec<ActionLogEntry> {
        let mut entry = match self.sessions.get_mut(session_id) {
            Some(e) => e,
            None => return Vec::new(),
        };
        let log = entry.action_log.clone();
        entry.action_log.clear();
        log
    }

    // ── Status ────────────────────────────────────────────────────────────────

    /// Return the current URL for `session_id`, if known.
    #[must_use]
    pub fn current_url(&self, session_id: &str) -> Option<String> {
        self.sessions
            .get(session_id)
            .and_then(|e| e.current_url.clone())
    }

    /// Return whether a browser is currently active for `session_id`.
    #[must_use]
    pub fn is_running(&self, session_id: &str) -> bool {
        self.sessions.contains_key(session_id)
    }

    /// Enumerate all active sessions and their current status.
    #[must_use]
    pub fn all_status(&self) -> Vec<BrowserStatusEntry> {
        self.sessions
            .iter()
            .map(|e| BrowserStatusEntry {
                session_id: e.key().clone(),
                running: true,
                url: e.current_url.clone(),
            })
            .collect()
    }
}
