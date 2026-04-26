//! Browser control system — Tauri event payloads and emission helpers.
//!
//! This module defines the `BrowserStatusEvent` payload and provides the
//! `emit_browser_status` async helper that the browser tool calls after each
//! action so the frontend's BrowserCard stays in sync.
//!
//! Design constraints (§3.1.4):
//! - `emit_browser_status` must be `async`; thumbnail capture may block briefly.
//! - A failed thumbnail must not prevent the event from being emitted.
//! - The event name is `"browser-status"` — matches the frontend listener.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tracing::warn;

use crate::modules::browser::registry::BrowserRegistry;
use crate::modules::smart_browser::contract::{SmartBrowserBackend, SmartBrowserEscalationState};

/// Payload emitted on the `"browser-status"` Tauri event after each browser
/// action, and returned by `get_browser_sessions`.
///
/// The frontend's BrowserCard subscribes to this event to show live status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BrowserStatusEvent {
    /// Chat-session identifier that owns this browser.
    pub session_id: String,
    /// Whether the browser process is currently running.
    pub running: bool,
    /// Current page URL, if available.
    pub url: Option<String>,
    /// Base-64 JPEG thumbnail of the current viewport, if available.
    ///
    /// `None` when the browser has not yet navigated to a page, or when
    /// screenshot capture fails. Failure to capture must not block the event.
    pub thumbnail: Option<String>,
    /// Smart Browser backend that produced this status.
    pub backend: SmartBrowserBackend,
    /// Page title, if the emitting backend can provide it.
    pub title: Option<String>,
    /// Whether a human has taken over this browser session.
    pub taken_over: bool,
    /// Last browser action, if the emitter knows it.
    pub last_action: Option<String>,
    /// Number of tracked downloads in the session.
    pub downloads_count: usize,
    /// Number of tracked console warnings/errors in the session.
    pub console_count: usize,
    /// Number of tracked network errors in the session.
    pub network_error_count: usize,
    /// Current escalation state for browser-use/cloud flows.
    pub escalation_state: SmartBrowserEscalationState,
}

/// Capture the current browser state for `session_id` and emit a
/// `"browser-status"` Tauri event to all windows.
///
/// # Async contract
///
/// This function is `async` because taking a thumbnail requires a CDP round-
/// trip. It must not block the tool execution path — callers should `tokio::spawn`
/// if they want fire-and-forget semantics.
///
/// # Failure handling
///
/// - Thumbnail failure → `thumbnail: None` in the payload; event still emitted.
/// - `emit` failure → logged as `warn!`, not propagated.
pub async fn emit_browser_status(
    app: &AppHandle,
    session_id: &str,
    registry: &Arc<BrowserRegistry>,
) {
    let running = registry.is_running(session_id);
    let url = registry.current_url(session_id);
    let taken_over = registry.is_taken_over(session_id);

    // Attempt thumbnail; silently degrade to None on any failure.
    let thumbnail = if running {
        match registry.thumbnail(session_id).await {
            Ok(t) => t,
            Err(e) => {
                warn!(session_id, "thumbnail capture failed, emitting None: {e}");
                None
            }
        }
    } else {
        None
    };
    let downloads_count = if running {
        registry
            .list_downloads(session_id)
            .await
            .map(|items| items.len())
            .unwrap_or(0)
    } else {
        0
    };
    let console_count = if running {
        registry
            .list_console_events(session_id)
            .await
            .map(|items| items.len())
            .unwrap_or(0)
    } else {
        0
    };
    let network_error_count = if running {
        registry
            .list_network_errors(session_id)
            .await
            .map(|items| items.len())
            .unwrap_or(0)
    } else {
        0
    };

    let event = BrowserStatusEvent {
        session_id: session_id.to_owned(),
        running,
        url,
        thumbnail,
        backend: SmartBrowserBackend::LocalRustCdp,
        title: None,
        taken_over,
        last_action: None,
        downloads_count,
        console_count,
        network_error_count,
        escalation_state: SmartBrowserEscalationState::None,
    };

    if let Err(e) = app.emit("browser-status", &event) {
        warn!(session_id, "failed to emit browser-status event: {e}");
    }
}
