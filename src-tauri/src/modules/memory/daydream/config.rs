//! DayDream consolidation configuration.

use serde::{Deserialize, Serialize};

/// Pipeline strategy. The enum implicitly encodes the LLM-token budget;
/// there is intentionally no separate `llm_budget_tokens` knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationStrategy {
    /// Prune only. Zero LLM tokens.
    Conservative,
    /// Prune + Merge + Reflect. ~3000 tokens / cycle.
    Balanced,
    /// Prune + Merge + Refresh + Reflect. ~8000 tokens / cycle.
    Aggressive,
}

impl Default for ConsolidationStrategy {
    fn default() -> Self {
        Self::Balanced
    }
}

/// User-facing daydream configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DayDreamConfig {
    /// Master switch. Default `false` (opt-in).
    pub enabled: bool,
    /// Idle minutes before the engine fires a cycle. Default `30`.
    pub idle_trigger_minutes: u64,
    /// Hard cap on entries any single step processes. Default `100`.
    pub max_entries_per_cycle: usize,
    /// Pipeline strategy. Default `Balanced`.
    pub strategy: ConsolidationStrategy,
}

impl Default for DayDreamConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            idle_trigger_minutes: 30,
            max_entries_per_cycle: 100,
            strategy: ConsolidationStrategy::Balanced,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_spec() {
        let cfg = DayDreamConfig::default();
        assert!(!cfg.enabled, "must default off (opt-in)");
        assert_eq!(cfg.idle_trigger_minutes, 30);
        assert_eq!(cfg.max_entries_per_cycle, 100);
        assert_eq!(cfg.strategy, ConsolidationStrategy::Balanced);
    }

    #[test]
    fn strategy_serializes_snake_case() {
        let s = serde_json::to_string(&ConsolidationStrategy::Conservative).unwrap();
        assert_eq!(s, "\"conservative\"");
    }
}
