//! IPC surface for the manual `/compact` command.
//!
//! Core compact logic (`run_compact`, `spawn_auto_compact`,
//! `auto_compact_threshold`, `auto_compact_enabled`, `CompactReport`) lives
//! in [`crate::modules::application::compact_service`] so it can be called
//! from inside the modules tree (specifically `stream_finalize`) without a
//! circular dependency on `commands::AppState`.
//!
//! This file is a thin adapter:
//! - Re-exports the public compact types from the service module.
//! - Provides `try_resolve_services` which needs `AppState`.
//! - Exposes the `chat_compact_session` Tauri IPC command.

use std::sync::Arc;

use tauri::{Manager, State};

use super::AppState;
pub use crate::modules::application::compact_service::CompactReport;
use crate::modules::application::compact_service::{self};
use crate::modules::memory::summary::RollingSummarizer;

pub use compact_service::COMPACT_COMPLETED_EVENT;

/// Look up shared services from a Tauri `AppHandle` via `AppState`.
/// Used by callers that only hold an `AppHandle` (e.g. the auto-compact
/// path registered in `spawn_auto_compact`).
pub fn try_resolve_services(
    app: &tauri::AppHandle,
) -> Option<(
    Arc<crate::modules::session::SessionManager>,
    Arc<RollingSummarizer>,
)> {
    let state = app.try_state::<AppState>()?;
    Some((
        state.session_manager.clone(),
        state.rolling_summarizer.clone(),
    ))
}

/// Manual compact IPC. Bypasses the `COMPACT_SKIP_WINDOW_SECS` throttle —
/// the user explicitly asked for it via `/compact` or the ContextBar button.
#[tauri::command]
pub async fn chat_compact_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CompactReport, String> {
    let session_manager = state.session_manager.clone();
    let rolling_summarizer = state.rolling_summarizer.clone();

    compact_service::run_compact(session_manager, rolling_summarizer, session_id).await
}
