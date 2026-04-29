//! Safety-valve configuration for the agentic loop.
//!
//! Mirrors Steward's `AgenticLoopConfig`. All fields have defaults that
//! preserve current production behaviour.

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
