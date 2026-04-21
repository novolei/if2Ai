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

use std::path::PathBuf;

// Sanitize sub-module — extracted from `commands::agent` in GFR-002a.
pub mod sanitize;
pub(crate) use sanitize::{extend_sample_ids, sanitize_messages_for_provider, SanitizationStats};

use crate::modules::runtime::prompt::{load_system_prompt, PromptBuildError, SystemPromptBuilder};
use crate::modules::runtime::prompt_tools_guide::web_tools_routing_block;

use super::memory_injection_service::{MemoryInjectionArtifacts, MemoryInjectionSectionKind};

/// Canonical kinds of prompt blocks a [`PromptPlan`] can hold.
///
/// Closed enum on purpose: M4 harness traces depend on the alphabet
/// being stable. Adding a kind requires a contract bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptBlockKind {
    /// The static system prompt produced by
    /// [`SystemPromptBuilder`] (one or more lines pre-joined by
    /// [`load_system_prompt`]).
    System,
    /// Web-tool routing guide (`web_search` → `web_fetch` →
    /// `browser` escalation order). Present only when at least two
    /// of those tools are registered.
    WebToolsRoutingGuide,
    /// Pinned-memory section produced by the memory injection
    /// service.
    MemoryInjectionPinned,
    /// Compiled-memory section.
    MemoryInjectionCompiled,
    /// Memory-rules section (always emitted by
    /// [`crate::modules::memory::build_memory_injection`] when memory
    /// injection is enabled).
    MemoryInjectionRules,
    /// Per-turn retrieved-memory fragment produced by
    /// [`crate::modules::application::memory_injection_service::retrieve_memory_for_turn`].
    RetrievedMemory,
    /// Phase M5 closeout — overlay rendered from every
    /// currently `Active` candidate strategy in the M5
    /// learning registry.  Empty when no strategy is active or
    /// when active strategies carry only `Noop` definitions.
    ActiveStrategyOverlay,
}

impl PromptBlockKind {
    /// Map a memory-section kind onto its canonical prompt-block
    /// kind. Used while consuming
    /// [`MemoryInjectionArtifacts::prompt_sections`] in declared
    /// order.
    fn from_memory_section(kind: MemoryInjectionSectionKind) -> Self {
        match kind {
            MemoryInjectionSectionKind::Pinned => Self::MemoryInjectionPinned,
            MemoryInjectionSectionKind::Compiled => Self::MemoryInjectionCompiled,
            MemoryInjectionSectionKind::Rules => Self::MemoryInjectionRules,
            MemoryInjectionSectionKind::Retrieved => Self::RetrievedMemory,
        }
    }
}

/// Single block inside a [`PromptPlan`].
///
/// `title` is a short human-readable tag for trace UIs; `content`
/// is the raw text appended into the final prompt string.
//
// Only `Serialize` is derived: M4 harness needs to project the plan
// to JSON traces, but we never deserialise plans (the `&'static str`
// title would force a non-`'static` lifetime on `Deserialize`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptBlock {
    pub kind: PromptBlockKind,
    pub title: &'static str,
    pub content: String,
}

/// Ordered collection of [`PromptBlock`]s representing the static
/// prompt for one turn.
///
/// Use [`PromptPlan::join_into_text`] to render the canonical newline
/// separator (single `"\n"`) used by the legacy assembly path.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PromptPlan {
    pub blocks: Vec<PromptBlock>,
}

impl PromptPlan {
    /// Render the plan into a single string with the legacy
    /// `"\n"` separator (matches the previous `Vec<String>::join`
    /// in `commands/agent.rs`).
    #[must_use]
    pub fn join_into_text(&self) -> String {
        let parts: Vec<&str> = self.blocks.iter().map(|b| b.content.as_str()).collect();
        parts.join("\n")
    }

    /// Number of blocks. Useful for harness trace summaries.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
}

