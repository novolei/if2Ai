//! Channel connection test service — verifies platform connectivity.
//!
//! Provides `test_channel_connection()` which attempts a real API call
//! to each supported platform to validate credentials before the user
//! proceeds with onboarding Step 5.
//!
//! Supported platforms:
//! | Platform   | Test Endpoint                                                |
//! |------------|--------------------------------------------------------------|
//! | Telegram   | `GET /bot{token}/getMe`                                      |
//! | Feishu     | `POST /auth/v3/tenant_access_token/internal`                 |
//! | QQ         | `POST /app/getAppAccessToken` + `GET /users/@me`             |
//! | WhatsApp   | `GET graph.facebook.com/v17.0/{phone_number_id}`            |
//! | MS Teams   | `POST login.microsoftonline.com/botframework.com/oauth2/v2.0/token` |
//!
//! Reference: UClaw `api/routes/channels.rs:test_platform`

use std::time::Instant;

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use super::types::PlatformConfig;
use crate::modules::provider::types::TestResult;

// ── Constants ───────────────────────────────────────────────────────────────

/// Timeout for channel connection tests (seconds).
const TEST_TIMEOUT_SECS: u64 = 10;

/// Channel-level error codes.
mod error_codes {
    pub const CHANNEL_CONFIG_INVALID: &str = "channel_config_invalid";
    pub const CHANNEL_AUTH_ERROR: &str = "channel_auth_error";
    pub const CHANNEL_TIMEOUT: &str = "channel_timeout";
    pub const CHANNEL_NETWORK_ERROR: &str = "channel_network_error";
    pub const CHANNEL_UPSTREAM_UNAVAILABLE: &str = "channel_upstream_unavailable";
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Test a channel connection by dispatching to the platform-specific test.
///
/// # Arguments
///
/// * `platform` — Platform identifier (e.g. "telegram", "feishu", "qq")
/// * `config` — Platform credentials to test
///
/// # Returns
///
/// A `TestResult` with success status, latency, and any error details.
pub async fn test_channel_connection(platform: &str, config: &PlatformConfig) -> TestResult {
    let start = Instant::now();

    let result = match platform {
        "telegram" => test_telegram(config).await,
        "feishu" => test_feishu(config).await,
        "qq" => test_qq(config).await,
        "whatsapp" => test_whatsapp(config).await,
        "teams" => test_msteams(config).await,
        other => Err(TestError {
            code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
            message: format!("Unsupported platform: {}", other),
        }),
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

/// Validate that a platform config has at least one credential set.
fn validate_config(platform: &str, config: &PlatformConfig) -> Option<TestError> {
    if !config.has_credentials() {
        return Some(TestError {
            code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
            message: format!(
                "No credentials configured for {}. At least one credential field is required.",
                platform
            ),
        });
    }

    // Platform-specific validation
    match platform {
        "telegram" if config.bot_token.is_none() => Some(TestError {
            code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
            message: "Bot token is required for Telegram".to_string(),
        }),
        "feishu" if config.app_id.is_none() || config.app_secret.is_none() => Some(TestError {
            code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
            message: "App ID and App Secret are required for Feishu".to_string(),
        }),
        "qq" if config.app_id.is_none() || config.app_secret.is_none() => Some(TestError {
            code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
            message: "App ID and App Secret are required for QQ".to_string(),
        }),
        "whatsapp" if config.phone_number_id.is_none() => Some(TestError {
            code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
            message: "Phone Number ID is required for WhatsApp".to_string(),
        }),
        "teams" if config.app_id_teams.is_none() || config.app_password.is_none() => {
            Some(TestError {
                code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
                message: "App ID and App Password are required for MS Teams".to_string(),
            })
        }
        _ => None,
    }
}

// ── Telegram ────────────────────────────────────────────────────────────────

/// Test Telegram by calling `GET /bot{token}/getMe`.
///
/// Returns the bot's display name on success.
async fn test_telegram(config: &PlatformConfig) -> Result<String, TestError> {
    let token = config.bot_token.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "Bot token is required for Telegram".to_string(),
    })?;

    let client = build_client()?;
    let url = format!("https://api.telegram.org/bot{}/getMe", redact_token(token));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| classify_error(&e))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Failed to read response: {}", e),
    })?;

    if !status.is_success() {
        return Err(TestError {
            code: error_codes::CHANNEL_AUTH_ERROR.to_string(),
            message: format!("Telegram API returned {}: {}", status, truncate(&body, 200)),
        });
    }

    // Parse response to extract bot name
    #[derive(Deserialize)]
    struct TelegramGetMe {
        ok: bool,
        result: Option<TelegramBotInfo>,
        description: Option<String>,
    }

    #[derive(Deserialize)]
    struct TelegramBotInfo {
        #[serde(alias = "first_name")]
        name: Option<String>,
        username: Option<String>,
    }

    let parsed: TelegramGetMe = serde_json::from_str(&body).map_err(|_| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Invalid Telegram response: {}", truncate(&body, 200)),
    })?;

    if !parsed.ok {
        return Err(TestError {
            code: error_codes::CHANNEL_AUTH_ERROR.to_string(),
            message: parsed
                .description
                .unwrap_or_else(|| "Telegram authentication failed".to_string()),
        });
    }

    let bot_name = parsed
        .result
        .and_then(|r| r.name.or(r.username))
        .unwrap_or_else(|| "bot".to_string());

    Ok(format!("Connected to Telegram bot: {}", bot_name))
}

