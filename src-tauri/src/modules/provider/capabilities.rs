//! P-MULTI-API — Resolve a model's runtime capability profile from
//! (1) global policy override, (2) per-model user override, (3) the
//! built-in [`known_models`] dictionary, in that priority.
//!
//! Consumers (request serializers, frontend chips) read the resolved
//! [`ModelCapability`] instead of branching on substrings themselves.

use super::known_models::{self, Quirk};

/// Tri-state global policy. Mirrors `IF2AI_THINKING_MODE_OVERRIDE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlobalThinkingPolicy {
    /// Use the dictionary + user override (default).
    #[default]
    Auto,
    /// Force every reasoning capability on, even for unknown models.
    /// Useful for debugging providers that secretly require it.
    ForceOn,
    /// Disable reasoning wire fields entirely. Useful when a provider
    /// rejects them and you need to keep working until a fix lands.
    ForceOff,
}

impl GlobalThinkingPolicy {
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("IF2AI_THINKING_MODE_OVERRIDE")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("on" | "force-on" | "required" | "1" | "true") => Self::ForceOn,
            Some("off" | "force-off" | "0" | "false") => Self::ForceOff,
            _ => Self::Auto,
        }
    }
}

/// Provenance for the resolved capability — surfaced in observer events
/// + Settings UI tooltip so users can tell why a chip lit up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilitySource {
    GlobalPolicy,
    UserOverride,
    UserConfig,
    KnownDict,
    Default,
}

/// Resolved capability profile passed to request serialization layers.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelCapability {
    pub reasoning: bool,
    pub reasoning_required_in_tool_calls: bool,
    /// `Some(true)` → write `enable_thinking: true` at request top level
    /// (Qwen). `Some(false)` → `enable_thinking: false`. `None` → omit.
    pub enable_thinking_flag: Option<bool>,
    pub supports_reasoning_effort: bool,
    pub source: CapabilitySource,
}

impl ModelCapability {
    fn disabled() -> Self {
        Self {
            reasoning: false,
            reasoning_required_in_tool_calls: false,
            enable_thinking_flag: None,
            supports_reasoning_effort: false,
            source: CapabilitySource::Default,
        }
    }

    fn forced_on() -> Self {
        Self {
            reasoning: true,
            reasoning_required_in_tool_calls: true,
            enable_thinking_flag: None,
            supports_reasoning_effort: true,
            source: CapabilitySource::GlobalPolicy,
        }
    }
}

/// Resolve a model's capability. Priority:
///
/// 1. `global_policy == ForceOff` → all-off, source `GlobalPolicy`.
/// 2. `global_policy == ForceOn`  → all-on, source `GlobalPolicy`.
/// 3. `user_override == Some(false)` → all-off, source `UserOverride`.
/// 4. `user_override == Some(true)`  → force-on **but** only fields
///    sensible for the wire (reasoning + required_in_tool_calls), source
///    `UserOverride`.
/// 5. Fall through to dictionary; unknown → `Default`/all-off.
#[must_use]
pub fn resolve(
    provider_id: &str,
    model_id: &str,
    user_override: Option<bool>,
    global_policy: GlobalThinkingPolicy,
) -> ModelCapability {
    if matches!(global_policy, GlobalThinkingPolicy::ForceOff) {
        let mut cap = ModelCapability::disabled();
        cap.source = CapabilitySource::GlobalPolicy;
        return cap;
    }
    if matches!(global_policy, GlobalThinkingPolicy::ForceOn) {
        return ModelCapability::forced_on();
    }
    if let Some(false) = user_override {
        let mut cap = ModelCapability::disabled();
        cap.source = CapabilitySource::UserOverride;
        return cap;
    }
    if let Some(true) = user_override {
        let mut cap = ModelCapability::disabled();
        cap.reasoning = true;
        cap.reasoning_required_in_tool_calls = true;
        cap.source = CapabilitySource::UserOverride;
        return cap;
    }

    if supports_thinking_from_models_json(provider_id, model_id) {
        let mut cap = ModelCapability::disabled();
        cap.reasoning = true;
        cap.reasoning_required_in_tool_calls = true;
        cap.source = CapabilitySource::UserConfig;
        return cap;
    }

    if let Some(km) = known_models::lookup(provider_id, model_id) {
        let mut cap = ModelCapability::disabled();
        cap.reasoning = km.reasoning;
        cap.source = CapabilitySource::KnownDict;
        for q in km.quirks {
            match q {
                Quirk::ReasoningRequiredInToolCalls => {
                    cap.reasoning_required_in_tool_calls = true;
                }
                Quirk::EnableThinkingFlag => {
                    cap.enable_thinking_flag = Some(km.reasoning);
                }
                Quirk::ReasoningEffort => {
                    cap.supports_reasoning_effort = true;
                }
            }
        }
        return cap;
    }

    ModelCapability::disabled()
}

