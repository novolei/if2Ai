//! MIG-010 — local gateway bootstrap contract.
//!
//! Establishes a stable, typed seam between the desktop host and
//! the frontend bootstrap path so the latter can observe "is the
//! backend gateway reachable / ready?" without having to probe
//! individual business commands.
//!
//! Design intent (per MIG-010 pack §3 and the cc-haha benchmark):
//!
//! - **Today**: If2Ai is a Tauri-only app — there is no real local
//!   HTTP server. The gateway URL is a synthetic but stable
//!   `tauri-ipc://local` marker; readiness is derived from
//!   AppState construction having succeeded.
//! - **Future** (post-MIG-011 / -015): swap the transport tag to
//!   `LocalHttp` and have the URL point at a real sidecar. The
//!   frontend bootstrap code keeps the same call shape.
//!
//! The frontend `src/transport/gateway.ts` consumes these payloads
//! verbatim. Field names are `camelCase` on the wire (matches the
//! project-wide `serde(rename_all = "camelCase")` convention for
//! all IPC-facing DTOs).

use serde::{Deserialize, Serialize};

/// Canonical schema version for the local gateway bootstrap
/// contract. Bump on any breaking change to either payload type
/// in this module so the frontend can refuse to bootstrap against
/// an incompatible backend.
pub const GATEWAY_SCHEMA_VERSION: &str = "1.0.0";

/// Wire payload returned by the `get_gateway_url` IPC command.
///
/// Stable identity surface the frontend bootstrap path can call
/// once at startup to discover (a) the canonical entry URL and
/// (b) which transport (Tauri IPC today, local HTTP later) it
/// should multiplex business calls on top of.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayUrlPayload {
    /// Canonical entry URL. Synthetic today (`tauri-ipc://local`);
    /// will become a real `http://127.0.0.1:<port>` once the
    /// future sidecar bootstrap lands.
    pub url: String,
    /// Schema version of this payload + [`GatewayHealthPayload`];
    /// matches [`GATEWAY_SCHEMA_VERSION`].
    pub schema_version: String,
    /// Active transport tag the frontend should use to dispatch
    /// business calls.
    pub transport: GatewayTransport,
}

/// Wire payload returned by the `get_gateway_health` IPC command.
///
/// Frontend bootstrap polls this until `status == Ready` before
/// any business command is allowed to fire. `Initializing` and
/// `Degraded` are advisory; the frontend should surface them in
/// the splash / status UI rather than crash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayHealthPayload {
    /// High-level readiness state.
    pub status: GatewayStatus,
    /// Schema version of this payload (matches
    /// [`GATEWAY_SCHEMA_VERSION`]).
    pub schema_version: String,
    /// Human-readable reason when [`GatewayStatus::Degraded`] (or
    /// `None` when the gateway is healthy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// RFC3339 UTC timestamp when this health snapshot was taken.
    pub checked_at: String,
}

/// Transport tag describing how the frontend should multiplex
/// business calls on top of the gateway URL. Currently always
/// `TauriIpc`; future MIG packs introduce `LocalHttp`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GatewayTransport {
    /// Direct Tauri IPC (`invoke` / `listen`). Today's default.
    TauriIpc,
    /// Local HTTP sidecar. Reserved for the post-MIG-011 boot
    /// bootstrap; not emitted by the current backend.
    LocalHttp,
}

/// High-level readiness state surfaced by
/// [`GatewayHealthPayload::status`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GatewayStatus {
    /// Backend is still constructing critical subsystems. The
    /// frontend should keep polling `get_gateway_health` rather
    /// than firing business commands.
    Initializing,
    /// All critical subsystems are constructed and the gateway
    /// is willing to accept business commands.
    Ready,
    /// At least one non-critical subsystem failed to initialise.
    /// Business commands MAY still work; the frontend should
    /// surface `reason` to the user.
    Degraded,
}

/// Build the canonical [`GatewayUrlPayload`] for this build.
///
/// Pure / dependency-free — the URL is synthetic in MIG-010 and
/// changes only when [`GATEWAY_SCHEMA_VERSION`] or
/// [`GatewayTransport`] changes.
#[must_use]
pub fn current_gateway_url() -> GatewayUrlPayload {
    GatewayUrlPayload {
        url: "tauri-ipc://local".to_string(),
        schema_version: GATEWAY_SCHEMA_VERSION.to_string(),
        transport: GatewayTransport::TauriIpc,
    }
}

