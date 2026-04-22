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
use merge::merge_external_contributions;

use crate::modules::runtime::prompt::{load_system_prompt, PromptBuildError, SystemPromptBuilder};
use crate::modules::runtime::prompt_tools_guide::web_tools_routing_block;

use super::memory_injection_service::{MemoryInjectionArtifacts, MemoryInjectionSectionKind};

/// Canonical kinds of prompt blocks a [`PromptPlan`] can hold.
///
/// Closed enum on purpose: M4 harness traces depend on the alphabet
/// being stable. Adding a kind requires a contract bump.
///
/// MIG-006: Added Persona and Skill kinds for external contributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptBlockKind {
    /// The static system prompt produced by
    /// [`SystemPromptBuilder`] (one or more lines pre-joined by
    /// [`load_system_prompt`]).
    System,
    /// Persona block (optional, from persona_id).
    Persona,
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
    /// Skills block (optional, from active_skill_ids).
    Skill,
    /// MCP (Model Context Protocol) contribution block.
    Mcp,
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

/// Source metadata for a [`PromptBlock`].
///
/// Tracks which subsystem contributed this block and optionally
/// references the specific source (e.g., file path, memory ID).
/// Used by harness for attribution and debugging.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PromptBlockSource {
    /// Subsystem that produced this block (e.g., "system_prompt", "memory", "learning").
    pub subsystem: String,
    /// Optional reference to the specific source within the subsystem.
    pub reference: Option<String>,
}

/// Validation issue encountered during prompt plan construction.
///
/// Used to record warnings or errors without failing the build
/// (unless strict validation mode is enabled in future phases).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PromptValidationIssue {
    /// Machine-readable issue code (e.g., "forbidden_sensitive_external_block").
    pub code: String,
    /// Human-readable issue description.
    pub message: String,
}

/// External contribution to a prompt plan.
///
/// MIG-006: Allows subsystems (Memory, MCP, Skills, Learning) to
/// contribute blocks without modifying the planner core.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PromptContribution {
    pub kind: PromptBlockKind,
    pub title: String,
    pub body: String,
    pub source: PromptBlockSource,
}

/// Prompt build mode.
///
/// MIG-006: Distinguishes between different execution contexts.
/// Chat mode is the default; Coding mode will have specialized
/// augmentation in Phase 4 (MIG-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptBuildMode {
    Chat,
    Coding,
}

impl Default for PromptBuildMode {
    fn default() -> Self {
        Self::Chat
    }
}

/// Options controlling prompt plan construction.
///
/// MIG-006: Enables strict validation mode and future extensions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptBuildOptions {
    /// Whether to include full diagnostics in the plan.
    pub include_diagnostics: bool,
    /// Whether to fail on validation issues (strict mode).
    pub strict_block_validation: bool,
}

impl Default for PromptBuildOptions {
    fn default() -> Self {
        Self {
            include_diagnostics: true,
            strict_block_validation: false,
        }
    }
}

/// Single block inside a [`PromptPlan`].
///
/// `title` is a short human-readable tag for trace UIs; `content`
/// is the raw text appended into the final prompt string.
///
/// MIG-005: Added `source`, `priority`, and `is_sensitive` fields
/// to align with UClaw's PromptBlock structure for harness attribution
/// and future token budget optimization.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptBlock {
    /// Unique identifier for this block within the plan (e.g., "system", "memory_pinned-0").
    pub id: String,
    pub kind: PromptBlockKind,
    pub title: String,
    pub content: String,
    /// Source metadata for attribution and debugging.
    pub source: PromptBlockSource,
    /// Priority for ordering (100=highest, 0=lowest). Used for future token budget optimization.
    pub priority: i32,
    /// Whether this block contains sensitive content that should be redacted in diagnostics.
    pub is_sensitive: bool,
}

/// Diagnostic metadata for a [`PromptPlan`].
///
/// Provides traceability and debugging information without exposing
/// sensitive prompt content. Used by harness traces and diagnostics UIs.
///
/// MIG-005: Added `validation_issues` field to record warnings/errors
/// encountered during plan construction.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptPlanDiagnostics {
    /// Unique trace identifier for this plan.
    pub trace_id: String,
    /// Ordered list of block kinds in the plan.
    pub block_kinds: Vec<PromptBlockKind>,
    /// Total number of blocks.
    pub block_count: usize,
    /// Redacted preview of each block (first 48 chars or "[REDACTED]" for sensitive content).
    pub redacted_preview: Vec<String>,
    /// Validation issues encountered during plan construction.
    pub validation_issues: Vec<PromptValidationIssue>,
}