fn supports_thinking_from_models_json(provider_id: &str, model_id: &str) -> bool {
    let path = crate::modules::config::store::models_json_path();
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return false,
    };
    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let providers = match parsed.get("providers").and_then(|p| p.as_object()) {
        Some(providers) => providers,
        None => return false,
    };
    let provider_entries = providers.iter().filter(|(key, _)| {
        key.as_str() == provider_id || key.starts_with(&format!("{provider_id}::"))
    });
    for (_, entry) in provider_entries {
        let Some(models) = entry.get("models").and_then(|m| m.as_array()) else {
            continue;
        };
        for model in models {
            let id = model
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or_default();
            if id == model_id
                && model
                    .get("supportsThinking")
                    .and_then(|flag| flag.as_bool())
                    .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_off_wins_over_dict() {
        let cap = resolve(
            "moonshot",
            "kimi-thinking-preview",
            None,
            GlobalThinkingPolicy::ForceOff,
        );
        assert!(!cap.reasoning);
        assert!(!cap.reasoning_required_in_tool_calls);
        assert_eq!(cap.source, CapabilitySource::GlobalPolicy);
    }

    #[test]
    fn force_on_wins_over_unknown() {
        let cap = resolve(
            "any",
            "completely-unknown",
            None,
            GlobalThinkingPolicy::ForceOn,
        );
        assert!(cap.reasoning);
        assert_eq!(cap.source, CapabilitySource::GlobalPolicy);
    }

    #[test]
    fn user_override_false_wins_over_dict() {
        let cap = resolve(
            "moonshot",
            "kimi-thinking-preview",
            Some(false),
            GlobalThinkingPolicy::Auto,
        );
        assert!(!cap.reasoning);
        assert_eq!(cap.source, CapabilitySource::UserOverride);
    }

    #[test]
    fn dict_resolves_kimi_thinking_required() {
        let cap = resolve(
            "moonshot",
            "kimi-thinking-preview",
            None,
            GlobalThinkingPolicy::Auto,
        );
        assert!(cap.reasoning);
        assert!(cap.reasoning_required_in_tool_calls);
        assert_eq!(cap.source, CapabilitySource::KnownDict);
    }

    #[test]
    fn dict_resolves_qwen_enable_thinking_flag() {
        let cap = resolve("dashscope", "qwen3-plus", None, GlobalThinkingPolicy::Auto);
        assert!(cap.reasoning);
        assert_eq!(cap.enable_thinking_flag, Some(true));
        // DashScope requires reasoning_content on assistant tool_call messages
        // when enable_thinking is active.
        assert!(cap.reasoning_required_in_tool_calls);
    }

    #[test]
    fn dict_resolves_ollama_minimax_cloud_reasoning_required() {
        let cap = resolve(
            "ollama",
            "minimax-m2.7:cloud",
            None,
            GlobalThinkingPolicy::Auto,
        );
        assert!(cap.reasoning);
        assert!(cap.reasoning_required_in_tool_calls);
        assert_eq!(cap.source, CapabilitySource::KnownDict);
    }

    #[test]
    fn dict_resolves_o1_reasoning_effort() {
        let cap = resolve("openai", "o1-2024-12-17", None, GlobalThinkingPolicy::Auto);
        assert!(cap.reasoning);
        assert!(cap.supports_reasoning_effort);
        assert!(!cap.reasoning_required_in_tool_calls);
    }

    #[test]
    fn unknown_falls_through_to_default() {
        let cap = resolve(
            "openai",
            "future-unknown-model-9000",
            None,
            GlobalThinkingPolicy::Auto,
        );
        assert!(!cap.reasoning);
        assert_eq!(cap.source, CapabilitySource::Default);
    }
}
