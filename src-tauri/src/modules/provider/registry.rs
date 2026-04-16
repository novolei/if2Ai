//! Builtin provider registry.
//!
//! Provides `builtin_providers()` returning 14 pre-configured provider entries
//! for the onboarding UI step 4 (Provider Setup).
//!
//! Providers are grouped into 4 categories:
//! - Domestic (9): Z.AI, Moonshot, Qianfan, Xunfei, Volcengine, BytePlus, Baidu, Minimax, Mistral
//! - International (4): OpenAI, Anthropic, Google, OpenRouter
//! - Local (1): Ollama

use super::types::{Provider, ProviderCategory, ProviderStatus};

/// Returns all 14 builtin providers.
///
/// The list is ordered: Local first, then International, then Domestic.
/// Custom providers are not included here (added by user at runtime).
#[must_use]
pub fn builtin_providers() -> Vec<Provider> {
    vec![
        // ── Local ───────────────────────────────────────────────────────────
        Provider {
            id: "ollama".to_string(),
            name: "Ollama (本地)".to_string(),
            category: ProviderCategory::Local,
            status: ProviderStatus::Available,
            supports_models: true,
            is_local: true,
            logo_path: Some("ollama".to_string()),
        },
        // ── International ───────────────────────────────────────────────────
        Provider {
            id: "openai".to_string(),
            name: "OpenAI (GPT)".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("openai".to_string()),
        },
        Provider {
            id: "anthropic".to_string(),
            name: "Anthropic (Claude)".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("anthropic".to_string()),
        },
        Provider {
            id: "google".to_string(),
            name: "Google (Gemini)".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("google".to_string()),
        },
        Provider {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("openrouter".to_string()),
        },
        // ── Domestic (Chinese) ──────────────────────────────────────────────
        Provider {
            id: "zai".to_string(),
            name: "Z.AI (智谱)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("zai".to_string()),
        },
        Provider {
            id: "moonshot".to_string(),
            name: "Moonshot (Kimi)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("moonshot".to_string()),
        },
        Provider {
            id: "qianfan".to_string(),
            name: "Qianfan (文心)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("qianfan".to_string()),
        },
        Provider {
            id: "xunfei".to_string(),
            name: "Xunfei (讯飞)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("xunfei".to_string()),
        },
        Provider {
            id: "volcengine".to_string(),
            name: "Volcengine (火山)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("volcengine".to_string()),
        },
        Provider {
            id: "byteplus".to_string(),
            name: "BytePlus (豆包)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("byteplus".to_string()),
        },
        Provider {
            id: "baidu".to_string(),
            name: "Baidu (百度)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("baidu".to_string()),
        },
        Provider {
            id: "minimax".to_string(),
            name: "MiniMax (海螺)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("minimax".to_string()),
        },
        Provider {
            id: "mistral".to_string(),
            name: "Mistral".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("mistral".to_string()),
        },
    ]
}

/// Look up a builtin provider by ID.
#[must_use]
pub fn find_provider(provider_id: &str) -> Option<Provider> {
    builtin_providers()
        .into_iter()
        .find(|p| p.id == provider_id)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_providers_count() {
        let providers = builtin_providers();
        assert_eq!(providers.len(), 14, "Expected 14 builtin providers");
    }

    #[test]
    fn test_local_provider_exists() {
        let ollama = find_provider("ollama");
        assert!(ollama.is_some(), "Ollama should be in builtin providers");
        assert!(ollama.unwrap().is_local);
    }

    #[test]
    fn test_openai_provider_exists() {
        let openai = find_provider("openai");
        assert!(openai.is_some(), "OpenAI should be in builtin providers");
    }

    #[test]
    fn test_anthropic_provider_exists() {
        let anthropic = find_provider("anthropic");
        assert!(
            anthropic.is_some(),
            "Anthropic should be in builtin providers"
        );
    }

    #[test]
    fn test_domestic_provider_count() {
        let domestic = builtin_providers()
            .into_iter()
            .filter(|p| matches!(p.category, ProviderCategory::Domestic))
            .collect::<Vec<_>>();
        assert_eq!(domestic.len(), 9, "Expected 9 domestic providers");
    }

    #[test]
    fn test_international_provider_count() {
        let international = builtin_providers()
            .into_iter()
            .filter(|p| matches!(p.category, ProviderCategory::International))
            .collect::<Vec<_>>();
        assert_eq!(international.len(), 4, "Expected 4 international providers");
    }

    #[test]
    fn test_no_duplicate_ids() {
        let providers = builtin_providers();
        let mut ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        let unique: Vec<&str> = ids.clone();
        unique.windows(2).for_each(|w| {
            assert_ne!(w[0], w[1], "Duplicate provider ID: {}", w[0]);
        });
    }
}
