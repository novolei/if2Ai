//! P-MULTI-API — Built-in provider dictionary, lifted from
//! openhanako-main's `lib/providers/*.js` plugin set + the
//! settings tabs three-segment grouping (OAuth / Coding Plan / API).
//!
//! `KnownProvider` is `'static` so registration is zero-allocation at
//! boot. User-added providers live elsewhere (config store) and may
//! override any subset of these defaults.

use super::types::ProviderCategory as GeoCategory;
use crate::modules::api::ApiType;

/// Auth scheme used to obtain the bearer / API key for outbound requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    /// Static API key supplied by the user (most providers).
    ApiKey,
    /// OAuth flow (Codex Plus/Pro). Token persisted in `~/.if2ai/auth.json`
    /// under [`KnownProvider::auth_json_key`] (or `id` when absent).
    OAuth,
    /// No auth (e.g. local Ollama default).
    None,
}

/// Settings tab grouping. Mirrors the three list segments in
/// openhanako's `ProvidersTab.tsx`. Geographical category (CN / Intl /
/// Local) lives separately on [`GeoCategory`] for the Onboarding cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceCategory {
    /// OAuth providers (`auth_type == OAuth`). Currently just Codex.
    OAuth,
    /// Coding Plan SKUs (`id.ends_with("-coding")` in openhanako).
    CodingPlan,
    /// Standard API providers (everything else).
    Api,
}

impl ServiceCategory {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::OAuth => "OAUTH",
            Self::CodingPlan => "CODING PLAN",
            Self::Api => "API",
        }
    }

    #[must_use]
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::CodingPlan => "coding-plan",
            Self::Api => "api",
        }
    }
}

/// One built-in provider entry.
#[derive(Debug, Clone)]
pub struct KnownProvider {
    pub id: &'static str,
    pub display_name: &'static str,
    pub auth_type: AuthType,
    pub default_base_url: &'static str,
    pub default_api: ApiType,
    pub service_category: ServiceCategory,
    /// Geographical bucket for the Onboarding card grid.
    pub geo_category: GeoCategory,
    pub auth_json_key: Option<&'static str>,
    /// `true` when the provider exposes a `GET /models` endpoint we can
    /// auto-list. `false` for OAuth-only or providers without listing.
    pub supports_models: bool,
}

