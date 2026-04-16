//! Built-in channel registry.
//!
//! Provides the `builtin_channels()` function that returns all 13 supported
//! communication channels across three categories:
//! - **Social** (4): Feishu, QQ, WeChat, Telegram
//! - **Messaging** (6): WhatsApp, Teams, Discord, Slack, iMessage, LINE
//! - **Desktop** (3): Signal, Mattermost, Matrix
//!
//! Also provides `find_channel(channel_id)` for looking up a specific channel.

use super::types::{Channel, ChannelCategory};

/// Return all 13 built-in channels.
///
/// The channels are grouped by category and match the ADR-014 Section 8.5
/// channel registry specification.
///
/// # Categories
///
/// | Category    | Count | Channels                                     |
/// |-------------|-------|----------------------------------------------|
/// | Social      | 4     | Feishu, QQ, WeChat, Telegram                 |
/// | Messaging   | 6     | WhatsApp, Teams, Discord, Slack, iMessage, LINE |
/// | Desktop     | 3     | Signal, Mattermost, Matrix                   |
#[must_use]
pub fn builtin_channels() -> Vec<Channel> {
    vec![
        // ── Social ──────────────────────────────────────────────────────
        Channel {
            id: "feishu".to_string(),
            name: "飞书 (Feishu/Lark)".to_string(),
            category: ChannelCategory::Social,
            icon: "message-square".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_feishu.png".to_string()),
            requires_token: true,
            requires_secret: true,
            requires_webhook: false,
            node_version_required: Some("18+".to_string()),
        },
        Channel {
            id: "qq".to_string(),
            name: "QQ Bot".to_string(),
            category: ChannelCategory::Social,
            icon: "message-circle".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_qq.png".to_string()),
            requires_token: true,
            requires_secret: true,
            requires_webhook: false,
            node_version_required: Some("18+".to_string()),
        },
        Channel {
            id: "wechat".to_string(),
            name: "WeChat (微信)".to_string(),
            category: ChannelCategory::Social,
            icon: "message-circle".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_wechat.png".to_string()),
            requires_token: true,
            requires_secret: true,
            requires_webhook: true,
            node_version_required: Some("18+".to_string()),
        },
        Channel {
            id: "telegram".to_string(),
            name: "Telegram (Bot API)".to_string(),
            category: ChannelCategory::Social,
            icon: "send".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_telegram.png".to_string()),
            requires_token: true,
            requires_secret: false,
            requires_webhook: false,
            node_version_required: None,
        },
        // ── Messaging ───────────────────────────────────────────────────
        Channel {
            id: "whatsapp".to_string(),
            name: "WhatsApp (QR link)".to_string(),
            category: ChannelCategory::Messaging,
            icon: "phone".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_whatsapp.png".to_string()),
            requires_token: false,
            requires_secret: false,
            requires_webhook: true,
            node_version_required: None,
        },
        Channel {
            id: "teams".to_string(),
            name: "Microsoft Teams (Bot Framework)".to_string(),
            category: ChannelCategory::Messaging,
            icon: "users".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_msteams.png".to_string()),
            requires_token: true,
            requires_secret: true,
            requires_webhook: true,
            node_version_required: Some("18+".to_string()),
        },
        Channel {
            id: "discord".to_string(),
            name: "Discord (Bot API)".to_string(),
            category: ChannelCategory::Messaging,
            icon: "hash".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_discord.png".to_string()),
            requires_token: true,
            requires_secret: false,
            requires_webhook: false,
            node_version_required: None,
        },
        Channel {
            id: "slack".to_string(),
            name: "Slack (Socket Mode)".to_string(),
            category: ChannelCategory::Messaging,
            icon: "slack".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_slack.png".to_string()),
            requires_token: true,
            requires_secret: false,
            requires_webhook: false,
            node_version_required: None,
        },
        Channel {
            id: "imessage".to_string(),
            name: "iMessage (imsg)".to_string(),
            category: ChannelCategory::Messaging,
            icon: "message-square".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_imessage.png".to_string()),
            requires_token: false,
            requires_secret: false,
            requires_webhook: false,
            node_version_required: None,
        },
        Channel {
            id: "line".to_string(),
            name: "LINE (Messaging API)".to_string(),
            category: ChannelCategory::Messaging,
            icon: "message-circle".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_line.png".to_string()),
            requires_token: true,
            requires_secret: true,
            requires_webhook: true,
            node_version_required: Some("18+".to_string()),
        },
        // ── Desktop ─────────────────────────────────────────────────────
        Channel {
            id: "signal".to_string(),
            name: "Signal (signal-cli)".to_string(),
            category: ChannelCategory::Desktop,
            icon: "shield".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_signal.png".to_string()),
            requires_token: false,
            requires_secret: false,
            requires_webhook: false,
            node_version_required: None,
        },
        Channel {
            id: "mattermost".to_string(),
            name: "Mattermost (plugin)".to_string(),
            category: ChannelCategory::Desktop,
            icon: "server".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_mattermost.png".to_string()),
            requires_token: true,
            requires_secret: false,
            requires_webhook: true,
            node_version_required: Some("18+".to_string()),
        },
        Channel {
            id: "matrix".to_string(),
            name: "Matrix (glgnt)".to_string(),
            category: ChannelCategory::Desktop,
            icon: "globe".to_string(),
            logo_path: Some("src/assets/ChannelLogos/channel_logo_matrix.png".to_string()),
            requires_token: true,
            requires_secret: false,
            requires_webhook: false,
            node_version_required: None,
        },
    ]
}

