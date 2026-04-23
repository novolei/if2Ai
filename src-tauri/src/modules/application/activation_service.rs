//! Activation service skeleton (Phase M1.7).
//!
//! Wraps the existing onboarding-style activation logic
//! ([`crate::modules::onboarding::flow::OnboardingFlow`] +
//! [`crate::modules::config::ConfigService`] +
//! [`crate::modules::provider::test`]) behind a typed application
//! service so:
//!
//! 1. The IPC adapter [`crate::commands::activation`] becomes a
//!    thin pass-through.
//! 2. `commands/activation.rs` no longer reaches into config,
//!    onboarding state, and provider test independently — that
//!    composition lives here.
//! 3. M1.4-style single seam (`prepare_*`) is preserved for future
//!    M2 boot-shell projection.
//!
//! M1.7 explicitly does NOT:
//!
//! - Remove the legacy onboarding-ceremony commands; they remain
//!   the user-facing entry until M2 boot shell ships. This service
//!   *complements* them with a typed surface for the new platform
//!   activation commands listed in
//!   [`docs/exec-plans/active/phase-m1-routing-activation-control-plane-file-level-plan.md`](../../../../../docs/exec-plans/active/phase-m1-routing-activation-control-plane-file-level-plan.md)
//!   §5.4.
//! - Wire a real remote license backend; that hangs off
//!   [`super::license_lifecycle_service::LicenseLifecycleService`]
//!   in a follow-up slice.
//!
//! Hard rules:
//! 1. This service MUST NOT import from `crate::commands::*`.
//! 2. The 10-state alphabet comes from
//!    `runtime::contracts::activation`; never invent local kinds.
//! 3. `allows_main_shell` is computed by
//!    [`super::license_lifecycle_service::snapshot_with_kind`],
//!    never by the caller.

#![allow(dead_code)]

use crate::modules::config::{ChannelRouting, ConfigService};
use crate::modules::onboarding::flow::OnboardingFlow;
use crate::modules::onboarding::state::OnboardingState;
use crate::modules::onboarding::store::{load_state, save_state};
use crate::modules::provider::test::{send_greeting, test_provider_connection};
use crate::modules::provider::types::TestResult;
use crate::modules::runtime::contracts::activation::{ActivationSnapshot, ActivationStatusKind};

use super::license_lifecycle_service::{snapshot_with_kind, LicenseLifecycleService};

/// Activation precondition checklist surfaced to the onboarding UI.
///
/// Mirrors the legacy `commands::activation::ActivationChecklist`
/// shape verbatim (same field names, same JSON serialisation) so
/// the IPC adapter can `From`-convert without breaking the wire
/// contract.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActivationChecklist {
    pub system_check: bool,
    pub security_confirmed: bool,
    pub provider_configured: bool,
    pub channels_configured: bool,
}

/// Result of the legacy ceremony-style activation start. Held here
/// so the IPC adapter can stay a single-line pass-through.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActivationCeremonyResult {
    pub success: bool,
    pub session_id: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_response: Option<String>,
}

/// Application service composing the legacy onboarding-style
/// activation with the new typed lifecycle.
pub struct ActivationService {
    config_service: ConfigService,
    lifecycle: LicenseLifecycleService,
}

