//! Canonical tool-pool builder and tool classification helpers.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::modules::api::ToolDefinition;
use crate::modules::runtime::contracts::agent_loop::{WorkLoopDecision, WorkLoopKind};

use super::evidence::{requires_memory_recall_evidence, requires_single_shell_command_evidence};

pub const MEMORY_READ_TOOLS: &[&str] =
    &["memory_recall", "memory_recall_explicit", "memory_export"];
pub const READ_ONLY_PLANNING_TOOLS: &[&str] = &[
    "read_file",
    "glob_search",
    "grep_search",
    "content_search",
    "web_search",
    "web_fetch",
    "web_research",
    "memory_recall",
    "memory_recall_explicit",
    "conversation_search",
    "skill",
    "skill_find",
    "skill_search",
    "skill_view",
    "skills_list",
    "skills_categories",
    "tool_search",
    "json_parse",
    "StructuredOutput",
    "cron_list",
    "cron_runs",
    "memory_export",
];
pub const MUTATING_TOOL_NAMES: &[&str] = &[
    "file_write",
    "write_file",
    "file_edit",
    "edit_file",
    "NotebookEdit",
    "TodoWrite",
    "memory_store",
    "memory_forget",
    "memory_purge",
    "memory_update",
    "memory_link",
    "memory_consolidate",
    "pin_memory",
    "unpin_memory",
    "cron_add",
    "cron_remove",
    "cron_run",
    "skill_manage",
    "bash",
    "PowerShell",
    "REPL",
    "agent",
    "browser",
    "http_request",
    "SendUserMessage",
];

/// Canonical provider-visible tool pool after skill and work-loop policy.
#[derive(Debug, Clone)]
pub struct CanonicalToolPool {
    pub definitions: Vec<ToolDefinition>,
    pub tool_names: Vec<String>,
    pub schema_hash: String,
    pub policy: String,
}

/// Build the canonical tool pool for a work-loop turn after applying skill
/// attenuation and loop-level tool-exposure policy.
#[must_use]
pub fn build_canonical_tool_pool(
    tool_defs: Vec<ToolDefinition>,
    active_skill_ids: &[String],
    work_loop: &WorkLoopDecision,
) -> CanonicalToolPool {
    let tool_defs = crate::modules::skills::attenuation::attenuate_tool_definitions(
        tool_defs,
        active_skill_ids,
    );
    let mut definitions = enforce_tool_definitions_for_loop(work_loop, tool_defs);
    definitions.sort_by(|left, right| left.name.cmp(&right.name));
    let tool_names = definitions
        .iter()
        .map(|definition| definition.name.clone())
        .collect::<Vec<_>>();
    let schema_hash = stable_tool_schema_hash(&definitions);
    CanonicalToolPool {
        definitions,
        tool_names,
        schema_hash,
        policy: tool_pool_policy_label(work_loop).to_string(),
    }
}

/// Compute a stable hash over the full tool schema set for change-detection.
pub fn stable_tool_schema_hash(definitions: &[ToolDefinition]) -> String {
    let mut hasher = DefaultHasher::new();
    for definition in definitions {
        definition.name.hash(&mut hasher);
        definition.description.hash(&mut hasher);
        serde_json::to_string(&definition.input_schema)
            .unwrap_or_default()
            .hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

pub fn tool_pool_policy_label(work_loop: &WorkLoopDecision) -> &'static str {
    match work_loop.loop_kind {
        WorkLoopKind::DirectAnswer => "direct_answer_hidden",
        WorkLoopKind::SpecializedSurface => "specialized_surface_hidden",
        WorkLoopKind::PlanThenConfirm => "plan_then_confirm_read_only",
        WorkLoopKind::DirectExecute if requires_memory_recall_evidence(work_loop) => {
            "direct_execute_memory_read"
        }
        WorkLoopKind::DirectExecute => "direct_execute_visible",
        WorkLoopKind::AutonomousWork => "autonomous_work_visible",
    }
}

/// Apply the selected loop's tool-exposure policy before provider request assembly.
#[must_use]
pub fn enforce_tool_definitions_for_loop(
    work_loop: &WorkLoopDecision,
    tool_defs: Vec<ToolDefinition>,
) -> Vec<ToolDefinition> {
    match work_loop.loop_kind {
        WorkLoopKind::DirectAnswer | WorkLoopKind::SpecializedSurface => Vec::new(),
        WorkLoopKind::PlanThenConfirm => tool_defs
            .into_iter()
            .filter(|tool| is_read_only_planning_tool(&tool.name))
            .collect(),
        WorkLoopKind::DirectExecute if requires_memory_recall_evidence(work_loop) => tool_defs
            .into_iter()
            .filter(|tool| is_memory_read_tool(&tool.name))
            .collect(),
        WorkLoopKind::DirectExecute if requires_single_shell_command_evidence(work_loop) => {
            tool_defs
                .into_iter()
                .filter(|tool| tool.name == "bash")
                .collect()
        }
        WorkLoopKind::DirectExecute | WorkLoopKind::AutonomousWork => tool_defs,
    }
}

/// Return true when `tool_name` is a read-only planning tool.
pub fn is_read_only_planning_tool(tool_name: &str) -> bool {
    READ_ONLY_PLANNING_TOOLS.contains(&tool_name)
}

/// Return true when `tool_name` is a memory-read tool.
pub fn is_memory_read_tool(tool_name: &str) -> bool {
    MEMORY_READ_TOOLS.contains(&tool_name)
}

/// Return true when the tool is likely to mutate external state.
pub fn tool_likely_mutates(tool_name: &str, input_json: &str) -> bool {
    if MUTATING_TOOL_NAMES.contains(&tool_name) {
        return true;
    }
    input_json_contains_mutating_method(input_json)
}

/// Return true when the JSON input carries an HTTP mutating method.
pub fn input_json_contains_mutating_method(input_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(input_json) else {
        return false;
    };
    let method = value
        .get("method")
        .or_else(|| value.get("http_method"))
        .and_then(|method| method.as_str())
        .map(str::to_ascii_uppercase);
    matches!(method.as_deref(), Some("POST" | "PUT" | "PATCH" | "DELETE"))
}

/// Return true when a `ContentBlock` constitutes successful mutating tool evidence.
pub fn block_is_mutating_tool_evidence(
    block: &crate::modules::runtime::session::ContentBlock,
) -> bool {
    use crate::modules::runtime::session::ContentBlock;
    match block {
        ContentBlock::ToolUse { .. } => false,
        ContentBlock::ToolResult {
            tool_name,
            is_error,
            ..
        } => !*is_error && MUTATING_TOOL_NAMES.contains(&tool_name.as_str()),
        ContentBlock::Text { .. } => false,
    }
}
