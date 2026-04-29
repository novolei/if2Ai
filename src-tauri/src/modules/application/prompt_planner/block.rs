//! Block types for prompt planning.
//!
//! MIG-007: Extracted from mod.rs for better modularity.

use crate::modules::application::memory_injection_service::MemoryInjectionSectionKind;

/// Canonical kinds of prompt blocks a [`super::PromptPlan`] can hold.
///
/// Closed enum on purpose: M4 harness traces depend on the alphabet
/// being stable. Adding a kind requires a contract bump.
///
/// MIG-006: Added Persona and Skill kinds for external contributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptBlockKind {
    /// The static system prompt produced by
    /// [`crate::modules::runtime::prompt::SystemPromptBuilder`] (one or more lines pre-joined by
    /// [`crate::modules::runtime::prompt::load_system_prompt`]).
    System,
    /// Soul block (optional, from resolved identity).
    Soul,
    /// Persona block (optional, from persona_id).
    Persona,
    /// Scenario profile block (chat/coding/research/planning/review).
    Scenario,
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
    /// MIG-008: Coding context block (workspace + tool surface).
    CodingContext,
    /// MIG-008: Continuation block for long coding sessions.
    Continuation,
    /// MEM-MOD-PD0: Day Awareness — gives the agent a sense of
    /// "today" (logical day per 04:00 cutoff), time-of-day, gap since
    /// the last conversation, and cumulative days in the relationship.
    /// Sits at priority 92 alongside Scenario.
    DayAwareness,
    /// MEM-MOD-P7 — durable cross-session observations about the user
    /// ("RL prefers terse replies", "RL ships at 3 AM"). Sits at
    /// priority 93 — above Scenario but below Soul/Persona — so the
    /// LLM reads identity first, then user-specific traits, then the
    /// scenario instructions that depend on them.
    LearnedTraits,
    /// FEAT-TE-003 — L1 mini index over the session history
    /// (last user goal, last assistant action, recent tool calls).
    /// Sits at priority 95 alongside `Soul` so the model gets a
    /// dynamic situational anchor *immediately* after identity but
    /// *before* the per-task scenario header. Always rendered as
    /// plain text by `crate::modules::runtime::context_compression::
    /// mini_index::build_mini_index`.
    MiniIndex,
}

impl PromptBlockKind {
    /// Map a memory-section kind onto its canonical prompt-block
    /// kind. Used while consuming
    /// [`super::memory_injection_service::MemoryInjectionArtifacts::prompt_sections`] in declared
    /// order.
    pub(super) fn from_memory_section(kind: MemoryInjectionSectionKind) -> Self {
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

/// Single block inside a [`super::PromptPlan`].
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
