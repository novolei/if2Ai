//! Work-loop routing and skill-resolution helpers for `TurnService`.

mod evidence;
mod routing;
mod skill_resolution;
mod tool_intent_detection;
mod tool_pool;

pub(super) use evidence::{
    CONTINUATION_INTENT_REASON, INHERITED_TOOL_REQUIRED_WORK_INTENT_REASON,
    MEMORY_RECALL_INTENT_REASON, SIMPLE_SHELL_COMMAND_INTENT_REASON,
    TOOL_REQUIRED_WORK_INTENT_REASON, continuation_context_prompt_contribution,
    memory_recall_prompt_contribution, mutation_block_reason, requires_memory_recall_evidence,
    requires_single_shell_command_evidence, requires_tool_execution_evidence,
    should_force_final_after_memory_recall, single_shell_command_prompt_contribution,
    tool_required_prompt_contribution,
};
pub(super) use tool_pool::{
    CanonicalToolPool, block_is_mutating_tool_evidence, build_canonical_tool_pool,
    enforce_tool_definitions_for_loop, input_json_contains_mutating_method, is_memory_read_tool,
    is_read_only_planning_tool, stable_tool_schema_hash, tool_likely_mutates,
    tool_pool_policy_label,
};
pub(super) use tool_pool::{MEMORY_READ_TOOLS, MUTATING_TOOL_NAMES, READ_ONLY_PLANNING_TOOLS};
pub(super) use routing::{
    WorkLoopRouteContext, assistant_claimed_tool_execution_without_tool_message,
    augment_route_context_from_run_log, context_requires_tool_execution, is_continuation_intent,
    is_direct_shell_command_intent, is_memory_recall_intent, is_tool_required_work_intent,
    is_trivially_simple_single_tool_intent, looks_like_direct_answer, message_text,
    recent_tool_required_without_tool, route_context_from_messages, route_work_loop,
    route_work_loop_with_context,
};
pub(super) use skill_resolution::{
    SkillRuntimeMetadata, asks_for_skill_discovery, auto_load_trusted_skill_context,
    excerpt_skill_body, parse_frontmatter_list, parse_skill_runtime_metadata, query_tokens,
    resolve_skill_plan, score_skill_candidate, skill_candidate, skill_metadata_prompt_block,
    source_family_can_auto_load,
};
pub(super) use tool_intent_detection::{
    assistant_claims_tool_execution_without_tool, assistant_signals_tool_intent,
    detect_repetitive_model_output, detect_textual_tool_call_markup,
    extract_textual_tool_calls, is_tool_required_terminal_reason, normalize_dsml_delimiters,
    tool_intent_nudge_message,
};

use crate::modules::runtime::contracts::agent_loop::{
    FinalRunReport, LoopOutcomeKind, SkillResolutionPlan, WorkLoopDecision, WorkLoopKind,
};

#[cfg(test)]
use crate::modules::api::ToolDefinition;
#[cfg(test)]
use crate::modules::application::prompt_planner::PromptBlockKind;
#[cfg(test)]
use crate::modules::runtime::contracts::execution_mode::{
    ComplexityLevel, ExecutionMode, ExecutionModeDecision,
};
#[cfg(test)]
use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};

