//! Activation contract v1 skeleton (Phase M0.4).
//!
//! Defines the canonical state-machine alphabet for the
//! `activation_license` entity (see canonical domain model §3.9).
//! No state transitions are enforced here — this is the wire-level
//! contract only.
//!
//! The boot truth narrative (`startup -> onboarding -> activation
//! gate -> main shell`) is documented in
//! [`docs/staff-remediation/if2ai-workflow-truth.md`](../../../../../docs/staff-remediation/if2ai-workflow-truth.md).
//!
//! The current `commands/activation.rs` implementation only covers
//! the `activated` segment of this state machine; M1
//! `activation_service` will land the full transitions.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::common::CorrelationIds;

/// Canonical activation status discriminator.
///
/// All ten states are part of the v1 contract even though the
/// current backend only produces `Activated` after onboarding. The
/// remaining states MUST be preserved in code that touches the
/// contract so M1 can wire transitions without reshaping the wire
/// format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationStatusKind {
    /// Boot is reading the local license cache; no decision yet.
    CheckingLocal,
    /// No local license found and no request in flight.
    NeedsActivation,
    /// User has submitted activation credentials; remote call in
    /// flight.
    RequestingActivation,
    /// Remote acknowledged the request and is awaiting a manual /
    /// async approval step (e.g. invite-only beta).
    PendingApproval,
    /// Approved license is being redeemed locally (decoded, written
    /// to keychain / config).
    Redeeming,
    /// License is valid and not expired; main shell is permitted.
    Activated,
    /// License refresh failed but offline grace window is still
    /// open; main shell stays usable for the grace period.
    OfflineGrace,
    /// License is past its expiry date; main shell must be gated.
    Expired,
    /// Remote has revoked the license; main shell must be gated and
    /// any cached license erased.
    Revoked,
    /// User has explicitly deactivated this install.
    Deactivated,
}

/// Rich activation status carrying optional per-state metadata.
///
/// Most fields are optional; an emitter SHOULD fill in everything it
/// knows so the projection layer can render explainable UI without a
/// follow-up call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationStatus {
    /// Canonical state.
    pub kind: ActivationStatusKind,
    /// Human-friendly transient message (e.g. `"等待邮件审批"`),
    /// optional. Stable copy lives in the frontend i18n catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// When `kind == Expired | OfflineGrace`, the moment the grace
    /// window closes. RFC3339 string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_until: Option<String>,
    /// When `kind == PendingApproval`, the upstream request id so
    /// support tooling can correlate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_request_id: Option<String>,
    /// When `kind == Revoked | Expired | Deactivated`, the reason
    /// recorded by the remote service or local action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<ActivationFailureReason>,
}

/// Coarse failure / terminal-state reason taxonomy.
///
/// Open enum on purpose: M1 can extend without a contract bump as
/// long as new variants are appended. Frontends MUST handle the
/// `Other` fallback.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationFailureReason {
    /// License data not present locally and remote unreachable.
    NoLocalLicenseOffline,
    /// Remote replied that the license expired.
    ServerExpired,
    /// Remote replied that the license was revoked.
    ServerRevoked,
    /// User chose to deactivate this device.
    UserDeactivated,
    /// Refresh attempt failed but grace window is still open.
    RefreshTransient,
    /// Catch-all; carry detail in [`ActivationStatus::message`].
    Other,
}

/// Compact license summary safe to expose to the frontend.
///
/// Sensitive fields (raw signed payload, signature) MUST stay
/// backend-side; only display-relevant fields belong here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationLicense {
    /// Stable license id (opaque to UI).
    pub license_id: String,
    /// Optional human-friendly plan name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Issued at, RFC3339.
    pub issued_at: String,
    /// Expires at, RFC3339. `None` for non-expiring licenses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// Last successful refresh, RFC3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refreshed_at: Option<String>,
}