/// Look up a channel by its ID. Returns `None` if not found.
#[must_use]
pub fn find_channel(channel_id: &str) -> Option<Channel> {
    builtin_channels().into_iter().find(|c| c.id == channel_id)
}

/// Count channels by category.
///
/// Useful for UI rendering (e.g., "4 Social channels").
#[must_use]
pub fn count_by_category() -> Vec<(ChannelCategory, usize)> {
    let channels = builtin_channels();
    let mut counts: std::collections::HashMap<ChannelCategory, usize> =
        std::collections::HashMap::new();

    for channel in channels {
        *counts.entry(channel.category).or_insert(0) += 1;
    }

    // Return in a stable order
    vec![
        (
            ChannelCategory::Social,
            *counts.get(&ChannelCategory::Social).unwrap_or(&0),
        ),
        (
            ChannelCategory::Messaging,
            *counts.get(&ChannelCategory::Messaging).unwrap_or(&0),
        ),
        (
            ChannelCategory::Desktop,
            *counts.get(&ChannelCategory::Desktop).unwrap_or(&0),
        ),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_channels_count() {
        let channels = builtin_channels();
        assert_eq!(channels.len(), 13);
    }

    #[test]
    fn test_builtin_channels_no_duplicate_ids() {
        let channels = builtin_channels();
        let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for channel in channels {
            assert!(
                ids.insert(channel.id.clone()),
                "Duplicate channel ID: {}",
                channel.id
            );
        }
    }

    #[test]
    fn test_builtin_channels_social_count() {
        let channels: Vec<_> = builtin_channels()
            .into_iter()
            .filter(|c| matches!(c.category, ChannelCategory::Social))
            .collect();
        assert_eq!(channels.len(), 4);
    }

    #[test]
    fn test_builtin_channels_messaging_count() {
        let channels: Vec<_> = builtin_channels()
            .into_iter()
            .filter(|c| matches!(c.category, ChannelCategory::Messaging))
            .collect();
        assert_eq!(channels.len(), 6);
    }

    #[test]
    fn test_builtin_channels_desktop_count() {
        let channels: Vec<_> = builtin_channels()
            .into_iter()
            .filter(|c| matches!(c.category, ChannelCategory::Desktop))
            .collect();
        assert_eq!(channels.len(), 3);
    }

    #[test]
    fn test_find_channel_existing() {
        let channel = find_channel("telegram");
        assert!(channel.is_some());
        let channel = channel.unwrap();
        assert_eq!(channel.name, "Telegram (Bot API)");
        assert!(channel.requires_token);
    }

    #[test]
    fn test_find_channel_feishu() {
        let channel = find_channel("feishu");
        assert!(channel.is_some());
        let channel = channel.unwrap();
        assert_eq!(channel.category, ChannelCategory::Social);
        assert!(channel.requires_secret);
    }

    #[test]
    fn test_find_channel_nonexistent() {
        let channel = find_channel("nonexistent-channel");
        assert!(channel.is_none());
    }

    #[test]
    fn test_count_by_category() {
        let counts = count_by_category();
        assert_eq!(counts.len(), 3);
        assert_eq!(counts[0], (ChannelCategory::Social, 4));
        assert_eq!(counts[1], (ChannelCategory::Messaging, 6));
        assert_eq!(counts[2], (ChannelCategory::Desktop, 3));
    }

    #[test]
    fn test_all_channels_have_logo_or_fallback() {
        let channels = builtin_channels();
        for channel in channels {
            assert!(
                channel.logo_path.is_some(),
                "Channel '{}' missing logo_path",
                channel.id
            );
        }
    }

    #[test]
    fn test_telegram_is_polling_friendly() {
        // Telegram should require token but not secret or webhook
        let telegram = find_channel("telegram").unwrap();
        assert!(telegram.requires_token);
        assert!(!telegram.requires_secret);
        assert!(!telegram.requires_webhook);
    }

    #[test]
    fn test_whatsapp_does_not_require_token() {
        // WhatsApp uses QR link, not token-based auth
        let whatsapp = find_channel("whatsapp").unwrap();
        assert!(!whatsapp.requires_token);
        assert!(whatsapp.requires_webhook);
    }
}
