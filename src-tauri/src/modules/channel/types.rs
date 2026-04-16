//! Channel type definitions.
//!
//! Defines the core types for the channel system (Onboarding Step 5):
//! - `Channel`: Registry entry describing a communication channel
//! - `ChannelCategory`: Social / Messaging / Desktop classification
//! - `ConnectMode`: Polling vs Webhook connection strategy
//! - `ChannelEnvelope`: Unified message format across platforms
//! - `SenderInfo`: Sender identification extracted from platform messages
//! - `PlatformConfig`: Adapter-level credentials (distinct from `ChannelConfig`)
//!
//! Note: `ChannelConfig`, `ChannelConfigRedacted`, and `ChannelRouting` are
//! defined in `crate::modules::config::types` as they are part of the unified
//! config model. They are re-exported via this module for convenience.

use serde::{Deserialize, Serialize};

// ── Channel Category ────────────────────────────────────────────────────────

/// Classification of communication channels.
///
/// | Category    | Examples                          |
/// |-------------|-----------------------------------|
/// | Social      | Feishu, QQ, WeChat, Telegram      |
/// | Messaging   | WhatsApp, Teams, Discord, Slack   |
/// | Desktop     | Signal, Mattermost, Matrix        |
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelCategory {
    /// Social messaging: Feishu, QQ, WeChat, Telegram
    Social,
    /// Messaging platforms: WhatsApp, Teams, Discord, Slack, iMessage, LINE
    Messaging,
    /// Desktop collaboration: Signal, Mattermost, Matrix
    Desktop,
}

impl ChannelCategory {
    /// Human-readable label for the category.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Social => "Social",
            Self::Messaging => "Messaging",
            Self::Desktop => "Desktop",
        }
    }
}

// ── Channel Registry Entry ──────────────────────────────────────────────────

/// Channel registry entry — describes an available communication channel.
///
/// Used by `builtin_channels()` to populate the Step 5 channel selection UI.
/// Distinct from `ChannelConfig` which stores user-provided credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Channel identifier, e.g. "feishu", "telegram", "qq"
    pub id: String,
    /// Display name shown in UI, e.g. "飞书", "Telegram (Bot API)"
    pub name: String,
    /// Category classification (Social / Messaging / Desktop)
    pub category: ChannelCategory,
    /// Icon identifier for UI rendering
    pub icon: String,
    /// Path to the channel's logo asset
    pub logo_path: Option<String>,
    /// Whether this channel requires a bot token
    pub requires_token: bool,
    /// Whether this channel requires an app secret
    pub requires_secret: bool,
    /// Whether this channel uses webhook-based communication
    pub requires_webhook: bool,
    /// Minimum Node.js version required, e.g. "18+"
    pub node_version_required: Option<String>,
}

// ── Connect Mode ────────────────────────────────────────────────────────────

/// Channel connection mode — how the platform receives messages.
///
/// | Mode    | Description                          | Examples              |
/// |---------|--------------------------------------|-----------------------|
/// | Polling | Periodically fetch from API          | Telegram, Feishu, QQ  |
/// | Webhook | Receive callbacks from platform      | Slack, Discord        |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectMode {
    /// Polling — periodically fetch messages from the platform API
    /// (e.g., Telegram getUpdates, Feishu polling)
    Polling,
    /// Webhook — receive HTTP callbacks from the platform
    /// (e.g., Slack Socket Mode, Discord Gateway)
    Webhook,
}

// ── Channel Envelope ────────────────────────────────────────────────────────

/// Unified message envelope — normalizes message format across all platforms.
///
/// Each `ChannelAdapter` converts its platform-specific message format
/// into this common structure before passing it to the message handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEnvelope {
    /// Platform ID that sent this message, e.g. "telegram", "feishu"
    pub channel: String,
    /// Session/chat identifier for this conversation
    pub chat_id: String,
    /// Information about the message sender
    pub sender: SenderInfo,
    /// Raw platform-specific message JSON for adapter-specific processing
    pub raw: serde_json::Value,
}

// ── Sender Info ─────────────────────────────────────────────────────────────

/// Sender identification extracted from platform messages.
///
/// Fields vary by platform — some may not provide all information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderInfo {
    /// Platform-specific sender ID
    pub id: String,
    /// Display name (may be derived from username or profile)
    pub display_name: String,
    /// Platform username handle, if available
    pub username: Option<String>,
}

