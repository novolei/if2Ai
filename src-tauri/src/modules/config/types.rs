//! Onboarding configuration type definitions.
//!
//! Defines the core types for the onboarding configuration system:
//! - `AppConfig`: Complete application configuration (Layer 1)
//! - `ProviderConfig`: Provider credentials and connection settings
//! - `ChannelConfig`: Channel credentials and connection settings
//! - `ModelSelection`: Active provider + model pairing
//! - `TripleFiles`: Layer 2 file formats (providers.yaml, auth.json, models.json)

use std::collections::HashMap;

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::modules::onboarding::OnboardingState;

// ── Version ─────────────────────────────────────────────────────────────────

/// Current configuration format version.
///
/// Used for migration in `ConfigService::load_config()`.
/// Increment when: new fields added, fields removed, or field types change.
pub const CONFIG_VERSION: u32 = 1;

// ── Provider ────────────────────────────────────────────────────────────────

/// Provider configuration — credentials and connection settings.
///
/// Stored per-provider in `~/.if2ai/providers/<provider_id>.json`
/// and also embedded in `AppConfig.active_provider`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider identifier, e.g. "ollama", "openai", "zai"
    pub provider_id: String,
    /// Display name shown in UI, e.g. "Ollama (Local)"
    pub display_name: String,
    /// API key (may be `None` for local providers like Ollama)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Base URL for the provider's API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Auth variant sub-key for same-provider different auth
    /// (e.g. "cn" for Moonshot China, "code" for Moonshot Code)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_variant: Option<String>,
}

impl ProviderConfig {
    /// Check if this provider has the required fields populated.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        match self.provider_id.as_str() {
            "ollama" => self.base_url.as_ref().is_some_and(|u| !u.is_empty()),
            _ => {
                self.base_url.as_ref().is_some_and(|u| !u.is_empty())
                    && self.api_key.as_ref().is_some_and(|k| !k.is_empty())
            }
        }
    }
}

// ── Model Selection ─────────────────────────────────────────────────────────

/// Records the user's active model choice.
///
/// Saved alongside `ProviderConfig` to track which model
/// the onboarding flow selected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelection {
    /// Provider ID that owns this model, e.g. "ollama"
    pub provider_id: String,
    /// Model identifier, e.g. "qwen3:4b", "claude-sonnet-4-6"
    pub model_id: String,
    /// Auth variant for same-provider different auth configurations
    /// (e.g. "moonshot-cn" vs "moonshot-code" share provider_id but differ in credentials)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_variant: Option<String>,
}

/// Model reference utility — parses and formats "provider_id/model_id" strings.
///
/// Reference: openhanako uses "provider/model" format for precise model
/// identification across providers with overlapping model names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    pub provider_id: String,
    pub model_id: String,
}

impl ModelRef {
    /// Parse "provider/model" format into a `ModelRef`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (provider, model) = s.split_once('/')?;
        if provider.is_empty() || model.is_empty() {
            return None;
        }
        Some(Self {
            provider_id: provider.to_string(),
            model_id: model.to_string(),
        })
    }
}

impl fmt::Display for ModelRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.provider_id, self.model_id)
    }
}

impl FromStr for ModelRef {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| {
            format!("Invalid model reference '{s}'. Expected 'provider_id/model_id' format.")
        })
    }
}

// ── Model Role Config ───────────────────────────────────────────────────────

/// Per-role model assignment — maps a usage scenario to a specific model.
///
/// Reference: openhanako execution-router roles (chat, utility, summarizer, compiler).
/// Each role can use a different model, enabling cost/performance optimization
/// (e.g. cheap model for summarization, capable model for complex reasoning).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoleConfig {
    /// Role identifier: "chat", "utility", "utility_large", "summarizer", "compiler"
    pub role: String,
    /// Model reference in "provider_id/model_id" format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_ref: Option<String>,
}

/// Available model role names.
pub const MODEL_ROLES: &[&str] = &["chat", "utility", "utility_large", "summarizer", "compiler"];

