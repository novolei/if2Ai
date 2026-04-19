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
use crate::modules::browser::profile::{
    delete_profile as delete_profile_inner, list_profiles as list_profiles_inner, BrowserSettings,
    ProfileEntry,
};
use crate::modules::browser::registry::{BrowserRegistry, BrowserStatusEntry};
use crate::modules::browser::session::{ActionLogEntry, NavigateResult};

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

// ── Profile management (Phase 7C, slice 7C.1) ────────────────────────────────

/// List every persistent browser profile on disk under
/// `<if2ai_home>/browser-profiles/`.
///
/// Returns the per-session id, absolute path, recursive disk usage and last
/// modified timestamp for each profile.  An empty `Vec` is returned when no
/// profiles exist yet (e.g. fresh install).
#[tauri::command]
pub async fn list_browser_profiles(
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<Vec<ProfileEntry>, String> {
    Ok(list_profiles_inner(registry.if2ai_home()))
}

/// Wipe the persistent profile directory for `session_id` (cookies,
/// localStorage, cache — everything Chromium stored under that user data
/// dir).
///
/// **Refuses** to delete the profile while a `BrowserSession` is still
/// running for that id; the caller must `close_browser_session` first so
/// Chromium releases the file locks.  This is what surfaces as the
/// `'session must be closed first'` error message in the UI.
#[tauri::command]
pub async fn clear_browser_profile(
    session_id: String,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<(), String> {
    if registry.is_running(&session_id) {
        return Err(format!(
            "session '{session_id}' must be closed before clearing its profile"
        ));
    }
    delete_profile_inner(registry.if2ai_home(), &session_id).map_err(|e| e.to_string())
}

// ── Browser settings (Phase 7C, Settings UI entry point) ─────────────────────

/// Read the persisted browser settings (`<if2ai_home>/browser.toml`).
///
/// Surfaces the *next-launch* `profile_mode`, the live `active_mode` of
/// the running registry, and any active `IF2AI_BROWSER_PROFILE_MODE`
/// environment override.  The Settings UI uses this to render the
/// dropdown plus a banner when env / running mode disagree with the
/// persisted preference.
#[tauri::command]
pub async fn get_browser_settings(
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<BrowserSettings, String> {
    Ok(BrowserSettings::load(
        registry.if2ai_home(),
        registry.profile_mode(),
    ))
}

/// Persist user-edited browser settings.  Takes effect on next app
/// restart for `profile_mode`; the disk caps are reserved for future LRU
/// cleanup but stored now so the UI is the single source of truth.
#[tauri::command]
pub async fn set_browser_settings(
    settings: BrowserSettings,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<(), String> {
    settings
        .save(registry.if2ai_home())
        .map_err(|e| e.to_string())
}

// ── Headed mode + takeover (Phase 7C, slice 7C.3) ────────────────────────────

/// Hand control of the browser to the human user.
///
/// The headless Chromium for `session_id` is closed, then a fresh
/// **headed** Chromium is launched against the same persistent profile
/// (so cookies / login state survive — see Phase 7C, slice 7C.1
/// `BrowserProfileMode::PerSessionPersistent`).  While the takeover flag
/// is set, all AI `browser` tool calls return a "paused" error so they
/// don't fight the user's input.
///
/// Returns the [`NavigateResult`] of the post-relaunch navigation (back
/// to whatever URL was active) so the UI can update its address bar.
#[tauri::command]
pub async fn request_browser_takeover(
    session_id: String,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<NavigateResult, String> {
    registry.set_takeover(&session_id, true);
    match registry.relaunch_with_mode(&session_id, true).await {
        Ok(result) => Ok(result),
        Err(err) => {
            // Roll back the flag so the AI is not stuck "paused" if the
            // headed launch fails (e.g. no display on a CI Linux box).
            registry.set_takeover(&session_id, false);
            Err(err.to_string())
        }
    }
}

/// Phase 7C, slice 7C.12 (12d) — return the live (non-clearing) action
/// log for `session_id`, surfaced by the BrowserCard "Action Log"
/// drawer.  Returns an empty vec when no session exists for that id.
#[tauri::command]
pub async fn get_browser_action_log(
    session_id: String,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<Vec<ActionLogEntry>, String> {
    registry
        .read_action_log(&session_id)
        .await
        .or_else(|_| Ok(Vec::new()))
}

/// Release the takeover flag and (optionally) collapse the browser back
/// to headless so the user reclaims their screen real estate.
///
/// Setting `back_to_headless = true` re-launches the session against the
/// same profile so the AI inherits whatever state the user produced
/// (cookies, scroll position is lost — only persistent state survives).
#[tauri::command]
pub async fn release_browser_takeover(
    session_id: String,
    back_to_headless: bool,
    registry: State<'_, Arc<BrowserRegistry>>,
) -> Result<(), String> {
    registry.set_takeover(&session_id, false);
    if back_to_headless {
        if let Err(err) = registry.relaunch_with_mode(&session_id, false).await {
            tracing::warn!(
                session_id = %session_id,
                error = %err,
                "browser takeover released but headless relaunch failed; AI may need to call 'navigate' again"
            );
            return Err(err.to_string());
        }
    }
    Ok(())
}