// ── Platform Config ─────────────────────────────────────────────────────────

/// Platform-level credentials and connection settings used by `ChannelAdapter`.
///
/// This is distinct from `ChannelConfig` (which is the Onboarding-layer
/// configuration). `PlatformConfig` contains all possible credential fields
/// that various platform adapters may need.
#[derive(Debug, Clone, Default)]
pub struct PlatformConfig {
    /// Bot token for authentication (Telegram, Discord, etc.)
    pub bot_token: Option<String>,
    /// Application ID (Feishu, Teams, etc.)
    pub app_id: Option<String>,
    /// Application secret (Feishu, etc.)
    pub app_secret: Option<String>,
    /// Webhook secret for signature verification
    pub webhook_secret: Option<String>,
    /// Webhook URL for callback-based channels
    pub webhook_url: Option<String>,
    /// OAuth access token (WhatsApp, etc.)
    pub access_token: Option<String>,
    /// Phone number ID (WhatsApp Business API)
    pub phone_number_id: Option<String>,
    /// Teams application ID
    pub app_id_teams: Option<String>,
    /// Teams application password
    pub app_password: Option<String>,
    /// Bridge server URL (for platforms that need a relay)
    pub bridge_url: Option<String>,
    /// Bridge server secret
    pub bridge_secret: Option<String>,
    /// Encryption key (Feishu event encryption)
    pub encrypt_key: Option<String>,
    /// Verification token (Feishu, Slack, etc.)
    pub verification_token: Option<String>,
    /// Bot username for display and routing
    pub bot_username: Option<String>,
}

impl PlatformConfig {
    /// Check if this config has at least one credential set.
    #[must_use]
    pub fn has_credentials(&self) -> bool {
        self.bot_token.is_some()
            || self.app_id.is_some()
            || self.app_secret.is_some()
            || self.webhook_url.is_some()
            || self.access_token.is_some()
            || self.phone_number_id.is_some()
            || self.app_id_teams.is_some()
    }
}

// ── Test Result ─────────────────────────────────────────────────────────────
// Re-exported from provider module to avoid duplication.

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_category_label() {
        assert_eq!(ChannelCategory::Social.label(), "Social");
        assert_eq!(ChannelCategory::Messaging.label(), "Messaging");
        assert_eq!(ChannelCategory::Desktop.label(), "Desktop");
    }

    #[test]
    fn test_channel_serialization() {
        let channel = Channel {
            id: "telegram".to_string(),
            name: "Telegram (Bot API)".to_string(),
            category: ChannelCategory::Social,
            icon: "send".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_telegram.png".to_string()),
            requires_token: true,
            requires_secret: false,
            requires_webhook: false,
            node_version_required: None,
        };

        let json = serde_json::to_string(&channel).unwrap();
        assert!(json.contains("telegram"));
        assert!(json.contains("social"));

        let restored: Channel = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "telegram");
        assert_eq!(restored.category, ChannelCategory::Social);
    }

    #[test]
    fn test_connect_mode_serialization() {
        let polling = ConnectMode::Polling;
        let json = serde_json::to_string(&polling).unwrap();
        assert!(json.contains("polling"));

        let restored: ConnectMode = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, ConnectMode::Polling);
    }

    #[test]
    fn test_platform_config_has_credentials() {
        let empty = PlatformConfig::default();
        assert!(!empty.has_credentials());

        let with_token = PlatformConfig {
            bot_token: Some("123456:ABC".to_string()),
            ..Default::default()
        };
        assert!(with_token.has_credentials());

        let with_app_id = PlatformConfig {
            app_id: Some("cli_xxx".to_string()),
            ..Default::default()
        };
        assert!(with_app_id.has_credentials());
    }

    #[test]
    fn test_channel_envelope_serialization() {
        let envelope = ChannelEnvelope {
            channel: "telegram".to_string(),
            chat_id: "12345".to_string(),
            sender: SenderInfo {
                id: "user_1".to_string(),
                display_name: "Test User".to_string(),
                username: Some("@testuser".to_string()),
            },
            raw: serde_json::json!({"text": "hello"}),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let restored: ChannelEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.channel, "telegram");
        assert_eq!(restored.chat_id, "12345");
        assert_eq!(restored.sender.username, Some("@testuser".to_string()));
    }
}