const AUTO_SKILL_TOOLS: &[&str] = &["skill_find", "skill_search", "skill_view"];





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
    diagnostic_warnings: Vec<String>,
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
    if terminal_status == "tool_required_no_tool" {
        failed_items.push(
            "This request required mutating tool execution, but no mutating tool completed successfully.".to_string(),
        );
    }
    if terminal_status == "todo_ledger_incomplete" {
        failed_items.push(
            "The TodoWrite ledger still has unfinished tool-backed steps, so the run cannot be marked complete.".to_string(),
        );
    }
    if terminal_status == "provider_textual_tool_call_markup" {
        failed_items.push(
            "The provider emitted textual tool-call markup instead of a structured tool call."
                .to_string(),
        );
    }
    if terminal_status == "repetitive_model_output" {
        failed_items.push(
            "The provider produced a repetitive response without progressing the task.".to_string(),
        );
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
    if !diagnostic_warnings.is_empty() {
        failed_items.push(format!(
            "Provider/tool diagnostics: {}.",
            diagnostic_warnings.join(" | ")
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
        diagnostic_warnings,
    }
}


fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn loop_outcome_for(
    task_outcome: &str,
    terminal_status: &str,
    resume_available: bool,
) -> LoopOutcomeKind {
    if terminal_status == "max_iterations_reached"
        || terminal_status == "repeated_tool_batch_no_progress"
        || terminal_status == "invalid_tool_args_repeated"
        || terminal_status == "repetitive_model_output"
    {
        return LoopOutcomeKind::ExhaustedWithSummary;
    }
    if terminal_status == "failed_to_start_stream"
        || terminal_status == "provider_prepare_failed"
        || terminal_status == "prompt_prepare_failed"
        || terminal_status == "memory_recall_required_no_tool"
        || terminal_status == "tool_required_no_tool"
        || terminal_status == "todo_ledger_incomplete"
        || terminal_status == "provider_textual_tool_call_markup"
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
    fn creation_request_requires_tools_not_direct_answer() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "帮我创建一个泡泡龙网页游戏 需要有声效 界面美观大方",
        );
        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
        assert!(requires_tool_execution_evidence(&routed));
        assert!(!routed
            .reason_codes
            .iter()
            .any(|reason| reason == "direct_answer_request"));
        let visible_tools = enforce_tool_definitions_for_loop(
            &routed,
            vec![tool_def("file_write"), tool_def("bash")],
        );
        assert_eq!(visible_tools.len(), 2);
    }

    #[test]
    fn exact_shell_command_does_not_inherit_unfinished_artifact_work() {
        let messages = vec![ConversationMessage::user_text(
            "帮我创建一个泡泡龙网页游戏 需要有声效 界面美观大方",
        )];
        let context = route_context_from_messages(&messages);
        let routed = route_work_loop_with_context(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "ls -a",
            &context,
        );

        assert_eq!(routed.loop_kind, WorkLoopKind::DirectExecute);
        assert!(requires_single_shell_command_evidence(&routed));
        assert!(!requires_tool_execution_evidence(&routed));
    }

    #[test]
    fn exact_shell_command_only_exposes_bash() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "ls -a",
        );
        let visible_tools = enforce_tool_definitions_for_loop(
            &routed,
            vec![tool_def("bash"), tool_def("read_file")],
        );

        assert_eq!(visible_tools.len(), 1);
        assert_eq!(visible_tools[0].name, "bash");
    }

    #[test]
    fn continuation_artifact_completion_requires_autonomous_tools() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "继续完成这个网页游戏",
        );

        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
        assert!(requires_tool_execution_evidence(&routed));
        assert!(routed
            .reason_codes
            .iter()
            .any(|reason| reason == TOOL_REQUIRED_WORK_INTENT_REASON));
        assert!(!routed
            .reason_codes
            .iter()
            .any(|reason| reason == "direct_answer_request"));
    }

    #[test]
    fn complete_web_artifact_prevents_short_direct_answer() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "我还是希望完整网页版的泡泡龙 不使用python",
        );

        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
        assert!(requires_tool_execution_evidence(&routed));
    }

    #[test]
    fn continue_inherits_tool_required_work_from_previous_request() {
        let messages = vec![
            ConversationMessage::user_text("帮我创建一个泡泡龙网页游戏 需要有声效 界面美观大方"),
            ConversationMessage {
                role: MessageRole::Assistant,
                blocks: vec![ContentBlock::Text {
                    text: "未执行工具，无法确认完成。".to_string(),
                }],
                usage: None,
                thinking: None,
                task_outcome: Some("failed".to_string()),
                degraded_reason: Some("tool_required_no_tool".to_string()),
                resume_available: Some(true),
                resume_cursor: Some("cursor".to_string()),
                request_id: None,
                finish_reason: None,
            },
        ];
        let context = route_context_from_messages(&messages);
        let routed = route_work_loop_with_context(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "继续",
            &context,
        );

        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
        assert!(requires_tool_execution_evidence(&routed));
        assert!(routed
            .reason_codes
            .iter()
            .any(|reason| reason == CONTINUATION_INTENT_REASON));
        assert!(routed
            .reason_codes
            .iter()
            .any(|reason| reason == INHERITED_TOOL_REQUIRED_WORK_INTENT_REASON));
        assert!(!routed
            .reason_codes
            .iter()
            .any(|reason| reason == "direct_answer_request"));
        let visible_tools = enforce_tool_definitions_for_loop(
            &routed,
            vec![tool_def("file_write"), tool_def("bash")],
        );
        assert_eq!(visible_tools.len(), 2);
    }

    #[test]
    fn tool_required_prompt_tells_model_to_use_tools() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "create a polished web game with sound effects",
        );
        let contribution =
            tool_required_prompt_contribution(&routed).expect("tool-required prompt");
        assert_eq!(contribution.kind, PromptBlockKind::CodingContext);
        assert!(contribution.body.contains("[tool_required_work_intent]"));
        assert!(contribution.body.contains("must use available tools"));
    }

    #[test]
    fn continuation_tool_required_prompt_names_prior_unfinished_work() {
        let context = WorkLoopRouteContext {
            last_user_message: Some(
                "帮我创建一个泡泡龙网页游戏 需要有声效 界面美观大方".to_string(),
            ),
            last_assistant_text: Some("未执行工具，无法确认完成。".to_string()),
            last_assistant_claimed_tool_execution_without_tool: false,
            last_task_outcome: Some("failed".to_string()),
            last_degraded_reason: Some("tool_required_no_tool".to_string()),
            last_resume_available: Some(true),
            recent_tool_required_without_tool_message: None,
            event_log_resume_cursor: None,
            event_log_unfinished_goal: None,
        };
        let routed = route_work_loop_with_context(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "继续",
            &context,
        );
        let contribution =
            tool_required_prompt_contribution(&routed).expect("tool-required prompt");

        assert!(contribution
            .body
            .contains("continuing a previous unfinished tool-required task"));
    }

    #[test]
    fn continue_without_unfinished_tool_context_can_direct_answer() {
        let context = WorkLoopRouteContext {
            last_user_message: Some("你好".to_string()),
            last_assistant_text: Some("你好，有什么需要帮忙？".to_string()),
            last_assistant_claimed_tool_execution_without_tool: false,
            last_task_outcome: Some("completed".to_string()),
            last_degraded_reason: None,
            last_resume_available: Some(false),
            recent_tool_required_without_tool_message: None,
            event_log_resume_cursor: None,
            event_log_unfinished_goal: None,
        };
        let routed = route_work_loop_with_context(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "继续",
            &context,
        );

        assert_eq!(routed.loop_kind, WorkLoopKind::DirectAnswer);
        assert!(!requires_tool_execution_evidence(&routed));
    }

    #[test]
    fn continue_recovers_earlier_tool_required_goal_when_prior_continue_faked_tool_text() {
        let messages = vec![
            ConversationMessage::user_text("帮我创建一个泡泡龙网页游戏 需要有声效 界面美观大方"),
            ConversationMessage::assistant(vec![ContentBlock::Text {
                text: "未执行工具，无法确认完成。".to_string(),
            }]),
            ConversationMessage::user_text("继续"),
            ConversationMessage::assistant(vec![ContentBlock::Text {
                text: "我来创建文件：<function_calls><invoke name=\"text_editor\">...</invoke>"
                    .to_string(),
            }]),
        ];
        let context = route_context_from_messages(&messages);
        let routed = route_work_loop_with_context(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "继续",
            &context,
        );

        assert!(context.recent_tool_required_without_tool_message.is_some());
        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
        assert!(requires_tool_execution_evidence(&routed));
    }

    #[test]
    fn failed_mutating_tool_attempt_does_not_satisfy_continuation_evidence() {
        let messages = vec![
            ConversationMessage::user_text("帮我创建一个泡泡龙网页游戏 需要有声效 界面美观大方"),
            ConversationMessage::tool_use("REPL:1", "REPL", "{}"),
            ConversationMessage::tool_result(
                "REPL:1",
                "REPL",
                "invalid tool arguments: missing required parameter: code",
                true,
            ),
        ];
        let context = route_context_from_messages(&messages);
        let routed = route_work_loop_with_context(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "继续",
            &context,
        );

        assert!(context.recent_tool_required_without_tool_message.is_some());
        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
        assert!(requires_tool_execution_evidence(&routed));
    }

    #[test]
    fn fake_bash_write_text_keeps_continuation_tool_required() {
        let messages = vec![ConversationMessage::assistant(vec![ContentBlock::Text {
            text: "好，这次一口气写完。我把完整的泡泡龙网页游戏代码用 bash 直接写入：".to_string(),
        }])];
        let context = route_context_from_messages(&messages);
        let routed = route_work_loop_with_context(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "继续",
            &context,
        );

        assert!(context.last_assistant_claimed_tool_execution_without_tool);
        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
        assert!(requires_tool_execution_evidence(&routed));
    }

    #[test]
    fn assistant_claims_tool_execution_without_tool_detects_fake_write() {
        assert!(assistant_claims_tool_execution_without_tool(
            "我把完整的泡泡龙网页游戏代码用 bash 直接写入："
        ));
        assert!(assistant_claims_tool_execution_without_tool(
            "<function_calls><invoke name=\"text_editor\">"
        ));
        assert!(!assistant_claims_tool_execution_without_tool(
            "你可以手动创建 index.html，不过我不会声称已经写入。"
        ));
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
        );
        assert_eq!(report.outcome, LoopOutcomeKind::FailedWithPlan);
        assert!(!report
            .completed_items
            .iter()
            .any(|item| { item == "The assistant produced a final response." }));
        assert!(!report.failed_items.is_empty());
    }

    #[test]
    fn tool_required_no_tool_report_failed_with_plan() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "帮我创建一个泡泡龙网页游戏 需要有声效 界面美观大方",
        );
        let report = build_final_run_report(
            &routed,
            "failed".to_string(),
            "tool_required_no_tool".to_string(),
            "req-tool-required".to_string(),
            1,
            false,
            false,
            true,
            Some("cursor".to_string()),
            None,
            Vec::new(),
        );
        assert_eq!(report.outcome, LoopOutcomeKind::FailedWithPlan);
        assert!(report.resume_available);
        assert!(report.failed_items.iter().any(|item| {
            item.contains("required tool execution") || item.contains("tool_required_no_tool")
        }));
    }

    #[test]
    fn todo_ledger_incomplete_report_failed_with_plan() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "继续完成这个网页游戏",
        );
        let report = build_final_run_report(
            &routed,
            "failed".to_string(),
            "todo_ledger_incomplete".to_string(),
            "req-todo-ledger".to_string(),
            2,
            true,
            true,
            true,
            Some("cursor".to_string()),
            None,
            Vec::new(),
        );
        assert_eq!(report.outcome, LoopOutcomeKind::FailedWithPlan);
        assert!(report.resume_available);
        assert!(report
            .failed_items
            .iter()
            .any(|item| item.contains("TodoWrite ledger")));
    }

    #[test]
    fn assistant_tool_intent_nudge_detects_announced_action() {
        assert!(assistant_signals_tool_intent(
            "好，这次我用 bash 直接写入文件。"
        ));
        assert!(assistant_signals_tool_intent(
            "Let me inspect the workspace and write the file."
        ));
        assert!(!assistant_signals_tool_intent("我可以解释一下这个概念。"));
        assert!(tool_intent_nudge_message().contains("tool_calls"));
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
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
            Vec::new(),
        );
        assert_eq!(report.outcome, LoopOutcomeKind::NeedsApproval);
    }

    #[test]
    fn provider_textual_tool_call_markup_detected() {
        assert_eq!(
            detect_textual_tool_call_markup(
                "<function_calls><invoke name=\"file_write\" /></function_calls>"
            ),
            Some("xml_function_calls".to_string())
        );
    }

    #[test]
    fn deepseek_dsml_textual_tool_call_markup_detected() {
        assert_eq!(
            detect_textual_tool_call_markup(
                "<｜DSML｜tool_calls><｜DSML｜invoke name=\"bash\"></｜DSML｜invoke></｜DSML｜tool_calls>"
            ),
            Some("deepseek_dsml_tool_calls".to_string())
        );
    }

    #[test]
    fn extracts_deepseek_dsml_tool_call() {
        let calls = extract_textual_tool_calls(
            "<｜DSML｜tool_calls>\n\
<｜DSML｜invoke name=\"bash\">\n\
<｜DSML｜parameter name=\"command\" string=\"true\">touch /Users/ryanliu/Desktop/me/r.md && ls -a /Users/ryanliu/Desktop/me/</｜DSML｜parameter>\n\
</｜DSML｜invoke>\n\
</｜DSML｜tool_calls>",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "bash");
        let args: serde_json::Value =
            serde_json::from_str(&calls[0].2).expect("dsml args should parse as json");
        assert_eq!(
            args.get("command").and_then(|value| value.as_str()),
            Some("touch /Users/ryanliu/Desktop/me/r.md && ls -a /Users/ryanliu/Desktop/me/")
        );
    }

    #[test]
    fn extract_textual_tool_calls_handles_ascii_single_pipe_dsml() {
        let raw = "<|DSML|tool_calls>\n<|DSML|invoke name=\"bash\">\n<|DSML|parameter name=\"command\">date</|DSML|parameter>\n</|DSML|invoke>\n</|DSML|tool_calls>";
        let calls = extract_textual_tool_calls(raw);
        assert_eq!(
            calls.len(),
            1,
            "expected 1 ascii-pipe DSML call, got {calls:?}"
        );
        assert_eq!(calls[0].1, "bash");
        assert!(calls[0].2.contains("date"), "missing param: {}", calls[0].2);
    }

    #[test]
    fn extract_textual_tool_calls_handles_ascii_double_pipe_dsml() {
        let raw = "<||DSML||tool_calls>\n<||DSML||invoke name=\"bash\">\n<||DSML||parameter name=\"command\">date</||DSML||parameter>\n</||DSML||invoke>\n</||DSML||tool_calls>";
        let calls = extract_textual_tool_calls(raw);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "bash");
        assert!(calls[0].2.contains("date"));
    }

    #[test]
    fn extract_textual_tool_calls_unicode_fullwidth_still_works() {
        let raw = "<\u{FF5C}DSML\u{FF5C}tool_calls>\n<\u{FF5C}DSML\u{FF5C}invoke name=\"bash\">\n<\u{FF5C}DSML\u{FF5C}parameter name=\"command\">date</\u{FF5C}DSML\u{FF5C}parameter>\n</\u{FF5C}DSML\u{FF5C}invoke>\n</\u{FF5C}DSML\u{FF5C}tool_calls>";
        let calls = extract_textual_tool_calls(raw);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "bash");
    }

    #[test]
    fn detect_textual_tool_call_markup_recognizes_ascii_variants() {
        assert_eq!(
            detect_textual_tool_call_markup("<|DSML|tool_calls>"),
            Some("deepseek_dsml_tool_calls".to_string())
        );
        assert_eq!(
            detect_textual_tool_call_markup("<||DSML||invoke name=\"x\">"),
            Some("deepseek_dsml_tool_calls".to_string())
        );
        assert_eq!(
            detect_textual_tool_call_markup("</||DSML||tool_calls>"),
            Some("deepseek_dsml_tool_calls".to_string())
        );
        assert_eq!(
            detect_textual_tool_call_markup("<\u{FF5C}DSML\u{FF5C}tool_calls"),
            Some("deepseek_dsml_tool_calls".to_string())
        );
        assert_eq!(
            detect_textual_tool_call_markup("plain text without any markup"),
            None
        );
    }

    #[test]
    fn normalize_dsml_delimiters_idempotent_on_canonical_form() {
        let canonical = "<\u{FF5C}DSML\u{FF5C}invoke name=\"x\">";
        assert_eq!(normalize_dsml_delimiters(canonical), canonical);
    }

    #[test]
    fn normalize_dsml_delimiters_replaces_double_then_single() {
        let d = "<||DSML||tool_calls></||DSML||tool_calls>";
        let n = normalize_dsml_delimiters(d);
        assert!(!n.contains("<||"));
        assert!(!n.contains("</||"));
        assert!(n.contains("<\u{FF5C}DSML\u{FF5C}tool_calls"));

        let s = "<|DSML|tool_calls></|DSML|tool_calls>";
        let n = normalize_dsml_delimiters(s);
        assert!(!n.contains("<|DSML"));
        assert!(n.contains("<\u{FF5C}DSML\u{FF5C}tool_calls"));
    }

    #[test]
    fn normalize_handles_spaces_inside_brackets() {
        let raw = "< | DSML | tool_calls>";
        let n = normalize_dsml_delimiters(raw);
        assert!(n.contains("<\u{FF5C}DSML\u{FF5C}tool_calls"), "got: {n}");
    }

    #[test]
    fn normalize_handles_double_pipes_with_spaces() {
        let raw = "< | | DSML | | tool_calls>";
        let n = normalize_dsml_delimiters(raw);
        assert!(n.contains("<\u{FF5C}DSML\u{FF5C}tool_calls"), "got: {n}");
    }

    #[test]
    fn normalize_handles_mixed_pipe_types() {
        let raw = "<\u{FF5C}DSML|tool_calls>";
        let n = normalize_dsml_delimiters(raw);
        assert!(n.contains("<\u{FF5C}DSML\u{FF5C}tool_calls"), "got: {n}");
    }

    #[test]
    fn extract_handles_real_world_screenshot_variant() {
        let raw = "< | | DSML | | tool_calls>\n< | | DSML | | invoke name=\"bash\">\n< | | DSML | | parameter name=\"command\" string=\"true\">date</ | | DSML | | parameter>\n</ | | DSML | | invoke>\n</ | | DSML | | tool_calls>";
        let calls = extract_textual_tool_calls(raw);
        assert_eq!(calls.len(), 1, "expected 1 call, got {:?}", calls);
        assert_eq!(calls[0].1, "bash");
        assert!(calls[0].2.contains("date"), "param missing: {}", calls[0].2);
    }

    #[test]
    fn extracts_minimax_xml_tool_call() {
        let calls = extract_textual_tool_calls(
            "<minimax:tool_call>\n\
<invoke name=\"bash\">\n\
<parameter name=\"command\">rm /Users/ryanliu/Desktop/me/r*.md && ls -a /Users/ryanliu/Desktop/me/</parameter>\n\
</invoke>\n\
</minimax:tool_call>",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "bash");
        let args: serde_json::Value =
            serde_json::from_str(&calls[0].2).expect("xml args should parse as json");
        assert_eq!(
            args.get("command").and_then(|value| value.as_str()),
            Some("rm /Users/ryanliu/Desktop/me/r*.md && ls -a /Users/ryanliu/Desktop/me/")
        );
    }

    #[test]
    fn create_markdown_file_is_tool_required_work() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "创建 r.md 文件",
        );

        assert!(requires_tool_execution_evidence(&routed));
        assert_eq!(routed.loop_kind, WorkLoopKind::AutonomousWork);
    }

    #[test]
    fn textual_tool_markup_without_tool_event_stays_failed() {
        let routed = route_work_loop(
            &decision(ExecutionMode::DirectExecute, ComplexityLevel::Trivial),
            "创建一个网页游戏",
        );
        let report = build_final_run_report(
            &routed,
            "failed".to_string(),
            "provider_textual_tool_call_markup".to_string(),
            "req-provider-markup".to_string(),
            1,
            false,
            false,
            true,
            Some("cursor".to_string()),
            None,
            vec![
                "provider emitted xml_function_calls text instead of structured tool_calls"
                    .to_string(),
            ],
        );
        assert_eq!(report.outcome, LoopOutcomeKind::FailedWithPlan);
        assert!(report
            .diagnostic_warnings
            .iter()
            .any(|warning| warning.contains("xml_function_calls")));
    }

    #[test]
    fn provider_tool_call_compat_warning_is_sanitized() {
        let payload = serde_json::json!({
            "stream_id": "run-1",
            "session_id": "session-1",
            "provider_id": "kimi-coding",
            "model": "kimi-k2.6",
            "markup_family": "xml_function_calls",
            "text_len": 4096,
            "recovery_action": "nudge_with_required_tool_choice",
            "sanitized": true,
        });
        assert_eq!(
            payload.get("sanitized").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(payload.get("text").is_none());
        assert!(payload.get("raw_payload").is_none());
    }

    #[test]
    fn repetitive_model_output_is_detected() {
        let text = [
            "我来继续完成泡泡龙游戏。先检查目录状态。",
            "继续。先查看目录。",
            "继续完成泡泡龙游戏。先检查目录状态，然后分段写入。",
            "继续。检查目录。",
            "继续。查看目录。",
            "继续完成泡泡龙游戏。先检查目录状态。",
            "继续。先查看目录。",
            "继续完成泡泡龙游戏。先检查目录状态，然后分段写入。",
            "继续。检查目录。",
            "继续。查看目录。",
            "继续完成泡泡龙游戏。先检查目录状态。",
            "继续。先查看目录。",
            "继续完成泡泡龙游戏。先检查目录状态，然后分段写入。",
            "继续。检查目录。",
            "继续。查看目录。",
            "继续完成泡泡龙游戏。先检查目录状态。",
            "继续。先查看目录。",
            "继续完成泡泡龙游戏。先检查目录状态，然后分段写入。",
            "继续。检查目录。",
            "继续。查看目录。",
        ]
        .join("\n");

        assert!(detect_repetitive_model_output(&text));
    }

    #[test]
    fn tool_001_canonical_tool_pool_sorts_and_hashes_schema() {
        let routed = route_work_loop(
            &decision(ExecutionMode::AutoPlanExecute, ComplexityLevel::Complex),
            "build a page",
        );
        let pool = build_canonical_tool_pool(
            vec![tool_def("write_file"), tool_def("read_file")],
            &[],
            &routed,
        );
        assert_eq!(pool.tool_names, vec!["read_file", "write_file"]);
        assert_eq!(pool.policy, "autonomous_work_visible");
        assert!(!pool.schema_hash.is_empty());
    }

    #[test]
    fn ctx_001_replays_unfinished_goal_from_event_log() {
        let mut context = WorkLoopRouteContext::default();
        let entries = vec![
            crate::modules::runtime::event_log::RunLogEntry {
                event_id: "e1".to_string(),
                session_id: "s1".to_string(),
                run_id: "r1".to_string(),
                seq: 1,
                event_type: "run_started".to_string(),
                occurred_at: "now".to_string(),
                payload: serde_json::json!({
                    "message_preview": "帮我创建一个泡泡龙网页游戏 需要有声效"
                }),
                causation_id: None,
                correlation_id: None,
                tool_call_id: None,
                attempt_id: None,
                team_id: None,
                member_id: None,
                role_id: None,
                parent_run_id: None,
                delegation_id: None,
            },
            crate::modules::runtime::event_log::RunLogEntry {
                event_id: "e2".to_string(),
                session_id: "s1".to_string(),
                run_id: "r1".to_string(),
                seq: 2,
                event_type: "final_run_report".to_string(),
                occurred_at: "now".to_string(),
                payload: serde_json::json!({
                    "tool_args": {
                        "terminalStatus": "tool_required_no_tool",
                        "taskOutcome": "failed",
                        "resumeAvailable": true,
                        "resumeCursor": "cursor-1"
                    }
                }),
                causation_id: None,
                correlation_id: None,
                tool_call_id: None,
                attempt_id: None,
                team_id: None,
                member_id: None,
                role_id: None,
                parent_run_id: None,
                delegation_id: None,
            },
        ];
        augment_route_context_from_run_log(&mut context, &entries);
        assert_eq!(
            context.last_degraded_reason.as_deref(),
            Some("tool_required_no_tool")
        );
        assert_eq!(context.event_log_resume_cursor.as_deref(), Some("cursor-1"));
        assert!(context
            .recent_tool_required_without_tool_message
            .as_deref()
            .is_some_and(|message| message.contains("泡泡龙")));
    }

    #[test]
    fn skill_004_parses_frontmatter_runtime_metadata() {
        let metadata = parse_skill_runtime_metadata(
            "---\nname: web-game\nwhenToUse: building browser games\nallowedTools: [read_file, write_file, bash]\nmodelHint: coding\n---\n# Body",
        );
        assert_eq!(
            metadata.when_to_use.as_deref(),
            Some("building browser games")
        );
        assert_eq!(
            metadata.allowed_tools,
            vec!["read_file", "write_file", "bash"]
        );
        assert_eq!(metadata.model_hint.as_deref(), Some("coding"));
    }

    #[test]
    fn excerpt_skill_body_preserves_frontmatter() {
        let base = "---\nname: foo\nversion: 1.0\n---\n# Foo skill\n\nThis is the body, with a lot more content that should be truncated after a couple hundred characters of meaningful prose to simulate a typical SKILL.md.";
        let raw = format!("{}{}", base, "x".repeat(500));
        let out = excerpt_skill_body("foo", &raw);
        assert!(
            out.contains("---\nname: foo\nversion: 1.0\n---"),
            "frontmatter preserved: {out}"
        );
        assert!(out.contains("[truncated"));
        assert!(out.contains("call `skill_view name=\"foo\"`"));
        assert!(out.len() < raw.len(), "out shorter than input");
        assert!(out.contains("# Foo skill") || out.contains("This is the body"));
    }

    #[test]
    fn excerpt_skill_body_handles_no_frontmatter() {
        let raw = "Plain content with no yaml header. ".repeat(20);
        let out = excerpt_skill_body("bar", &raw);
        assert!(out.contains("[truncated"));
        assert!(out.contains("call `skill_view name=\"bar\"`"));
        assert!(out.len() < raw.len());
        assert!(
            !out.contains("---"),
            "should not have erroneously added frontmatter delimiters: {out}"
        );
    }

    #[test]
    fn excerpt_skill_body_short_content_passthrough_with_footer() {
        let raw = "Short body.";
        let out = excerpt_skill_body("baz", raw);
        assert!(out.contains("Short body."));
        assert!(out.contains("[truncated"));
    }

    // ────────── Wave A retro #4 — trivial single-tool intent bypass ──────────

    /// The reproducer from the user's smoke after PR #7 — must NOT be
    /// classified as autonomous_work (which would trigger TodoWrite
    /// discipline). The downstream `terminal_status_with_todo_ledger`
    /// bypass (PR #8 commit 57402e4) is a safety net; this test pins
    /// the upstream classifier behavior so the safety net rarely fires.
    #[test]
    fn classifier_skips_simple_create_folder_zh() {
        assert!(!is_tool_required_work_intent(
            "在工作目录创建一个新的文件夹叫eee"
        ));
    }

    #[test]
    fn classifier_skips_simple_delete_folder_zh() {
        assert!(!is_tool_required_work_intent("删除文件夹 eee"));
    }

    #[test]
    fn classifier_skips_simple_create_directory_en() {
        assert!(!is_tool_required_work_intent(
            "create a folder named build at /tmp"
        ));
    }

    /// REGRESSION GUARD — the classifier MUST still catch genuine
    /// multi-step asks. "完整的待办事项管理网页应用" is multi-step
    /// (HTML + CSS + JS + state) and needs TodoWrite discipline.
    #[test]
    fn classifier_keeps_complex_app_request_zh() {
        assert!(is_tool_required_work_intent(
            "请帮我创建一个完整的待办事项管理网页应用"
        ));
    }

    #[test]
    fn classifier_keeps_complex_app_request_en() {
        assert!(is_tool_required_work_intent(
            "build a complete TODO web app with persistence"
        ));
    }

    /// Long messages bypass the trivial bail-out and go through the
    /// main classifier — a long message asking for a folder is
    /// suspicious and should be classified normally.
    #[test]
    fn classifier_does_not_bail_on_long_messages_even_if_trivial_artifact() {
        let long = "请帮我在工作目录下创建一个名为 eee 的新文件夹，然后在里面初始化 git 仓库并写一个 README";
        // "并" + "然后" are multi-step signals → bail-out skipped → main classifier path
        // → matches 创建 + 文件夹 → returns true (correctly: there's clearly more than mkdir here).
        assert!(is_tool_required_work_intent(long));
    }

    /// Direct unit test on the helper so future tuning of the trivial
    /// signals doesn't accidentally re-introduce the false-positive case.
    #[test]
    fn trivially_simple_helper_recognizes_folder_only_artifacts() {
        assert!(is_trivially_simple_single_tool_intent(
            "在工作目录创建一个新的文件夹叫eee"
        ));
        assert!(is_trivially_simple_single_tool_intent("delete folder build"));
    }

    #[test]
    fn trivially_simple_helper_skips_complex_artifacts() {
        assert!(!is_trivially_simple_single_tool_intent("创建一个网站"));
        assert!(!is_trivially_simple_single_tool_intent("build a system"));
    }
}
