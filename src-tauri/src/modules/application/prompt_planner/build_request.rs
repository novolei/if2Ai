//! Build request types for prompt planning.
//!
//! MIG-007: Extracted from mod.rs for better modularity.

use std::path::PathBuf;

use crate::modules::application::prompt_coordinator::PromptAssemblyDecision;
use crate::modules::identity::ResolvedIdentity;
use crate::modules::runtime::contracts::execution_mode::ScenarioProfileHint;

use crate::modules::application::memory_injection_service::MemoryInjectionArtifacts;

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
    Research,
    Planning,
    Review,
}

impl Default for PromptBuildMode {
    fn default() -> Self {
        Self::Chat
    }
}

impl PromptBuildMode {
    /// Map a runtime scenario hint onto the planner mode used to render
    /// scenario-aware prompt guidance.
    #[must_use]
    pub const fn from_scenario_hint(hint: ScenarioProfileHint) -> Self {
        match hint {
            ScenarioProfileHint::Chat => Self::Chat,
            ScenarioProfileHint::Coding => Self::Coding,
            ScenarioProfileHint::Research => Self::Research,
            ScenarioProfileHint::Planning => Self::Planning,
            ScenarioProfileHint::Review => Self::Review,
        }
    }
}

/// Options controlling prompt plan construction.
///
/// MIG-006: Enables strict validation mode and future extensions.
/// MIG-008: Added coding compaction fields.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptBuildOptions {
    /// Whether to include full diagnostics in the plan.
    pub include_diagnostics: bool,
    /// Whether to fail on validation issues (strict mode).
    pub strict_block_validation: bool,
    /// MIG-008: Estimated token count threshold for coding compaction.
    pub coding_compaction_estimated_tokens: Option<usize>,
    /// MIG-008: Message count threshold for coding compaction.
    pub coding_compaction_message_count: Option<usize>,
    /// MIG-008: Session snapshot for continuation block generation.
    pub coding_session_snapshot: Option<String>,
}

impl Default for PromptBuildOptions {
    fn default() -> Self {
        Self {
            include_diagnostics: true,
            strict_block_validation: false,
            coding_compaction_estimated_tokens: None,
            coding_compaction_message_count: None,
            coding_session_snapshot: None,
        }
    }
}

/// Per-turn input bundle for [`super::build_prompt_plan`].
///
/// Held as a struct so future slices can add fields (e.g. token
/// budget, reflection notes) without breaking call sites.
///
/// MIG-006: Added mode, identity, scenario, active_skill_ids, options fields.
pub struct BuildPromptPlanRequest {
    /// Session ID for trace identity computation.
    pub session_id: String,
    /// User message for this turn (used in trace ID computation).
    pub user_message: String,
    /// Working directory for this turn.
    pub workdir: PathBuf,
    /// Calendar date string (`%Y-%m-%d`) for system prompt
    /// interpolation.
    pub current_date: String,
    /// `std::env::consts::OS`.
    pub os_name: String,
    /// `std::env::consts::FAMILY`.
    pub os_family: String,
    /// Registered tool names for web-tool routing guide generation.
    pub registered_tool_names: Vec<String>,
    /// Optional memory injection artifacts produced by
    /// [`crate::modules::application::memory_injection_service::prepare_memory_injection`].
    pub memory_injection: Option<MemoryInjectionArtifacts>,
    /// Optional active strategy overlay text from the learning
    /// module.
    pub active_strategy_overlay: Option<String>,
    /// Caller tag for tracing (e.g. `"run_agent_turn"`,
    /// `"start_agent_stream"`).
    pub caller: &'static str,
    /// MIG-006: Prompt build mode (chat/coding/research/planning/review).
    pub mode: PromptBuildMode,
    /// Effective turn-level identity after settings/session resolution.
    pub resolved_identity: Option<ResolvedIdentity>,
    /// Advisory scenario profile surfaced by request intelligence.
    pub scenario_profile: Option<ScenarioProfileHint>,
    /// Explainable control-plane decision produced by the prompt coordinator.
    pub prompt_assembly_decision: Option<PromptAssemblyDecision>,
    /// MIG-006: Active skill identifiers.
    pub active_skill_ids: Vec<String>,
    /// MIG-006: Build options (diagnostics, strict validation).
    pub options: PromptBuildOptions,
    /// MEM-MOD-P7 — cross-session learned traits the agent has
    /// accumulated about the user.  Empty vec when the
    /// LearnedTraitsStore is unavailable or holds nothing yet, in
    /// which case the planner skips the `LearnedTraits` block
    /// entirely (no empty header).
    pub learned_traits: Vec<crate::modules::memory::learned_traits::LearnedTrait>,
}
