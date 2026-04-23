//! P-MULTI-API — Known model dictionary keyed by `(provider, model_id_substr)`.
//!
//! Adopts the openhanako-main convention: explicit `reasoning: bool` plus
//! a small `Quirk` enum that captures the wire-level requirements
//! providers like Kimi-thinking-preview and DeepSeek-R1 enforce. Lives
//! as `'static` so look-up is allocation-free at request build time.

/// Wire-level oddities that flow into request serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quirk {
    /// Qwen family: top-level `enable_thinking: true` flag toggles
    /// reasoning mode on the request body.
    EnableThinkingFlag,
    /// Kimi-thinking-preview / DeepSeek-R1: every assistant message in
    /// the history that carries `tool_calls` must also include a
    /// `reasoning_content` field, otherwise the API returns 400
    /// `request_validation_error`. We pad with the empty string when
    /// the history pre-dates thinking capture.
    ReasoningRequiredInToolCalls,
    /// OpenAI o1 / o3 / GPT-5: top-level `reasoning_effort` accepts
    /// `low | medium | high` (no `reasoning_content` field).
    ReasoningEffort,
}

/// One known model entry. Matched on `provider == provider_id` AND
/// `model_id.to_lowercase().contains(id_substr)` so a single row covers
/// versioned variants (`kimi-thinking-preview-2025-06`,
/// `deepseek-r1-0528`, …).
#[derive(Debug, Clone, Copy)]
pub struct KnownModel {
    pub provider: &'static str,
    pub id_substr: &'static str,
    pub display: &'static str,
    pub context: u64,
    pub reasoning: bool,
    pub quirks: &'static [Quirk],
}

pub static KNOWN_MODELS: &[KnownModel] = &[
    // ── Moonshot / Kimi ──
    KnownModel {
        provider: "moonshot",
        id_substr: "kimi-thinking",
        display: "Kimi Thinking Preview",
        context: 200_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    KnownModel {
        provider: "moonshot",
        id_substr: "kimi-k2-thinking",
        display: "Kimi K2 Thinking",
        context: 256_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    KnownModel {
        provider: "kimi-coding",
        id_substr: "kimi",
        display: "Kimi (Coding Plan)",
        context: 256_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    KnownModel {
        provider: "moonshot",
        id_substr: "minimax-m2",
        display: "MiniMax M2",
        context: 256_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    KnownModel {
        provider: "minimax",
        id_substr: "minimax-m2",
        display: "MiniMax M2",
        context: 256_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    // ── DeepSeek ──
    KnownModel {
        provider: "deepseek",
        id_substr: "deepseek-r1",
        display: "DeepSeek R1",
        context: 128_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    KnownModel {
        provider: "deepseek",
        id_substr: "deepseek-reasoner",
        display: "DeepSeek Reasoner",
        context: 64_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    // ── Alibaba Qwen ──
    KnownModel {
        provider: "dashscope",
        id_substr: "qwen3",
        display: "Qwen 3",
        context: 1_000_000,
        reasoning: true,
        quirks: &[Quirk::EnableThinkingFlag],
    },
    KnownModel {
        provider: "dashscope",
        id_substr: "qwen-plus",
        display: "Qwen Plus",
        context: 1_000_000,
        reasoning: true,
        quirks: &[Quirk::EnableThinkingFlag],
    },
    KnownModel {
        provider: "dashscope",
        id_substr: "qwen-max",
        display: "Qwen Max",
        context: 32_000,
        reasoning: false,
        quirks: &[],
    },
    // ── OpenAI reasoning ──
    KnownModel {
        provider: "openai",
        id_substr: "o3-mini",
        display: "OpenAI o3-mini",
        context: 200_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningEffort],
    },
    KnownModel {
        provider: "openai",
        id_substr: "o3",
        display: "OpenAI o3",
        context: 200_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningEffort],
    },
    KnownModel {
        provider: "openai",
        id_substr: "o1-mini",
        display: "OpenAI o1-mini",
        context: 128_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningEffort],
    },
    KnownModel {
        provider: "openai",
        id_substr: "o1",
        display: "OpenAI o1",
        context: 200_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningEffort],
    },
    KnownModel {
        provider: "openai",
        id_substr: "gpt-5",
        display: "GPT-5",
        context: 200_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningEffort],
    },
    KnownModel {
        provider: "openai",
        id_substr: "gpt-4o",
        display: "GPT-4o",
        context: 128_000,
        reasoning: false,
        quirks: &[],
    },
    // ── Anthropic ──
    KnownModel {
        provider: "anthropic",
        id_substr: "opus",
        display: "Claude Opus",
        context: 200_000,
        reasoning: false,
        quirks: &[],
    },
    KnownModel {
        provider: "anthropic",
        id_substr: "sonnet",
        display: "Claude Sonnet",
        context: 200_000,
        reasoning: false,
        quirks: &[],
    },
    KnownModel {
        provider: "anthropic",
        id_substr: "haiku",
        display: "Claude Haiku",
        context: 200_000,
        reasoning: false,
        quirks: &[],
    },
    // ── Zhipu GLM ──
    KnownModel {
        provider: "zhipu",
        id_substr: "glm-z1",
        display: "GLM-Z1 (推理)",
        context: 128_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    KnownModel {
        provider: "zhipu",
        id_substr: "glm-zero",
        display: "GLM Zero (推理)",
        context: 128_000,
        reasoning: true,
        quirks: &[Quirk::ReasoningRequiredInToolCalls],
    },
    KnownModel {
        provider: "zhipu",
        id_substr: "glm-4.6",
        display: "GLM-4.6",
        context: 128_000,
        reasoning: false,
        quirks: &[],
    },
];

/// Find the first known-model entry matching the substring.  Substring
/// matching is case-insensitive on the model id; provider id must equal.
#[must_use]
pub fn lookup(provider_id: &str, model_id: &str) -> Option<&'static KnownModel> {
    let needle = model_id.to_ascii_lowercase();
    KNOWN_MODELS
        .iter()
        .find(|m| m.provider == provider_id && needle.contains(m.id_substr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_matches_versioned_kimi_thinking() {
        let m = lookup("moonshot", "kimi-thinking-preview-2025-06").unwrap();
        assert!(m.reasoning);
        assert!(m.quirks.contains(&Quirk::ReasoningRequiredInToolCalls));
    }

    #[test]
    fn lookup_matches_versioned_deepseek_r1() {
        let m = lookup("deepseek", "deepseek-r1-0528").unwrap();
        assert!(m.quirks.contains(&Quirk::ReasoningRequiredInToolCalls));
    }

    #[test]
    fn lookup_returns_none_for_unknown_provider_model() {
        assert!(lookup("openai", "fake-model-7b").is_none());
        assert!(lookup("nonexistent-provider", "gpt-4o").is_none());
    }

    #[test]
    fn lookup_picks_specific_o3_mini_over_o3() {
        // Order matters: `o3-mini` row precedes `o3`.
        let m = lookup("openai", "o3-mini").unwrap();
        assert_eq!(m.display, "OpenAI o3-mini");
    }

    #[test]
    fn known_quirks_compile() {
        // Smoke: Quirk variants used in the table.
        let q = &[Quirk::EnableThinkingFlag, Quirk::ReasoningRequiredInToolCalls, Quirk::ReasoningEffort];
        assert_eq!(q.len(), 3);
    }
}
