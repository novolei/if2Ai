//! Work-loop routing and skill-resolution helpers for `TurnService`.

use std::path::Path;

use crate::modules::api::ToolDefinition;
use crate::modules::application::prompt_planner::{
    PromptBlockKind, PromptBlockSource, PromptContribution,
};
use crate::modules::runtime::contracts::agent_loop::{
    FinalRunReport, LoopOutcomeKind, SkillResolutionCandidate, SkillResolutionPlan,
    WorkLoopDecision, WorkLoopKind,
};
use crate::modules::runtime::contracts::execution_mode::{
    ComplexityLevel, ExecutionMode, ExecutionModeDecision,
};
use crate::modules::runtime::prompt::{collect_skill_index_entries, SkillIndexEntry};
use crate::modules::tools::builtin::skill::{
    check_required_env_vars, local_review_skill_content, resolve_skill_config_block,
    resolve_skill_path,
};

const AUTO_SKILL_TOOLS: &[&str] = &["skill_find", "skill_search", "skill_view"];
const MEMORY_RECALL_INTENT_REASON: &str = "memory_recall_intent";
const MEMORY_READ_TOOLS: &[&str] = &["memory_recall", "memory_recall_explicit", "memory_export"];
const READ_ONLY_PLANNING_TOOLS: &[&str] = &[
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
const MUTATING_TOOL_NAMES: &[&str] = &[
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

/// Route a classifier decision to the internal work-loop taxonomy.
#[must_use]
pub(super) fn route_work_loop(
    decision: &ExecutionModeDecision,
    user_message: &str,
) -> WorkLoopDecision {
    let mut reason_codes = decision
        .reason_codes
        .iter()
        .map(|code| code.0.clone())
        .collect::<Vec<_>>();
    let memory_recall_intent = is_memory_recall_intent(user_message);
    if memory_recall_intent {
        push_unique(&mut reason_codes, MEMORY_RECALL_INTENT_REASON.to_string());
    }

    let loop_kind = match decision.execution_mode {
        ExecutionMode::SpecializedSurface => WorkLoopKind::SpecializedSurface,
        ExecutionMode::PlanThenConfirm => WorkLoopKind::PlanThenConfirm,
        ExecutionMode::AutoPlanExecute => WorkLoopKind::AutonomousWork,
        ExecutionMode::DirectExecute => {
            if !memory_recall_intent
                && looks_like_direct_answer(user_message, decision.complexity_level)
            {
                reason_codes.push("direct_answer_request".to_string());
                WorkLoopKind::DirectAnswer
            } else {
                WorkLoopKind::DirectExecute
            }
        }
    };

    WorkLoopDecision {
        loop_kind,
        reason_codes,
        requires_confirmation: matches!(loop_kind, WorkLoopKind::PlanThenConfirm),
        route_hint: decision.route_hint.as_ref().map(|hint| hint.0.clone()),
    }
}

/// Build the deterministic skill-resolution plan for one turn.
#[must_use]
pub(super) fn resolve_skill_plan(
    workdir: &Path,
    user_message: &str,
    active_skill_ids: &[String],
    available_toolsets: &[String],
) -> SkillResolutionPlan {
    let entries = collect_skill_index_entries(workdir, Some(available_toolsets));
    let query_tokens = query_tokens(user_message);
    let mut candidates = entries
        .iter()
        .filter_map(|entry| {
            let score = score_skill_candidate(&entry.name, &entry.description, &query_tokens);
            (score > 0).then(|| skill_candidate(entry, "matched current request keywords", score))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.name.cmp(&right.name))
    });
    candidates.truncate(5);

    for active_skill_id in active_skill_ids {
        if candidates
            .iter()
            .any(|candidate| candidate.name.eq_ignore_ascii_case(active_skill_id))
        {
            continue;
        }
        if let Some(entry) = entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(active_skill_id))
        {
            candidates.push(skill_candidate(
                entry,
                "active skill selected for this turn",
                100,
            ));
        } else {
            candidates.push(SkillResolutionCandidate {
                skill_id: Some(active_skill_id.clone()),
                name: active_skill_id.clone(),
                source: "unknown".to_string(),
                reason: "active skill id was not present in the approved local index".to_string(),
                score: 0,
                trusted_source: false,
                auto_load_allowed: false,
                loaded: false,
                blocked_reason: Some(
                    "active skill id not found in approved local/builtin skill index".to_string(),
                ),
                load_warning: None,
            });
        }
    }

    SkillResolutionPlan {
        active_skill_ids: active_skill_ids.to_vec(),
        candidates,
        auto_discovery_tools: AUTO_SKILL_TOOLS
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        should_load_find_skills: asks_for_skill_discovery(user_message),
        remote_install_policy: "quarantine_requires_user_approval".to_string(),
        loaded_skill_names: Vec::new(),
        blocked_skill_names: Vec::new(),
        load_warnings: Vec::new(),
    }
}

