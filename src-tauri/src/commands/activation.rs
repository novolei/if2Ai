//! Activation Tauri commands.
//!
//! Two layers coexist on this surface:
//!
//! 1. **Legacy ceremony** (Phase M1.7) — `activation_validate /
//!    activation_start / activation_test_message / activation_complete`
//!    drive the existing onboarding Step 6 "Wake Agent" flow.  Wire
//!    shape preserved verbatim.  Bodies delegate to
//!    [`ActivationService`].
//!
//! 2. **Activation gate** (Phase M2.6 — UClaw activation server port)
//!    — six new commands let the boot-shell modal talk to the real
//!    `iclaw-activation-server` v0.2 backend:
//!
//!    - `activation_get_installation_id`
//!    - `activation_request_license`
//!    - `activation_poll_request_status`
//!    - `activation_redeem_with_request_id`
//!    - `activation_redeem_by_invite_code`
//!    - `activation_refresh`
//!    - `activation_revoke_check`
//!    - `activation_deactivate`
//!
//!    Retry status is forwarded to the frontend as a Tauri event
//!    named `activation_retry_status` whose payload mirrors UClaw's
//!    `(http_status, attempt, max_attempts)` triple.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::modules::application::activation::installation_id::{device_indicator, installation_id};
use crate::modules::application::activation::models::{
    ActivationRequestResponse as InternalRequestResponse,
    ActivationStatusResponse as InternalStatusResponse, NetworkFailure,
};
use crate::modules::application::activation_service::{
    ActivationCeremonyResult as ServiceCeremonyResult, ActivationChecklist as ServiceChecklist,
    ActivationService,
};
use crate::modules::application::license_lifecycle_service::LicenseLifecycleService;
use crate::modules::provider::types::TestResult;
use crate::modules::runtime::contracts::activation::ActivationSnapshot;

// ─────────────────────────────────────────────────────────────────────
// Legacy ceremony surface
// ─────────────────────────────────────────────────────────────────────

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationResult {
    pub success: bool,
    pub session_id: Option<String>,
    pub message: String,
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

#[tauri::command]
pub async fn activation_validate() -> Result<ActivationChecklist, String> {
    ActivationService::new()
        .validate_preconditions()
        .await
        .map(ActivationChecklist::from)
}

#[tauri::command]
pub async fn activation_start() -> Result<ActivationResult, String> {
    ActivationService::new()
        .run_activation_ceremony()
        .await
        .map(ActivationResult::from)
}

#[tauri::command]
pub async fn activation_test_message() -> Result<TestResult, String> {
    ActivationService::new().test_active_provider().await
}

#[tauri::command]
pub async fn activation_complete() -> Result<(), String> {
    ActivationService::new().complete_activation_legacy().await
}

#[tauri::command]
pub async fn activation_get_status() -> Result<ActivationSnapshot, String> {
    Ok(ActivationService::new().current_snapshot().await)
}

// ─────────────────────────────────────────────────────────────────────
// Phase M2.6 — activation gate surface
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationIdentity {
    pub installation_id: String,
    pub device_indicator: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationRequestPayloadDto {
    pub request_id: String,
    pub device_request_code: String,
    pub status: String,
    pub expires_at: String,
    pub server_time: String,
}

impl From<InternalRequestResponse> for ActivationRequestPayloadDto {
    fn from(v: InternalRequestResponse) -> Self {
        Self {
            request_id: v.request_id,
            device_request_code: v.device_request_code,
            status: v.status,
            expires_at: v.expires_at,
            server_time: v.server_time,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationPollResponseDto {
    pub request_id: String,
    pub status: String,
    pub can_redeem: bool,
    pub server_time: String,
}

impl From<InternalStatusResponse> for ActivationPollResponseDto {
    fn from(v: InternalStatusResponse) -> Self {
        Self {
            request_id: v.request_id,
            status: v.status,
            can_redeem: v.can_redeem,
            server_time: v.server_time,
        }
    }
}

/// Stable wire shape for activation errors.  Includes a UI-friendly
/// `code` plus the raw server text so the frontend can render the
/// right copy without parsing English error strings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationErrorDto {
    pub code: String,
    pub http_status: Option<u16>,
    pub message: String,
}

impl From<NetworkFailure> for ActivationErrorDto {
    fn from(err: NetworkFailure) -> Self {
        let code = err.ui_code().to_string();
        let http_status = match &err {
            NetworkFailure::ServerError { status, .. } => Some(*status),
            _ => None,
        };
        let message = err.to_string();
        Self {
            code,
            http_status,
            message,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivationRetryStatusEvent {
    status_code: i32,
    attempt: u32,
    max_attempts: u32,
}

const ACTIVATION_RETRY_EVENT: &str = "activation_retry_status";

fn lifecycle(app: &AppHandle) -> LicenseLifecycleService {
    let app = app.clone();
    let emitter: Arc<dyn Fn(i32, u32, u32) + Send + Sync> =
        Arc::new(move |status_code, attempt, max_attempts| {
            let payload = ActivationRetryStatusEvent {
                status_code,
                attempt,
                max_attempts,
            };
            if let Err(err) = app.emit(ACTIVATION_RETRY_EVENT, payload) {
                tracing::warn!(target: "if2ai::activation", "failed to emit retry status: {err}");
            }
        });
    LicenseLifecycleService::new().with_retry_emitter(emitter)
}

#[tauri::command]
pub async fn activation_get_installation_id() -> Result<InstallationIdentity, String> {
    let id = installation_id();
    Ok(InstallationIdentity {
        device_indicator: device_indicator(&id),
        installation_id: id,
    })
}

#[tauri::command]
pub async fn activation_request_license(
    app: AppHandle,
    installation_id: String,
) -> Result<ActivationRequestPayloadDto, ActivationErrorDto> {
    lifecycle(&app)
        .request_license(installation_id)
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn activation_poll_request_status(
    app: AppHandle,
    request_id: String,
) -> Result<ActivationPollResponseDto, ActivationErrorDto> {
    lifecycle(&app)
        .poll_request_status(request_id)
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn activation_redeem_with_request_id(
    app: AppHandle,
    request_id: String,
    installation_id: String,
) -> Result<ActivationSnapshot, ActivationErrorDto> {
    lifecycle(&app)
        .redeem_with_request_id(request_id, installation_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn activation_redeem_by_invite_code(
    app: AppHandle,
    invite_code: String,
    installation_id: String,
) -> Result<ActivationSnapshot, ActivationErrorDto> {
    lifecycle(&app)
        .redeem_by_invite_code(invite_code, installation_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn activation_refresh(app: AppHandle) -> Result<ActivationSnapshot, ActivationErrorDto> {
    lifecycle(&app).refresh().await.map_err(Into::into)
}

#[tauri::command]
pub async fn activation_revoke_check(
    app: AppHandle,
) -> Result<ActivationSnapshot, ActivationErrorDto> {
    lifecycle(&app).revoke_check().await.map_err(Into::into)
}

#[tauri::command]
pub async fn activation_deactivate(app: AppHandle) -> Result<ActivationSnapshot, String> {
    Ok(lifecycle(&app).deactivate().await)
}
