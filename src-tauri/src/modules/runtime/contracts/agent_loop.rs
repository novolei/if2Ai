//! Agent work-loop contracts.
//!
//! These payloads are emitted by the streaming turn service as
//! `agent-token` metadata events.  They intentionally describe the
//! routing / skill / final-report truth without introducing a second
//! runtime state store.

use serde::{Deserialize, Serialize};

/// Internal work-loop selected for a classified request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkLoopKind {
    /// Answer directly without a tool loop when the request is a tiny chat/question.
    DirectAnswer,
    /// Execute a single low-risk action directly.
    DirectExecute,
    /// Produce a plan and require confirmation before mutating work.
    PlanThenConfirm,
    /// Run the multi-step autonomous work loop under policy/approval controls.
    AutonomousWork,
    /// Route to a specialized surface instead of the default chat loop.
    SpecializedSurface,
}

/// Terminal outcome family every work loop must report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopOutcomeKind {
    /// The request completed normally.
    Completed,
    /// Execution is blocked on user approval.
    NeedsApproval,
    /// Execution needs more user input.
    NeedsUserInput,
    /// Execution failed but produced a concrete recovery plan.
    FailedWithPlan,
    /// Execution reached a configured budget/iteration limit.
    ExhaustedWithSummary,
}

/// Operation metadata captured when a loop pauses on approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOperationMetadata {
    /// Provider/tool-call identifier for the blocked operation.
    pub tool_call_id: String,
    /// Tool name that requires approval before the loop can continue.
    pub tool_name: String,
    /// Human-readable reason the operation was blocked.
    pub reason: String,
}

/// Work-loop decision projected for one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkLoopDecision {
    /// Selected internal loop.
    pub loop_kind: WorkLoopKind,
    /// Human-readable reason codes copied from the classifier/router.
    #[serde(default)]
    pub reason_codes: Vec<String>,
    /// Whether mutating tools require explicit user confirmation.
    pub requires_confirmation: bool,
    /// Optional route target for specialized surfaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_hint: Option<String>,
}

/// One skill candidate selected before the LLM/tool loop starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillResolutionCandidate {
    /// Stable skill identifier when known. Falls back to `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
    /// Skill name as exposed to `skill_view`.
    pub name: String,
    /// Skill source label: workspace, user, builtin, etc.
    pub source: String,
    /// Short reason explaining why the resolver surfaced it.
    pub reason: String,
    /// Deterministic score used only for sorting/debugging.
    pub score: u32,
    /// True when the source family is eligible for trusted auto-loading.
    #[serde(default)]
    pub trusted_source: bool,
    /// True when the loop may auto-load this skill into provider context.
    #[serde(default)]
    pub auto_load_allowed: bool,
    /// True when the SKILL.md content was loaded into provider context.
    #[serde(default)]
    pub loaded: bool,
    /// Reason this candidate was blocked from auto-loading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    /// Non-fatal warning captured while trying to load the skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_warning: Option<String>,
}

/// Per-turn skill-resolution snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillResolutionPlan {
    /// Session-selected active skill ids.
    #[serde(default)]
    pub active_skill_ids: Vec<String>,
    /// Deterministic local candidates from approved skill roots.
    #[serde(default)]
    pub candidates: Vec<SkillResolutionCandidate>,
    /// Discovery tools allowed to run automatically.
    #[serde(default)]
    pub auto_discovery_tools: Vec<String>,
    /// Whether the request explicitly asks to find/discover skills.
    pub should_load_find_skills: bool,
    /// Remote install policy for this turn.
    pub remote_install_policy: String,
    /// Skills whose trusted content was loaded into provider context.
    #[serde(default)]
    pub loaded_skill_names: Vec<String>,
    /// Candidates blocked from auto-loading.
    #[serde(default)]
    pub blocked_skill_names: Vec<String>,
    /// Non-fatal warnings produced by the skill loader.
    #[serde(default)]
    pub load_warnings: Vec<String>,
}

/// Final report emitted for every streaming work loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalRunReport {
    /// Selected work-loop kind for this run.
    pub loop_kind: WorkLoopKind,
    /// Final loop outcome.
    pub outcome: LoopOutcomeKind,
    /// Existing coarse task outcome: completed, partial_success, or failed.
    pub task_outcome: String,
    /// Terminal status/reason from the stream loop.
    pub terminal_status: String,
    /// Provider request id when available.
    pub request_id: String,
    /// Number of outer LLM/tool iterations.
    pub tool_loop_iterations: usize,
    /// True when at least one tool completed successfully.
    pub has_successful_tool: bool,
    /// True when a mutating tool completed successfully.
    pub has_successful_mutating_tool: bool,
    /// Retry cursor availability.
    pub resume_available: bool,
    /// Optional resume cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_cursor: Option<String>,
    /// Completed summary bullets.
    #[serde(default)]
    pub completed_items: Vec<String>,
    /// Failed/blocked summary bullets.
    #[serde(default)]
    pub failed_items: Vec<String>,
    /// Concrete next-step recommendations for the user.
    #[serde(default)]
    pub user_next_steps: Vec<String>,
    /// Skills loaded into provider context before the run.
    #[serde(default)]
    pub loaded_skills: Vec<String>,
    /// Skills blocked from provider context before the run.
    #[serde(default)]
    pub blocked_skills: Vec<String>,
    /// Skill-loader warnings captured during preparation.
    #[serde(default)]
    pub skill_warnings: Vec<String>,
}