/// Ordered collection of [`PromptBlock`]s representing the static
/// prompt for one turn.
///
/// Use [`PromptPlan::join_into_text`] to render the canonical newline
/// separator (single `"\n"`) used by the legacy assembly path.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptPlan {
    /// Unique trace identifier computed from session context and block hash.
    pub trace_id: String,
    /// SHA256 hash of all block content for plan identity.
    pub block_hash: String,
    /// Diagnostic metadata for traceability.
    pub diagnostics: PromptPlanDiagnostics,
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
///
/// MIG-006: Added mode, persona_id, active_skill_ids, options fields.
pub struct BuildPromptPlanRequest {
    /// Session ID for trace identity computation.
    pub session_id: String,
    /// User message for trace identity computation.
    pub user_message: String,
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
    /// MIG-006: Prompt build mode (Chat / Coding).
    pub mode: PromptBuildMode,
    /// MIG-006: Optional persona identifier.
    pub persona_id: Option<String>,
    /// MIG-006: Active skill identifiers.
    pub active_skill_ids: Vec<String>,
    /// MIG-006: Build options (diagnostics, strict validation).
    pub options: PromptBuildOptions,
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
/// MIG-006: Now accepts external_contributions parameter for subsystem
/// contributions (Memory, MCP, Skills, Learning).
///
/// Block order (matches legacy `commands/agent.rs` assembly):
///
/// 1. `System` — `load_system_prompt` lines joined by `"\n"`.
/// 2. `Persona` — optional, from persona_id.
/// 3. `WebToolsRoutingGuide` — only when at least two web tools are
///    registered.
/// 4. Memory injection sections in the order produced by
///    [`crate::modules::application::memory_injection_service::prepare_memory_injection`]:
///    Pinned → Compiled → Rules → Retrieved (any subset may be
///    absent).
/// 5. `Skill` — optional, from active_skill_ids.
/// 6. External contributions merged by kind.
///
/// On `load_system_prompt` failure the planner falls back to the
/// minimal `SystemPromptBuilder::new().render()` output, identical
/// to the legacy fallback path.
pub async fn build_prompt_plan(
    request: BuildPromptPlanRequest,
    external_contributions: Vec<PromptContribution>,
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
        id: "system".to_string(),
        kind: PromptBlockKind::System,
        title: "system".to_string(),
        content: system_lines.join("\n"),
        source: PromptBlockSource {
            subsystem: "system_prompt".to_string(),
            reference: None,
        },
        priority: 100,
        is_sensitive: true,
    });

    // 1b. MIG-006: Persona block (optional).
    if let Some(persona_id) = request.persona_id {
        blocks.push(PromptBlock {
            id: "persona".to_string(),
            kind: PromptBlockKind::Persona,
            title: "persona".to_string(),
            content: persona_id,
            source: PromptBlockSource {
                subsystem: "persona".to_string(),
                reference: None,
            },
            priority: 95,
            is_sensitive: true,
        });
    }

    // 2. Web-tool routing guide (Phase 7C, slice 7C.4 parity).
    if let Some(guide) = web_tools_routing_block(&request.registered_tool_names) {
        blocks.push(PromptBlock {
            id: "web_tools_routing_guide".to_string(),
            kind: PromptBlockKind::WebToolsRoutingGuide,
            title: "web_tools_routing_guide".to_string(),
            content: guide,
            source: PromptBlockSource {
                subsystem: "tool_routing".to_string(),
                reference: None,
            },
            priority: 90,
            is_sensitive: false,
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
                id: "active_strategy_overlay".to_string(),
                kind: PromptBlockKind::ActiveStrategyOverlay,
                title: "active_strategy_overlay".to_string(),
                content: overlay,
                source: PromptBlockSource {
                    subsystem: "learning".to_string(),
                    reference: None,
                },
                priority: 85,
                is_sensitive: false,
            });
        }
    }

    // 3. Memory injection blocks (Pinned / Compiled / Rules /
    //    Retrieved) in the order produced by the memory service.
    if let Some(artifacts) = request.memory_injection {
        let mut memory_block_counters: std::collections::HashMap<MemoryInjectionSectionKind, usize> =
            std::collections::HashMap::new();
        for section in artifacts.prompt_sections {
            let counter = memory_block_counters.entry(section.kind).or_insert(0);
            let block_id = format!("{}-{}", title_for_memory_section(section.kind), counter);
            let title = title_for_memory_section(section.kind);
            let kind = PromptBlockKind::from_memory_section(section.kind);

            // MIG-005: Assign priority and is_sensitive based on memory section kind
            let (priority, is_sensitive) = match section.kind {
                MemoryInjectionSectionKind::Pinned => (80, false),
                MemoryInjectionSectionKind::Compiled => (70, false),
                MemoryInjectionSectionKind::Rules => (60, false),
                MemoryInjectionSectionKind::Retrieved => (50, true),
            };

            *counter += 1;
            blocks.push(PromptBlock {
                id: block_id,
                kind,
                title: title.to_string(),
                content: section.content,
                source: PromptBlockSource {
                    subsystem: "memory".to_string(),
                    reference: None,
                },
                priority,
                is_sensitive,
            });
        }
    }

    // 3b. MIG-006: Skill block (optional).
    if !request.active_skill_ids.is_empty() {
        blocks.push(PromptBlock {
            id: "skill".to_string(),
            kind: PromptBlockKind::Skill,
            title: "skill".to_string(),
            content: request.active_skill_ids.join(", "),
            source: PromptBlockSource {
                subsystem: "skill_registry".to_string(),
                reference: None,
            },
            priority: 40,
            is_sensitive: false,
        });
    }

    // 4. MIG-006: Merge external contributions.
    let (blocks, validation_issues) = merge_external_contributions(
        blocks,
        external_contributions,
        request.options.strict_block_validation,
    )?;

    // Compute trace metadata
    let block_hash = compute_block_hash(&blocks);
    let trace_id = compute_trace_id(&request.session_id, &request.user_message, &block_hash);
    let diagnostics = build_diagnostics(trace_id.clone(), &blocks, validation_issues);

    let plan = PromptPlan {
        trace_id,
        block_hash,
        diagnostics,
        blocks,
    };
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

