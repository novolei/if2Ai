//! Streaming response wrapper + HTTP error handling.
//!
//! Extracted from `claw_provider/mod.rs` in GFR-T1-I-1 (pure structural
//! move; function bodies byte-identical).

use std::collections::VecDeque;
use std::time::Duration;

use serde::Deserialize;

use crate::modules::api::error::ApiError;
use crate::modules::api::sse::SseParser;
use crate::modules::api::types::StreamEvent;

#[derive(Debug)]
pub struct MessageStream {
    pub(super) request_id: Option<String>,
    pub(super) response: reqwest::Response,
    pub(super) parser: SseParser,
    pub(super) pending: VecDeque<StreamEvent>,
    pub(super) done: bool,
    pub(super) stream_read_timeout: Duration,
}

impl MessageStream {
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }

            if self.done {
                let remaining = self.parser.finish()?;
                self.pending.extend(remaining);
                if let Some(event) = self.pending.pop_front() {
                    return Ok(Some(event));
                }
                return Ok(None);
            }

            let chunk_result =
                tokio::time::timeout(self.stream_read_timeout, self.response.chunk())
                    .await
                    .map_err(|_| ApiError::Timeout("stream-event read timeout".to_string()))?;
            let next_chunk = chunk_result?;
            match next_chunk {
                Some(chunk) => {
                    self.pending.extend(self.parser.push(&chunk)?);
                }
                None => {
                    self.done = true;
                }
            }
        }
    }
}

pub(super) async fn expect_success(
    response: reqwest::Response,
) -> Result<reqwest::Response, ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let retry_after = parse_retry_after_header(response.headers());
    let body = response.text().await.unwrap_or_else(|_| String::new());
    let parsed_error = serde_json::from_str::<ApiErrorEnvelope>(&body).ok();
    let retryable = is_retryable_status(status);

    Err(ApiError::Api {
        status,
        error_type: parsed_error
            .as_ref()
            .map(|error| error.error.error_type.clone()),
        message: parsed_error
            .as_ref()
            .map(|error| error.error.message.clone()),
        body,
        retryable,
        retry_after,
    })
}

pub(super) fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 409 | 429 | 500 | 502 | 503 | 504)
}

/// Parse the `Retry-After` HTTP header into a [`Duration`].
///
/// Supports two formats per RFC 7231 §7.1.3:
/// - Seconds (e.g. `Retry-After: 30`)
/// - HTTP-date (e.g. `Retry-After: Thu, 01 Dec 2024 16:00:00 GMT`)
fn parse_retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    let trimmed = value.trim();
    // Try seconds first
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    // Try HTTP-date format via chrono
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(trimmed) {
        let now = chrono::Utc::now();
        let target = dt.with_timezone(&chrono::Utc);
        if target > now {
            let delta = (target - now).to_std().ok()?;
            return Some(delta);
        }
        return Some(Duration::ZERO);
    }
    None
}

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}
