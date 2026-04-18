//! Browser control IPC commands.
//!
//! Exposes four Tauri commands that the frontend can `invoke()`:
//!
//! | Command | Purpose |
//! |---|---|
//! | `get_browser_sessions` | List all active browser sessions and their state |
//! | `close_browser_session` | Gracefully shut down a specific browser session |
//! | `get_chrome_status` | Report whether a Chrome/Chromium binary is available |
//! | `request_browser_status` | Trigger an immediate `browser-status` event for the viewer |
//!
//! The `BrowserRegistry` is registered as Tauri managed state in `main.rs`
//! and injected here via `State<'_, Arc<BrowserRegistry>>`.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::modules::browser::chrome_finder::find_chrome_binary;
use crate::modules::browser::events::emit_browser_status;
use crate::modules::browser::registry::{BrowserRegistry, BrowserStatusEntry};

/// Response payload for `get_chrome_status`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChromeStatusPayload {
    /// Whether a Chrome or Chromium binary was detected on this machine.
    pub found: bool,
    /// Absolute path to the binary, or `None` when not found.
    pub path: Option<String>,
}

/// List all currently active browser sessions and their status.
///
/// Returns an empty list when no AI session has launched a browser yet.
#[tauri::command]
pub async fn get_browser_sessions(
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<Vec<BrowserStatusEntry>, String> {
    Ok(registry.get_all_status())
}

/// Close the browser session owned by `session_id`.
///
/// Returns an error string when no session with that id is found.
#[tauri::command]
pub async fn close_browser_session(
    session_id: String,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<(), String> {
    registry.close(&session_id).await.map_err(|e| e.to_string())
}

/// Check whether a Chrome or Chromium binary is available on this machine.
///
/// The frontend uses this to show a warning when no browser is installed.
#[tauri::command]
pub fn get_chrome_status() -> ChromeStatusPayload {
    let status = find_chrome_binary();
    ChromeStatusPayload {
        found: status.found,
        path: status.path.map(|p| p.to_string_lossy().into_owned()),
    }
}

/// Trigger an immediate `"browser-status"` event for `session_id`.
///
/// Called by the BrowserViewer window when it first opens so it can
/// initialise its display from the current session state — without waiting
/// for the next AI browser action to fire an event naturally.
/// If the session is not running the event is still emitted (`running: false`),
/// which causes the viewer to show the idle placeholder.
#[tauri::command]
pub async fn request_browser_status(
    session_id: String,
    app: AppHandle,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<(), String> {
    emit_browser_status(&app, &session_id, &registry).await;
    Ok(())
}