/// Human-readable label for a model role.
#[must_use]
pub fn model_role_label(role: &str) -> &'static str {
    match role {
        "chat" => "主对话模型",
        "utility" => "轻工具模型（摘要/翻译）",
        "utility_large" => "重工具模型（复杂推理）",
        "summarizer" => "摘要模型（记忆编译）",
        "compiler" => "编译模型（快速响应）",
        _ => "未知角色",
    }
}

// ── Channel ─────────────────────────────────────────────────────────────────

/// Channel configuration — credentials for a communication channel.
///
/// Stored per-channel in `~/.if2ai/channels/<channel_id>.json`
/// and also in `AppConfig.channels`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel identifier, e.g. "feishu", "telegram"
    pub channel_id: String,
    /// Display name shown in UI, e.g. "Feishu/Lark"
    pub display_name: String,
    /// Bot token for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot_token: Option<String>,
    /// Application secret (for platforms that require it)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_secret: Option<String>,
    /// Webhook URL (for webhook-based channels)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

impl ChannelConfig {
    /// Check if this channel has the minimum required fields.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.bot_token.as_ref().is_some_and(|t| !t.is_empty())
            || self.app_secret.as_ref().is_some_and(|s| !s.is_empty())
            || self.webhook_url.as_ref().is_some_and(|u| !u.is_empty())
    }
}

/// Redacted channel configuration — safe to return to frontend.
///
/// Sensitive fields (bot_token, app_secret) are replaced with
/// boolean flags indicating presence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfigRedacted {
    pub channel_id: String,
    pub display_name: String,
    pub has_bot_token: bool,
    pub has_app_secret: bool,
    pub has_webhook_url: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

impl ChannelConfig {
    /// Convert to redacted form (safe for frontend display).
    #[must_use]
    pub fn redact(&self) -> ChannelConfigRedacted {
        ChannelConfigRedacted {
            channel_id: self.channel_id.clone(),
            display_name: self.display_name.clone(),
            has_bot_token: self.bot_token.is_some(),
            has_app_secret: self.app_secret.is_some(),
            has_webhook_url: self.webhook_url.is_some(),
            webhook_url: self.webhook_url.clone(),
        }
    }
}

// ── Channel Routing ─────────────────────────────────────────────────────────

/// Channel routing configuration — message dispatch settings.
///
/// Written automatically when the user configures their first channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRouting {
    /// Owner user IDs per platform (set by user in Settings later)
    #[serde(default)]
    pub owner_user_ids: HashMap<String, Vec<String>>,
    /// Default agent ID for routing
    #[serde(default = "default_agent_id")]
    pub default_agent_id: String,
    /// Debounce window in milliseconds
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: Option<u64>,
    /// Rate limit per minute
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: Option<u32>,
}

fn default_agent_id() -> String {
    "default".to_string()
}

fn default_debounce_ms() -> Option<u64> {
    Some(300)
}

fn default_rate_limit() -> Option<u32> {
    Some(20)
}

impl Default for ChannelRouting {
    fn default() -> Self {
        Self {
            owner_user_ids: HashMap::new(),
            default_agent_id: default_agent_id(),
            debounce_ms: default_debounce_ms(),
            rate_limit_per_minute: default_rate_limit(),
        }
    }
}

// ── AppConfig (Layer 1) ─────────────────────────────────────────────────────

/// Complete application configuration (Layer 1 — shortcut format).
///
/// Written to `~/.if2ai/config.json`. Contains the user's full
/// onboarding configuration in a single flat JSON file.
///
/// `ConfigService::save_config()` writes this and syncs to Layer 2
/// triple files used by runtime resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration format version for migration support.
    pub version: u32,
    /// Currently active provider (the one the user selected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_provider: Option<ProviderConfig>,
    /// Currently selected default model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_model: Option<ModelSelection>,
    /// All models selected for the active provider during onboarding.
    /// The first entry is the default model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_models: Vec<ModelSelection>,
    /// All configured providers with their full connection details.
    ///
    /// Unlike `active_provider` (which changes when user switches provider),
    /// this list accumulates every provider the user has ever configured.
    /// Used by `sync_to_triple_files` so that models from non-active providers
    /// still resolve their `base_url` and `api_key` correctly in `models.json`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configured_providers: Vec<ProviderConfig>,
    /// Per-role model assignments (chat, utility, summarizer, compiler)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_models: Vec<ModelRoleConfig>,
    /// All configured channels
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
    /// Channel routing defaults (written when first channel configured)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<ChannelRouting>,
    /// Embedded onboarding state (tracks completion, step, etc.)
    pub onboarding: OnboardingState,
    /// Whether the user has confirmed the security items (Step 3)
    #[serde(default)]
    pub security_confirmed: bool,
}

