//! HTTP client for `iclaw-activation-server` v0.2.
//!
//! Mirrors UClaw `ActivationService.send`'s retry policy:
//! `max_attempts = 5`, base `0.5s`, max `4s`, exponential backoff +
//! ≤20% jitter; only 429 / 503 are retried (they signal transient
//! server pressure); `Retry-After` (header *or* body
//! `retry_after_sec`) is honored.  Each attempt times out at 12s.
//!
//! Configuration:
//! - `IF2AI_ACTIVATION_BASE_URL` env var → base URL（无尾部 `/`）。
//! - 未设置或仅空白时，使用 [`DEFAULT_ACTIVATION_BASE_URL`]（与线上
//!   Nginx `/license-api` 反代一致）；自建服务时覆盖该环境变量即可。
//!
//! Telemetry surface: a [`RetryStatusEmitter`] callback is invoked
//! before every backoff sleep so the IPC layer can forward retry
//! status to the UI via Tauri events.

use std::env;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};

use super::models::{
    ActivationRequestPayload, ActivationRequestResponse, ActivationStatusResponse, NetworkFailure,
    RedeemByCodePayload, RedeemPayload, RedeemResponse, RefreshPayload, RefreshResponse,
    RevokeCheckPayload, RevokeCheckResponse, ServerErrorResponse,
};

/// Environment variable holding the activation backend base URL.
pub const ACTIVATION_BASE_URL_ENV: &str = "IF2AI_ACTIVATION_BASE_URL";

/// 与 UClaw / if2Ai 共用的公网激活 API 前缀（覆盖方式：设置
/// [`ACTIVATION_BASE_URL_ENV`]）。
pub const DEFAULT_ACTIVATION_BASE_URL: &str = "https://console.iclaw.us/license-api";

/// Default request timeout (per attempt).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);

/// Retry policy hard-coded to UClaw's defaults.
struct RetryPolicy {
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
}

impl RetryPolicy {
    const fn uclaw_default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(4),
        }
    }
}

/// Callback invoked before each retry sleep.
///
/// Arguments are `(http_status_or_-1_for_transport, attempt,
/// max_attempts)`. `Send + Sync` so the IPC layer can hand in a
/// closure that emits a Tauri event.
pub type RetryStatusEmitter = Arc<dyn Fn(i32, u32, u32) + Send + Sync>;

#[derive(Clone)]
pub struct ActivationHttpClient {
    client: Client,
    base_url: String,
    on_retry: Option<RetryStatusEmitter>,
}