/// Inputs needed to build a chat-turn prompt plan.
///
/// Held as a struct so future slices can add fields (e.g. token
/// budget, reflection notes) without breaking call sites.
pub struct BuildPromptPlanRequest {
    /// Working directory the turn runs against. Drives runtime
    /// config + project context discovery in `load_system_prompt`.
    pub workdir: PathBuf,
    /// Calendar date string (`%Y-%m-%d`); kept caller-controlled so
    /// tests / replays can inject a fixed date.
    pub current_date: String,
    /// OS / family identifiers (`std::env::consts::OS` /
    /// `std::env::consts::FAMILY` in production).
    pub os_name: String,
    pub os_family: String,
    /// Names of tools currently registered, used by
    /// [`web_tools_routing_block`] to decide whether to emit the
    /// guide.
    pub registered_tool_names: Vec<String>,
    /// Phase M5 closeout — pre-resolved active-strategy overlay
    /// text appended as a `PromptBlockKind::ActiveStrategyOverlay`
    /// block.  `None` (or empty string) skips the block.
    /// Resolved by [`crate::modules::learning::ActiveStrategyOverlayResolver`]
    /// before this call.
    pub active_strategy_overlay: Option<String>,
    /// Pre-computed memory injection artifacts. `None` skips the
    /// memory blocks entirely (useful for tests or for callers that
    /// already injected memory through another path).
    pub memory_injection: Option<MemoryInjectionArtifacts>,
    /// Caller tag for tracing (e.g. `"run_agent_turn"`,
    /// `"start_agent_stream"`).
    pub caller: &'static str,
}

/// Output of [`build_prompt_plan`] — both the structured plan and
/// its rendered text, so callers do not have to render twice.
pub struct PromptPlanResult {
    pub plan: PromptPlan,
    pub text: String,
}

