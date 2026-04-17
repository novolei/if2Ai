//! Builtin provider registry.
//!
//! Provides `builtin_providers()` returning 14 pre-configured provider entries
//! for the onboarding UI step 4 (Provider Setup).
//!
//! Providers are grouped into 4 categories:
//! - Domestic (9): Z.AI, Moonshot, Qianfan, Xunfei, Volcengine, BytePlus, Baidu, Minimax, Mistral
//! - International (4): OpenAI, Anthropic, Google, OpenRouter
//! - Local (1): Ollama

use super::types::{Provider, ProviderCategory, ProviderStatus, ProviderSubChoice};

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
            sub_choices: Some(vec![
                ProviderSubChoice {
                    id: "ollama-default".to_string(),
                    label: "Ollama 本地（推荐）".to_string(),
                    description: Some("自动连接本地 Ollama 服务".to_string()),
                    credential_label: "无需 API Key".to_string(),
                    credential_placeholder: "（留空即可）".to_string(),
                    docs_url: Some("https://github.com/ollama/ollama".to_string()),
                    base_url: "http://localhost:11434".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "ollama-remote".to_string(),
                    label: "远程 Ollama".to_string(),
                    description: Some("连接远程 Ollama 服务器".to_string()),
                    credential_label: "API Key".to_string(),
                    credential_placeholder: "（可选）".to_string(),
                    docs_url: Some(
                        "https://github.com/ollama/ollama/blob/main/docs/openai.md".to_string(),
                    ),
                    base_url: "".to_string(),
                    api: "openai-compat".to_string(),
                },
            ]),
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
            sub_choices: None,
        },
        Provider {
            id: "anthropic".to_string(),
            name: "Anthropic (Claude)".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("anthropic".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "google".to_string(),
            name: "Google (Gemini)".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("google".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("openrouter".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "mistral".to_string(),
            name: "Mistral".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("mistral".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "xai".to_string(),
            name: "xAI (Grok)".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("xai".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            category: ProviderCategory::International,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("deepseek".to_string()),
            sub_choices: Some(vec![
                ProviderSubChoice {
                    id: "deepseek-official".to_string(),
                    label: "DeepSeek 官方 API".to_string(),
                    description: Some("直连 DeepSeek 官方接口，无需中转".to_string()),
                    credential_label: "API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://platform.deepseek.com/api_keys".to_string()),
                    base_url: "https://api.deepseek.com/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "deepseek-siliconflow".to_string(),
                    label: "SiliconFlow（国内加速）".to_string(),
                    description: Some("通过 SiliconFlow 调用 DeepSeek，国内网络友好".to_string()),
                    credential_label: "SiliconFlow API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://cloud.siliconflow.cn/account/ak".to_string()),
                    base_url: "https://api.siliconflow.cn/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
            ]),
        },
        // ── Domestic (Chinese) ──────────────────────────────────────────────
        Provider {
            id: "zai".to_string(),
            name: "Z.AI (GLM)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("zai".to_string()),
            sub_choices: Some(vec![
                ProviderSubChoice {
                    id: "zai-coding-global".to_string(),
                    label: "Coding-Plan-Global".to_string(),
                    description: Some("GLM Coding Plan (api.z.ai)".to_string()),
                    credential_label: "Z.AI API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://z.ai".to_string()),
                    base_url: "https://api.z.ai/api/paas/v4".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "zai-coding-cn".to_string(),
                    label: "Coding-Plan-CN".to_string(),
                    description: Some("GLM Coding Plan（国内）".to_string()),
                    credential_label: "Z.AI API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://open.bigmodel.cn".to_string()),
                    base_url: "https://open.bigmodel.cn/api/paas/v4".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "zai-global".to_string(),
                    label: "Global".to_string(),
                    description: Some("GLM Global (api.z.ai)".to_string()),
                    credential_label: "Z.AI API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://z.ai".to_string()),
                    base_url: "https://api.z.ai/api/paas/v4".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "zai-cn".to_string(),
                    label: "CN".to_string(),
                    description: Some("GLM 国内版".to_string()),
                    credential_label: "Z.AI API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://open.bigmodel.cn".to_string()),
                    base_url: "https://open.bigmodel.cn/api/paas/v4".to_string(),
                    api: "openai-compat".to_string(),
                },
            ]),
        },
        Provider {
            id: "moonshot".to_string(),
            name: "Moonshot (Kimi)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("moonshot".to_string()),
            sub_choices: Some(vec![
                ProviderSubChoice {
                    id: "moonshot-cn".to_string(),
                    label: "Kimi API key (.cn)".to_string(),
                    description: Some("国内版 · api.moonshot.cn".to_string()),
                    credential_label: "Moonshot API Key (.cn)".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://platform.moonshot.cn/console/api-keys".to_string()),
                    base_url: "https://api.moonshot.cn/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "moonshot-global".to_string(),
                    label: "Kimi API key (.ai)".to_string(),
                    description: Some("国际版 · api.moonshot.ai".to_string()),
                    credential_label: "Moonshot API Key (.ai)".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://platform.moonshot.ai".to_string()),
                    base_url: "https://api.moonshot.ai/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "moonshot-code".to_string(),
                    label: "Kimi Code API key".to_string(),
                    description: Some("Code 订阅用户".to_string()),
                    credential_label: "Kimi Code API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://platform.moonshot.cn/console/api-keys".to_string()),
                    base_url: "https://api.moonshot.cn/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
            ]),
        },
        Provider {
            id: "qianfan".to_string(),
            name: "Qianfan (文心)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("qianfan".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "qwen".to_string(),
            name: "Qwen (阿里云)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("qwen".to_string()),
            sub_choices: Some(vec![
                ProviderSubChoice {
                    id: "qwen-cn".to_string(),
                    label: "国内版（DashScope）".to_string(),
                    description: Some("dashscope.aliyuncs.com".to_string()),
                    credential_label: "DashScope API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://bailian.console.aliyun.com/".to_string()),
                    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "qwen-global".to_string(),
                    label: "国际版（DashScope Intl）".to_string(),
                    description: Some("dashscope-intl.aliyuncs.com".to_string()),
                    credential_label: "DashScope API Key".to_string(),
                    credential_placeholder: "sk-…".to_string(),
                    docs_url: Some("https://bailian.console.aliyun.com/".to_string()),
                    base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
            ]),
        },
        Provider {
            id: "xiaomi".to_string(),
            name: "Xiaomi AI".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("xiaomi".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "volcengine".to_string(),
            name: "Volcengine (火山)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("volcengine".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "byteplus".to_string(),
            name: "BytePlus (豆包)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("byteplus".to_string()),
            sub_choices: None,
        },
        Provider {
            id: "minimax".to_string(),
            name: "MiniMax (海螺)".to_string(),
            category: ProviderCategory::Domestic,
            status: ProviderStatus::ApiKeyRequired,
            supports_models: true,
            is_local: false,
            logo_path: Some("minimax".to_string()),
            sub_choices: Some(vec![
                ProviderSubChoice {
                    id: "minimax-global".to_string(),
                    label: "国际版 API Key".to_string(),
                    description: Some("api.minimaxi.com".to_string()),
                    credential_label: "MiniMax API Key".to_string(),
                    credential_placeholder: "…".to_string(),
                    docs_url: Some("https://minimax.io".to_string()),
                    base_url: "https://api.minimaxi.com/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
                ProviderSubChoice {
                    id: "minimax-cn".to_string(),
                    label: "国内版 API Key".to_string(),
                    description: Some("api.minimax.chat".to_string()),
                    credential_label: "MiniMax API Key".to_string(),
                    credential_placeholder: "…".to_string(),
                    docs_url: Some("https://minimaxi.com".to_string()),
                    base_url: "https://api.minimax.chat/v1".to_string(),
                    api: "openai-compat".to_string(),
                },
            ]),
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
        assert_eq!(providers.len(), 16, "Expected 16 builtin providers");
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
        assert_eq!(domestic.len(), 8, "Expected 8 domestic providers");
    }

    #[test]
    fn test_international_provider_count() {
        let international = builtin_providers()
            .into_iter()
            .filter(|p| matches!(p.category, ProviderCategory::International))
            .collect::<Vec<_>>();
        assert_eq!(international.len(), 7, "Expected 7 international providers");
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
