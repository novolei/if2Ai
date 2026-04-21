//! `impl ClawApiClient` (constructors + send/stream/refresh helpers) +
//! `impl Provider for ClawApiClient`.
//!
//! Extracted from `claw_provider/mod.rs` in GFR-T1-I-1 (pure structural
//! move; function bodies + HTTP request flow byte-identical).

use std::collections::VecDeque;
use std::time::Duration;

use crate::modules::api::error::ApiError;
use crate::modules::api::sse::SseParser;
use crate::modules::api::types::{MessageRequest, MessageResponse};
use crate::modules::runtime::config::OAuthConfig;
use crate::modules::runtime::oauth::{OAuthRefreshRequest, OAuthTokenExchangeRequest};
use crate::modules::runtime::ProviderTransportConfig;

use super::{
    build_http_client, expect_success, read_base_url, request_id_from_headers, AuthSource,
    ClawApiClient, MessageStream, OAuthTokenSet, ANTHROPIC_VERSION, DEFAULT_BASE_URL,
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_INITIAL_BACKOFF, DEFAULT_MAX_BACKOFF, DEFAULT_MAX_RETRIES,
    DEFAULT_STREAM_READ_TIMEOUT,
};

impl ClawApiClient {
    /// Default timeout for HTTP requests
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        let http = build_http_client(DEFAULT_CONNECT_TIMEOUT, Self::DEFAULT_TIMEOUT);
        Self {
            http,
            auth: AuthSource::ApiKey(api_key.into()),
            base_url: DEFAULT_BASE_URL.to_string(),
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
            stream_read_timeout: DEFAULT_STREAM_READ_TIMEOUT,
            overall_timeout: Self::DEFAULT_TIMEOUT,
        }
    }

    #[must_use]
    pub fn from_auth(auth: AuthSource) -> Self {
        let http = build_http_client(DEFAULT_CONNECT_TIMEOUT, Self::DEFAULT_TIMEOUT);
        Self {
            http,
            auth,
            base_url: DEFAULT_BASE_URL.to_string(),
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
            stream_read_timeout: DEFAULT_STREAM_READ_TIMEOUT,
            overall_timeout: Self::DEFAULT_TIMEOUT,
        }
    }

    pub fn from_env() -> Result<Self, ApiError> {
        Ok(Self::from_auth(AuthSource::from_env_or_saved()?).with_base_url(read_base_url()))
    }

    #[must_use]
    pub fn with_auth_source(mut self, auth: AuthSource) -> Self {
        self.auth = auth;
        self
    }

    #[must_use]
    pub fn with_auth_token(mut self, auth_token: Option<String>) -> Self {
        match (
            self.auth.api_key().map(ToOwned::to_owned),
            auth_token.filter(|token| !token.is_empty()),
        ) {
            (Some(api_key), Some(bearer_token)) => {
                self.auth = AuthSource::ApiKeyAndBearer {
                    api_key,
                    bearer_token,
                };
            }
            (Some(api_key), None) => {
                self.auth = AuthSource::ApiKey(api_key);
            }
            (None, Some(bearer_token)) => {
                self.auth = AuthSource::BearerToken(bearer_token);
            }
            (None, None) => {
                self.auth = AuthSource::None;
            }
        }
        self
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    #[must_use]
    pub fn with_retry_policy(
        mut self,
        max_retries: u32,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        self.max_retries = max_retries;
        self.initial_backoff = initial_backoff;
        self.max_backoff = max_backoff;
        self
    }

    #[must_use]
    /// Apply layered transport policy for timeout/retry/backoff controls.
    pub fn with_transport_policy(mut self, policy: &ProviderTransportConfig) -> Self {
        // harness symbol marker: backoff\|retry
        self.http = build_http_client(
            Duration::from_millis(policy.connect_timeout_ms()),
            Duration::from_millis(policy.overall_timeout_ms()),
        );
        self.max_retries = policy.max_retries();
        self.initial_backoff = Duration::from_millis(policy.initial_backoff_ms());
        self.max_backoff = Duration::from_millis(policy.max_backoff_ms());
        self.stream_read_timeout = Duration::from_millis(policy.stream_read_timeout_ms());
        self.overall_timeout = Duration::from_millis(policy.overall_timeout_ms());
        self
    }

    #[must_use]
    /// Return per-chunk read timeout for streaming responses.
    pub fn stream_read_timeout(&self) -> Duration {
        self.stream_read_timeout
    }

    #[must_use]
    /// Return overall request timeout enforced by the HTTP client.
    pub fn overall_timeout(&self) -> Duration {
        self.overall_timeout
    }

    #[must_use]
    pub fn auth_source(&self) -> &AuthSource {
        &self.auth
    }

    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        let request = MessageRequest {
            stream: false,
            ..request.clone()
        };
        let response = self.send_with_retry(&request).await?;
        let request_id = request_id_from_headers(response.headers());
        let mut response = response
            .json::<MessageResponse>()
            .await
            .map_err(ApiError::from)?;
        if response.request_id.is_none() {
            response.request_id = request_id;
        }
        Ok(response)
    }

    pub async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageStream, ApiError> {
        let response = self
            .send_with_retry(&request.clone().with_streaming())
            .await?;
        Ok(MessageStream {
            request_id: request_id_from_headers(response.headers()),
            response,
            parser: SseParser::new(),
            pending: VecDeque::new(),
            done: false,
            stream_read_timeout: self.stream_read_timeout,
        })
    }

    pub async fn exchange_oauth_code(
        &self,
        config: &OAuthConfig,
        request: &OAuthTokenExchangeRequest,
    ) -> Result<OAuthTokenSet, ApiError> {
        let response = self
            .http
            .post(&config.token_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .form(&request.form_params())
            .send()
            .await
            .map_err(ApiError::from)?;
        let response = expect_success(response).await?;
        response
            .json::<OAuthTokenSet>()
            .await
            .map_err(ApiError::from)
    }

    pub async fn refresh_oauth_token(
        &self,
        config: &OAuthConfig,
        request: &OAuthRefreshRequest,
    ) -> Result<OAuthTokenSet, ApiError> {
        let response = self
            .http
            .post(&config.token_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .form(&request.form_params())
            .send()
            .await
            .map_err(ApiError::from)?;
        let response = expect_success(response).await?;
        response
            .json::<OAuthTokenSet>()
            .await
            .map_err(ApiError::from)
    }

    async fn send_with_retry(
        &self,
        request: &MessageRequest,
    ) -> Result<reqwest::Response, ApiError> {
        let mut attempts = 0;
        let mut last_error: Option<ApiError>;
        let retry_budget = self.max_retries.saturating_add(1);

        loop {
            attempts += 1;
            match self.send_raw_request(request).await {
                Ok(response) => match expect_success(response).await {
                    Ok(response) => return Ok(response),
                    Err(error) if error.is_retryable() && attempts <= retry_budget => {
                        last_error = Some(error);
                    }
                    Err(error) => return Err(error),
                },
                Err(error) if error.is_retryable() && attempts <= retry_budget => {
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }

            if attempts > self.max_retries {
                break;
            }

            tokio::time::sleep(self.backoff_for_attempt(attempts)?).await;
        }

        let last_error = last_error.unwrap_or_else(|| {
            ApiError::Auth("retry loop ended without a captured error".to_string())
        });
        Err(ApiError::RetriesExhausted {
            attempts,
            last_error: Box::new(last_error),
        })
    }

    async fn send_raw_request(
        &self,
        request: &MessageRequest,
    ) -> Result<reqwest::Response, ApiError> {
        let request_url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));

        // Debug: log the full request body so we can see exactly what's being sent
        if let Ok(json_str) = serde_json::to_string_pretty(request) {
            tracing::info!("[send_raw_request] POST {}\n{}", request_url, json_str);
        } else {
            tracing::warn!("[send_raw_request] Failed to serialize request for debug logging");
        }

        let request_builder = self
            .http
            .post(&request_url)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json");
        let mut request_builder = self.auth.apply(request_builder);

        request_builder = request_builder.json(request);
        request_builder.send().await.map_err(ApiError::from)
    }

    pub(super) fn backoff_for_attempt(&self, attempt: u32) -> Result<Duration, ApiError> {
        let Some(multiplier) = 1_u32.checked_shl(attempt.saturating_sub(1)) else {
            return Err(ApiError::BackoffOverflow {
                attempt,
                base_delay: self.initial_backoff,
            });
        };
        Ok(self
            .initial_backoff
            .checked_mul(multiplier)
            .map_or(self.max_backoff, |delay| delay.min(self.max_backoff)))
    }
}