/// All built-in providers, alphabetised within each service category.
///
/// IDs are stable wire strings — they appear in user config, lock files,
/// and IPC. **Do not rename**; add new entries at the end of the relevant
/// section instead. Sourced from openhanako-main's `BUILTIN_PLUGINS` list
/// (`core/provider-registry.js`) + `lib/providers/*.js` for default URLs.
pub static KNOWN_PROVIDERS: &[KnownProvider] = &[
    // ── OAuth ──────────────────────────────────────────────────────
    KnownProvider {
        id: "openai-codex-oauth",
        display_name: "ChatGPT Plus/Pro (Codex)",
        auth_type: AuthType::OAuth,
        default_base_url: "",
        default_api: ApiType::OpenAiCodexResponses,
        service_category: ServiceCategory::OAuth,
        geo_category: GeoCategory::International,
        auth_json_key: Some("openai-codex"),
        supports_models: false,
    },
    // ── Coding Plan SKUs ──────────────────────────────────────────
    KnownProvider {
        id: "kimi-coding",
        display_name: "Kimi Coding Plan",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.kimi.com/coding/",
        default_api: ApiType::AnthropicMessages,
        service_category: ServiceCategory::CodingPlan,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "dashscope-coding",
        display_name: "百炼 Coding Plan",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://coding.dashscope.aliyuncs.com/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::CodingPlan,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "volcengine-coding",
        display_name: "火山引擎 Coding Plan",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://ark.cn-beijing.volces.com/api/coding/v3",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::CodingPlan,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    // ── Standard API providers ────────────────────────────────────
    KnownProvider {
        id: "openai",
        display_name: "OpenAI",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.openai.com/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "anthropic",
        display_name: "Anthropic",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.anthropic.com",
        default_api: ApiType::AnthropicMessages,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "deepseek",
        display_name: "DeepSeek",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.deepseek.com/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "moonshot",
        display_name: "Moonshot (Kimi)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.moonshot.cn/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "ollama",
        display_name: "Ollama (本地)",
        auth_type: AuthType::None,
        default_base_url: "http://localhost:11434/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Local,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "openrouter",
        display_name: "OpenRouter",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://openrouter.ai/api/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "gemini",
        display_name: "Google Gemini",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "minimax",
        display_name: "MiniMax",
        auth_type: AuthType::ApiKey,
        // MiniMax exposes an Anthropic-compatible Messages API at this URL.
        default_base_url: "https://api.minimaxi.com/anthropic",
        default_api: ApiType::AnthropicMessages,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "dashscope",
        display_name: "阿里云百炼 (DashScope)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "siliconflow",
        display_name: "SiliconFlow (硅基流动)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.siliconflow.cn/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "zhipu",
        display_name: "智谱 AI (GLM)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://open.bigmodel.cn/api/paas/v4",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "baichuan",
        display_name: "百川智能",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.baichuan-ai.com/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "stepfun",
        display_name: "阶跃星辰 (StepFun)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.stepfun.com/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "volcengine",
        display_name: "火山引擎 (豆包)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://ark.cn-beijing.volces.com/api/v3",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "hunyuan",
        display_name: "腾讯混元",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.hunyuan.cloud.tencent.com/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "baidu-cloud",
        display_name: "百度智能云 (文心)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://qianfan.baidubce.com/v2",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "modelscope",
        display_name: "魔搭 (ModelScope)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api-inference.modelscope.cn/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "infini",
        display_name: "无问芯穹 (Infini)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://cloud.infini-ai.com/maas/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "mimo",
        display_name: "Xiaomi (MiMo)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.xiaomimimo.com/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::Domestic,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "groq",
        display_name: "Groq",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.groq.com/openai/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "together",
        display_name: "Together AI",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.together.xyz/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "fireworks",
        display_name: "Fireworks AI",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.fireworks.ai/inference/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "mistral",
        display_name: "Mistral AI",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.mistral.ai/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "perplexity",
        display_name: "Perplexity",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.perplexity.ai",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
    KnownProvider {
        id: "xai",
        display_name: "xAI (Grok)",
        auth_type: AuthType::ApiKey,
        default_base_url: "https://api.x.ai/v1",
        default_api: ApiType::OpenAiCompletions,
        service_category: ServiceCategory::Api,
        geo_category: GeoCategory::International,
        auth_json_key: None,
        supports_models: true,
    },
];

/// Look up a built-in provider by stable id.
#[must_use]
pub fn find(provider_id: &str) -> Option<&'static KnownProvider> {
    KNOWN_PROVIDERS.iter().find(|p| p.id == provider_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = KNOWN_PROVIDERS.iter().map(|p| p.id).collect();
        ids.sort();
        let len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), len, "duplicate provider id detected");
    }

    #[test]
    fn coding_plan_ids_have_suffix() {
        for p in KNOWN_PROVIDERS
            .iter()
            .filter(|p| p.service_category == ServiceCategory::CodingPlan)
        {
            assert!(
                p.id.ends_with("-coding"),
                "coding-plan provider {} should end with -coding",
                p.id
            );
        }
    }

    #[test]
    fn oauth_providers_have_oauth_auth_type() {
        for p in KNOWN_PROVIDERS
            .iter()
            .filter(|p| p.service_category == ServiceCategory::OAuth)
        {
            assert_eq!(p.auth_type, AuthType::OAuth);
        }
    }

    #[test]
    fn anthropic_messages_providers_have_correct_default_api() {
        // Sanity check the two known providers that ship Anthropic wire by default.
        assert_eq!(
            find("anthropic").unwrap().default_api,
            ApiType::AnthropicMessages
        );
        assert_eq!(
            find("minimax").unwrap().default_api,
            ApiType::AnthropicMessages
        );
        assert_eq!(
            find("kimi-coding").unwrap().default_api,
            ApiType::AnthropicMessages
        );
    }
}