/// Load trusted skill contents into a dedicated prompt contribution.
#[must_use]
pub(super) fn auto_load_trusted_skill_context(
    workdir: &Path,
    plan: &mut SkillResolutionPlan,
) -> Option<PromptContribution> {
    plan.loaded_skill_names.clear();
    plan.blocked_skill_names.clear();
    plan.load_warnings.clear();

    let mut loaded_sections = Vec::new();
    for candidate in &mut plan.candidates {
        candidate
            .skill_id
            .get_or_insert_with(|| candidate.name.clone());
        candidate.loaded = false;
        candidate.load_warning = None;
        candidate.trusted_source = source_family_can_auto_load(&candidate.source);
        candidate.auto_load_allowed = candidate.trusted_source;
        candidate.blocked_reason = None;

        if !candidate.trusted_source {
            let reason = format!(
                "source `{}` is not eligible for automatic skill loading",
                candidate.source
            );
            candidate.auto_load_allowed = false;
            candidate.blocked_reason = Some(reason);
            push_unique(&mut plan.blocked_skill_names, candidate.name.clone());
            continue;
        }

        let skill_path = match resolve_skill_path(&candidate.name, workdir) {
            Ok(path) => path,
            Err(reason) => {
                candidate.auto_load_allowed = false;
                candidate.blocked_reason = Some(reason.clone());
                let warning = format!("skill `{}` was not auto-loaded: {reason}", candidate.name);
                candidate.load_warning = Some(warning.clone());
                push_unique(&mut plan.blocked_skill_names, candidate.name.clone());
                push_unique(&mut plan.load_warnings, warning);
                continue;
            }
        };

        let content = match std::fs::read_to_string(&skill_path) {
            Ok(content) => content,
            Err(error) => {
                let warning = format!(
                    "failed to load skill `{}` from {}: {error}",
                    candidate.name,
                    skill_path.display()
                );
                candidate.load_warning = Some(warning.clone());
                push_unique(&mut plan.blocked_skill_names, candidate.name.clone());
                push_unique(&mut plan.load_warnings, warning);
                continue;
            }
        };

        if let Err(reason) = local_review_skill_content(&content) {
            candidate.auto_load_allowed = false;
            candidate.blocked_reason = Some(reason.clone());
            let warning = format!(
                "skill `{}` failed local content review during auto-load: {reason}",
                candidate.name
            );
            candidate.load_warning = Some(warning.clone());
            push_unique(&mut plan.blocked_skill_names, candidate.name.clone());
            push_unique(&mut plan.load_warnings, warning);
            continue;
        }

        let env_warning = check_required_env_vars(&content);
        if let Some(warning) = env_warning.as_ref() {
            candidate.load_warning = Some(warning.clone());
            push_unique(&mut plan.load_warnings, warning.clone());
        }
        let config_block = resolve_skill_config_block(&candidate.name, &content, workdir);
        let mut section = format!(
            "## Skill: {} [{}]\nThe full SKILL.md content is already loaded. Do not reload it with tools.\n\n{}",
            candidate.name, candidate.source, content
        );
        if let Some(warning) = env_warning {
            section.push_str("\n\n");
            section.push_str(&warning);
        }
        if !config_block.is_empty() {
            section.push_str("\n\n");
            section.push_str(&config_block);
        }

        candidate.loaded = true;
        push_unique(&mut plan.loaded_skill_names, candidate.name.clone());
        loaded_sections.push(section);
    }

    if loaded_sections.is_empty() {
        return None;
    }

    Some(PromptContribution {
        kind: PromptBlockKind::Skill,
        title: "Auto-loaded Skills".to_string(),
        body: format!(
            "[auto_loaded_skills]\nOnly the trusted skills below were loaded automatically.\n\n{}",
            loaded_sections.join("\n\n---\n\n")
        ),
        source: PromptBlockSource {
            subsystem: "skills".to_string(),
            reference: Some("turn_service.work_loop.auto_load".to_string()),
        },
    })
}