/// Inputs for [`compute_gateway_health`]. Bundled into a struct
/// so the IPC adapter (`commands::gateway`) can pass only the
/// `AppState` fields the readiness check actually inspects, per
/// CHARTER §2.1 (application/* must not import
/// `crate::commands::*`).
pub struct GatewayHealthInputs {
    /// `true` when the trajectory manager initialised. `None` is
    /// non-fatal but surfaced as a degraded reason so operators
    /// see it in the splash.
    pub trajectory_ready: bool,
    /// `true` when the learning module initialised. `None` is
    /// non-fatal but surfaced as a degraded reason.
    pub learning_ready: bool,
    /// `true` when the active retrieval manager initialised.
    /// `None` is non-fatal but surfaced as a degraded reason.
    pub active_retrieval_ready: bool,
}

/// Compute a [`GatewayHealthPayload`] snapshot from the provided
/// readiness signals.
///
/// Treats trajectory / learning / active retrieval as
/// non-critical: missing any of them yields `Degraded` rather
/// than `Initializing`. The IPC layer ([`current_gateway_url`])
/// always returns a populated URL so a healthy snapshot is the
/// common case in production.
#[must_use]
pub fn compute_gateway_health(inputs: GatewayHealthInputs) -> GatewayHealthPayload {
    let mut degraded_reasons: Vec<&'static str> = Vec::new();
    if !inputs.trajectory_ready {
        degraded_reasons.push("trajectory_manager not initialised");
    }
    if !inputs.learning_ready {
        degraded_reasons.push("learning_module not initialised");
    }
    if !inputs.active_retrieval_ready {
        degraded_reasons.push("active_retrieval_manager not initialised");
    }

    let checked_at = chrono::Utc::now().to_rfc3339();
    let schema_version = GATEWAY_SCHEMA_VERSION.to_string();

    if degraded_reasons.is_empty() {
        GatewayHealthPayload {
            status: GatewayStatus::Ready,
            schema_version,
            reason: None,
            checked_at,
        }
    } else {
        GatewayHealthPayload {
            status: GatewayStatus::Degraded,
            schema_version,
            reason: Some(degraded_reasons.join("; ")),
            checked_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_gateway_url_returns_canonical_marker() {
        let payload = current_gateway_url();
        assert_eq!(payload.url, "tauri-ipc://local");
        assert_eq!(payload.schema_version, GATEWAY_SCHEMA_VERSION);
        assert_eq!(payload.transport, GatewayTransport::TauriIpc);
    }

    #[test]
    fn compute_gateway_health_all_ready_returns_ready() {
        let payload = compute_gateway_health(GatewayHealthInputs {
            trajectory_ready: true,
            learning_ready: true,
            active_retrieval_ready: true,
        });
        assert_eq!(payload.status, GatewayStatus::Ready);
        assert!(payload.reason.is_none());
        assert_eq!(payload.schema_version, GATEWAY_SCHEMA_VERSION);
        // `checked_at` parses as a valid RFC3339 timestamp.
        assert!(chrono::DateTime::parse_from_rfc3339(&payload.checked_at).is_ok());
    }

    #[test]
    fn compute_gateway_health_partial_returns_degraded_with_reason() {
        let payload = compute_gateway_health(GatewayHealthInputs {
            trajectory_ready: false,
            learning_ready: true,
            active_retrieval_ready: false,
        });
        assert_eq!(payload.status, GatewayStatus::Degraded);
        let reason = payload.reason.expect("degraded must carry a reason");
        assert!(reason.contains("trajectory_manager"));
        assert!(reason.contains("active_retrieval_manager"));
        assert!(!reason.contains("learning_module"));
    }

    #[test]
    fn gateway_url_serializes_camel_case() {
        let payload = current_gateway_url();
        let json = serde_json::to_value(&payload).expect("serialize");
        assert!(json.get("schemaVersion").is_some());
        assert!(json.get("schema_version").is_none());
        assert_eq!(
            json.get("transport").and_then(|v| v.as_str()),
            Some("tauriIpc")
        );
    }
}
