#![allow(dead_code)]

use super::session::Session;
use serde::{Deserialize, Serialize};

const DEFAULT_INPUT_COST_PER_MILLION: f64 = 15.0;
const DEFAULT_OUTPUT_COST_PER_MILLION: f64 = 75.0;
const DEFAULT_CACHE_CREATION_COST_PER_MILLION: f64 = 18.75;
const DEFAULT_CACHE_READ_COST_PER_MILLION: f64 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPricing {
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
    pub cache_creation_cost_per_million: f64,
    pub cache_read_cost_per_million: f64,
}

impl ModelPricing {
    #[must_use]
    pub const fn default_sonnet_tier() -> Self {
        Self {
            input_cost_per_million: DEFAULT_INPUT_COST_PER_MILLION,
            output_cost_per_million: DEFAULT_OUTPUT_COST_PER_MILLION,
            cache_creation_cost_per_million: DEFAULT_CACHE_CREATION_COST_PER_MILLION,
            cache_read_cost_per_million: DEFAULT_CACHE_READ_COST_PER_MILLION,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsageCostEstimate {
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cache_creation_cost_usd: f64,
    pub cache_read_cost_usd: f64,
}

impl UsageCostEstimate {
    #[must_use]
    pub fn total_cost_usd(self) -> f64 {
        self.input_cost_usd
            + self.output_cost_usd
            + self.cache_creation_cost_usd
            + self.cache_read_cost_usd
    }
}

/// Best-effort price lookup keyed by substring on lower-cased model name.
///
/// Covers the vendors if2Ai ships providers for; unknown models fall through
/// to `None` and the caller should warn + use [`ModelPricing::default_sonnet_tier`].
/// Numbers are USD per 1M tokens, sourced from each vendor's public price
/// page as of 2026-Q1; bump these when prices change.
#[must_use]
pub fn pricing_for_model(model: &str) -> Option<ModelPricing> {
    let normalized = model.to_ascii_lowercase();

    // ── Anthropic ──
    if normalized.contains("haiku") {
        return Some(ModelPricing {
            input_cost_per_million: 1.0,
            output_cost_per_million: 5.0,
            cache_creation_cost_per_million: 1.25,
            cache_read_cost_per_million: 0.1,
        });
    }
    if normalized.contains("opus") {
        return Some(ModelPricing {
            input_cost_per_million: 15.0,
            output_cost_per_million: 75.0,
            cache_creation_cost_per_million: 18.75,
            cache_read_cost_per_million: 1.5,
        });
    }
    if normalized.contains("sonnet") {
        return Some(ModelPricing::default_sonnet_tier());
    }

    // ── OpenAI (GPT-5/4o family + o-series) ──
    if normalized.contains("gpt-5") || normalized.contains("gpt5") {
        return Some(ModelPricing {
            input_cost_per_million: 2.5,
            output_cost_per_million: 10.0,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 1.25,
        });
    }
    if normalized.contains("gpt-4o-mini") || normalized.contains("4o-mini") {
        return Some(ModelPricing {
            input_cost_per_million: 0.15,
            output_cost_per_million: 0.6,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.075,
        });
    }
    if normalized.contains("gpt-4o") || normalized.contains("4o") {
        return Some(ModelPricing {
            input_cost_per_million: 2.5,
            output_cost_per_million: 10.0,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 1.25,
        });
    }
    if normalized.contains("o3-mini") {
        return Some(ModelPricing {
            input_cost_per_million: 1.1,
            output_cost_per_million: 4.4,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.55,
        });
    }
    if normalized.contains("o3") {
        return Some(ModelPricing {
            input_cost_per_million: 10.0,
            output_cost_per_million: 40.0,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 2.5,
        });
    }
    if normalized.contains("o1-mini") {
        return Some(ModelPricing {
            input_cost_per_million: 3.0,
            output_cost_per_million: 12.0,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 1.5,
        });
    }
    if normalized.contains("o1") {
        return Some(ModelPricing {
            input_cost_per_million: 15.0,
            output_cost_per_million: 60.0,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 7.5,
        });
    }

    // ── DeepSeek ──
    if normalized.contains("deepseek-reasoner") || normalized.contains("deepseek-r1") {
        return Some(ModelPricing {
            input_cost_per_million: 0.55,
            output_cost_per_million: 2.19,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.14,
        });
    }
    if normalized.contains("deepseek") {
        return Some(ModelPricing {
            input_cost_per_million: 0.27,
            output_cost_per_million: 1.1,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.07,
        });
    }

    // ── Alibaba Qwen ──
    if normalized.contains("qwen-max") {
        return Some(ModelPricing {
            input_cost_per_million: 1.6,
            output_cost_per_million: 6.4,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.0,
        });
    }
    if normalized.contains("qwen-plus") {
        return Some(ModelPricing {
            input_cost_per_million: 0.4,
            output_cost_per_million: 1.2,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.0,
        });
    }
    if normalized.contains("qwen") {
        return Some(ModelPricing {
            input_cost_per_million: 0.3,
            output_cost_per_million: 0.6,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.0,
        });
    }

    // ── Google Gemini ──
    if normalized.contains("gemini-2.5-pro") || normalized.contains("gemini-2-5-pro") {
        return Some(ModelPricing {
            input_cost_per_million: 1.25,
            output_cost_per_million: 10.0,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.31,
        });
    }
    if normalized.contains("gemini-2.5-flash") || normalized.contains("gemini-2-5-flash") {
        return Some(ModelPricing {
            input_cost_per_million: 0.3,
            output_cost_per_million: 2.5,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.075,
        });
    }

    None
}

impl TokenUsage {
    #[must_use]
    pub fn total_tokens(self) -> u32 {
        self.input_tokens
            + self.output_tokens
            + self.cache_creation_input_tokens
            + self.cache_read_input_tokens
    }

    #[must_use]
    pub fn estimate_cost_usd(self) -> UsageCostEstimate {
        self.estimate_cost_usd_with_pricing(ModelPricing::default_sonnet_tier())
    }

    #[must_use]
    pub fn estimate_cost_usd_with_pricing(self, pricing: ModelPricing) -> UsageCostEstimate {
        UsageCostEstimate {
            input_cost_usd: cost_for_tokens(self.input_tokens, pricing.input_cost_per_million),
            output_cost_usd: cost_for_tokens(self.output_tokens, pricing.output_cost_per_million),
            cache_creation_cost_usd: cost_for_tokens(
                self.cache_creation_input_tokens,
                pricing.cache_creation_cost_per_million,
            ),
            cache_read_cost_usd: cost_for_tokens(
                self.cache_read_input_tokens,
                pricing.cache_read_cost_per_million,
            ),
        }
    }

    #[must_use]
    pub fn summary_lines(self, label: &str) -> Vec<String> {
        self.summary_lines_for_model(label, None)
    }

    #[must_use]
    pub fn summary_lines_for_model(self, label: &str, model: Option<&str>) -> Vec<String> {
        let pricing = model.and_then(pricing_for_model);
        let cost = pricing.map_or_else(
            || self.estimate_cost_usd(),
            |pricing| self.estimate_cost_usd_with_pricing(pricing),
        );
        let model_suffix =
            model.map_or_else(String::new, |model_name| format!(" model={model_name}"));
        let pricing_suffix = if pricing.is_some() {
            ""
        } else if model.is_some() {
            " pricing=estimated-default"
        } else {
            ""
        };
        vec![
            format!(
                "{label}: total_tokens={} input={} output={} cache_write={} cache_read={} estimated_cost={}{}{}",
                self.total_tokens(),
                self.input_tokens,
                self.output_tokens,
                self.cache_creation_input_tokens,
                self.cache_read_input_tokens,
                format_usd(cost.total_cost_usd()),
                model_suffix,
                pricing_suffix,
            ),
            format!(
                "  cost breakdown: input={} output={} cache_write={} cache_read={}",
                format_usd(cost.input_cost_usd),
                format_usd(cost.output_cost_usd),
                format_usd(cost.cache_creation_cost_usd),
                format_usd(cost.cache_read_cost_usd),
            ),
        ]
    }
}

fn cost_for_tokens(tokens: u32, usd_per_million_tokens: f64) -> f64 {
    f64::from(tokens) / 1_000_000.0 * usd_per_million_tokens
}

/// Compute total USD cost for `usage` against `model`. Falls back to
/// `default_sonnet_tier` and warns when `model` is non-empty but unknown so
/// price-table drift is observable in logs.
#[must_use]
pub fn cost_for_usage(usage: TokenUsage, model: &str) -> f64 {
    let pricing = pricing_for_model(model).unwrap_or_else(|| {
        if !model.is_empty() {
            tracing::warn!(
                model = %model,
                "[usage] no pricing for model; using default_sonnet_tier as fallback"
            );
        }
        ModelPricing::default_sonnet_tier()
    });
    usage
        .estimate_cost_usd_with_pricing(pricing)
        .total_cost_usd()
}

#[must_use]
pub fn format_usd(amount: f64) -> String {
    format!("${amount:.4}")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageTracker {
    latest_turn: TokenUsage,
    cumulative: TokenUsage,
    turns: u32,
}

impl UsageTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_session(session: &Session) -> Self {
        let mut tracker = Self::new();
        for message in &session.messages {
            if let Some(usage) = message.usage {
                tracker.record(usage);
            }
        }
        tracker
    }

    pub fn record(&mut self, usage: TokenUsage) {
        self.latest_turn = usage;
        self.cumulative.input_tokens += usage.input_tokens;
        self.cumulative.output_tokens += usage.output_tokens;
        self.cumulative.cache_creation_input_tokens += usage.cache_creation_input_tokens;
        self.cumulative.cache_read_input_tokens += usage.cache_read_input_tokens;
        self.turns += 1;
    }

    #[must_use]
    pub fn current_turn_usage(&self) -> TokenUsage {
        self.latest_turn
    }

    #[must_use]
    pub fn cumulative_usage(&self) -> TokenUsage {
        self.cumulative
    }

    #[must_use]
    pub fn turns(&self) -> u32 {
        self.turns
    }
}

#[cfg(test)]
mod tests {
    use super::{format_usd, pricing_for_model, TokenUsage, UsageTracker};
    use crate::modules::runtime::session::{
        ContentBlock, ConversationMessage, MessageRole, Session,
    };

    #[test]
    fn tracks_true_cumulative_usage() {
        let mut tracker = UsageTracker::new();
        tracker.record(TokenUsage {
            input_tokens: 10,
            output_tokens: 4,
            cache_creation_input_tokens: 2,
            cache_read_input_tokens: 1,
        });
        tracker.record(TokenUsage {
            input_tokens: 20,
            output_tokens: 6,
            cache_creation_input_tokens: 3,
            cache_read_input_tokens: 2,
        });

        assert_eq!(tracker.turns(), 2);
        assert_eq!(tracker.current_turn_usage().input_tokens, 20);
        assert_eq!(tracker.current_turn_usage().output_tokens, 6);
        assert_eq!(tracker.cumulative_usage().output_tokens, 10);
        assert_eq!(tracker.cumulative_usage().input_tokens, 30);
        assert_eq!(tracker.cumulative_usage().total_tokens(), 48);
    }

    #[test]
    fn computes_cost_summary_lines() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cache_creation_input_tokens: 100_000,
            cache_read_input_tokens: 200_000,
        };

        let cost = usage.estimate_cost_usd();
        assert_eq!(format_usd(cost.input_cost_usd), "$15.0000");
        assert_eq!(format_usd(cost.output_cost_usd), "$37.5000");
        let lines = usage.summary_lines_for_model("usage", Some("claude-sonnet-4-6"));
        assert!(lines[0].contains("estimated_cost=$54.6750"));
        assert!(lines[0].contains("model=claude-sonnet-4-6"));
        assert!(lines[1].contains("cache_read=$0.3000"));
    }

    #[test]
    fn supports_model_specific_pricing() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };

