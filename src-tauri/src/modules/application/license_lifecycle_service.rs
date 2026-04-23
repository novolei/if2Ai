//! License lifecycle service (Phase M2.6 — UClaw activation server port).
//!
//! Real implementation of the lifecycle methods originally stubbed by
//! `snapshot_with_kind(...)`.  Composes:
//!
//! - [`crate::modules::application::activation::http_client::ActivationHttpClient`]
//!   for the `iclaw-activation-server` v0.2 HTTP surface.
//! - [`crate::modules::application::activation::license_store::LicenseStore`]
//!   (default [`FileLicenseStore`]) for `~/.if2ai/activation/license.json`.
//! - [`crate::modules::application::activation::license_evaluator`]
//!   for local validity + **Ed25519** `license_jws` verification.
//!
//! Hard rules (preserved from the M1.7 skeleton):
//! 1. This service MUST NOT import from `crate::commands::*`.
//! 2. The contract enums (`ActivationStatusKind`,
//!    `ActivationActionKind`, `ActivationFailureReason`) are owned
//!    by `runtime::contracts::activation`. Adding states here is a
//!    contract break and belongs in M0.
//! 3. Snapshot `allows_main_shell` is computed by
//!    [`snapshot_with_kind`], never by the caller.

#![allow(dead_code)]

use std::sync::Arc;

use crate::modules::runtime::contracts::activation::{
    ActivationFailureReason, ActivationLicense, ActivationSnapshot, ActivationStatus,
    ActivationStatusKind,
};
use crate::modules::runtime::contracts::common::CorrelationIds;

use super::activation::http_client::{ActivationHttpClient, RetryStatusEmitter};
use super::activation::installation_id::installation_id;
use super::activation::license_evaluator::evaluate;
use super::activation::license_store::{
    max_trusted_server_time, FileLicenseStore, LicenseStore, LicenseStoreError,
};
use super::activation::models::{
    ActivationRequestPayload, ActivationRequestResponse, ActivationStatusResponse, LicenseValidity,
    NetworkFailure, RedeemByCodePayload, RedeemPayload, RedeemResponse, RefreshPayload,
    RevokeCheckPayload, StoredLicense,
};

/// Process-wide installation id cache; populated lazily.
fn current_installation_id() -> String {
    installation_id()
}

/// Default `aud` claim. Must match the value the activation server
/// has registered in its `APP_IDS` list.  Future flexibility: read
/// from env or config — not exposed via UI today.
pub const APP_ID: &str = "ai.if2.if2Ai";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_BUILD: &str = "1";
pub const APP_PLATFORM: &str = if cfg!(target_os = "macos") {
    "macOS"
} else if cfg!(target_os = "windows") {
    "Windows"
} else if cfg!(target_os = "linux") {
    "Linux"
} else {
    "Other"
};

pub struct LicenseLifecycleService {
    store: Arc<dyn LicenseStore>,
    retry_emitter: Option<RetryStatusEmitter>,
}

impl Default for LicenseLifecycleService {
    fn default() -> Self {
        Self::new()
    }
}