/// Compute SHA256 hash of all blocks for plan identity.
/// MIG-005: Include source.subsystem in hash computation for stability.
fn compute_block_hash(blocks: &[PromptBlock]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for block in blocks {
        hasher.update(format!(
            "{:?}|{}|{}|{}\n",
            block.kind, block.title, block.source.subsystem, block.content
        ));
    }
    hex::encode(hasher.finalize())
}

/// Compute unique trace ID from session context and block hash.
fn compute_trace_id(session_id: &str, user_message: &str, block_hash: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(user_message.as_bytes());
    hasher.update(block_hash.as_bytes());
    let hex_hash = hex::encode(hasher.finalize());
    format!("trace_{}", &hex_hash[..16])
}

/// Build diagnostic metadata for the plan.
/// MIG-005: Use block.is_sensitive instead of kind matching for redaction.
/// MIG-006: Accept validation_issues parameter.
fn build_diagnostics(
    trace_id: String,
    blocks: &[PromptBlock],
    validation_issues: Vec<PromptValidationIssue>,
) -> PromptPlanDiagnostics {
    let redacted_preview = blocks
        .iter()
        .map(|b| {
            if b.is_sensitive {
                format!("{:?}: [REDACTED {} chars]", b.kind, b.content.chars().count())
            } else {
                let preview: String = b.content.chars().take(48).collect();
                format!("{:?}: {}", b.kind, preview)
            }
        })
        .collect();

    PromptPlanDiagnostics {
        trace_id: trace_id.clone(),
        block_kinds: blocks.iter().map(|b| b.kind).collect(),
        block_count: blocks.len(),
        redacted_preview,
        validation_issues,
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

    #[test]
    fn compute_block_hash_is_stable() {
        let blocks = vec![
            PromptBlock {
                id: "system".to_string(),
                kind: PromptBlockKind::System,
                title: "system".to_string(),
                content: "test content".into(),
                source: PromptBlockSource {
                    subsystem: "system_prompt".to_string(),
                    reference: None,
                },
                priority: 100,
                is_sensitive: true,
            },
        ];
        let hash1 = compute_block_hash(&blocks);
        let hash2 = compute_block_hash(&blocks);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 hex is 64 chars
    }

    #[test]
    fn compute_trace_id_includes_session_and_message() {
        let trace1 = compute_trace_id("session1", "hello", "hash123");
        let trace2 = compute_trace_id("session2", "hello", "hash123");
        let trace3 = compute_trace_id("session1", "world", "hash123");

        assert!(trace1.starts_with("trace_"));
        assert_ne!(trace1, trace2); // Different session
        assert_ne!(trace1, trace3); // Different message
        assert_eq!(trace1.len(), 22); // "trace_" + 16 hex chars
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
            session_id: "test_session".to_string(),
            user_message: "test message".to_string(),
            workdir: PathBuf::from("/nonexistent/path/for/m1-tests"),
            current_date: "2026-04-20".into(),
            os_name: "macos".into(),
            os_family: "unix".into(),
            registered_tool_names: Vec::new(),
            memory_injection: Some(artifacts),
            active_strategy_overlay: None,
            caller: "prompt_planner_test",
            mode: PromptBuildMode::default(),
            persona_id: None,
            active_skill_ids: Vec::new(),
            options: PromptBuildOptions::default(),
        };
        let result = build_prompt_plan(req, Vec::new()).await.expect("plan builds");
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

    // MIG-006: Tests for external contributions
    #[tokio::test]
    async fn external_contributions_merge_correctly() {
        let req = BuildPromptPlanRequest {
            session_id: "test".to_string(),
            user_message: "test".to_string(),
            workdir: PathBuf::from("/nonexistent"),
            current_date: "2026-04-22".into(),
            os_name: "macos".into(),
            os_family: "unix".into(),
            registered_tool_names: Vec::new(),
            memory_injection: None,
            active_strategy_overlay: None,
            caller: "test",
            mode: PromptBuildMode::Chat,
            persona_id: None,
            active_skill_ids: Vec::new(),
            options: PromptBuildOptions::default(),
        };
        let external = vec![PromptContribution {
            kind: PromptBlockKind::Mcp,
            title: "mcp_test".to_string(),
            body: "mcp content".to_string(),
            source: PromptBlockSource {
                subsystem: "mcp".to_string(),
                reference: None,
            },
        }];
        let result = build_prompt_plan(req, external).await.expect("plan builds");
        assert!(result.plan.blocks.iter().any(|b| b.kind == PromptBlockKind::Mcp));
    }

    #[tokio::test]
    async fn strict_mode_rejects_forbidden_external_blocks() {
        let req = BuildPromptPlanRequest {
            session_id: "test".to_string(),
            user_message: "test".to_string(),
            workdir: PathBuf::from("/nonexistent"),
            current_date: "2026-04-22".into(),
            os_name: "macos".into(),
            os_family: "unix".into(),
            registered_tool_names: Vec::new(),
            memory_injection: None,
            active_strategy_overlay: None,
            caller: "test",
            mode: PromptBuildMode::Chat,
            persona_id: None,
            active_skill_ids: Vec::new(),
            options: PromptBuildOptions {
                include_diagnostics: true,
                strict_block_validation: true,
            },
        };
        let external = vec![PromptContribution {
            kind: PromptBlockKind::System,
            title: "override".to_string(),
            body: "nope".to_string(),
            source: PromptBlockSource {
                subsystem: "ext".to_string(),
                reference: None,
            },
        }];
        assert!(build_prompt_plan(req, external).await.is_err());
    }

    #[tokio::test]
    async fn persona_and_skill_blocks_generated() {
        let req = BuildPromptPlanRequest {
            session_id: "test".to_string(),
            user_message: "test".to_string(),
            workdir: PathBuf::from("/nonexistent"),
            current_date: "2026-04-22".into(),
            os_name: "macos".into(),
            os_family: "unix".into(),
            registered_tool_names: Vec::new(),
            memory_injection: None,
            active_strategy_overlay: None,
            caller: "test",
            mode: PromptBuildMode::Chat,
            persona_id: Some("test_persona".to_string()),
            active_skill_ids: vec!["skill1".to_string(), "skill2".to_string()],
            options: PromptBuildOptions::default(),
        };
        let result = build_prompt_plan(req, Vec::new()).await.expect("plan builds");
        assert!(result.plan.blocks.iter().any(|b| b.kind == PromptBlockKind::Persona));
        assert!(result.plan.blocks.iter().any(|b| b.kind == PromptBlockKind::Skill));
    }

    #[tokio::test]
    async fn validation_issues_recorded_in_non_strict_mode() {
        let req = BuildPromptPlanRequest {
            session_id: "test".to_string(),
            user_message: "test".to_string(),
            workdir: PathBuf::from("/nonexistent"),
            current_date: "2026-04-22".into(),
            os_name: "macos".into(),
            os_family: "unix".into(),
            registered_tool_names: Vec::new(),
            memory_injection: None,
            active_strategy_overlay: None,
            caller: "test",
            mode: PromptBuildMode::Chat,
            persona_id: None,
            active_skill_ids: Vec::new(),
            options: PromptBuildOptions {
                include_diagnostics: true,
                strict_block_validation: false,
            },
        };
        let external = vec![PromptContribution {
            kind: PromptBlockKind::Persona,
            title: "override".to_string(),
            body: "nope".to_string(),
            source: PromptBlockSource {
                subsystem: "ext".to_string(),
                reference: None,
            },
        }];
        let result = build_prompt_plan(req, external).await.expect("plan builds");
        assert!(!result.plan.diagnostics.validation_issues.is_empty());
    }
