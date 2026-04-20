//! Activation Tauri commands (Phase M1.7 thin adapter).
//!
//! The 4 ceremony commands required by ADR-014 §18.9
//! (`activation_validate / activation_start / activation_test_message
//! / activation_complete`) are preserved verbatim on the wire — the
//! frontend onboarding flow continues to call them with the same
//! shapes — but their bodies now delegate to
//! [`crate::modules::application::activation_service::ActivationService`].
//!
//! The new platform-style commands (`activation_get_status`,
//! `activation_request_license`, `activation_redeem`,
//! `activation_refresh`, `activation_revoke_check`,
//! `activation_deactivate`) listed in
//! [`docs/exec-plans/active/phase-m1-routing-activation-control-plane-file-level-plan.md`](../../../docs/exec-plans/active/phase-m1-routing-activation-control-plane-file-level-plan.md)
//! §5.4 land in a follow-up slice — the M1.7 surface intentionally
//! ships only the typed service edge so the boot-shell work in M2
//! can adopt them without IPC churn.

use serde::{Deserialize, Serialize};

use crate::modules::application::activation_service::{
    ActivationCeremonyResult as ServiceCeremonyResult, ActivationChecklist as ServiceChecklist,
    ActivationService,
};
use crate::modules::provider::types::TestResult;

/// Activation checklist — IPC wire shape. Field-for-field mirror of
/// [`ServiceChecklist`] held here so the existing TS twin in
/// [`src/lib/tauri.ts`](../../../src/lib/tauri.ts) keeps compiling
/// during the M1 transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationChecklist {
    pub system_check: bool,
    pub security_confirmed: bool,
    pub provider_configured: bool,
    pub channels_configured: bool,
}

impl From<ServiceChecklist> for ActivationChecklist {
    fn from(value: ServiceChecklist) -> Self {
        Self {
            system_check: value.system_check,
            security_confirmed: value.security_confirmed,
            provider_configured: value.provider_configured,
            channels_configured: value.channels_configured,
        }
    }
}

/// Result of starting the agent — wire shape preserved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationResult {
    pub success: bool,
    pub session_id: Option<String>,
    pub message: String,
    /// First response from the configured LLM — displayed during
    /// the onboarding ceremony.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_response: Option<String>,
}

impl From<ServiceCeremonyResult> for ActivationResult {
    fn from(value: ServiceCeremonyResult) -> Self {
        Self {
            success: value.success,
            session_id: value.session_id,
            message: value.message,
            ai_response: value.ai_response,
        }
    }
}

/// Validate that all activation preconditions are met.
#[tauri::command]
pub async fn activation_validate() -> Result<ActivationChecklist, String> {
    ActivationService::new()
        .validate_preconditions()
        .await
        .map(ActivationChecklist::from)
}

/// Start the agent for the first time — runs the onboarding
/// ceremony greeting via the application service.
#[tauri::command]
pub async fn activation_start() -> Result<ActivationResult, String> {
    ActivationService::new()
        .run_activation_ceremony()
        .await
        .map(ActivationResult::from)
}

/// Send a test message to verify end-to-end connectivity.
#[tauri::command]
pub async fn activation_test_message() -> Result<TestResult, String> {
    ActivationService::new().test_active_provider().await
}

/// Mark activation as complete and finish onboarding.
///
/// Returns `()` on the wire to match the legacy IPC shape;
/// [`ActivationService::complete_activation`] returns a typed
/// snapshot for future M2 boot-shell consumers — the legacy adapter
/// uses the `_legacy` shim to discard it.
#[tauri::command]
pub async fn activation_complete() -> Result<(), String> {
    ActivationService::new().complete_activation_legacy().await
}
