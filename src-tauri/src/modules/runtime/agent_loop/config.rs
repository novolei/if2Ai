//! Safety-valve configuration for the agentic loop.
//!
//! Mirrors Steward's `AgenticLoopConfig`. All fields have defaults that
//! preserve current production behaviour.
//!
//! Relocated from `application::turn_service::loop_config` in Phase 3 T1.
//!
//! ## Wiring status (post-Phase 2)
//!
//! - `max_iterations` — **fully wired**: production reads from
//!   `agent_max_iterations()` env-var via T11/T12 single-source-of-truth.
//! - `enable_tool_intent_nudge` / `max_tool_intent_nudges` — **inert in
//!   production**: both delegates short-circuit to `RespondResult::Text`
//!   when no tool calls are present, so `run_agentic_loop`'s nudge path
//!   is unreachable. Real nudge logic lives delegate-side
//!   (`stream_iteration::handle_no_tool_calls` for streaming, no equivalent
//!   in sync). Tracked for unification in Phase 3.
//! - `force_text_after_truncations` — **inert in production**: neither
//!   delegate consults `ctx.force_text` from the loop. Stream uses its own
//!   `force_final_response_next` (set by `repeated_tool_batch_count`),
//!   sync uses no force-text mechanism. Tracked for unification in Phase 3.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticLoopConfig {
    pub max_iterations: usize,
    pub enable_tool_intent_nudge: bool,
    pub max_tool_intent_nudges: u32,
    pub force_text_after_truncations: u32,
}

impl Default for AgenticLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            enable_tool_intent_nudge: true,
            max_tool_intent_nudges: 2,
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
        assert!(c.enable_tool_intent_nudge);
        assert_eq!(c.max_tool_intent_nudges, 2);
        assert_eq!(c.force_text_after_truncations, 2);
    }
}