        let haiku = pricing_for_model("claude-haiku-4-5-20251213").expect("haiku pricing");
        let opus = pricing_for_model("claude-opus-4-6").expect("opus pricing");
        let haiku_cost = usage.estimate_cost_usd_with_pricing(haiku);
        let opus_cost = usage.estimate_cost_usd_with_pricing(opus);
        assert_eq!(format_usd(haiku_cost.total_cost_usd()), "$3.5000");
        assert_eq!(format_usd(opus_cost.total_cost_usd()), "$52.5000");
    }

    #[test]
    fn extended_pricing_table_covers_known_vendors() {
        // P0 confidence: each vendor branch hits a pricing entry; we don't
        // pin exact dollar values here (those drift) — just that the lookup
        // succeeds so unknown-model fallback warnings stay rare.
        for model in [
            "gpt-5-2026-04",
            "gpt-4o-mini",
            "gpt-4o",
            "o3-mini",
            "o3",
            "o1",
            "deepseek-reasoner",
            "deepseek-chat",
            "qwen-max",
            "qwen-plus",
            "qwen-turbo",
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "claude-haiku-4-5",
            "claude-opus-4-6",
            "claude-sonnet-4-6",
        ] {
            assert!(
                pricing_for_model(model).is_some(),
                "expected pricing for {model}"
            );
        }
    }

    #[test]
    fn cost_for_usage_falls_back_to_default_for_unknown_model() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let cost = super::cost_for_usage(usage, "completely-unknown-model-id");
        // default_sonnet_tier input cost is $15 / M tokens.
        assert!((cost - 15.0).abs() < 1e-6, "cost={cost}");
    }

    #[test]
    fn marks_unknown_model_pricing_as_fallback() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 100,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let lines = usage.summary_lines_for_model("usage", Some("custom-model"));
        assert!(lines[0].contains("pricing=estimated-default"));
    }

    #[test]
    fn reconstructs_usage_from_session_messages() {
        let session = Session {
            version: 1,
            messages: vec![ConversationMessage {
                role: MessageRole::Assistant,
                blocks: vec![ContentBlock::Text {
                    text: "done".to_string(),
                }],
                thinking: None,
                usage: Some(TokenUsage {
                    input_tokens: 5,
                    output_tokens: 2,
                    cache_creation_input_tokens: 1,
                    cache_read_input_tokens: 0,
                }),
                task_outcome: None,
                degraded_reason: None,
                resume_available: None,
                resume_cursor: None,
                request_id: None,
                finish_reason: None,
            }],
        };

        let tracker = UsageTracker::from_session(&session);
        assert_eq!(tracker.turns(), 1);
        assert_eq!(tracker.cumulative_usage().total_tokens(), 8);
    }
}
