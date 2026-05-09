//! `requires_*` predicates and `*_prompt_contribution` builders for the work loop.

use crate::modules::application::prompt_planner::{
    PromptBlockKind, PromptBlockSource, PromptContribution,
};
use crate::modules::runtime::contracts::agent_loop::WorkLoopDecision;

use super::WorkLoopRouteContext;
use super::tool_pool::{tool_likely_mutates, is_read_only_planning_tool};

pub const MEMORY_RECALL_INTENT_REASON: &str = "memory_recall_intent";
pub const TOOL_REQUIRED_WORK_INTENT_REASON: &str = "tool_required_work_intent";
pub const CONTINUATION_INTENT_REASON: &str = "continuation_intent";
pub const INHERITED_TOOL_REQUIRED_WORK_INTENT_REASON: &str =
    "inherited_tool_required_work_intent";
pub const SIMPLE_SHELL_COMMAND_INTENT_REASON: &str = "simple_shell_command_intent";

/// Return true when a work loop must retrieve memory evidence before answering.
#[must_use]
pub fn requires_memory_recall_evidence(work_loop: &WorkLoopDecision) -> bool {
    work_loop
        .reason_codes
        .iter()
        .any(|reason| reason == MEMORY_RECALL_INTENT_REASON)
}

/// Prompt contribution that turns memory-introspection into evidence-seeking work.
#[must_use]
pub fn memory_recall_prompt_contribution(
    work_loop: &WorkLoopDecision,
) -> Option<PromptContribution> {
    requires_memory_recall_evidence(work_loop).then(|| PromptContribution {
        kind: PromptBlockKind::System,
        title: "Memory Recall Intent".to_string(),
        body: "[memory_recall_intent]\nThe user is asking what you remember about them. Before answering, call `memory_recall` with a concise query about the user, or `memory_export` if a broad memory overview is needed. The memory tools return `[memory_recall_evidence]` with deduplicated facts and duplicate counts. Base the answer only on those returned facts, merge equivalent facts into one user-facing item, and do not repeat duplicate facts from Episode/canonical variants. Do not answer with a filler like \"let me check\" unless a memory tool result has been observed. If no memories are returned, say that clearly and explain how the user can add or correct memory.".to_string(),
        source: PromptBlockSource {
            subsystem: "memory".to_string(),
            reference: Some("turn_service.work_loop.memory_recall_intent".to_string()),
        },
    })
}

/// Return true when a work loop must produce concrete tool evidence.
#[must_use]
pub fn requires_tool_execution_evidence(work_loop: &WorkLoopDecision) -> bool {
    work_loop
        .reason_codes
        .iter()
        .any(|reason| reason == TOOL_REQUIRED_WORK_INTENT_REASON)
}

/// Return true when the turn is an exact shell command request that should
/// execute once and summarize, not resume broader project work.
#[must_use]
pub fn requires_single_shell_command_evidence(work_loop: &WorkLoopDecision) -> bool {
    work_loop
        .reason_codes
        .iter()
        .any(|reason| reason == SIMPLE_SHELL_COMMAND_INTENT_REASON)
}

/// Prompt contribution for exact shell-command turns.
#[must_use]
pub fn single_shell_command_prompt_contribution(
    work_loop: &WorkLoopDecision,
) -> Option<PromptContribution> {
    requires_single_shell_command_evidence(work_loop).then(|| PromptContribution {
        kind: PromptBlockKind::CodingContext,
        title: "Single Shell Command".to_string(),
        body: "[simple_shell_command_intent]\nThe user's latest message is an exact shell command. Call `bash` once with that command, then stop using tools and answer with the command output only plus a short note if needed. Ignore stale compacted-summary \"current work\" instructions for this turn. Do not inspect unrelated files or continue previous artifact work unless the latest user message explicitly asks to continue it.".to_string(),
        source: PromptBlockSource {
            subsystem: "work_loop".to_string(),
            reference: Some("turn_service.work_loop.simple_shell_command_intent".to_string()),
        },
    })
}

