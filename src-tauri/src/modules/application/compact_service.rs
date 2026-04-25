//! Auto-compact helpers that are callable from inside the `modules` tree.
//!
//! Placing these pure / Arc-based functions here avoids a circular
//! dependency between `modules::application::turn_service::stream_finalize`
//! (which is *inside* the modules crate-half) and
//! `commands::chat_compact` (which is *outside*, in the binary-only
//! commands layer that imports `AppState`).
//!
//! The IPC surface (`chat_compact_session` Tauri command) and anything
//! that needs `AppState` lives in `crate::commands::chat_compact`.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use crate::modules::memory::scope::MemoryScopeResolver;
use crate::modules::memory::summary::RollingSummarizer;
use crate::modules::runtime::budget::estimate_tokens;
use crate::modules::runtime::session::ContentBlock;
use crate::modules::session::SessionManager;

/// Tauri event name emitted when a compact background task lands.
pub const COMPACT_COMPLETED_EVENT: &str = "chat_compact_completed";

/// Result returned from a manual compact, and emitted as the payload for
/// `chat_compact_completed` after an auto-compact tick.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactReport {
    pub session_id: String,
    pub summarized_messages: usize,
    pub freed_tokens: i64,
    pub summary_excerpt: String,
    pub did_compact: bool,
}

impl CompactReport {
    pub fn noop(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            summarized_messages: 0,
            freed_tokens: 0,
            summary_excerpt: String::new(),
            did_compact: false,
        }
    }
}

/// Read the auto-compact threshold from env, clamped to `[0.5, 0.99]`.
/// Defaults to `0.85`.
#[must_use]
pub fn auto_compact_threshold() -> f32 {
    std::env::var("IF2AI_AUTO_COMPACT_THRESHOLD")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.85_f32)
        .clamp(0.5_f32, 0.99_f32)
}

/// Whether auto-compact is enabled. Defaults to `true`.
#[must_use]
pub fn auto_compact_enabled() -> bool {
    !matches!(
        std::env::var("IF2AI_AUTO_COMPACT")
            .unwrap_or_default()
            .as_str(),
        "0" | "false" | "off" | "no"
    )
}

/// Core compact logic: run `RollingSummarizer` against the session's full
/// message history and return a report.
pub async fn run_compact(
    session_manager: Arc<SessionManager>,
    rolling_summarizer: Arc<RollingSummarizer>,
    session_id: String,
) -> Result<CompactReport, String> {
    let session = session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| format!("compact: failed to load session '{session_id}': {e}"))?;

    if session.messages.is_empty() {
        return Ok(CompactReport::noop(&session_id));
    }

    let scope = MemoryScopeResolver::resolve(
        Some(&session_id),
        if session.project_id.is_empty() {
            None
        } else {
            Some(session.project_id.as_str())
        },
        None,
    );

    let slice_token_estimate: usize = session
        .messages
        .iter()
        .map(|m| {
            m.blocks
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => estimate_tokens(text),
                    _ => 0,
                })
                .sum::<usize>()
        })
        .sum();

    let record = rolling_summarizer
        .rolling_summary(&session_id, &scope, &session.messages)
        .await
        .map_err(|e| format!("compact: rolling_summary failed: {e}"))?;

    let Some(record) = record else {
        return Ok(CompactReport::noop(&session_id));
    };

    let summary_token_estimate = estimate_tokens(&record.summary);
    let freed_tokens = (slice_token_estimate as i64).saturating_sub(summary_token_estimate as i64);
    let summary_excerpt: String = record.summary.chars().take(140).collect();

    Ok(CompactReport {
        session_id,
        summarized_messages: session.messages.len(),
        freed_tokens,
        summary_excerpt,
        did_compact: true,
    })
}

/// Spawn a background auto-compact task that emits `chat_compact_completed`.
pub fn spawn_auto_compact<R: Runtime>(
    app: AppHandle<R>,
    session_manager: Arc<SessionManager>,
    rolling_summarizer: Arc<RollingSummarizer>,
    session_id: String,
) {
    tauri::async_runtime::spawn(async move {
        match run_compact(session_manager, rolling_summarizer, session_id.clone()).await {
            Ok(report) if report.did_compact => {
                if let Err(err) = app.emit(COMPACT_COMPLETED_EVENT, &report) {
                    tracing::warn!(
                        target: "if2ai::compact",
                        "auto-compact: failed to emit {COMPACT_COMPLETED_EVENT}: {err}"
                    );
                } else {
                    tracing::info!(
                        target: "if2ai::compact",
                        session_id, "auto-compact: emitted summary report"
                    );
                }
            }
            Ok(_) => {
                tracing::debug!(
                    target: "if2ai::compact",
                    session_id,
                    "auto-compact: nothing to fold"
                );
            }
            Err(err) => {
                tracing::warn!(
                    target: "if2ai::compact",
                    session_id,
                    "auto-compact failed: {err}"
                );
            }
        }
    });
}