impl ActivationHttpClient {
    /// Build a client by reading [`ACTIVATION_BASE_URL_ENV`], falling
    /// back to [`DEFAULT_ACTIVATION_BASE_URL`] when unset or blank.
    pub fn from_env() -> Result<Self, NetworkFailure> {
        let raw = env::var(ACTIVATION_BASE_URL_ENV).unwrap_or_default();
        let trimmed = raw.trim().trim_end_matches('/').to_string();
        let base_url = if trimmed.is_empty() {
            DEFAULT_ACTIVATION_BASE_URL.to_string()
        } else {
            trimmed
        };
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| NetworkFailure::Transport(e.to_string()))?;
        Ok(Self {
            client,
            base_url,
            on_retry: None,
        })
    }

    /// Resolved HTTP origin (after env + default handling).
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Inject a retry-status emitter (used by the IPC layer to forward
    /// to the frontend via `app_handle.emit("activation_retry_status", …)`).
    pub fn with_retry_emitter(mut self, emitter: RetryStatusEmitter) -> Self {
        self.on_retry = Some(emitter);
        self
    }

    pub async fn request_activation(
        &self,
        payload: &ActivationRequestPayload,
    ) -> Result<ActivationRequestResponse, NetworkFailure> {
        self.send_json("POST", "/v1/activations/request", Some(payload))
            .await
    }

    pub async fn fetch_activation_status(
        &self,
        request_id: &str,
    ) -> Result<ActivationStatusResponse, NetworkFailure> {
        let path = format!("/v1/activations/request/{}", request_id);
        self.send_json::<(), _>("GET", &path, None).await
    }

    pub async fn redeem(&self, payload: &RedeemPayload) -> Result<RedeemResponse, NetworkFailure> {
        self.send_json("POST", "/v1/activations/redeem", Some(payload))
            .await
    }

    pub async fn redeem_by_code(
        &self,
        payload: &RedeemByCodePayload,
    ) -> Result<RedeemResponse, NetworkFailure> {
        self.send_json("POST", "/v1/activations/redeem-by-code", Some(payload))
            .await
    }

    pub async fn refresh(
        &self,
        payload: &RefreshPayload,
    ) -> Result<RefreshResponse, NetworkFailure> {
        self.send_json("POST", "/v1/licenses/refresh", Some(payload))
            .await
    }

    pub async fn revoke_check(
        &self,
        payload: &RevokeCheckPayload,
    ) -> Result<RevokeCheckResponse, NetworkFailure> {
        self.send_json("POST", "/v1/licenses/revoke-check", Some(payload))
            .await
    }

    async fn send_json<B, R>(
        &self,
        method: &str,
        path: &str,
        body: Option<&B>,
    ) -> Result<R, NetworkFailure>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let policy = RetryPolicy::uclaw_default();
        let mut last_err = NetworkFailure::InvalidResponse;

        for attempt in 1..=policy.max_attempts {
            let mut req = match method {
                "GET" => self.client.get(&url),
                "POST" => self.client.post(&url),
                _ => return Err(NetworkFailure::Transport(format!("bad method {method}"))),
            };
            req = req.header("Accept", "application/json");
            if let Some(b) = body {
                req = req.json(b);
            }

            match req.send().await {
                Err(err) => {
                    last_err = NetworkFailure::Transport(err.to_string());
                    if attempt < policy.max_attempts {
                        if let Some(cb) = &self.on_retry {
                            cb(-1, attempt, policy.max_attempts);
                        }
                        sleep_with_jitter(backoff_delay(&policy, attempt)).await;
                        continue;
                    }
                    return Err(last_err);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let bytes = resp
                        .bytes()
                        .await
                        .map_err(|e| NetworkFailure::Transport(e.to_string()))?;

                    if status.is_success() {
                        return serde_json::from_slice::<R>(&bytes)
                            .map_err(|e| NetworkFailure::Decoding(e.to_string()));
                    }

                    let server_err: Option<ServerErrorResponse> = serde_json::from_slice(&bytes).ok();
                    let code = server_err
                        .as_ref()
                        .and_then(|e| e.code.clone())
                        .unwrap_or_else(|| format!("http_{}", status.as_u16()));
                    let retry_after_sec =
                        server_err.as_ref().and_then(|e| e.retry_after_sec).filter(|n| *n > 0);
                    let message = server_err
                        .as_ref()
                        .map(|e| e.error.clone())
                        .unwrap_or_else(|| format!("http {}", status.as_u16()));

                    let failure = NetworkFailure::ServerError {
                        status: status.as_u16(),
                        code: code.clone(),
                        retry_after_sec,
                        message,
                    };

                    if matches!(status.as_u16(), 429 | 503) && attempt < policy.max_attempts {
                        if let Some(cb) = &self.on_retry {
                            cb(status.as_u16() as i32, attempt, policy.max_attempts);
                        }
                        let wait = retry_after_sec
                            .map(|s| Duration::from_secs(s as u64))
                            .unwrap_or_else(|| backoff_delay(&policy, attempt));
                        sleep_with_jitter(wait).await;
                        last_err = failure;
                        continue;
                    }
                    return Err(failure);
                }
            }
        }
        Err(last_err)
    }
}

fn backoff_delay(policy: &RetryPolicy, attempt: u32) -> Duration {
    let exp = policy
        .base_delay
        .saturating_mul(1u32 << (attempt - 1).min(20));
    exp.min(policy.max_delay)
}

async fn sleep_with_jitter(base: Duration) {
    let jitter_max_ms = (base.as_millis() as u64) / 5; // 20%
    let jitter_ms = if jitter_max_ms == 0 {
        0
    } else {
        rand::thread_rng().gen_range(0..=jitter_max_ms)
    };
    tokio::time::sleep(base + Duration::from_millis(jitter_ms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_uses_default_when_env_unset() {
        let prev = std::env::var(ACTIVATION_BASE_URL_ENV).ok();
        std::env::remove_var(ACTIVATION_BASE_URL_ENV);
        let client = ActivationHttpClient::from_env().expect("default base URL");
        assert_eq!(client.base_url(), DEFAULT_ACTIVATION_BASE_URL);
        if let Some(v) = prev {
            std::env::set_var(ACTIVATION_BASE_URL_ENV, v);
        }
    }

    #[test]
    fn from_env_respects_explicit_override() {
        let prev = std::env::var(ACTIVATION_BASE_URL_ENV).ok();
        std::env::set_var(ACTIVATION_BASE_URL_ENV, "https://example.test/api/");
        let client = ActivationHttpClient::from_env().expect("override");
        assert_eq!(client.base_url(), "https://example.test/api");
        match prev {
            Some(v) => std::env::set_var(ACTIVATION_BASE_URL_ENV, v),
            None => std::env::remove_var(ACTIVATION_BASE_URL_ENV),
        }
    }

    #[test]
    fn backoff_grows_then_caps() {
        let p = RetryPolicy::uclaw_default();
        assert_eq!(backoff_delay(&p, 1), Duration::from_millis(500));
        assert_eq!(backoff_delay(&p, 2), Duration::from_millis(1000));
        assert_eq!(backoff_delay(&p, 3), Duration::from_millis(2000));
        assert_eq!(backoff_delay(&p, 4), Duration::from_millis(4000));
        assert_eq!(backoff_delay(&p, 5), Duration::from_millis(4000));
    }
}
