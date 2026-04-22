//! Prompt planner skeleton (Phase M1.3, refined in M1.4).
//!
//! Migrates the static-side prompt assembly out of
//! [`crate::commands::agent`] into a structured [`PromptPlan`] so:
//!
//! - `commands::agent::run_agent_turn` and `start_agent_stream` stop
//!   directly stitching `Vec<String>` from `load_system_prompt` +
//!   `web_tools_routing_block` + memory injection helpers.
//! - M4 harness can record prompt composition by serialising the
//!   plan, instead of re-walking the produced string.
//! - Per-turn dynamic fragments (retrieved memory, future
//!   reflection notes) attach as additional [`PromptBlock`]s without
//!   re-running the static path.
//!
//! M1.4 refactor: this module no longer reaches into
//! `crate::modules::memory` directly. Callers must produce a
//! [`crate::modules::application::memory_injection_service::MemoryInjectionArtifacts`]
//! first (typically via
//! [`crate::modules::application::memory_injection_service::prepare_memory_injection`])
//! and pass it through [`BuildPromptPlanRequest::memory_injection`].
//!
//! MIG-007: Modularized into submodules (block, diagnostics, build_request, planner).
//!
//! Out of scope for this skeleton (deferred to later slices):
//! - Token budgeting / block reordering / dropping (M4 governor +
//!   M3 memory policy).
//! - Plan→harness trace projection (M4).
//!
//! Hard rules:
//! 1. This module MUST NOT import from `crate::commands::*`.
//! 2. The order of blocks produced by [`build_prompt_plan`] MUST
//!    match the legacy assembly order in `commands/agent.rs` so the
//!    M1.3 / M1.4 cut-over preserves the rendered prompt
//!    byte-for-byte (modulo trailing newline behaviour, which is
//!    unchanged).

// Sanitize sub-module — extracted from `commands::agent` in GFR-002a.
pub mod sanitize;
pub(crate) use sanitize::{extend_sample_ids, sanitize_messages_for_provider, SanitizationStats};

// Preflight estimators sub-module — extracted from `commands::agent` in GFR-002c.
pub mod preflight;
pub(crate) use preflight::{
    estimate_messages_char_count, estimate_messages_token_count, summarize_message_for_budget,
};

// Governor sub-module — extracted from `commands::agent` in GFR-002b.
pub mod governor;
pub(crate) use governor::{ContextGovernor, RequestPreflightStats};

// MIG-006: External contributions merging logic.
mod merge;

// MIG-007: Modularized prompt planner components.
mod block;
mod build_request;
mod diagnostics;
mod planner;

// Re-export public types for external callers.
pub use block::{PromptBlock, PromptBlockKind, PromptBlockSource, PromptContribution};
pub use build_request::{BuildPromptPlanRequest, PromptBuildMode, PromptBuildOptions};
pub use diagnostics::{PromptPlanDiagnostics, PromptValidationIssue};
pub use planner::{build_prompt_plan, PromptPlan, PromptPlanResult, PromptPlannerError};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::modules::application::memory_injection_service::{
        MemoryInjectionArtifacts, MemoryInjectionSection, MemoryInjectionSectionKind,
    };

    #[test]
    fn join_into_text_uses_legacy_newline_separator() {
        let plan = PromptPlan {
            trace_id: "trace_test123".to_string(),
            block_hash: "hash123".to_string(),
            diagnostics: PromptPlanDiagnostics {
                trace_id: "trace_test123".to_string(),
                block_kinds: vec![PromptBlockKind::System, PromptBlockKind::RetrievedMemory],
                block_count: 2,
                redacted_preview: vec![],
                validation_issues: vec![],
            },
            blocks: vec![
                PromptBlock {
                    id: "system".to_string(),
                    kind: PromptBlockKind::System,
                    title: "system".to_string(),
                    content: "a".into(),
                    source: PromptBlockSource {
                        subsystem: "system_prompt".to_string(),
                        reference: None,
                    },
                    priority: 100,
                    is_sensitive: true,
                },
                PromptBlock {
                    id: "retrieved_memory-0".to_string(),
                    kind: PromptBlockKind::RetrievedMemory,
                    title: "retrieved_memory".to_string(),
                    content: "b".into(),
                    source: PromptBlockSource {
                        subsystem: "memory".to_string(),
                        reference: None,
                    },
                    priority: 50,
                    is_sensitive: true,
                },
            ],
        };
        assert_eq!(plan.join_into_text(), "a\nb");
        assert_eq!(plan.block_count(), 2);
    }
}