/// Errors surfaced by the planner.
#[derive(Debug, thiserror::Error)]
pub enum PromptPlannerError {
    #[error("failed to build system prompt: {0}")]
    Build(#[from] PromptBuildError),
}

/// Build the chat-turn prompt plan.
///
/// Block order (matches legacy `commands/agent.rs` assembly):
///
/// 1. `System` — `load_system_prompt` lines joined by `"\n"`.
/// 2. `WebToolsRoutingGuide` — only when at least two web tools are
///    registered.
/// 3. Memory injection sections in the order produced by
///    [`crate::modules::application::memory_injection_service::prepare_memory_injection`]:
///    Pinned → Compiled → Rules → Retrieved (any subset may be
///    absent).
///
/// On `load_system_prompt` failure the planner falls back to the
/// minimal `SystemPromptBuilder::new().render()` output, identical
/// to the legacy fallback path.
pub async fn build_prompt_plan(
    request: BuildPromptPlanRequest,
) -> Result<PromptPlanResult, PromptPlannerError> {
    let mut blocks: Vec<PromptBlock> = Vec::new();

    // 1. System prompt — preserve the legacy fallback shape.
    let system_lines = match load_system_prompt(
        request.workdir.clone(),
        request.current_date.clone(),
        request.os_name.clone(),
        request.os_family.clone(),
    ) {
        Ok(lines) => lines,
        Err(e) => {
            tracing::warn!(
                caller = request.caller,
                "[prompt_planner] Failed to build system prompt: {}, using fallback",
                e
            );
            vec![SystemPromptBuilder::new().render()]
        }
    };
    blocks.push(PromptBlock {
        kind: PromptBlockKind::System,
        title: "system",
        content: system_lines.join("\n"),
    });

    // 2. Web-tool routing guide (Phase 7C, slice 7C.4 parity).
    if let Some(guide) = web_tools_routing_block(&request.registered_tool_names) {
        blocks.push(PromptBlock {
            kind: PromptBlockKind::WebToolsRoutingGuide,
            title: "web_tools_routing_guide",
            content: guide,
        });
    }

    // 2b. Phase M5 closeout — active-strategy overlay block.
    // The active strategy registry resolves to zero or one
    // overlay text (singleton-active enforced by the rollout
    // service); empty string means no active strategy with a
    // runtime effect.
    if let Some(overlay) = request.active_strategy_overlay {
        if !overlay.trim().is_empty() {
            blocks.push(PromptBlock {
                kind: PromptBlockKind::ActiveStrategyOverlay,
                title: "active_strategy_overlay",
                content: overlay,
            });
        }
    }

    // 3. Memory injection blocks (Pinned / Compiled / Rules /
    //    Retrieved) in the order produced by the memory service.
    if let Some(artifacts) = request.memory_injection {
        for section in artifacts.prompt_sections {
            blocks.push(PromptBlock {
                kind: PromptBlockKind::from_memory_section(section.kind),
                title: title_for_memory_section(section.kind),
                content: section.content,
            });
        }
    }

    let plan = PromptPlan { blocks };
    let text = plan.join_into_text();
    Ok(PromptPlanResult { plan, text })
}

fn title_for_memory_section(kind: MemoryInjectionSectionKind) -> &'static str {
    match kind {
        MemoryInjectionSectionKind::Pinned => "memory_pinned",
        MemoryInjectionSectionKind::Compiled => "memory_compiled",
        MemoryInjectionSectionKind::Rules => "memory_rules",
        MemoryInjectionSectionKind::Retrieved => "retrieved_memory",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::application::memory_injection_service::{
        MemoryInjectionArtifacts, MemoryInjectionSection,
    };

    #[test]
    fn join_into_text_uses_legacy_newline_separator() {
        let plan = PromptPlan {
            blocks: vec![
                PromptBlock {
                    kind: PromptBlockKind::System,
                    title: "system",
                    content: "a".into(),
                },
                PromptBlock {
                    kind: PromptBlockKind::RetrievedMemory,
                    title: "retrieved_memory",
                    content: "b".into(),
                },
            ],
        };
        assert_eq!(plan.join_into_text(), "a\nb");
        assert_eq!(plan.block_count(), 2);
    }

    #[test]
    fn empty_plan_renders_to_empty_string() {
        assert_eq!(PromptPlan::default().join_into_text(), "");
    }

    #[tokio::test]
    async fn memory_section_kinds_map_to_canonical_block_kinds() {
        // Validate the kind mapping without touching the filesystem
        // (we skip the real `load_system_prompt` path by passing a
        // workdir that triggers the fallback branch).
        let artifacts = MemoryInjectionArtifacts {
            prompt_sections: vec![
                MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Pinned,
                    content: "P".into(),
                },
                MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Compiled,
                    content: "C".into(),
                },
                MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Rules,
                    content: "R".into(),
                },
                MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Retrieved,
                    content: "X".into(),
                },
            ],
            memory_items: Vec::new(),
        };
        let req = BuildPromptPlanRequest {
            workdir: PathBuf::from("/nonexistent/path/for/m1-tests"),
            current_date: "2026-04-20".into(),
            os_name: "macos".into(),
            os_family: "unix".into(),
            registered_tool_names: Vec::new(),
            memory_injection: Some(artifacts),
            active_strategy_overlay: None,
            caller: "prompt_planner_test",
        };
        let result = build_prompt_plan(req).await.expect("plan builds");
        // First block is always System (fallback or real).
        assert_eq!(result.plan.blocks[0].kind, PromptBlockKind::System);
        // Then the four memory sections in declared order.
        let memory_kinds: Vec<PromptBlockKind> =
            result.plan.blocks.iter().skip(1).map(|b| b.kind).collect();
        assert_eq!(
            memory_kinds,
            vec![
                PromptBlockKind::MemoryInjectionPinned,
                PromptBlockKind::MemoryInjectionCompiled,
                PromptBlockKind::MemoryInjectionRules,
                PromptBlockKind::RetrievedMemory,
            ]
        );
    }
}
