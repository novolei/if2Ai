//! P-MULTI-API — Wire-format enum that mirrors openhanako-main's
//! `API_FORMAT_OPTIONS`.
//!
//! Serde rename keeps the on-disk / IPC string identical to upstream
//! (`openai-completions`, `anthropic-messages`, `openai-responses`,
//! `openai-codex-responses`) so user configs can round-trip with
//! openhanako without translation.

use serde::{Deserialize, Serialize};

/// LLM provider wire formats supported by if2Ai.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApiType {
    /// `POST {base}/chat/completions` — OpenAI-compatible, the default
    /// for most providers (DeepSeek, Moonshot non-coding, Ollama, …).
    #[serde(rename = "openai-completions")]
    OpenAiCompletions,
    /// `POST {base}/v1/messages` — Anthropic's Messages API (also used
    /// by MiniMax `https://api.minimaxi.com/anthropic` and Kimi
    /// Coding Plan `https://api.kimi.com/coding/`).
    #[serde(rename = "anthropic-messages")]
    AnthropicMessages,
    /// `POST {base}/responses` — OpenAI's newer Responses API
    /// (reasoning-effort capable models).
    #[serde(rename = "openai-responses")]
    OpenAiResponses,
    /// Same wire as [`ApiType::OpenAiResponses`], rebadged for the
    /// ChatGPT Plus / Pro Codex OAuth path. Authentication uses an
    /// OAuth bearer minted from `auth.json` instead of a static API key.
    #[serde(rename = "openai-codex-responses")]
    OpenAiCodexResponses,
}

impl ApiType {
    /// Endpoint suffix appended to the configured `base_url`.
    #[must_use]
    pub fn endpoint_suffix(self) -> &'static str {
        match self {
            Self::OpenAiCompletions => "/chat/completions",
            Self::AnthropicMessages => "/v1/messages",
            Self::OpenAiResponses | Self::OpenAiCodexResponses => "/responses",
        }
    }

    /// HTTP auth headers for an outbound request (excluding `Content-Type`).
    /// `api_key` may be a static API key or an OAuth bearer; both go into
    /// the same Authorization header for OpenAI-style endpoints.
    #[must_use]
    pub fn auth_headers(self, api_key: &str) -> Vec<(&'static str, String)> {
        match self {
            Self::AnthropicMessages => vec![
                ("x-api-key", api_key.to_string()),
                ("anthropic-version", "2023-06-01".to_string()),
            ],
            Self::OpenAiCompletions
            | Self::OpenAiResponses
            | Self::OpenAiCodexResponses => {
                vec![("Authorization", format!("Bearer {api_key}"))]
            }
        }
    }

    /// Stable wire string (matches serde rename + openhanako convention).
    #[must_use]
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::OpenAiCompletions => "openai-completions",
            Self::AnthropicMessages => "anthropic-messages",
            Self::OpenAiResponses => "openai-responses",
            Self::OpenAiCodexResponses => "openai-codex-responses",
        }
    }

    /// Parse the wire string. Used by command layer / settings IPC.
    #[must_use]
    pub fn from_wire_str(s: &str) -> Option<Self> {
        match s {
            "openai-completions" => Some(Self::OpenAiCompletions),
            "anthropic-messages" => Some(Self::AnthropicMessages),
            "openai-responses" => Some(Self::OpenAiResponses),
            "openai-codex-responses" => Some(Self::OpenAiCodexResponses),
            _ => None,
        }
    }
}

impl Default for ApiType {
    fn default() -> Self {
        Self::OpenAiCompletions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip_uses_wire_strings() {
        for variant in [
            ApiType::OpenAiCompletions,
            ApiType::AnthropicMessages,
            ApiType::OpenAiResponses,
            ApiType::OpenAiCodexResponses,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, format!("\"{}\"", variant.as_wire_str()));
            let back: ApiType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn endpoint_suffix_matches_openhanako() {
        assert_eq!(
            ApiType::OpenAiCompletions.endpoint_suffix(),
            "/chat/completions"
        );
        assert_eq!(
            ApiType::AnthropicMessages.endpoint_suffix(),
            "/v1/messages"
        );
        assert_eq!(ApiType::OpenAiResponses.endpoint_suffix(), "/responses");
        assert_eq!(
            ApiType::OpenAiCodexResponses.endpoint_suffix(),
            "/responses"
        );
    }

    #[test]
    fn anthropic_uses_x_api_key_header() {
        let h = ApiType::AnthropicMessages.auth_headers("k");
        assert!(h.iter().any(|(k, v)| *k == "x-api-key" && v == "k"));
        assert!(h.iter().any(|(k, _)| *k == "anthropic-version"));
    }

    #[test]
    fn openai_family_uses_bearer() {
        for at in [
            ApiType::OpenAiCompletions,
            ApiType::OpenAiResponses,
            ApiType::OpenAiCodexResponses,
        ] {
            let h = at.auth_headers("token");
            assert_eq!(h, vec![("Authorization", "Bearer token".to_string())]);
        }
    }

    #[test]
    fn parses_unknown_string_to_none() {
        assert!(ApiType::from_wire_str("garbage").is_none());
    }
}
