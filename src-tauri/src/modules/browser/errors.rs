//! Browser module error types.

use thiserror::Error;

/// Errors that can occur during browser operations.
#[derive(Debug, Error)]
pub enum BrowserError {
    /// No Chrome or Chromium binary found on this system.
    #[error("Chrome/Chromium not found. Please install Google Chrome or Chromium and try again.")]
    ChromeNotFound,

    /// Browser session does not exist for the given session id.
    #[error("No browser session found for session '{0}'. Call 'start' first.")]
    SessionNotFound(String),

    /// Browser is not running for this session.
    #[error("Browser is not running for session '{0}'. Call 'start' first.")]
    NotRunning(String),

    /// Chrome DevTools Protocol error.
    #[error("CDP error: {0}")]
    Cdp(String),

    /// Operation timed out.
    #[error("Browser operation timed out after {0}ms")]
    Timeout(u64),

    /// DOM element with the given ref number was not found on the page.
    #[error("Element with ref [{0}] not found. Take a fresh snapshot to get current refs.")]
    RefNotFound(u32),

    /// Snapshot script execution failed.
    #[error("Snapshot failed: {0}")]
    Snapshot(String),

    /// Screenshot capture failed.
    #[error("Screenshot failed: {0}")]
    Screenshot(String),

    /// JavaScript evaluation returned an unexpected type.
    #[error("Evaluate error: {0}")]
    Evaluate(String),

    /// Cold-state persistence I/O error.
    #[error("Cold state I/O error: {0}")]
    ColdState(String),

    /// Browser profile (Phase 7C, slice 7C.1) I/O / policy error.
    /// Raised by `BrowserProfileMode::resolve` and `delete_profile`.
    #[error("Profile error: {0}")]
    ProfileError(String),

    /// Operation refused because a `BrowserSession` is still running.
    /// Currently emitted by `clear_browser_profile` when the caller forgot
    /// to `close` the session first.
    #[error("Session '{0}' must be closed before this operation")]
    SessionStillRunning(String),

    /// Phase 7C, slice 7C.6 — `switch_tab` / `close_tab` received an
    /// `idx` that does not match any open page.  The LLM should call
    /// `list_tabs` (or `snapshot`, which appends a tab summary) to
    /// discover valid indices.
    #[error("Tab index {0} is out of range; call action='tabs' for the current list")]
    TabIndexOutOfRange(usize),

    /// Phase 7C, slice 7C.12 (12a) — `evaluate()` rejected an oversized
    /// expression.  The LLM is expected to break the script into
    /// smaller chunks rather than retry.
    #[error("evaluate expression too long: {0} bytes (max 10000); split into smaller scripts")]
    EvaluateTooLong(usize),
}

impl From<chromiumoxide::error::CdpError> for BrowserError {
    fn from(e: chromiumoxide::error::CdpError) -> Self {
        Self::Cdp(e.to_string())
    }
}
