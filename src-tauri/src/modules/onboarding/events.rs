//! Onboarding Tauri event constants and emit helpers.
//!
//! Defines event names and payload types for real-time push from
//! backend to frontend during the onboarding flow.
//!
//! ## Events
//! | Event | Purpose |
//! |-------|---------|
//! | `onboarding://step_changed` | Step transition notification |
//! | `onboarding://download_progress` | Embedded model download progress |
//! | `onboarding://test_completed` | Provider/channel/model test result |
//! | `onboarding://error` | Error notification with step context |
//!
//! Per ADR-014 Section 18.6.

use serde::{Deserialize, Serialize};
use tauri::Emitter;

// ── Event Name Constants ─────────────────────────────────────────────────────

/// Emitted when the onboarding step changes (next/prev/complete).
pub const EVENT_STEP_CHANGED: &str = "onboarding://step_changed";

/// Emitted during embedded model download (throttled ~500ms).
pub const EVENT_DOWNLOAD_PROGRESS: &str = "onboarding://download_progress";

/// Emitted when a provider/channel/model test completes.
pub const EVENT_TEST_COMPLETED: &str = "onboarding://test_completed";

/// Emitted when an error occurs during onboarding.
pub const EVENT_ERROR: &str = "onboarding://error";

// ── Event Payload Types ──────────────────────────────────────────────────────

/// Payload for `EVENT_STEP_CHANGED`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepChangedEvent {
    /// Previous step number (0 = FirstLaunch).
    pub from_step: u8,
    /// New step number.
    pub to_step: u8,
}

/// Payload for `EVENT_DOWNLOAD_PROGRESS`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    /// Model name being downloaded.
    pub model_name: String,
    /// Bytes downloaded so far.
    pub downloaded_bytes: u64,
    /// Total bytes expected.
    pub total_bytes: u64,
    /// Progress percentage (0-100).
    pub percent: f64,
}

/// Test type discriminator for `EVENT_TEST_COMPLETED`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestType {
    Provider,
    Channel,
    Model,
}

/// Payload for `EVENT_TEST_COMPLETED`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCompletedEvent {
    /// What kind of test completed.
    pub test_type: TestType,
    /// Target identifier (provider_id, channel_id, or model_id).
    pub target_id: String,
    /// Whether the test succeeded.
    pub success: bool,
    /// Latency in milliseconds (if available).
    pub latency_ms: Option<u64>,
    /// Error message (if failed).
    pub error: Option<String>,
}

/// Payload for `EVENT_ERROR`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingErrorEvent {
    /// Step number where the error occurred.
    pub step: u8,
    /// Machine-readable error code (e.g. "provider_timeout").
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Whether the user can retry.
    pub retriable: bool,
}

// ── Emit Helpers ─────────────────────────────────────────────────────────────

/// Emit a step-changed event to all listeners.
pub fn emit_step_changed(app: &tauri::AppHandle, from_step: u8, to_step: u8) {
    let _ = app.emit(EVENT_STEP_CHANGED, StepChangedEvent { from_step, to_step });
}

/// Emit a download progress event to all listeners.
pub fn emit_download_progress(
    app: &tauri::AppHandle,
    model_name: &str,
    downloaded_bytes: u64,
    total_bytes: u64,
) {
    let percent = if total_bytes > 0 {
        (downloaded_bytes as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let _ = app.emit(
        EVENT_DOWNLOAD_PROGRESS,
        DownloadProgressEvent {
            model_name: model_name.to_string(),
            downloaded_bytes,
            total_bytes,
            percent,
        },
    );
}

/// Emit a test-completed event to all listeners.
pub fn emit_test_completed(app: &tauri::AppHandle, event: TestCompletedEvent) {
    let _ = app.emit(EVENT_TEST_COMPLETED, event);
}

/// Emit an error event to all listeners.
pub fn emit_error(app: &tauri::AppHandle, event: OnboardingErrorEvent) {
    let _ = app.emit(EVENT_ERROR, event);
}
