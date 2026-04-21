//! MIG-010 — Tauri IPC commands for the local gateway bootstrap
//! seam.
//!
//! Thin adapters over
//! `crate::modules::application::gateway_service::{current_gateway_url, compute_gateway_health}`.
//! The gateway service owns all wire-payload definitions; this
//! module only forwards the readiness signals it can read off
//! `AppState`.
//!
//! Frontend bootstrap flow:
//!
//! 1. Call `get_gateway_url()` once at startup → caches the
//!    canonical entry URL + transport tag.
//! 2. Poll `get_gateway_health()` until `status == 'ready'` (or
//!    until a UI-defined timeout) before any business command.
//!
//! Both commands are zero-cost in the steady state and are
//! intentionally separate from every business command registered
//! in `main.rs::generate_handler!`.

use tauri::State;

use crate::commands::AppState;
use crate::modules::application::{
    compute_gateway_health, current_gateway_url, GatewayHealthInputs, GatewayHealthPayload,
    GatewayUrlPayload,
};

/// Return the canonical gateway URL + transport tag for this
/// build. Pure / synchronous; safe to call before any other
/// command.
#[tauri::command]
pub fn get_gateway_url() -> Result<GatewayUrlPayload, String> {
    Ok(current_gateway_url())
}

/// Return a fresh readiness snapshot. Inspects optional
/// `AppState` subsystems (trajectory, learning, active retrieval)
/// and surfaces any `None` as a degraded reason. Intended to be
/// polled by the frontend bootstrap path.
#[tauri::command]
pub fn get_gateway_health(state: State<'_, AppState>) -> Result<GatewayHealthPayload, String> {
    let inputs = GatewayHealthInputs {
        trajectory_ready: state.trajectory_manager.is_some(),
        learning_ready: state.learning_module.is_some(),
        active_retrieval_ready: state.active_retrieval_manager.is_some(),
    };
    Ok(compute_gateway_health(inputs))
}