/// Prompt contribution that prevents artifact-building turns from answering in prose only.
#[must_use]
pub fn tool_required_prompt_contribution(
    work_loop: &WorkLoopDecision,
) -> Option<PromptContribution> {
    let continuation_line = if work_loop
        .reason_codes
        .iter()
        .any(|reason| reason == INHERITED_TOOL_REQUIRED_WORK_INTENT_REASON)
    {
        "\nThe user is continuing a previous unfinished tool-required task. Reconstruct the concrete goal from recent conversation context and continue the work with tools."
    } else {
        ""
    };
    requires_tool_execution_evidence(work_loop).then(|| PromptContribution {
        kind: PromptBlockKind::CodingContext,
        title: "Tool Required Work Intent".to_string(),
        body: format!("[tool_required_work_intent]\nThe user is asking you to create, implement, build, generate, or modify a concrete artifact. You must use available tools to inspect the workspace, create or edit files, and verify the result before claiming completion. Prefer `file_write` / `write_file` for creating complete files and `REPL` / `bash` for verification or commands. Every tool call must use a complete JSON object with all required fields; never call a tool with `{{}}`. Do not answer only with prose or a plan. If tools are unavailable or blocked, say that explicitly and provide a resumable recovery plan instead of claiming the work is done.{continuation_line}"),
        source: PromptBlockSource {
            subsystem: "work_loop".to_string(),
            reference: Some("turn_service.work_loop.tool_required_work_intent".to_string()),
        },
    })
}

/// Prompt contribution with durable continuation facts from the event log.
#[must_use]
pub fn continuation_context_prompt_contribution(
    work_loop: &WorkLoopDecision,
    context: &WorkLoopRouteContext,
) -> Option<PromptContribution> {
    if !requires_tool_execution_evidence(work_loop) {
        return None;
    }
    let mut facts = Vec::new();
    if let Some(goal) = context
        .recent_tool_required_without_tool_message
        .as_deref()
        .or(context.event_log_unfinished_goal.as_deref())
    {
        facts.push(format!("unfinishedGoal: {goal}"));
    }
    if let Some(reason) = context.last_degraded_reason.as_ref() {
        facts.push(format!("lastFailureReason: {reason}"));
    }
    if let Some(cursor) = context.event_log_resume_cursor.as_ref() {
        facts.push(format!("resumeCursor: {cursor}"));
    }
    if facts.is_empty() {
        return None;
    }
    Some(PromptContribution {
        kind: PromptBlockKind::CodingContext,
        title: "Continuation Context".to_string(),
        body: format!(
            "[continuation_context]\nDurable event-log replay indicates this turn should continue unfinished tool-required work.\n{}\nUse tools to continue the unfinished goal; do not reclassify this as a direct answer.",
            facts.join("\n")
        ),
        source: PromptBlockSource {
            subsystem: "work_loop".to_string(),
            reference: Some("turn_service.work_loop.continuation_context".to_string()),
        },
    })
}

/// Return true once memory evidence exists and the loop should summarize.
#[must_use]
pub fn should_force_final_after_memory_recall(
    work_loop: &WorkLoopDecision,
    has_successful_tool: bool,
) -> bool {
    requires_memory_recall_evidence(work_loop) && has_successful_tool
}

/// Return a blocking reason when the selected loop may not execute this tool yet.
#[must_use]
pub fn mutation_block_reason(
    work_loop: &WorkLoopDecision,
    tool_name: &str,
    input_json: &str,
) -> Option<String> {
    use crate::modules::runtime::contracts::agent_loop::WorkLoopKind;
    if work_loop.loop_kind != WorkLoopKind::PlanThenConfirm
        || is_read_only_planning_tool(tool_name)
    {
        return None;
    }

    let reason = if tool_likely_mutates(tool_name, input_json) {
        "mutating tool requires user approval before PlanThenConfirm can continue"
    } else {
        "tool is not in the PlanThenConfirm read-only allowlist"
    };
    Some(format!(
        "{reason}: tool `{tool_name}` was blocked before execution"
    ))
}
