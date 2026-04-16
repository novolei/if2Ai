//! Provider connection testing service.
//!
//! Implements `test_provider_connection()` which validates that a provider's
//! API endpoint is reachable with the given credentials.
//!
//! Supports three protocol modes:
//! - **Ollama**: `GET {base_url}/api/tags` (local, no auth)
//! - **Anthropic-compatible**: `GET {base_url}/v1/messages` test
//! - **OpenAI-compatible**: `GET {base_url}/models` with Bearer token
//!
//! Error codes (from `types::error_codes`):
//! - `provider_config_invalid`: Missing required fields
//! - `provider_auth_error`: Invalid API key
//! - `provider_timeout`: Connection timed out
//! - `provider_network_error`: DNS failure, connection refused, etc.
//! - `provider_upstream_unavailable`: Provider returned 503/502

use std::time::Instant;

use reqwest::Client;

use super::types::{error_codes, TestResult};

/// Timeout for provider connection tests (seconds).
const TEST_TIMEOUT_SECS: u64 = 10;

/// Test a provider connection.
///
/// # Arguments
///
/// * `provider_id` - Provider identifier (e.g. "openai", "anthropic", "ollama")
/// * `base_url` - API base URL
/// * `api_key` - API key (may be `None` for local providers)
///
/// # Returns
///
/// A `TestResult` with success status, latency, and any error details.
pub async fn test_provider_connection(
    provider_id: &str,
    base_url: &str,
    api_key: Option<&str>,
) -> TestResult {
    // Validate config first
    if let Some(err) = validate_config(provider_id, base_url, api_key) {
        return err;
    }

    let start = Instant::now();

    let result = match provider_id {
        "ollama" => test_ollama(base_url).await,
        "anthropic" => {
            // validate_config guarantees api_key is Some for non-ollama
            let key = api_key.expect("api_key validated above");
            test_anthropic(base_url, key).await
        }
        _ => {
            let key = api_key.expect("api_key validated above");
            test_openai_compat(base_url, key).await
        }
    };

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(msg) => TestResult {
            success: true,
            message: msg,
            latency_ms: Some(latency_ms),
            details: None,
        },
        Err(err) => TestResult {
            success: false,
            message: err.message,
            latency_ms: Some(latency_ms),
            details: Some(err.code),
        },
    }
}

struct TestError {
    code: String,
    message: String,
}

/// Validate provider configuration before attempting connection.
fn validate_config(provider_id: &str, base_url: &str, api_key: Option<&str>) -> Option<TestResult> {
    if base_url.is_empty() {
        return Some(TestResult {
            success: false,
            message: "Base URL is required".to_string(),
            latency_ms: None,
            details: Some(error_codes::PROVIDER_CONFIG_INVALID.to_string()),
        });
    }

    // Ollama doesn't need an API key
    if provider_id != "ollama" && api_key.is_none_or(|k| k.is_empty()) {
        return Some(TestResult {
            success: false,
            message: "API key is required".to_string(),
            latency_ms: None,
            details: Some(error_codes::PROVIDER_CONFIG_INVALID.to_string()),
        });
    }

    None
}

/// Test Ollama connection via `/api/tags`.
async fn test_ollama(base_url: &str) -> Result<String, TestError> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(TEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| TestError {
            code: error_codes::PROVIDER_NETWORK_ERROR.to_string(),
            message: format!("Failed to create HTTP client: {e}"),
        })?;

    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let response = client.get(&url).send().await.map_err(|e| {
        if e.is_timeout() {
            TestError {
                code: error_codes::PROVIDER_TIMEOUT.to_string(),
                message: "Ollama connection timed out".to_string(),
            }
        } else if e.is_connect() {
            TestError {
                code: error_codes::PROVIDER_NETWORK_ERROR.to_string(),
                message: format!("Cannot connect to Ollama at {base_url}: {e}"),
            }
        } else {
            TestError {
                code: error_codes::PROVIDER_NETWORK_ERROR.to_string(),
                message: format!("Ollama connection failed: {e}"),
            }
        }
    })?;

    let status = response.status();
    if status.is_success() {
        Ok("Ollama is running locally".to_string())
    } else {
        handle_http_error(status, "Ollama")
    }
}