// ── Feishu ──────────────────────────────────────────────────────────────────

/// Test Feishu by calling
/// `POST /open-apis/auth/v3/tenant_access_token/internal`.
///
/// Returns the tenant access token (truncated) on success.
async fn test_feishu(config: &PlatformConfig) -> Result<String, TestError> {
    let app_id = config.app_id.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "App ID is required for Feishu".to_string(),
    })?;
    let app_secret = config.app_secret.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "App Secret is required for Feishu".to_string(),
    })?;

    let client = build_client()?;
    let url = "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal";

    let resp = client
        .post(url)
        .json(&json!({
            "app_id": app_id,
            "app_secret": app_secret,
        }))
        .send()
        .await
        .map_err(|e| classify_error(&e))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Failed to read response: {}", e),
    })?;

    if !status.is_success() {
        return Err(TestError {
            code: error_codes::CHANNEL_UPSTREAM_UNAVAILABLE.to_string(),
            message: format!("Feishu API returned {}: {}", status, truncate(&body, 200)),
        });
    }

    #[derive(Deserialize)]
    struct FeishuTokenResp {
        code: i64,
        #[serde(rename = "tenant_access_token")]
        token: Option<String>,
        msg: Option<String>,
    }

    let parsed: FeishuTokenResp = serde_json::from_str(&body).map_err(|_| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Invalid Feishu response: {}", truncate(&body, 200)),
    })?;

    if parsed.code != 0 {
        return Err(TestError {
            code: error_codes::CHANNEL_AUTH_ERROR.to_string(),
            message: parsed
                .msg
                .unwrap_or_else(|| format!("Feishu auth failed with code {}", parsed.code)),
        });
    }

    let token_preview = parsed
        .token
        .map(|t| format!("{}...", &t[..10.min(t.len())]))
        .unwrap_or_else(|| "N/A".to_string());

    Ok(format!(
        "Connected to Feishu. Access token: {}",
        token_preview
    ))
}

// ── QQ ──────────────────────────────────────────────────────────────────────

/// Test QQ by calling `POST /app/getAppAccessToken` then `GET /users/@me`.
///
/// Returns the bot username on success.
async fn test_qq(config: &PlatformConfig) -> Result<String, TestError> {
    let app_id = config.app_id.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "App ID is required for QQ".to_string(),
    })?;
    let app_secret = config.app_secret.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "App Secret is required for QQ".to_string(),
    })?;

    // Step 1: Get access token
    let client = build_client()?;
    let token_url = "https://bots.qq.com/app/getAppAccessToken";

    let resp = client
        .post(token_url)
        .json(&json!({
            "appId": app_id,
            "clientSecret": app_secret,
        }))
        .send()
        .await
        .map_err(|e| classify_error(&e))?;

    let body = resp.text().await.map_err(|e| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Failed to read QQ token response: {}", e),
    })?;

    #[derive(Deserialize)]
    struct QQTokenResp {
        #[serde(rename = "access_token")]
        token: Option<String>,
    }

    let token_resp: QQTokenResp = serde_json::from_str(&body).map_err(|_| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Invalid QQ token response: {}", truncate(&body, 200)),
    })?;

    let access_token = token_resp.token.ok_or_else(|| TestError {
        code: error_codes::CHANNEL_AUTH_ERROR.to_string(),
        message: "QQ did not return an access token".to_string(),
    })?;

    // Step 2: Verify with /users/@me
    let me_url = "https://api.sgroup.qq.com/users/@me";
    let resp = client
        .get(me_url)
        .header("Authorization", format!("QQBot {}", access_token))
        .send()
        .await
        .map_err(|e| classify_error(&e))?;

    let me_body = resp.text().await.map_err(|e| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Failed to read QQ @me response: {}", e),
    })?;

    #[derive(Deserialize)]
    struct QQMeResp {
        username: Option<String>,
    }

    let me_resp: QQMeResp = serde_json::from_str(&me_body).map_err(|_| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Invalid QQ @me response: {}", truncate(&me_body, 200)),
    })?;

    let username = me_resp.username.unwrap_or_else(|| "bot".to_string());
    Ok(format!("Connected to QQ bot: @{}", username))
}

