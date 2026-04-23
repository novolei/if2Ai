//! Activation wire schema & local types.
//!
//! Wire types are byte-for-byte aligned with `iclaw-activation-server`
//! v0.2（`activation-server-rs/src/main.rs`）。Snake-case JSON
//! over the wire; field names MUST NOT drift.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────
// Wire — POST /v1/activations/request
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ActivationRequestPayload {
    pub installation_id: String,
    pub app_id: String,
    pub app_version: String,
    pub build: String,
    pub platform: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActivationRequestResponse {
    pub request_id: String,
    pub device_request_code: String,
    /// `"pending"` or `"approved"` (server short-circuits to approved
    /// when the request fits the auto-approve quota).
    pub status: String,
    pub expires_at: String,
    pub server_time: String,
}

// ─────────────────────────────────────────────────────────────────────
// Wire — GET /v1/activations/request/:id
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActivationStatusResponse {
    pub request_id: String,
    pub status: String,
    pub can_redeem: bool,
    pub server_time: String,
}

// ─────────────────────────────────────────────────────────────────────
// Wire — POST /v1/activations/redeem (+by-code)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RedeemPayload {
    pub request_id: String,
    pub installation_id: String,
    pub app_id: String,
    pub app_version: String,
    pub build: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RedeemByCodePayload {
    pub invite_code: String,
    pub installation_id: String,
    pub app_id: String,
    pub app_version: String,
    pub build: String,
    pub platform: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RedeemResponse {
    pub license_jws: String,
    pub refresh_token: String,
    pub license_id: String,
    pub server_time: String,
    pub refresh_after_sec: i64,
}

// ─────────────────────────────────────────────────────────────────────
// Wire — POST /v1/licenses/refresh
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RefreshPayload {
    pub refresh_token: String,
    pub installation_id: String,
    pub license_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefreshResponse {
    pub license_jws: String,
    pub refresh_token: String,
    pub server_time: String,
    pub refresh_after_sec: i64,
}

// ─────────────────────────────────────────────────────────────────────
// Wire — POST /v1/licenses/revoke-check
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RevokeCheckPayload {
    pub license_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevokeCheckResponse {
    pub revoked: bool,
    pub server_time: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerErrorResponse {
    pub error: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub retry_after_sec: Option<i64>,
}

// ─────────────────────────────────────────────────────────────────────
// Local — license cache (`~/.if2ai/activation/license.json`, 0o600)
// ─────────────────────────────────────────────────────────────────────

/// On-disk shape for the cached license.
///
/// Only the `license_jws` carries authoritative claims; the other
/// fields are kept alongside so refresh / revoke / display flows do
/// not need to re-parse the JWS for routing decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredLicense {
    pub license_id: String,
    pub refresh_token: String,
    pub license_jws: String,
    /// Echo of the device this license was issued to. Cross-checked
    /// against the JWS payload's `installation_id` claim and the
    /// current device on every evaluation.
    pub installation_id: String,
    /// RFC3339 — monotonically increasing high-water mark of the
    /// largest `server_time` ever returned by the activation server.
    /// Used to defeat local clock rollback (see
    /// [`crate::modules::application::activation::license_evaluator`]).
    pub last_trusted_server_time: String,
}

// ─────────────────────────────────────────────────────────────────────
// Decoded JWS claims (signature verified in `license_evaluator`).
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct LicenseClaims {
    #[serde(default)]
    pub iss: Option<String>,
    #[serde(default)]
    pub aud: Option<String>,
    #[serde(default)]
    pub sub: Option<String>,
    #[serde(default)]
    pub installation_id: Option<String>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub iat: Option<i64>,
    #[serde(default)]
    pub nbf: Option<i64>,
    #[serde(default)]
    pub exp: Option<i64>,
    #[serde(default)]
    pub offline_grace_exp: Option<i64>,
    #[serde(default)]
    pub min_build: Option<String>,
    #[serde(default)]
    pub ver: Option<i64>,
}

/// Outcome of the local-only license evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseValidity {
    Valid,
    OfflineGrace,
    Expired,
    DeviceMismatch,
    AudienceMismatch,
    /// Ed25519 signature on `license_jws` does not match the baked-in pubkey.
    InvalidSignature,
    MalformedJws,
}

// ─────────────────────────────────────────────────────────────────────
// Network / IO errors
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum NetworkFailure {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("invalid response (no http status)")]
    InvalidResponse,
    #[error("server error: http={status} code={code} message={message}")]
    ServerError {
        status: u16,
        code: String,
        retry_after_sec: Option<i64>,
        message: String,
    },
    #[error("decoding failed: {0}")]
    Decoding(String),
    /// 保留：历史上在未设置 `IF2AI_ACTIVATION_BASE_URL` 时返回；当前客户端
    /// 会回退到公网默认端点，正常流程不应再出现。
    #[error("activation backend not configured (set IF2AI_ACTIVATION_BASE_URL)")]
    NotConfigured,
}

impl NetworkFailure {
    /// Stable wire string for the IPC layer to forward to the UI.
    /// Mirrors the categories expected by `useActivationGate`'s
    /// `failureText` mapping (429 / 503 / 400 invalid_app_id /
    /// transport / decoding / unavailable).
    pub fn ui_code(&self) -> &'static str {
        match self {
            NetworkFailure::Transport(_) => "transport",
            NetworkFailure::InvalidResponse => "invalid_response",
            NetworkFailure::ServerError { status: 429, .. } => "queueing",
            NetworkFailure::ServerError { status: 503, .. } => "busy",
            NetworkFailure::ServerError {
                status: 400, code, ..
            } if code.contains("invalid_app_id") => "invalid_app_id",
            NetworkFailure::ServerError { .. } => "server_error",
            NetworkFailure::Decoding(_) => "decoding",
            NetworkFailure::NotConfigured => "not_configured",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redeem_response_round_trip() {
        let json = serde_json::json!({
            "license_jws": "h.p.s",
            "refresh_token": "rt_abc",
            "license_id": "lic_x",
            "server_time": "2026-04-23T10:00:00Z",
            "refresh_after_sec": 86400_i64
        });
        let r: RedeemResponse = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(r.license_id, "lic_x");
        assert_eq!(r.refresh_after_sec, 86400);
    }

    #[test]
    fn ui_code_classifies_known_categories() {
        assert_eq!(
            NetworkFailure::Transport("oops".into()).ui_code(),
            "transport"
        );
        assert_eq!(NetworkFailure::NotConfigured.ui_code(), "not_configured");
        assert_eq!(
            NetworkFailure::ServerError {
                status: 503,
                code: "busy".into(),
                retry_after_sec: None,
                message: String::new()
            }
            .ui_code(),
            "busy"
        );
    }
}