/// Full activation snapshot — the canonical projection consumed by
/// the frontend boot shell and the (future) settings panel.
///
/// Producers SHOULD emit a snapshot on every transition so the
/// frontend never has to derive state from a stream of partial
/// events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationSnapshot {
    /// Current status.
    pub status: ActivationStatus,
    /// License summary, if a license is currently held (any
    /// `Activated | OfflineGrace | Expired | Revoked` state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<ActivationLicense>,
    /// Whether the main shell is allowed under the current snapshot.
    /// Computed by the backend; the frontend MUST NOT recompute this.
    pub allows_main_shell: bool,
    /// Correlation ids for traceability.
    #[serde(default)]
    pub correlation: CorrelationIds,
    /// Snapshot timestamp, RFC3339.
    pub captured_at: String,
}

/// Canonical action discriminator covering the six lifecycle
/// transitions enumerated by the M0 runbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationActionKind {
    /// User-initiated request to acquire a new license.
    Request,
    /// Local redemption of an approved license payload.
    Redeem,
    /// Background or user-triggered license refresh.
    Refresh,
    /// Background or user-triggered revoke check against the remote.
    RevokeCheck,
    /// Explicit user deactivation of this install.
    Deactivate,
    /// Boot-time restoration of the cached license (no network).
    LocalBootRestore,
}

/// Action descriptor: the input record for a state transition.
///
/// M0.4 only fixes the alphabet. M1 will add typed payloads per
/// action via family-specific submodules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationAction {
    /// Kind of action being requested.
    pub kind: ActivationActionKind,
    /// Free-form payload (M1 will narrow per-kind).
    #[serde(default)]
    pub payload: serde_json::Value,
    /// Correlation ids for traceability.
    #[serde(default)]
    pub correlation: CorrelationIds,
    /// Action timestamp, RFC3339.
    pub requested_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trips() {
        let snap = ActivationSnapshot {
            status: ActivationStatus {
                kind: ActivationStatusKind::Activated,
                message: None,
                grace_until: None,
                pending_request_id: None,
                failure_reason: None,
            },
            license: Some(ActivationLicense {
                license_id: "lic-1".into(),
                plan: Some("pro".into()),
                issued_at: "2026-01-01T00:00:00Z".into(),
                expires_at: Some("2027-01-01T00:00:00Z".into()),
                last_refreshed_at: None,
            }),
            allows_main_shell: true,
            correlation: CorrelationIds::default(),
            captured_at: "2026-04-20T00:00:00Z".into(),
        };
        let s = serde_json::to_string(&snap).unwrap();
        let back: ActivationSnapshot = serde_json::from_str(&s).unwrap();
        assert_eq!(snap, back);
        assert!(back.allows_main_shell);
        assert_eq!(back.status.kind, ActivationStatusKind::Activated);
    }

    #[test]
    fn all_status_variants_are_serializable() {
        for kind in [
            ActivationStatusKind::CheckingLocal,
            ActivationStatusKind::NeedsActivation,
            ActivationStatusKind::RequestingActivation,
            ActivationStatusKind::PendingApproval,
            ActivationStatusKind::Redeeming,
            ActivationStatusKind::Activated,
            ActivationStatusKind::OfflineGrace,
            ActivationStatusKind::Expired,
            ActivationStatusKind::Revoked,
            ActivationStatusKind::Deactivated,
        ] {
            let s = serde_json::to_string(&kind).unwrap();
            let back: ActivationStatusKind = serde_json::from_str(&s).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn all_action_kinds_are_serializable() {
        for kind in [
            ActivationActionKind::Request,
            ActivationActionKind::Redeem,
            ActivationActionKind::Refresh,
            ActivationActionKind::RevokeCheck,
            ActivationActionKind::Deactivate,
            ActivationActionKind::LocalBootRestore,
        ] {
            let s = serde_json::to_string(&kind).unwrap();
            let back: ActivationActionKind = serde_json::from_str(&s).unwrap();
            assert_eq!(kind, back);
        }
    }
}
