//! Safety-valve configuration for the agentic loop.
//!
//! Mirrors Steward's `AgenticLoopConfig`. All fields have defaults that
//! preserve current production behaviour.
//!
//! Relocated from `application::turn_service::loop_config` in Phase 3 T1.
//!
//! ## Wiring status (post-Phase 3 T3)
//!
//! - `max_iterations` — **fully wired**: production reads from
//!   `agent_max_iterations()` env-var via T11/T12 single-source-of-truth.
//! - `force_text_after_truncations` — **fully wired** as of Phase 3 T3 (N2-β):
//!   after N consecutive `length`-truncation finishes, `run_agentic_loop`
//!   sets `ctx.force_text = true`, and both
//!   `application::turn_service::stream_delegate::StreamDelegate::before_llm_call`
//!   (via `iteration_preflight` → `stream_preflight::build_iteration_request`)
//!   and `runtime::run_delegate::RunDelegate::call_llm` honour it by
//!   dropping tool definitions from the next outgoing LLM request.
//!
//! ## Phase 3 T2 (N1-α)
//!
//! Removed `enable_tool_intent_nudge` and `max_tool_intent_nudges` (inert in
//! production — both delegates short-circuit empty tool calls before the
//! loop's nudge path could fire). The corresponding nudge logic in
//! `run_agentic_loop` is replaced by a `LoopOutcome::Failure` guard that
//! detects delegate contract violations.

/// Strategy for determining the maximum number of agentic loop iterations.
///
/// `Fixed` preserves the legacy hard-coded behaviour. `Adaptive` adjusts
/// the cap dynamically based on the first-round LLM response (number of
/// tool calls), staying within configurable `[min, max]` bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IterationStrategy {
    /// Use a single fixed value for `max_iterations` (the current default).
    Fixed(usize),
    /// Scale `max_iterations` from `base` according to first-iteration
    /// tool-call count, clamped to `[min, max]`.
    Adaptive {
        /// Floor — never go below this.
        min: usize,
        /// Ceiling — never exceed this.
        max: usize,
        /// Starting point before scaling.
        base: usize,
    },
}

impl IterationStrategy {
    /// Resolve the effective `max_iterations` given the number of tool calls
    /// observed in the first LLM response.
    ///
    /// For `Fixed`, the value is returned unchanged regardless of
    /// `first_round_tool_calls`. For `Adaptive`:
    /// - 0 tool calls → `base`
    /// - 1–3 tool calls → `base * 1.5` (rounded down)
    /// - 4+ tool calls → `base * 2`
    /// The result is then clamped to `[min, max]`.
    #[must_use]
    pub fn resolve(&self, first_round_tool_calls: usize) -> usize {
        match self {
            Self::Fixed(n) => *n,
            Self::Adaptive { min, max, base } => {
                let scaled = match first_round_tool_calls {
                    0 => *base,
                    1..=3 => (*base as f64 * 1.5) as usize,
                    _ => base * 2,
                };
                scaled.clamp(*min, *max)
            }
        }
    }
}

impl Default for IterationStrategy {
    /// Default: `Fixed(50)` — preserves legacy behaviour.
    fn default() -> Self {
        Self::Fixed(50)
    }
}

/// Controls periodic progress-check injection and three-stage failure
/// escalation. All intervals are measured in loop iterations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressCheckConfig {
    /// Inject a `[PROGRESS_CHECK]` system hint every N iterations.
    pub progress_check_interval: usize,
    /// Re-inject working-memory / checkpoint hints every N iterations.
    pub memory_reinject_interval: usize,
    /// Fraction (0–100) of `max_iterations` at which a wrap-up warning fires.
    /// E.g. 80 means the warning fires at `max_iterations * 80 / 100`.
    pub wrap_up_threshold_pct: usize,
    /// Number of consecutive same-tool failures before escalation stages
    /// advance. Stage 1 → retry hint, Stage 2 → probe hint,
    /// Stage 3 → strategy-switch hint.
    pub max_failure_escalation_stages: usize,
}

impl Default for ProgressCheckConfig {
    fn default() -> Self {
        Self {
            progress_check_interval: 7,
            memory_reinject_interval: 10,
            wrap_up_threshold_pct: 80,
            max_failure_escalation_stages: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticLoopConfig {
    pub max_iterations: usize,
    pub force_text_after_truncations: u32,
    /// Configuration for periodic progress checks and failure escalation.
    pub progress_check: ProgressCheckConfig,
    /// Optional dynamic iteration strategy. When set, overrides
    /// `max_iterations` after the first LLM response.
    pub iteration_strategy: IterationStrategy,
}

impl Default for AgenticLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            force_text_after_truncations: 2,
            progress_check: ProgressCheckConfig::default(),
            iteration_strategy: IterationStrategy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_steward_baseline() {
        let c = AgenticLoopConfig::default();
        assert_eq!(c.max_iterations, 50);
        assert_eq!(c.force_text_after_truncations, 2);
        assert_eq!(c.iteration_strategy, IterationStrategy::Fixed(50));
    }

    #[test]
    fn fixed_strategy_ignores_tool_calls() {
        let s = IterationStrategy::Fixed(20);
        assert_eq!(s.resolve(0), 20);
        assert_eq!(s.resolve(5), 20);
    }

    #[test]
    fn adaptive_zero_tools_returns_base() {
        let s = IterationStrategy::Adaptive { min: 5, max: 30, base: 10 };
        assert_eq!(s.resolve(0), 10);
    }

    #[test]
    fn adaptive_few_tools_scales_1_5x() {
        let s = IterationStrategy::Adaptive { min: 5, max: 30, base: 10 };
        assert_eq!(s.resolve(1), 15);
        assert_eq!(s.resolve(3), 15);
    }

    #[test]
    fn adaptive_many_tools_scales_2x() {
        let s = IterationStrategy::Adaptive { min: 5, max: 30, base: 10 };
        assert_eq!(s.resolve(4), 20);
        assert_eq!(s.resolve(10), 20);
    }

    #[test]
    fn adaptive_clamps_to_max() {
        let s = IterationStrategy::Adaptive { min: 5, max: 15, base: 10 };
        // 10 * 2 = 20, clamped to 15
        assert_eq!(s.resolve(4), 15);
    }

    #[test]
    fn adaptive_clamps_to_min() {
        let s = IterationStrategy::Adaptive { min: 12, max: 30, base: 10 };
        // base=10, but min=12
        assert_eq!(s.resolve(0), 12);
    }
}