impl AppConfig {
    /// Create a fresh `AppConfig` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: CONFIG_VERSION,
            active_provider: None,
            active_model: None,
            selected_models: Vec::new(),
            configured_providers: Vec::new(),
            role_models: Vec::new(),
            channels: Vec::new(),
            routing: None,
            onboarding: OnboardingState::new(),
            security_confirmed: false,
        }
    }

    /// Upsert a provider into `configured_providers`.
    ///
    /// If the provider already exists (matched by `provider_id` + `auth_variant`),
    /// it is updated in-place. Otherwise it is appended.
    pub fn upsert_configured_provider(&mut self, provider: ProviderConfig) {
        let existing = self.configured_providers.iter_mut().find(|p| {
            p.provider_id == provider.provider_id && p.auth_variant == provider.auth_variant
        });
        if let Some(entry) = existing {
            *entry = provider;
        } else {
            self.configured_providers.push(provider);
        }
    }

    /// Look up a provider in `configured_providers` by id (and optional auth_variant).
    #[must_use]
    pub fn find_configured_provider(
        &self,
        provider_id: &str,
        auth_variant: Option<&str>,
    ) -> Option<&ProviderConfig> {
        self.configured_providers.iter().find(|p| {
            p.provider_id == provider_id
                && p.auth_variant.as_deref() == auth_variant
        })
    }

    /// Validate the configuration and return a list of issues.
    ///
    /// Returns an empty vector if the configuration is complete.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.active_provider.is_none() {
            issues.push("active_provider is not configured".to_string());
        } else if let Some(ref provider) = self.active_provider {
            if !provider.is_complete() {
                issues.push(format!(
                    "provider '{}' is incomplete (missing base_url or api_key)",
                    provider.provider_id
                ));
            }
        }

        if self.active_model.is_none() {
            issues.push("active_model is not selected".to_string());
        }

        if self.channels.is_empty() {
            issues.push("no channels configured".to_string());
        } else {
            for (i, channel) in self.channels.iter().enumerate() {
                if !channel.is_complete() {
                    issues.push(format!(
                        "channel '{}' (index {}) is incomplete",
                        channel.channel_id, i
                    ));
                }
            }
        }

        if !self.security_confirmed {
            issues.push("security confirmation is missing".to_string());
        }

        issues
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ── Triple Files (Layer 2) ──────────────────────────────────────────────────

/// Represents the providers.yaml structure.
///
/// Layer 2 format — consumed by `ConfigLoader` at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersYaml {
    /// Map of provider_id → provider definition
    pub providers: HashMap<String, ProviderYamlEntry>,
}

/// Single provider entry in providers.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderYamlEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
}

/// Represents the models.json structure.
///
/// Compatible with pi-coding-agent ModelsConfigSchema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsJson {
    /// Map of provider_id → provider models
    pub providers: HashMap<String, ModelsProviderEntry>,
}

/// Single provider entry in models.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsProviderEntry {
    #[serde(rename = "baseUrl", skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub models: Vec<ModelEntry>,
}

/// Single model entry in models.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(rename = "contextWindow", skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
}

/// Represents the auth.json structure.
///
/// Stores API keys that override providers.yaml values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthJson {
    /// Map of provider_id → auth credentials
    pub providers: HashMap<String, AuthEntry>,
}

/// Single auth entry in auth.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthEntry {
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}