// ── WhatsApp ────────────────────────────────────────────────────────────────

/// Test WhatsApp by calling `GET graph.facebook.com/v17.0/{phone_number_id}`.
///
/// Returns the phone number display name on success.
async fn test_whatsapp(config: &PlatformConfig) -> Result<String, TestError> {
    let phone_id = config.phone_number_id.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "Phone Number ID is required for WhatsApp".to_string(),
    })?;
    let access_token = config.access_token.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "Access Token is required for WhatsApp".to_string(),
    })?;

    let client = build_client()?;
    let url = format!("https://graph.facebook.com/v17.0/{}", phone_id);

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| classify_error(&e))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Failed to read response: {}", e),
    })?;

    if !status.is_success() {
        return Err(TestError {
            code: error_codes::CHANNEL_AUTH_ERROR.to_string(),
            message: format!("WhatsApp API returned {}: {}", status, truncate(&body, 200)),
        });
    }

    #[derive(Deserialize)]
    struct WhatsAppResp {
        #[serde(rename = "display_phone_number")]
        phone: Option<String>,
    }

    let parsed: WhatsAppResp = serde_json::from_str(&body).map_err(|_| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Invalid WhatsApp response: {}", truncate(&body, 200)),
    })?;

    let phone = parsed.phone.unwrap_or_else(|| phone_id.clone());
    Ok(format!("Connected to WhatsApp phone: {}", phone))
}

// ── MS Teams ────────────────────────────────────────────────────────────────

/// Test MS Teams by calling
/// `POST login.microsoftonline.com/botframework.com/oauth2/v2.0/token`.
///
/// Returns a token preview on success.
async fn test_msteams(config: &PlatformConfig) -> Result<String, TestError> {
    let app_id = config.app_id_teams.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "App ID is required for MS Teams".to_string(),
    })?;
    let app_password = config.app_password.as_ref().ok_or_else(|| TestError {
        code: error_codes::CHANNEL_CONFIG_INVALID.to_string(),
        message: "App Password is required for MS Teams".to_string(),
    })?;

    let client = build_client()?;
    let url = "https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token";

    let resp = client
        .post(url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("scope", "https://api.botframework.com/.default"),
            ("client_id", app_id.as_str()),
            ("client_secret", app_password.as_str()),
        ])
        .send()
        .await
        .map_err(|e| classify_error(&e))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Failed to read response: {}", e),
    })?;

    if !status.is_success() {
        return Err(TestError {
            code: error_codes::CHANNEL_AUTH_ERROR.to_string(),
            message: format!(
                "MS Teams OAuth returned {}: {}",
                status,
                truncate(&body, 200)
            ),
        });
    }

    #[derive(Deserialize)]
    struct TeamsTokenResp {
        access_token: Option<String>,
        expires_in: Option<u64>,
    }

    let parsed: TeamsTokenResp = serde_json::from_str(&body).map_err(|_| TestError {
        code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
        message: format!("Invalid Teams OAuth response: {}", truncate(&body, 200)),
    })?;

    let token_preview = parsed
        .access_token
        .map(|t| format!("{}...", &t[..10.min(t.len())]))
        .unwrap_or_else(|| "N/A".to_string());
    let expires = parsed.expires_in.unwrap_or(0);

    Ok(format!(
        "Connected to MS Teams. Token: {}, expires in {}s",
        token_preview, expires
    ))
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build a reqwest Client with the standard test timeout.
fn build_client() -> Result<Client, TestError> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(TEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| TestError {
            code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
            message: format!("Failed to build HTTP client: {}", e),
        })
}

