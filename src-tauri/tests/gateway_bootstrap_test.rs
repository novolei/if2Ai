//! MIG-010 — gateway bootstrap contract integration test.
//!
//! Validates the application-layer gateway service (the
//! synchronous, dependency-free half of the seam) against the
//! invariants the IPC adapter and frontend bootstrap path rely on:
//!
//! 1. `current_gateway_url()` returns a populated, schema-versioned
//!    payload that callers can act on without having to call any
//!    business command first.
//! 2. `compute_gateway_health()` reports `Ready` exactly when every
//!    optional subsystem signal is ready, and `Degraded` (with a
//!    descriptive reason) the moment any signal is missing — not
//!    `Initializing`, which is reserved for "AppState construction
//!    is still in flight".
//! 3. The wire payload field names are `camelCase`, matching the
//!    frontend transport seam in `src/transport/gateway.ts`.

use if2ai_backend::modules::application::{
    compute_gateway_health, current_gateway_url, GatewayHealthInputs, GatewayStatus,
    GatewayTransport, GATEWAY_SCHEMA_VERSION,
};

/// MIG-010 acceptance §1: the bootstrap URL is callable without
/// any AppState input — the synthetic Tauri-IPC marker is stable
/// and carries the canonical schema version.
#[test]
fn gateway_url_payload_is_self_describing() {
    let payload = current_gateway_url();
    assert_eq!(payload.url, "tauri-ipc://local");
    assert_eq!(payload.schema_version, GATEWAY_SCHEMA_VERSION);
    assert_eq!(payload.transport, GatewayTransport::TauriIpc);
}

/// MIG-010 acceptance §2: a fully-ready snapshot reports `Ready`
/// with no reason and a parseable RFC3339 timestamp.
#[test]
fn gateway_health_all_ready_reports_ready_with_no_reason() {
    let snapshot = compute_gateway_health(GatewayHealthInputs {
        trajectory_ready: true,
        learning_ready: true,
        active_retrieval_ready: true,
    });
    assert_eq!(snapshot.status, GatewayStatus::Ready);
    assert!(snapshot.reason.is_none());
    assert_eq!(snapshot.schema_version, GATEWAY_SCHEMA_VERSION);
    assert!(
        chrono::DateTime::parse_from_rfc3339(&snapshot.checked_at).is_ok(),
        "checked_at must be RFC3339, got: {}",
        snapshot.checked_at
    );
}

/// MIG-010 acceptance §2: any missing optional subsystem yields
/// `Degraded` (not `Initializing`) and surfaces a descriptive
/// reason so the frontend splash can render it.
#[test]
fn gateway_health_partial_reports_degraded_with_named_reason() {
    let snapshot = compute_gateway_health(GatewayHealthInputs {
        trajectory_ready: false,
        learning_ready: false,
        active_retrieval_ready: true,
    });
    assert_eq!(snapshot.status, GatewayStatus::Degraded);
    let reason = snapshot.reason.expect("Degraded must carry a reason");
    assert!(
        reason.contains("trajectory_manager"),
        "reason must name the missing subsystem: {reason}"
    );
    assert!(
        reason.contains("learning_module"),
        "reason must name every missing subsystem: {reason}"
    );
    assert!(
        !reason.contains("active_retrieval_manager"),
        "reason must not mention healthy subsystems: {reason}"
    );
}

/// MIG-010 wire-shape contract: payloads serialize with
/// `camelCase` field names so the frontend `src/transport/gateway.ts`
/// types line up without per-field rename annotations.
#[test]
fn gateway_url_payload_serializes_camel_case_on_the_wire() {
    let payload = current_gateway_url();
    let json = serde_json::to_value(&payload).expect("serialize");
    assert!(
        json.get("schemaVersion").is_some(),
        "expected camelCase `schemaVersion` field, got: {json}"
    );
    assert!(
        json.get("schema_version").is_none(),
        "snake_case `schema_version` must NOT appear on the wire: {json}"
    );
    assert_eq!(
        json.get("transport").and_then(|v| v.as_str()),
        Some("tauriIpc")
    );
}

/// MIG-010 wire-shape contract: health payload also serializes
/// `camelCase` so the frontend `awaitGatewayReady` poll loop can
/// destructure `{ status, schemaVersion, checkedAt, reason }`
/// directly.
#[test]
fn gateway_health_payload_serializes_camel_case_on_the_wire() {
    let snapshot = compute_gateway_health(GatewayHealthInputs {
        trajectory_ready: true,
        learning_ready: true,
        active_retrieval_ready: true,
    });
    let json = serde_json::to_value(&snapshot).expect("serialize");
    assert!(json.get("schemaVersion").is_some());
    assert!(json.get("checkedAt").is_some());
    assert!(json.get("schema_version").is_none());
    assert!(json.get("checked_at").is_none());
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("ready"));
}