/// Apply the selected loop's tool-exposure policy before provider request assembly.
#[must_use]
pub(super) fn enforce_tool_definitions_for_loop(
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
        WorkLoopKind::DirectExecute | WorkLoopKind::AutonomousWork => tool_defs,
    }
}

/// Return true when a work loop must retrieve memory evidence before answering.
#[must_use]
pub(super) fn requires_memory_recall_evidence(work_loop: &WorkLoopDecision) -> bool {
    work_loop
        .reason_codes
        .iter()
        .any(|reason| reason == MEMORY_RECALL_INTENT_REASON)
}

/// Prompt contribution that turns memory-introspection into evidence-seeking work.
#[must_use]
pub(super) fn memory_recall_prompt_contribution(
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

/// Return true when the assistant only announced a memory lookup but did not answer.
#[must_use]
pub(super) fn is_incomplete_memory_lookup_response(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "让我看看",
        "我看看",
        "查一下记忆",
        "看看记忆",
        "记忆中的你",
        "let me check",
        "i'll check",
        "checking memory",
        "check my memory",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Return true once memory evidence exists and the loop should summarize.
#[must_use]
pub(super) fn should_force_final_after_memory_recall(
    work_loop: &WorkLoopDecision,
    has_successful_tool: bool,
) -> bool {
    requires_memory_recall_evidence(work_loop) && has_successful_tool
}

/// Return a blocking reason when the selected loop may not execute this tool yet.
#[must_use]
pub(super) fn mutation_block_reason(
    work_loop: &WorkLoopDecision,
    tool_name: &str,
    input_json: &str,
) -> Option<String> {
    if work_loop.loop_kind != WorkLoopKind::PlanThenConfirm || is_read_only_planning_tool(tool_name)
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

/// Build the final report emitted at work-loop termination.
#[must_use]
pub(super) fn build_final_run_report(
    work_loop: &WorkLoopDecision,
    task_outcome: String,
    terminal_status: String,
    request_id: String,
    tool_loop_iterations: usize,
    has_successful_tool: bool,
    has_successful_mutating_tool: bool,
    resume_available: bool,
    resume_cursor: Option<String>,
    skill_resolution_plan: Option<&SkillResolutionPlan>,
) -> FinalRunReport {
    let outcome = loop_outcome_for(&task_outcome, &terminal_status, resume_available);
    let mut completed_items = Vec::new();
    if has_successful_tool {
        completed_items.push("At least one tool completed successfully.".to_string());
    }
    if has_successful_mutating_tool {
        completed_items.push("At least one mutating tool completed successfully.".to_string());
    }
    if task_outcome == "completed" && completed_items.is_empty() {
        completed_items.push("The assistant produced a final response.".to_string());
    }

    let mut failed_items = Vec::new();
    if task_outcome != "completed" {
        failed_items.push(format!("Run ended with status `{terminal_status}`."));
    }
    if terminal_status == "specialized_surface_routed" {
        failed_items.push("The request was routed to a specialized surface.".to_string());
    }

    let loaded_skills = skill_resolution_plan
        .map(|plan| plan.loaded_skill_names.clone())
        .unwrap_or_default();
    let blocked_skills = skill_resolution_plan
        .map(|plan| plan.blocked_skill_names.clone())
        .unwrap_or_default();
    let skill_warnings = skill_resolution_plan
        .map(|plan| plan.load_warnings.clone())
        .unwrap_or_default();
    if !loaded_skills.is_empty() {
        completed_items.push(format!(
            "Loaded trusted skills: {}.",
            loaded_skills.join(", ")
        ));
    }
    if !blocked_skills.is_empty() {
        failed_items.push(format!(
            "Blocked skill auto-load candidates: {}.",
            blocked_skills.join(", ")
        ));
    }

    let user_next_steps = next_steps_for(outcome, resume_available);
    FinalRunReport {
        loop_kind: work_loop.loop_kind,
        outcome,
        task_outcome,
        terminal_status,
        request_id,
        tool_loop_iterations,
        has_successful_tool,
        has_successful_mutating_tool,
        resume_available,
        resume_cursor,
        completed_items,
        failed_items,
        user_next_steps,
        loaded_skills,
        blocked_skills,
        skill_warnings,
    }
}

fn skill_candidate(
    entry: &SkillIndexEntry,
    reason: impl Into<String>,
    score: u32,
) -> SkillResolutionCandidate {
    let trusted_source = source_family_can_auto_load(&entry.source);
    SkillResolutionCandidate {
        skill_id: Some(entry.name.clone()),
        name: entry.name.clone(),
        source: entry.source.clone(),
        reason: reason.into(),
        score,
        trusted_source,
        auto_load_allowed: trusted_source,
        loaded: false,
        blocked_reason: None,
        load_warning: None,
    }
}

fn source_family_can_auto_load(source: &str) -> bool {
    matches!(source, "builtin" | "workspace" | "user")
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn looks_like_direct_answer(user_message: &str, complexity: ComplexityLevel) -> bool {
    if complexity != ComplexityLevel::Trivial {
        return false;
    }
    let lower = user_message.to_lowercase();
    let toolish = [
        "ls ", "cat ", "run ", "执行", "打开", "edit", "write", "create", "delete", "install",
        "build", "test",
    ];
    !toolish.iter().any(|needle| lower.contains(needle))
}

fn is_memory_recall_intent(user_message: &str) -> bool {
    let lower = user_message.to_lowercase();
    let direct_memory_question = [
        "你记得关于我",
        "你记得我的",
        "你记得我",
        "你还记得我",
        "你知道关于我",
        "你知道我的",
        "关于我的什么",
        "关于我的事情",
        "记忆中的我",
        "记忆中的你",
        "what do you remember about me",
        "what do you know about me",
        "tell me what you remember about me",
        "what's in your memory about me",
        "what is in your memory about me",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    let personal_subject = ["我", "儿子", "孩子", "老婆", "家人", "书维", "son", "child"]
        .iter()
        .any(|needle| lower.contains(needle));
    let memory_dependent_relation = [
        "共同喜欢",
        "共同爱吃",
        "喜欢吃什么",
        "爱吃什么",
        "最喜欢吃什么",
        "喜欢什么",
        "知道我",
        "知道我的",
        "do i like",
        "does my",
        "what do we both",
        "what does my",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    direct_memory_question || (personal_subject && memory_dependent_relation)
}

fn asks_for_skill_discovery(user_message: &str) -> bool {
    let lower = user_message.to_lowercase();
    [
        "find skill",
        "discover skill",
        "recommend skill",
        "查找skill",
        "推荐skill",
        "找一个skill",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn query_tokens(user_message: &str) -> Vec<String> {
    user_message
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .map(str::to_lowercase)
        .filter(|token| token.chars().count() >= 4)
        .take(24)
        .collect()
}

fn score_skill_candidate(name: &str, description: &str, query_tokens: &[String]) -> u32 {
    let haystack = format!("{} {}", name.to_lowercase(), description.to_lowercase());
    query_tokens
        .iter()
        .map(|token| {
            if name.to_lowercase().contains(token) {
                5
            } else if haystack.contains(token) {
                2
            } else {
                0
            }
        })
        .sum()
}

fn is_read_only_planning_tool(tool_name: &str) -> bool {
    READ_ONLY_PLANNING_TOOLS.contains(&tool_name)
}

fn is_memory_read_tool(tool_name: &str) -> bool {
    MEMORY_READ_TOOLS.contains(&tool_name)
}

fn tool_likely_mutates(tool_name: &str, input_json: &str) -> bool {
    if MUTATING_TOOL_NAMES.contains(&tool_name) {
        return true;
    }
    input_json_contains_mutating_method(input_json)
}

fn input_json_contains_mutating_method(input_json: &str) -> bool {
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

fn loop_outcome_for(
    task_outcome: &str,
    terminal_status: &str,
    resume_available: bool,
) -> LoopOutcomeKind {
    if terminal_status == "max_iterations_reached"
        || terminal_status == "repeated_tool_batch_no_progress"
        || terminal_status == "invalid_tool_args_repeated"
    {
        return LoopOutcomeKind::ExhaustedWithSummary;
    }
    if terminal_status == "failed_to_start_stream"
        || terminal_status == "provider_prepare_failed"
        || terminal_status == "prompt_prepare_failed"
        || terminal_status == "memory_recall_required_no_tool"
        || terminal_status == "specialized_surface_routed"
    {
        return LoopOutcomeKind::FailedWithPlan;
    }
    if terminal_status.contains("permission") || terminal_status.contains("approval") {
        return LoopOutcomeKind::NeedsApproval;
    }
    if task_outcome == "completed" {
        return LoopOutcomeKind::Completed;
    }
    if resume_available {
        LoopOutcomeKind::FailedWithPlan
    } else {
        LoopOutcomeKind::NeedsUserInput
    }
}

fn next_steps_for(outcome: LoopOutcomeKind, resume_available: bool) -> Vec<String> {
    match outcome {
        LoopOutcomeKind::Completed => {
            vec!["Review the completed result and continue if needed.".to_string()]
        }
        LoopOutcomeKind::NeedsApproval => {
            vec!["Approve or reject the pending tool action.".to_string()]
        }
        LoopOutcomeKind::NeedsUserInput => {
            vec!["Provide the missing input or adjust the request.".to_string()]
        }
        LoopOutcomeKind::FailedWithPlan if resume_available => {
            vec!["Resume the run from the provided cursor.".to_string()]
        }
        LoopOutcomeKind::FailedWithPlan => {
            vec!["Retry after addressing the reported failure.".to_string()]
        }
        LoopOutcomeKind::ExhaustedWithSummary => {
            vec!["Continue the run so the agent can proceed from existing context.".to_string()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::execution_mode::{ReasonCode, RiskLevel, RouteHint};

    fn decision(mode: ExecutionMode, complexity: ComplexityLevel) -> ExecutionModeDecision {
        ExecutionModeDecision {
            execution_mode: mode,
            risk_level: RiskLevel::Low,
            complexity_level: complexity,
            complexity_score: 0.1,
            reason_codes: vec![ReasonCode::new("test")],
            route_hint: None,
            requires_plan: false,
            scenario_profile_hint: None,
            classifier_policy_version: "test".to_string(),
            classifier_matched_rule_ids: Vec::new(),
            classifier_slot_summary: serde_json::json!({}),
            classifier_ambiguous_escalated: false,
            classifier_escalation_source: None,
        }
    }

    #[test]
    fn direct_question_routes_to_direct_answer_loop() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "what is this project?",
        );
        assert_eq!(routed.loop_kind, WorkLoopKind::DirectAnswer);
    }

    #[test]
    fn auto_plan_routes_to_autonomous_work_loop() {
        let routed = route_work_loop(
            &decision(ExecutionMode::AutoPlanExecute, ComplexityLevel::Moderate),
            "first inspect then fix",
        );
        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
    }

    #[test]
    fn specialized_surface_preserves_route_hint() {
        let mut d = decision(ExecutionMode::SpecializedSurface, ComplexityLevel::Simple);
        d.route_hint = Some(RouteHint("browser".to_string()));
        let routed = route_work_loop(&d, "open the page");
        assert_eq!(routed.loop_kind, WorkLoopKind::SpecializedSurface);
        assert_eq!(routed.route_hint.as_deref(), Some("browser"));
    }

    #[test]
    fn final_report_marks_iteration_exhaustion() {
        let routed = route_work_loop(
            &decision(ExecutionMode::AutoPlanExecute, ComplexityLevel::Complex),
            "do many things",
        );
        let report = build_final_run_report(
            &routed,
            "partial_success".to_string(),
            "max_iterations_reached".to_string(),
            "req-1".to_string(),
            10,
            true,
            false,
            true,
            Some("cursor".to_string()),
            None,
        );
        assert_eq!(report.outcome, LoopOutcomeKind::ExhaustedWithSummary);
        assert!(report.resume_available);
        assert!(!report.failed_items.is_empty());
    }

    fn tool_def(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: None,
            input_schema: serde_json::json!({ "type": "object" }),
        }
    }

    #[test]
    fn work_loop_router_direct_answer_hides_tools() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "what is this project?",
        );
        let visible_tools = enforce_tool_definitions_for_loop(
            &routed,
            vec![tool_def("read_file"), tool_def("write_file")],
        );
        assert!(visible_tools.is_empty());
        let report = build_final_run_report(
            &routed,
            "completed".to_string(),
            "model_stop_no_tools".to_string(),
            "req-direct".to_string(),
            1,
            false,
            false,
            false,
            None,
            None,
        );
        assert_eq!(report.loop_kind, WorkLoopKind::DirectAnswer);
        assert_eq!(report.outcome, LoopOutcomeKind::Completed);
    }

    #[test]
    fn memory_recall_intent_routes_to_direct_execute() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "你记得关于我的什么事情",
        );
        assert_eq!(routed.loop_kind, WorkLoopKind::DirectExecute);
        assert!(requires_memory_recall_evidence(&routed));
        assert!(!routed
            .reason_codes
            .iter()
            .any(|reason| reason == "direct_answer_request"));
    }

    #[test]
    fn memory_recall_intent_routes_family_food_question_to_direct_execute() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "我与我儿子共同喜欢吃什么？",
        );

        assert_eq!(routed.loop_kind, WorkLoopKind::DirectExecute);
        assert!(requires_memory_recall_evidence(&routed));
    }

    #[test]
    fn memory_recall_intent_exposes_memory_read_tools() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "what do you remember about me?",
        );
        let visible_tools = enforce_tool_definitions_for_loop(
            &routed,
            vec![
                tool_def("memory_recall"),
                tool_def("memory_export"),
                tool_def("read_file"),
                tool_def("write_file"),
            ],
        );
        assert_eq!(
            visible_tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["memory_recall", "memory_export"]
        );
    }

    #[test]
    fn memory_recall_intent_prompt_requires_recall_tool() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "你知道关于我的什么",
        );
        let contribution =
            memory_recall_prompt_contribution(&routed).expect("memory prompt contribution");
        assert!(contribution.body.contains("memory_recall"));
        assert!(contribution.body.contains("Do not answer with a filler"));
    }

    #[test]
    fn memory_recall_prompt_asks_for_deduped_answer() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "what do you remember about me?",
        );
        let contribution =
            memory_recall_prompt_contribution(&routed).expect("memory prompt contribution");

        assert!(contribution.body.contains("[memory_recall_evidence]"));
        assert!(contribution.body.contains("deduplicated facts"));
        assert!(contribution.body.contains("do not repeat duplicate facts"));
        assert!(contribution.body.contains("Episode/canonical variants"));
    }

    #[test]
    fn memory_recall_success_forces_final_response() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "你记得关于我的什么事情",
        );
        assert!(!should_force_final_after_memory_recall(&routed, false));
        assert!(should_force_final_after_memory_recall(&routed, true));

        let normal = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "what is this project?",
        );
        assert!(!should_force_final_after_memory_recall(&normal, true));
    }

    #[test]
    fn memory_recall_no_tool_filler_report_failed() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "你记得关于我的什么事情",
        );
        assert!(is_incomplete_memory_lookup_response(
            "让我看看记忆中的你～🔍"
        ));
        let report = build_final_run_report(
            &routed,
            "failed".to_string(),
            "memory_recall_required_no_tool".to_string(),
            "req-memory".to_string(),
            1,
            false,
            false,
            true,
            Some("cursor".to_string()),
            None,
        );
        assert_eq!(report.outcome, LoopOutcomeKind::FailedWithPlan);
        assert!(!report
            .completed_items
            .iter()
            .any(|item| { item == "The assistant produced a final response." }));
        assert!(!report.failed_items.is_empty());
    }

    #[test]
    fn work_loop_router_plan_then_confirm_blocks_mutation() {
        let routed = route_work_loop(
            &decision(ExecutionMode::PlanThenConfirm, ComplexityLevel::Moderate),
            "inspect the repo and propose edits",
        );
        let visible_tools = enforce_tool_definitions_for_loop(
            &routed,
            vec![tool_def("read_file"), tool_def("write_file")],
        );
        assert_eq!(
            visible_tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["read_file"]
        );
        assert!(mutation_block_reason(&routed, "write_file", "{}").is_some());
        assert!(mutation_block_reason(&routed, "Config", r#"{"key":"x","value":"y"}"#).is_some());
        assert!(mutation_block_reason(&routed, "read_file", "{}").is_none());
    }

    #[test]
    fn work_loop_router_specialized_surface_reports() {
        let mut d = decision(ExecutionMode::SpecializedSurface, ComplexityLevel::Simple);
        d.route_hint = Some(RouteHint("browser".to_string()));
        let routed = route_work_loop(&d, "open the browser surface");
        let report = build_final_run_report(
            &routed,
            "failed".to_string(),
            "specialized_surface_routed".to_string(),
            "req-specialized".to_string(),
            0,
            false,
            false,
            false,
            None,
            None,
        );
        assert_eq!(report.loop_kind, WorkLoopKind::SpecializedSurface);
        assert_eq!(report.outcome, LoopOutcomeKind::FailedWithPlan);
        assert!(!report.failed_items.is_empty());
    }

    #[test]
    fn work_loop_router_terminal_failures_report() {
        let routed = route_work_loop(
            &decision(ExecutionMode::AutoPlanExecute, ComplexityLevel::Complex),
            "inspect then repair",
        );
        let provider_failure = build_final_run_report(
            &routed,
            "failed".to_string(),
            "failed_to_start_stream".to_string(),
            "req-provider".to_string(),
            1,
            false,
            false,
            true,
            Some("cursor".to_string()),
            None,
        );
        let exhausted = build_final_run_report(
            &routed,
            "partial_success".to_string(),
            "max_iterations_reached".to_string(),
            "req-budget".to_string(),
            10,
            true,
            false,
            true,
            Some("cursor".to_string()),
            None,
        );
        assert_eq!(provider_failure.outcome, LoopOutcomeKind::FailedWithPlan);
        assert_eq!(exhausted.outcome, LoopOutcomeKind::ExhaustedWithSummary);
    }

    #[test]
    fn work_loop_router_approval_terminal_status_wins_over_completed_task() {
        let routed = route_work_loop(
            &decision(ExecutionMode::PlanThenConfirm, ComplexityLevel::Moderate),
            "plan and then edit",
        );
        let report = build_final_run_report(
            &routed,
            "completed".to_string(),
            "approval_required_for_mutation".to_string(),
            "req-approval".to_string(),
            2,
            false,
            false,
            false,
            None,
            None,
        );
        assert_eq!(report.outcome, LoopOutcomeKind::NeedsApproval);
    }
}