/// Test Anthropic connection via `/v1/messages` with a minimal request.
async fn test_anthropic(base_url: &str, api_key: &str) -> Result<String, TestError> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(TEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| TestError {
            code: error_codes::PROVIDER_NETWORK_ERROR.to_string(),
            message: format!("Failed to create HTTP client: {e}"),
        })?;

    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/v1/messages");

    // Minimal valid request to test auth
    let body = serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "Hi"}]
    });

    let response = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                TestError {
                    code: error_codes::PROVIDER_TIMEOUT.to_string(),
                    message: "Anthropic connection timed out".to_string(),
                }
            } else {
                TestError {
                    code: error_codes::PROVIDER_NETWORK_ERROR.to_string(),
                    message: format!("Anthropic connection failed: {e}"),
                }
            }
        })?;

    let status = response.status();
    if status.is_success() {
        Ok("Anthropic connection verified".to_string())
    } else if status.as_u16() == 401 || status.as_u16() == 403 {
        Err(TestError {
            code: error_codes::PROVIDER_AUTH_ERROR.to_string(),
            message: "Invalid Anthropic API key".to_string(),
        })
    } else {
        handle_http_error(status, "Anthropic")
    }
}

/// Test OpenAI-compatible provider via `/models` endpoint.
async fn test_openai_compat(base_url: &str, api_key: &str) -> Result<String, TestError> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(TEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| TestError {
            code: error_codes::PROVIDER_NETWORK_ERROR.to_string(),
            message: format!("Failed to create HTTP client: {e}"),
        })?;

    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                TestError {
                    code: error_codes::PROVIDER_TIMEOUT.to_string(),
                    message: "Provider connection timed out".to_string(),
                }
            } else if e.is_connect() {
                TestError {
                    code: error_codes::PROVIDER_NETWORK_ERROR.to_string(),
                    message: format!("Cannot connect to provider: {e}"),
                }
            } else {
                TestError {
                    code: error_codes::PROVIDER_NETWORK_ERROR.to_string(),
                    message: format!("Provider connection failed: {e}"),
                }
            }
        })?;

    let status = response.status();
    if status.is_success() {
        Ok("Provider connection verified".to_string())
    } else if status.as_u16() == 401 || status.as_u16() == 403 {
        Err(TestError {
            code: error_codes::PROVIDER_AUTH_ERROR.to_string(),
            message: "Invalid API key".to_string(),
        })
    } else {
        handle_http_error(status, "Provider")
    }
}

/// Handle non-success HTTP status codes.
fn handle_http_error(status: reqwest::StatusCode, label: &str) -> Result<String, TestError> {
    let code = match status.as_u16() {
        502..=504 => error_codes::PROVIDER_UPSTREAM_UNAVAILABLE,
        401 | 403 => error_codes::PROVIDER_AUTH_ERROR,
        408 => error_codes::PROVIDER_TIMEOUT,
        _ => error_codes::PROVIDER_NETWORK_ERROR,
    };

    Err(TestError {
        code: code.to_string(),
        message: format!("{label} returned {status}"),
    })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_config_missing_base_url() {
        let result = test_provider_connection("openai", "", Some("sk-test")).await;
        assert!(!result.success);
        assert_eq!(
            result.details,
            Some(error_codes::PROVIDER_CONFIG_INVALID.to_string())
        );
    }

    #[tokio::test]
    async fn test_validate_config_missing_api_key() {
        let result = test_provider_connection("openai", "https://api.openai.com", None).await;
        assert!(!result.success);
        assert_eq!(
            result.details,
            Some(error_codes::PROVIDER_CONFIG_INVALID.to_string())
        );
    }

    #[tokio::test]
    async fn test_validate_config_ollama_no_api_key_needed() {
        // Ollama doesn't need an API key, so it should pass validation
        // and attempt a real connection (result depends on whether Ollama is running)
        let result = test_provider_connection("ollama", "http://localhost:11434", None).await;
        // Should not be a config error — should be either success or a network error
        assert_ne!(
            result.details,
            Some(error_codes::PROVIDER_CONFIG_INVALID.to_string()),
            "Ollama should not require API key"
        );
    }

    #[tokio::test]
    async fn test_validate_config_empty_api_key() {
        let result = test_provider_connection("openai", "https://api.openai.com", Some("")).await;
        assert!(!result.success);
        assert_eq!(
            result.details,
            Some(error_codes::PROVIDER_CONFIG_INVALID.to_string())
        );
    }
}