/// Classify a reqwest error into a channel error code.
fn classify_error(err: &reqwest::Error) -> TestError {
    if err.is_timeout() {
        TestError {
            code: error_codes::CHANNEL_TIMEOUT.to_string(),
            message: "Connection timed out".to_string(),
        }
    } else if err.is_connect() {
        TestError {
            code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
            message: format!("Connection failed: {}", err),
        }
    } else if err.is_request() {
        TestError {
            code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
            message: format!("Request failed: {}", err),
        }
    } else {
        TestError {
            code: error_codes::CHANNEL_NETWORK_ERROR.to_string(),
            message: format!("HTTP error: {}", err),
        }
    }
}

/// Redact a sensitive token for safe logging.
/// Shows only the first 6 and last 4 characters.
fn redact_token(token: &str) -> String {
    if token.len() <= 10 {
        "***".to_string()
    } else {
        format!("{}***{}", &token[..6], &token[token.len() - 4..])
    }
}

/// Truncate a string for display, appending "..." if it was longer.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Internal error type for channel tests.
struct TestError {
    code: String,
    message: String,
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_token_short() {
        let result = redact_token("abc");
        assert_eq!(result, "***");
    }

    #[test]
    fn test_redact_token_long() {
        let token = "123456:ABC-DEF1234ghIkl-zyx57W2v1uXyz";
        let result = redact_token(token);
        assert!(result.contains("***"));
        assert!(result.starts_with("123456"));
        assert!(result.ends_with("Xyz"));
        assert!(!result.contains("ABC-DEF"));
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_long() {
        let result = truncate("this is a very long string", 10);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 13); // 10 chars + 3 for "..."
    }

    #[test]
    fn test_validate_config_empty() {
        let config = PlatformConfig::default();
        let err = validate_config("telegram", &config);
        assert!(err.is_some());
        let err = err.unwrap();
        assert_eq!(err.code, error_codes::CHANNEL_CONFIG_INVALID);
    }

    #[test]
    fn test_validate_config_telegram_ok() {
        let config = PlatformConfig {
            bot_token: Some("123456:ABC".to_string()),
            ..Default::default()
        };
        assert!(validate_config("telegram", &config).is_none());
    }

    #[test]
    fn test_validate_config_feishu_ok() {
        let config = PlatformConfig {
            app_id: Some("cli_xxx".to_string()),
            app_secret: Some("secret".to_string()),
            ..Default::default()
        };
        assert!(validate_config("feishu", &config).is_none());
    }

    #[test]
    fn test_validate_config_feishu_missing_secret() {
        let config = PlatformConfig {
            app_id: Some("cli_xxx".to_string()),
            ..Default::default()
        };
        let err = validate_config("feishu", &config).unwrap();
        assert_eq!(err.code, error_codes::CHANNEL_CONFIG_INVALID);
    }

    #[test]
    fn test_validate_config_whatsapp_missing_phone() {
        let config = PlatformConfig {
            access_token: Some("token".to_string()),
            ..Default::default()
        };
        let err = validate_config("whatsapp", &config).unwrap();
        assert_eq!(err.code, error_codes::CHANNEL_CONFIG_INVALID);
    }

    #[test]
    fn test_validate_config_whatsapp_ok() {
        let config = PlatformConfig {
            phone_number_id: Some("12345".to_string()),
            access_token: Some("token".to_string()),
            ..Default::default()
        };
        assert!(validate_config("whatsapp", &config).is_none());
    }

    #[test]
    fn test_validate_config_teams_ok() {
        let config = PlatformConfig {
            app_id_teams: Some("app-id".to_string()),
            app_password: Some("password".to_string()),
            ..Default::default()
        };
        assert!(validate_config("teams", &config).is_none());
    }

    #[tokio::test]
    async fn test_unsupported_platform() {
        let config = PlatformConfig {
            bot_token: Some("token".to_string()),
            ..Default::default()
        };
        let result = test_channel_connection("unsupported", &config).await;
        assert!(!result.success);
        assert!(result.message.contains("Unsupported platform"));
    }

    #[test]
    fn test_error_codes_constants() {
        assert_eq!(
            error_codes::CHANNEL_CONFIG_INVALID,
            "channel_config_invalid"
        );
        assert_eq!(error_codes::CHANNEL_AUTH_ERROR, "channel_auth_error");
        assert_eq!(error_codes::CHANNEL_TIMEOUT, "channel_timeout");
        assert_eq!(error_codes::CHANNEL_NETWORK_ERROR, "channel_network_error");
        assert_eq!(
            error_codes::CHANNEL_UPSTREAM_UNAVAILABLE,
            "channel_upstream_unavailable"
        );
    }
}
