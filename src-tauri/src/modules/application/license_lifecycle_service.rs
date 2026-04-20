//! License lifecycle service skeleton (Phase M1.7).
//!
//! Owns the canonical lifecycle transitions for an
//! [`crate::modules::runtime::contracts::activation::ActivationLicense`]:
//! `request → redeem → refresh → revoke_check → deactivate` plus
//! `local_boot_restore`.
//!
//! M1.7 scope is **typed boundary only**:
//!
//! - All methods compile and return a typed
//!   [`crate::modules::runtime::contracts::activation::ActivationSnapshot`].
//! - No remote API client is wired (would lock us into a vendor
//!   prematurely; deferred to a dedicated slice).
//! - Local persistence is a stub returning `NeedsActivation` snapshots
//!   so the boot flow keeps working unchanged.
//! - The service intentionally does NOT touch
//!   [`crate::modules::onboarding`] state — that stays the
//!   responsibility of the ceremony commands until M2 boot shell
//!   reorchestration.
//!
//! Hard rules:
//! 1. This service MUST NOT import from `crate::commands::*`.
//! 2. The contract enums (`ActivationStatusKind`,
//!    `ActivationActionKind`, `ActivationFailureReason`) are owned
//!    by `runtime::contracts::activation`. Adding states here is a
//!    contract break and belongs in M0.
//! 3. The service SHOULD always return a snapshot whose
//!    `allows_main_shell` field is computed by the backend, never
//!    by the caller.

#![allow(dead_code)]

use crate::modules::runtime::contracts::activation::{
    ActivationFailureReason, ActivationLicense, ActivationSnapshot, ActivationStatus,
    ActivationStatusKind,
};
use crate::modules::runtime::contracts::common::CorrelationIds;

/// Construct the service. Currently a marker type so future slices
/// can add state without breaking call sites.
pub struct LicenseLifecycleService {
    _placeholder: (),
}

impl Default for LicenseLifecycleService {
    fn default() -> Self {
        Self::new()
    }
}

impl LicenseLifecycleService {
    #[must_use]
    pub fn new() -> Self {
        Self { _placeholder: () }
    }

    /// Boot-time restore of any cached license. Currently always
    /// resolves to `NeedsActivation` — the local cache layer is a
    /// follow-up slice.
    pub async fn local_boot_restore(&self) -> ActivationSnapshot {
        snapshot_with_kind(ActivationStatusKind::NeedsActivation, None, None)
    }

    /// Request a new license from the activation backend.
    /// Skeleton: returns `RequestingActivation` snapshot.
    pub async fn request_license(&self) -> ActivationSnapshot {
        snapshot_with_kind(ActivationStatusKind::RequestingActivation, None, None)
    }

    /// Redeem an approved license payload.
    /// Skeleton: returns `Redeeming` snapshot.
    pub async fn redeem(&self) -> ActivationSnapshot {
        snapshot_with_kind(ActivationStatusKind::Redeeming, None, None)
    }

    /// Refresh the active license.
    /// Skeleton: returns `Activated` snapshot when caller is already
    /// activated, otherwise `NeedsActivation`.
    pub async fn refresh(&self, current_kind: ActivationStatusKind) -> ActivationSnapshot {
        match current_kind {
            ActivationStatusKind::Activated | ActivationStatusKind::OfflineGrace => {
                snapshot_with_kind(ActivationStatusKind::Activated, None, None)
            }
            _ => snapshot_with_kind(ActivationStatusKind::NeedsActivation, None, None),
        }
    }

    /// Check the remote revoke list.
    /// Skeleton: returns `Activated` (no-op) — real revoke check
    /// requires the backend client.
    pub async fn revoke_check(&self) -> ActivationSnapshot {
        snapshot_with_kind(ActivationStatusKind::Activated, None, None)
    }

    /// Explicit user-initiated deactivation.
    pub async fn deactivate(&self) -> ActivationSnapshot {
        snapshot_with_kind(
            ActivationStatusKind::Deactivated,
            Some(ActivationFailureReason::UserDeactivated),
            None,
        )
    }
}

/// Build a uniform snapshot. Centralised so `allows_main_shell` is
/// always derived from the kind (not from the caller).
#[must_use]
pub fn snapshot_with_kind(
    kind: ActivationStatusKind,
    failure_reason: Option<ActivationFailureReason>,
    license: Option<ActivationLicense>,
) -> ActivationSnapshot {
    let allows_main_shell = matches!(
        kind,
        ActivationStatusKind::Activated | ActivationStatusKind::OfflineGrace
    );
    ActivationSnapshot {
        status: ActivationStatus {
            kind,
            message: None,
            grace_until: None,
            pending_request_id: None,
            failure_reason,
        },
        license,
        allows_main_shell,
        correlation: CorrelationIds::default(),
        captured_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activated_snapshot_allows_main_shell() {
        let s = snapshot_with_kind(ActivationStatusKind::Activated, None, None);
        assert!(s.allows_main_shell);
        assert_eq!(s.status.kind, ActivationStatusKind::Activated);
    }

    #[test]
    fn revoked_snapshot_blocks_main_shell() {
        let s = snapshot_with_kind(
            ActivationStatusKind::Revoked,
            Some(ActivationFailureReason::ServerRevoked),
            None,
        );
        assert!(!s.allows_main_shell);
    }

    #[tokio::test]
    async fn deactivate_carries_user_deactivated_reason() {
        let svc = LicenseLifecycleService::new();
        let s = svc.deactivate().await;
        assert_eq!(s.status.kind, ActivationStatusKind::Deactivated);
        assert_eq!(
            s.status.failure_reason,
            Some(ActivationFailureReason::UserDeactivated)
        );
        assert!(!s.allows_main_shell);
    }
}