impl LicenseLifecycleService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(FileLicenseStore::new()),
            retry_emitter: None,
        }
    }

    /// Inject a Tauri-event emitter so retry status flows to the UI.
    /// Idempotent — last call wins.
    pub fn with_retry_emitter(mut self, emitter: RetryStatusEmitter) -> Self {
        self.retry_emitter = Some(emitter);
        self
    }

    fn http(&self) -> Result<ActivationHttpClient, NetworkFailure> {
        let mut c = ActivationHttpClient::from_env()?;
        if let Some(e) = self.retry_emitter.clone() {
            c = c.with_retry_emitter(e);
        }
        Ok(c)
    }

    /// Boot-time restore: read the cached license, evaluate it, and
    /// return the corresponding snapshot.
    pub async fn local_boot_restore(&self) -> ActivationSnapshot {
        match self.store.load().await {
            Ok(Some(stored)) => {
                let installation = current_installation_id();
                match evaluate(&stored, &installation, APP_ID) {
                    LicenseValidity::Valid => snapshot_with_kind(
                        ActivationStatusKind::Activated,
                        None,
                        Some(license_summary(&stored)),
                    ),
                    LicenseValidity::OfflineGrace => snapshot_with_kind(
                        ActivationStatusKind::OfflineGrace,
                        Some(ActivationFailureReason::RefreshTransient),
                        Some(license_summary(&stored)),
                    ),
                    LicenseValidity::Expired => snapshot_with_kind(
                        ActivationStatusKind::Expired,
                        Some(ActivationFailureReason::ServerExpired),
                        Some(license_summary(&stored)),
                    ),
                    LicenseValidity::DeviceMismatch
                    | LicenseValidity::AudienceMismatch
                    | LicenseValidity::InvalidSignature
                    | LicenseValidity::MalformedJws => {
                        let _ = self.store.clear().await;
                        snapshot_with_kind(ActivationStatusKind::NeedsActivation, None, None)
                    }
                }
            }
            Ok(None) => snapshot_with_kind(ActivationStatusKind::NeedsActivation, None, None),
            Err(err) => {
                tracing::warn!(target: "if2ai::activation", "license_store load failed: {err}");
                snapshot_with_kind(ActivationStatusKind::NeedsActivation, None, None)
            }
        }
    }

    /// Issue a new activation request; the response carries either
    /// `status="approved"` (auto-approve quota path) or
    /// `status="pending"` (frontend then polls).
    pub async fn request_license(
        &self,
        installation_id_value: String,
    ) -> Result<ActivationRequestResponse, NetworkFailure> {
        let http = self.http()?;
        let payload = ActivationRequestPayload {
            installation_id: installation_id_value,
            app_id: APP_ID.into(),
            app_version: APP_VERSION.into(),
            build: APP_BUILD.into(),
            platform: APP_PLATFORM.into(),
        };
        http.request_activation(&payload).await
    }

    /// Poll the request status until `can_redeem == true` is observed
    /// by the caller (frontend orchestrates the loop).
    pub async fn poll_request_status(
        &self,
        request_id: String,
    ) -> Result<ActivationStatusResponse, NetworkFailure> {
        let http = self.http()?;
        http.fetch_activation_status(&request_id).await
    }

    /// Redeem an approved request.  On success, the license is
    /// persisted under `~/.if2ai/activation/license.json` and the
    /// returned snapshot is `Activated`.
    pub async fn redeem_with_request_id(
        &self,
        request_id: String,
        installation_id_value: String,
    ) -> Result<ActivationSnapshot, NetworkFailure> {
        let http = self.http()?;
        let payload = RedeemPayload {
            request_id,
            installation_id: installation_id_value.clone(),
            app_id: APP_ID.into(),
            app_version: APP_VERSION.into(),
            build: APP_BUILD.into(),
            platform: APP_PLATFORM.into(),
        };
        let response = http.redeem(&payload).await?;
        self.persist_redeem_response(installation_id_value, response)
            .await
    }

    /// Redeem an 8-character invite code (admin pre-issued).
    pub async fn redeem_by_invite_code(
        &self,
        invite_code: String,
        installation_id_value: String,
    ) -> Result<ActivationSnapshot, NetworkFailure> {
        let http = self.http()?;
        let payload = RedeemByCodePayload {
            invite_code,
            installation_id: installation_id_value.clone(),
            app_id: APP_ID.into(),
            app_version: APP_VERSION.into(),
            build: APP_BUILD.into(),
            platform: APP_PLATFORM.into(),
        };
        let response = http.redeem_by_code(&payload).await?;
        self.persist_redeem_response(installation_id_value, response)
            .await
    }

    /// User-initiated refresh.  Skips the network call cleanly when
    /// no local license exists (returns `NeedsActivation` snapshot).
    pub async fn refresh(&self) -> Result<ActivationSnapshot, NetworkFailure> {
        let stored = match self.store.load().await.map_err(store_to_network)? {
            Some(s) => s,
            None => {
                return Ok(snapshot_with_kind(
                    ActivationStatusKind::NeedsActivation,
                    None,
                    None,
                ))
            }
        };
        let http = self.http()?;
        let payload = RefreshPayload {
            refresh_token: stored.refresh_token.clone(),
            installation_id: stored.installation_id.clone(),
            license_id: stored.license_id.clone(),
        };
        let response = http.refresh(&payload).await?;
        let next = StoredLicense {
            license_id: stored.license_id.clone(),
            refresh_token: response.refresh_token,
            license_jws: response.license_jws,
            installation_id: stored.installation_id.clone(),
            last_trusted_server_time: max_trusted_server_time(
                &stored.last_trusted_server_time,
                &response.server_time,
            ),
        };
        self.store.save(&next).await.map_err(store_to_network)?;
        Ok(self.local_boot_restore().await)
    }

    /// Server-side revoke check.  When the server reports `revoked
    /// = true`, the local cache is wiped and a `Revoked` snapshot is
    /// returned.
    pub async fn revoke_check(&self) -> Result<ActivationSnapshot, NetworkFailure> {
        let stored = match self.store.load().await.map_err(store_to_network)? {
            Some(s) => s,
            None => {
                return Ok(snapshot_with_kind(
                    ActivationStatusKind::NeedsActivation,
                    None,
                    None,
                ))
            }
        };
        let http = self.http()?;
        let response = http
            .revoke_check(&RevokeCheckPayload {
                license_id: stored.license_id.clone(),
            })
            .await?;
        if response.revoked {
            self.store.clear().await.map_err(store_to_network)?;
            return Ok(snapshot_with_kind(
                ActivationStatusKind::Revoked,
                Some(ActivationFailureReason::ServerRevoked),
                None,
            ));
        }
        Ok(self.local_boot_restore().await)
    }

    /// Explicit user deactivation: clear the local cache.  No remote
    /// "deactivate" endpoint exists on the server today.
    pub async fn deactivate(&self) -> ActivationSnapshot {
        if let Err(err) = self.store.clear().await {
            tracing::warn!(target: "if2ai::activation", "license_store clear failed during deactivate: {err}");
        }
        snapshot_with_kind(
            ActivationStatusKind::Deactivated,
            Some(ActivationFailureReason::UserDeactivated),
            None,
        )
    }

    async fn persist_redeem_response(
        &self,
        installation_id_value: String,
        response: RedeemResponse,
    ) -> Result<ActivationSnapshot, NetworkFailure> {
        let stored = StoredLicense {
            license_id: response.license_id.clone(),
            refresh_token: response.refresh_token,
            license_jws: response.license_jws,
            installation_id: installation_id_value,
            last_trusted_server_time: response.server_time,
        };
        self.store.save(&stored).await.map_err(store_to_network)?;
        Ok(self.local_boot_restore().await)
    }
}

fn store_to_network(err: LicenseStoreError) -> NetworkFailure {
    NetworkFailure::Transport(err.to_string())
}

fn license_summary(stored: &StoredLicense) -> ActivationLicense {
    let claims = super::activation::license_evaluator::decode_jws_claims(&stored.license_jws);
    let issued_at = claims
        .as_ref()
        .and_then(|c| c.iat)
        .and_then(|s| chrono::DateTime::<chrono::Utc>::from_timestamp(s, 0))
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| stored.last_trusted_server_time.clone());
    let expires_at = claims
        .as_ref()
        .and_then(|c| c.exp)
        .and_then(|s| chrono::DateTime::<chrono::Utc>::from_timestamp(s, 0))
        .map(|dt| dt.to_rfc3339());
    ActivationLicense {
        license_id: stored.license_id.clone(),
        plan: claims.as_ref().and_then(|c| c.tier.clone()),
        issued_at,
        expires_at,
        last_refreshed_at: Some(stored.last_trusted_server_time.clone()),
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
