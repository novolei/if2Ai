//! Channel adapter trait definition.
//!
//! Defines the `ChannelAdapter` trait that each platform (Telegram, Feishu, QQ,
//! etc.) must implement. The trait abstracts over the specific protocol details
//! and provides a uniform interface for the `ChannelManager`.
//!
//! Per ADR-014 Section 18.1, this is the core abstraction for dynamic channel
//! start/stop lifecycle management.

use std::sync::Arc;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::modules::config::ChannelConfig;
use crate::modules::provider::types::TestResult;

use super::types::{ChannelEnvelope, ConnectMode, PlatformConfig};

/// Message handler callback type used by the manager.
pub type MessageHandler = Arc<dyn Fn(ChannelEnvelope) + Send + Sync>;

/// Channel adapter trait — each platform implements this for its specific protocol.
///
/// # Implementors
///
/// | Platform   | ConnectMode | Test Endpoint                                |
/// |------------|-------------|--------------------------------------------|
/// | Telegram   | Polling     | `GET /bot{token}/getMe`                    |
/// | Feishu     | Polling     | `POST /auth/v3/tenant_access_token/internal` |
/// | QQ         | Polling     | `POST /app/getAppAccessToken` + `GET /users/@me` |
/// | WhatsApp   | Webhook     | `GET /v17.0/{phone_id}`                    |
/// | MS Teams   | Webhook     | `POST /oauth2/v2.0/token`                  |
///
/// # Thread Safety
///
/// `ChannelAdapter` requires `Send + Sync` because it is stored in a
/// `DashMap` and may be accessed concurrently by the `ChannelManager`.
///
/// # Object Safety
///
/// `run_polling` uses `MessageHandler` (a concrete type alias) instead of
/// a generic `F` parameter to maintain dyn compatibility for `Arc<dyn ChannelAdapter>`.
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Platform unique identifier, e.g. "telegram", "feishu".
    fn platform_id(&self) -> &str;

    /// Connection mode — determines how messages are received.
    fn connect_mode(&self) -> ConnectMode;

    /// Real connection test with the given credentials.
    ///
    /// This must perform an actual API call, not return a mock success.
    /// The `TestResult` includes latency and error details.
    fn test_connection(&self, config: &PlatformConfig) -> Result<TestResult, String>;

    /// Build a `ChannelConfig` from raw credentials JSON.
    ///
    /// Used during onboarding when the user submits a channel configuration
    /// form. The adapter parses the JSON and extracts the relevant fields.
    fn build_config(&self, creds: serde_json::Value) -> Result<ChannelConfig, String>;

    /// Run polling loop, invoking the `MessageHandler` for each received message.
    ///
    /// Only required for adapters with `ConnectMode::Polling`.
    /// The `cancel` token is used to gracefully stop the polling loop.
    ///
    /// # Arguments
    ///
    /// * `on_message` - Callback invoked for each received message envelope
    /// * `cancel` - `CancellationToken` used to stop polling (e.g., on app shutdown)
    async fn run_polling(&self, on_message: MessageHandler, cancel: CancellationToken);
}

/// Type alias for shared adapter instances.
pub type AdapterRef = Arc<dyn ChannelAdapter>;

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock adapter for testing the trait contract.
    struct MockAdapter {
        id: String,
        mode: ConnectMode,
    }

    #[async_trait]
    impl ChannelAdapter for MockAdapter {
        fn platform_id(&self) -> &str {
            &self.id
        }

        fn connect_mode(&self) -> ConnectMode {
            self.mode.clone()
        }

        fn test_connection(&self, _config: &PlatformConfig) -> Result<TestResult, String> {
            Ok(TestResult {
                success: true,
                message: "Mock connection test passed".to_string(),
                latency_ms: Some(1),
                details: None,
            })
        }

        fn build_config(&self, _creds: serde_json::Value) -> Result<ChannelConfig, String> {
            Ok(ChannelConfig {
                channel_id: self.id.clone(),
                display_name: format!("Mock {id}", id = self.id),
                bot_token: None,
                app_secret: None,
                webhook_url: None,
            })
        }

        async fn run_polling(&self, _on_message: MessageHandler, cancel: CancellationToken) {
            // Immediately check cancellation so the test doesn't block
            cancel.cancelled().await;
        }
    }

    #[test]
    fn test_mock_adapter_platform_id() {
        let adapter = MockAdapter {
            id: "test_platform".to_string(),
            mode: ConnectMode::Polling,
        };
        assert_eq!(adapter.platform_id(), "test_platform");
    }

    #[test]
    fn test_mock_adapter_connect_mode() {
        let polling = MockAdapter {
            id: "polling".to_string(),
            mode: ConnectMode::Polling,
        };
        assert_eq!(polling.connect_mode(), ConnectMode::Polling);

        let webhook = MockAdapter {
            id: "webhook".to_string(),
            mode: ConnectMode::Webhook,
        };
        assert_eq!(webhook.connect_mode(), ConnectMode::Webhook);
    }

    #[test]
    fn test_mock_adapter_test_connection() {
        let adapter = MockAdapter {
            id: "test".to_string(),
            mode: ConnectMode::Polling,
        };
        let config = PlatformConfig::default();
        let result = adapter.test_connection(&config).unwrap();
        assert!(result.success);
        assert!(result.latency_ms.is_some());
    }

    #[test]
    fn test_mock_adapter_build_config() {
        let adapter = MockAdapter {
            id: "test_channel".to_string(),
            mode: ConnectMode::Polling,
        };
        let creds = serde_json::json!({ "token": "abc123" });
        let config = adapter.build_config(creds).unwrap();
        assert_eq!(config.channel_id, "test_channel");
    }

    #[tokio::test]
    async fn test_mock_adapter_run_polling_respects_cancellation() {
        let adapter = MockAdapter {
            id: "cancel_test".to_string(),
            mode: ConnectMode::Polling,
        };

        let token = CancellationToken::new();
        // Cancel immediately — the mock should exit promptly
        token.cancel();

        // Should complete without hanging
        adapter.run_polling(Arc::new(|_envelope| {}), token).await;
    }

    #[test]
    fn test_adapter_ref_type_alias() {
        let adapter: AdapterRef = Arc::new(MockAdapter {
            id: "arc_test".to_string(),
            mode: ConnectMode::Polling,
        });
        assert_eq!(adapter.platform_id(), "arc_test");
    }
}