impl Default for ActivationService {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivationService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config_service: ConfigService::new(),
            lifecycle: LicenseLifecycleService::new(),
        }
    }

    /// Borrow the underlying license lifecycle service so platform
    /// commands (`activation_request_license` etc., M2+) can reach
    /// it without re-constructing one.
    #[must_use]
    pub fn license_lifecycle(&self) -> &LicenseLifecycleService {
        &self.lifecycle
    }

    /// Phase M2.5 — return the canonical
    /// [`ActivationSnapshot`] the frontend boot shell consumes via
    /// the new `activation_get_status` IPC command.
    ///
    /// Honest mapping today (no remote license backend yet):
    /// - `OnboardingState.onboarding_completed == true` →
    ///   [`ActivationStatusKind::Activated`] +
    ///   `allows_main_shell = true`.
    /// - Anything else (including a missing state file →
    ///   `OnboardingState::default()`) →
    ///   [`ActivationStatusKind::NeedsActivation`] +
    ///   `allows_main_shell = false`.
    ///
    /// `license` is always `None` because
    /// [`LicenseLifecycleService`] is a placeholder and no remote
    /// license API is wired. When the real backend lands, this
    /// method returns the lifecycle snapshot directly without a
    /// contract bump — the IPC + frontend projection seam stays the
    /// same.
    pub async fn current_snapshot(&self) -> ActivationSnapshot {
        // License is the single source of truth for `allows_main_shell`.
        //
        // Earlier revisions fell back to `OnboardingState.onboarding_completed`
        // when the local license cache was missing.  That mask hid genuine
        // post-boot transitions: once the lifecycle loop's `revoke_check`
        // observed a server-side revoke and wiped `license.json`, the
        // fallback would silently re-promote the snapshot to `Activated`,
        // so the activation modal never popped.  Treat a missing /
        // invalid license as the canonical `NeedsActivation` instead —
        // the activation gate then renders for fresh installs *and* for
        // post-revoke transitions, with no special casing.
        self.lifecycle.local_boot_restore().await
    }

    /// Compute the precondition checklist used by the onboarding
    /// "Step 6 — Activate" UI.
    pub async fn validate_preconditions(&self) -> Result<ActivationChecklist, String> {
        let config = self
            .config_service
            .load_config()
            .await
            .map_err(|e| e.to_string())?;
        Ok(ActivationChecklist {
            system_check: true,
            security_confirmed: config.security_confirmed,
            provider_configured: config
                .active_provider
                .as_ref()
                .is_some_and(|p| p.is_complete()),
            channels_configured: !config.channels.is_empty(),
        })
    }

    /// Run the legacy ceremony-style "first launch" greet so the
    /// onboarding UI can display the model's first reply.
    pub async fn run_activation_ceremony(&self) -> Result<ActivationCeremonyResult, String> {
        let config = self
            .config_service
            .load_config()
            .await
            .map_err(|e| e.to_string())?;

        if config.active_provider.is_none_or(|p| !p.is_complete()) {
            return Err("No provider configured. Please complete Step 4 first.".to_string());
        }

        // Greeting failure is non-fatal — activation succeeds regardless.
        let ai_response = tokio::time::timeout(std::time::Duration::from_secs(30), send_greeting())
            .await
            .map_err(|_| {
                tracing::warn!("[activation_service] Greeting timed out after 30s");
            })
            .ok()
            .and_then(|r| r.ok())
            .flatten();

        if ai_response.is_none() {
            tracing::info!(
                "[activation_service] No greeting response — proceeding without ceremony message"
            );
        }

        Ok(ActivationCeremonyResult {
            success: true,
            session_id: Some("activation".to_string()),
            message: "if2AI Agent 已启动".to_string(),
            ai_response,
        })
    }

    /// Send a verification test prompt to the configured provider.
    pub async fn test_active_provider(&self) -> Result<TestResult, String> {
        let config = self
            .config_service
            .load_config()
            .await
            .map_err(|e| e.to_string())?;

        let provider = config
            .active_provider
            .ok_or_else(|| "No provider configured".to_string())?;

        let base_url = provider.base_url.as_deref().unwrap_or_default();
        let result =
            test_provider_connection(&provider.provider_id, base_url, provider.api_key.as_deref())
                .await;

        Ok(result)
    }

    /// Mark onboarding as complete and persist the final
    /// `AppConfig`.  Returns the typed [`ActivationSnapshot`] so
    /// callers (and future M2 boot shell) can switch on the
    /// canonical status.
    pub async fn complete_activation(&self) -> Result<ActivationSnapshot, String> {
        let current = load_state()
            .await
            .map_err(|e| format!("Failed to load state: {e}"))?;
        let completed = OnboardingFlow::complete(&current).map_err(|e| e.to_string())?;
        save_state(&completed)
            .await
            .map_err(|e| format!("Failed to save state: {e}"))?;

        let mut config = self.config_service.load_config().await.unwrap_or_default();
        config.onboarding = completed;
        config.routing.get_or_insert_with(ChannelRouting::default);
        let _ = self.config_service.save_config(&config).await;

        // Today the local activation has no remote license; we
        // expose the snapshot anyway so the boot shell can already
        // gate on the canonical status. The `Activated` snapshot
        // here means "onboarding ceremony complete and provider
        // self-test passed"; it does NOT yet imply a remote-issued
        // license — that lands when the lifecycle service grows a
        // backend client.
        Ok(snapshot_with_kind(
            ActivationStatusKind::Activated,
            None,
            None,
        ))
    }

    /// Backward-compat shim for the legacy IPC adapter that needs
    /// `Result<(), String>` (see
    /// [`crate::commands::activation::activation_complete`]).
    pub async fn complete_activation_legacy(&self) -> Result<(), String> {
        self.complete_activation().await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checklist_round_trips_through_legacy_shape() {
        // Validate the wire shape stays compatible with the legacy
        // `commands::activation::ActivationChecklist` JSON.
        let c = ActivationChecklist {
            system_check: true,
            security_confirmed: true,
            provider_configured: false,
            channels_configured: false,
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["system_check"], true);
        assert_eq!(v["security_confirmed"], true);
        assert_eq!(v["provider_configured"], false);
        assert_eq!(v["channels_configured"], false);
    }

    #[test]
    fn ceremony_result_round_trips_through_legacy_shape() {
        let r = ActivationCeremonyResult {
            success: true,
            session_id: Some("sess".into()),
            message: "hi".into(),
            ai_response: Some("hello".into()),
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["success"], true);
        assert_eq!(v["session_id"], "sess");
        assert_eq!(v["ai_response"], "hello");
    }
}
