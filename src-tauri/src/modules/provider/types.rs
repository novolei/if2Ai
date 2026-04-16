//! Provider module type definitions.
//!
//! Defines types for the provider registry and connection testing:
//! - `Provider`: Provider registry entry (14 builtin providers)
//! - `ProviderCategory`: Domestic/International/Local/Custom
//! - `ProviderStatus`: Available/ApiKeyRequired/Unavailable
//! - `Model`: Model metadata (context window, modality, etc.)
//! - `ModelModality`: Text/Vision/Multimodal
//! - `TestResult`: Connection test outcome

use serde::{Deserialize, Serialize};

// ── Provider Category ───────────────────────────────────────────────────────

/// Provider category for grouping in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCategory {
    /// Domestic Chinese providers (Z.AI, Moonshot, Qianfan, etc.)
    Domestic,
    /// International providers (OpenAI, Anthropic, Google, etc.)
    International,
    /// Local providers (Ollama)
    Local,
    /// Custom provider (user-defined)
    Custom,
}

impl ProviderCategory {
    /// Human-readable label for the category.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Domestic => "国内",
            Self::International => "国际",
            Self::Local => "本地",
            Self::Custom => "自定义",
        }
    }
}

// ── Provider Status ─────────────────────────────────────────────────────────

/// Provider availability status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProviderStatus {
    /// Provider is available and ready to use.
    Available,
    /// Provider requires an API key to be configured.
    ApiKeyRequired,
    /// Provider is currently unavailable.
    Unavailable { reason: String },
}

// ── Model Modality ──────────────────────────────────────────────────────────

/// Model input modality.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelModality {
    /// Text-only model.
    Text,
    /// Vision-capable model (text + image).
    Vision,
    /// Multimodal model (text + image + audio + video).
    Multimodal,
}

impl ModelModality {
    /// Human-readable label for the modality.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Text => "文本",
            Self::Vision => "视觉",
            Self::Multimodal => "多模态",
        }
    }
}

// ── Provider ────────────────────────────────────────────────────────────────

/// Provider registry entry.
///
/// Represents a single LLM provider with metadata for the onboarding UI.
/// 14 builtin providers are registered via `builtin_providers()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// Provider identifier, e.g. "ollama", "openai", "anthropic"
    pub id: String,
    /// Display name shown in UI, e.g. "Ollama (本地)"
    pub name: String,
    /// Provider category
    pub category: ProviderCategory,
    /// Current availability status
    pub status: ProviderStatus,
    /// Whether this provider supports model listing
    pub supports_models: bool,
    /// Whether this is a local provider (no API key needed)
    pub is_local: bool,
    /// Icon/asset identifier for logo lookup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_path: Option<String>,
}

impl Provider {
    /// Check if this provider is currently available.
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self.status, ProviderStatus::Available)
    }
}

// ── Model ───────────────────────────────────────────────────────────────────

/// Model metadata.
///
/// Represents a single LLM model with its capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Model identifier, e.g. "claude-sonnet-4-6"
    pub id: String,
    /// Display name, e.g. "Claude Sonnet 4.6"
    pub name: String,
    /// Context window size in tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Maximum output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    /// Input modality
    pub modality: ModelModality,
}

// ── TestResult ──────────────────────────────────────────────────────────────

/// Connection test outcome.
///
/// Returned by `test_provider_connection()` after attempting to
/// reach a provider's API endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Whether the test succeeded
    pub success: bool,
    /// Human-readable message
    pub message: String,
    /// Round-trip latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    /// Additional details (error codes, suggestions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Error codes for provider connection tests.
pub mod error_codes {
    /// Provider configuration is invalid (missing API key, bad URL, etc.)
    pub const PROVIDER_CONFIG_INVALID: &str = "provider_config_invalid";
    /// Authentication failed (invalid API key)
    pub const PROVIDER_AUTH_ERROR: &str = "provider_auth_error";
    /// Connection timed out
    pub const PROVIDER_TIMEOUT: &str = "provider_timeout";
    /// Network error (DNS failure, connection refused, etc.)
    pub const PROVIDER_NETWORK_ERROR: &str = "provider_network_error";
    /// Provider's upstream service is unavailable (503, etc.)
    pub const PROVIDER_UPSTREAM_UNAVAILABLE: &str = "provider_upstream_unavailable";
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_category_label() {
        assert_eq!(ProviderCategory::Domestic.label(), "国内");
        assert_eq!(ProviderCategory::International.label(), "国际");
        assert_eq!(ProviderCategory::Local.label(), "本地");
        assert_eq!(ProviderCategory::Custom.label(), "自定义");
    }

    #[test]
    fn test_model_modality_label() {
        assert_eq!(ModelModality::Text.label(), "文本");
        assert_eq!(ModelModality::Vision.label(), "视觉");
        assert_eq!(ModelModality::Multimodal.label(), "多模态");
    }

    #[test]
    fn test_provider_is_available() {
        let available = Provider {
            id: "test".to_string(),
            name: "Test".to_string(),
            category: ProviderCategory::Custom,
            status: ProviderStatus::Available,
            supports_models: false,
            is_local: false,
            logo_path: None,
        };
        assert!(available.is_available());

        let unavailable = Provider {
            id: "test".to_string(),
            name: "Test".to_string(),
            category: ProviderCategory::Custom,
            status: ProviderStatus::Unavailable {
                reason: "test".to_string(),
            },
            supports_models: false,
            is_local: false,
            logo_path: None,
        };
        assert!(!unavailable.is_available());
    }

    #[test]
    fn test_test_result_serialization() {
        let result = TestResult {
            success: true,
            message: "Connection OK".to_string(),
            latency_ms: Some(120),
            details: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("Connection OK"));
        assert!(json.contains("120"));
    }
}
