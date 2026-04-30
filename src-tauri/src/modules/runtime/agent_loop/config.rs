//! Safety-valve configuration for the agentic loop.
//!
//! Mirrors Steward's `AgenticLoopConfig`. All fields have defaults that
//! preserve current production behaviour.
//!
//! Relocated from `application::turn_service::loop_config` in Phase 3 T1.
//!
//! ## Wiring status (post-Phase 3 T2)
//!
//! - `max_iterations` — **fully wired**: production reads from
//!   `agent_max_iterations()` env-var via T11/T12 single-source-of-truth.
//! - `force_text_after_truncations` — **inert in production** as of Phase 3 T2.
//!   Phase 3 T3 (N2-β) will wire it to drive force_text behavior in both
//!   delegates.
//!
//! ## Phase 3 T2 (N1-α)
//!
//! Removed `enable_tool_intent_nudge` and `max_tool_intent_nudges` (inert in
//! production — both delegates short-circuit empty tool calls before the
//! loop's nudge path could fire). The corresponding nudge logic in
//! `run_agentic_loop` is replaced by a `LoopOutcome::Failure` guard that
//! detects delegate contract violations.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticLoopConfig {
    pub max_iterations: usize,
    pub force_text_after_truncations: u32,
}

impl Default for AgenticLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            force_text_after_truncations: 2,
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
    }
}
