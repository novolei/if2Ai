//! Work-loop routing and skill-resolution helpers for `TurnService`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};
use crate::modules::tools::builtin::skill::{
    check_required_env_vars, local_review_skill_content, resolve_skill_config_block,
    resolve_skill_path,
};

const AUTO_SKILL_TOOLS: &[&str] = &["skill_find", "skill_search", "skill_view"];
const MEMORY_RECALL_INTENT_REASON: &str = "memory_recall_intent";
const TOOL_REQUIRED_WORK_INTENT_REASON: &str = "tool_required_work_intent";
const CONTINUATION_INTENT_REASON: &str = "continuation_intent";
const INHERITED_TOOL_REQUIRED_WORK_INTENT_REASON: &str = "inherited_tool_required_work_intent";
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

/// Recent session facts that can influence work-loop routing.
///
/// This is derived from the existing durable session transcript before the
/// current user message is appended. It is not a new runtime truth source.
#[derive(Debug, Clone, Default)]
pub(super) struct WorkLoopRouteContext {
    pub last_user_message: Option<String>,
    pub last_assistant_text: Option<String>,
    pub last_assistant_claimed_tool_execution_without_tool: bool,
    pub last_task_outcome: Option<String>,
    pub last_degraded_reason: Option<String>,
    pub last_resume_available: Option<bool>,
    pub recent_tool_required_without_tool_message: Option<String>,
    pub event_log_resume_cursor: Option<String>,
    pub event_log_unfinished_goal: Option<String>,
}

/// Build routing context from the current session transcript.
#[must_use]
pub(super) fn route_context_from_messages(
    messages: &[ConversationMessage],
) -> WorkLoopRouteContext {
    let last_user_message = messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::User)
        .and_then(message_text);
    let last_assistant = messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::Assistant);

    let recent_tool_required_without_tool_message = recent_tool_required_without_tool(messages);
    let last_assistant_claimed_tool_execution_without_tool =
        last_assistant.is_some_and(assistant_claimed_tool_execution_without_tool_message);

    WorkLoopRouteContext {
        last_user_message,
        last_assistant_text: last_assistant.and_then(message_text),
        last_assistant_claimed_tool_execution_without_tool,
        last_task_outcome: last_assistant.and_then(|message| message.task_outcome.clone()),
        last_degraded_reason: last_assistant.and_then(|message| message.degraded_reason.clone()),
        last_resume_available: last_assistant.and_then(|message| message.resume_available),
        recent_tool_required_without_tool_message,
        event_log_resume_cursor: None,
        event_log_unfinished_goal: None,
    }
}

/// Augment routing context from the durable run event log.
///
/// This replays already persisted facts; it does not create a second session
/// truth source. It is used mainly for short continuation turns like
/// "continue", where the newest transcript text may not contain enough
/// execution metadata to recover the unfinished goal.
pub(super) fn augment_route_context_from_run_log(
    context: &mut WorkLoopRouteContext,
    entries: &[crate::modules::runtime::event_log::RunLogEntry],
) {
    for entry in entries.iter().rev() {
        match entry.event_type.as_str() {
            "final_run_report" => {
                let report = entry
                    .payload
                    .get("tool_args")
                    .cloned()
                    .unwrap_or_else(|| entry.payload.clone());
                if let Some(status) = report
                    .get("terminalStatus")
                    .or_else(|| report.get("terminal_status"))
                    .and_then(|value| value.as_str())
                    .filter(|status| is_tool_required_terminal_reason(status))
                {
                    context.last_degraded_reason = Some(status.to_string());
                }
                if let Some(outcome) = report
                    .get("taskOutcome")
                    .or_else(|| report.get("task_outcome"))
                    .and_then(|value| value.as_str())
                {
                    context.last_task_outcome = Some(outcome.to_string());
                }
                if let Some(resume_available) = report
                    .get("resumeAvailable")
                    .or_else(|| report.get("resume_available"))
                    .and_then(|value| value.as_bool())
                {
                    context.last_resume_available = Some(resume_available);
                }
                if context.event_log_resume_cursor.is_none() {
                    context.event_log_resume_cursor = report
                        .get("resumeCursor")
                        .or_else(|| report.get("resume_cursor"))
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string);
                }
            }
            "run_started" | "user_message" => {
                if context.event_log_unfinished_goal.is_none() {
                    context.event_log_unfinished_goal = entry
                        .payload
                        .get("message_preview")
                        .or_else(|| entry.payload.get("user_message"))
                        .or_else(|| entry.payload.get("content"))
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string)
                        .filter(|message| is_tool_required_work_intent(message));
                }
            }
            "tool_required_no_tool_retry" => {
                context.last_degraded_reason = Some("tool_required_no_tool".to_string());
                context.last_resume_available = Some(true);
            }
            _ => {}
        }

        if context.event_log_unfinished_goal.is_some()
            && context
                .last_degraded_reason
                .as_deref()
                .is_some_and(is_tool_required_terminal_reason)
        {
            break;
        }
    }

    if context.recent_tool_required_without_tool_message.is_none() {
        context.recent_tool_required_without_tool_message =
            context.event_log_unfinished_goal.clone();
    }
}

/// Route a classifier decision to the internal work-loop taxonomy.
#[must_use]
pub(super) fn route_work_loop(
    decision: &ExecutionModeDecision,
    user_message: &str,
) -> WorkLoopDecision {
    route_work_loop_with_context(decision, user_message, &WorkLoopRouteContext::default())
}

/// Route a classifier decision with recent session context.
#[must_use]
pub(super) fn route_work_loop_with_context(
    decision: &ExecutionModeDecision,
    user_message: &str,
    context: &WorkLoopRouteContext,
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
    let tool_required_work_intent = is_tool_required_work_intent(user_message);
    if tool_required_work_intent {
        push_unique(
            &mut reason_codes,
            TOOL_REQUIRED_WORK_INTENT_REASON.to_string(),
        );
    }
    let continuation_intent = is_continuation_intent(user_message);
    if continuation_intent {
        push_unique(&mut reason_codes, CONTINUATION_INTENT_REASON.to_string());
    }
    let inherited_tool_required_work_intent =
        continuation_intent && context_requires_tool_execution(context);
    if inherited_tool_required_work_intent {
        push_unique(
            &mut reason_codes,
            TOOL_REQUIRED_WORK_INTENT_REASON.to_string(),
        );
        push_unique(
            &mut reason_codes,
            INHERITED_TOOL_REQUIRED_WORK_INTENT_REASON.to_string(),
        );
    }
    let tool_required_work_intent =
        tool_required_work_intent || inherited_tool_required_work_intent;

    let loop_kind = match decision.execution_mode {
        ExecutionMode::SpecializedSurface => WorkLoopKind::SpecializedSurface,
        ExecutionMode::PlanThenConfirm => WorkLoopKind::PlanThenConfirm,
        ExecutionMode::AutoPlanExecute => WorkLoopKind::AutonomousWork,
        ExecutionMode::DirectExecute => {
            if tool_required_work_intent {
                WorkLoopKind::AutonomousWork
            } else if !memory_recall_intent
                && !tool_required_work_intent
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
                when_to_use: None,
                allowed_tools: Vec::new(),
                model_hint: None,
                activation_evidence: vec!["active skill id requested".to_string()],
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
        candidate.activation_evidence = vec![candidate.reason.clone()];
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

        let metadata = parse_skill_runtime_metadata(&content);
        candidate.when_to_use = metadata.when_to_use.clone();
        candidate.allowed_tools = metadata.allowed_tools.clone();
        candidate.model_hint = metadata.model_hint.clone();
        if let Some(when_to_use) = metadata.when_to_use.as_ref() {
            candidate
                .activation_evidence
                .push(format!("frontmatter whenToUse: {when_to_use}"));
        }

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
        let metadata_block = skill_metadata_prompt_block(candidate);
        let mut section = format!(
            "## Skill: {} [{}]\nThe full SKILL.md content is already loaded. Do not reload it with tools.\n{}\n\n{}",
            candidate.name,
            candidate.source,
            metadata_block,
            content
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SkillRuntimeMetadata {
    when_to_use: Option<String>,
    allowed_tools: Vec<String>,
    model_hint: Option<String>,
}

fn parse_skill_runtime_metadata(content: &str) -> SkillRuntimeMetadata {
    let Some(body) = content.strip_prefix("---") else {
        return SkillRuntimeMetadata::default();
    };
    let Some((header, _)) = body.split_once("---") else {
        return SkillRuntimeMetadata::default();
    };

    let mut metadata = SkillRuntimeMetadata::default();
    for line in header.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key.trim() {
            "whenToUse" | "when_to_use" => metadata.when_to_use = Some(value.to_string()),
            "allowedTools" | "allowed_tools" => {
                metadata.allowed_tools = parse_frontmatter_list(value)
            }
            "modelHint" | "model_hint" | "model" => metadata.model_hint = Some(value.to_string()),
            _ => {}
        }
    }
    metadata
}

fn parse_frontmatter_list(value: &str) -> Vec<String> {
    let inner = value.trim_start_matches('[').trim_end_matches(']');
    inner
        .split(',')
        .map(|part| part.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn skill_metadata_prompt_block(candidate: &SkillResolutionCandidate) -> String {
    let mut lines = Vec::new();
    if let Some(when_to_use) = candidate.when_to_use.as_ref() {
        lines.push(format!("whenToUse: {when_to_use}"));
    }
    if !candidate.allowed_tools.is_empty() {
        lines.push(format!(
            "allowedTools: {}",
            candidate.allowed_tools.join(", ")
        ));
    }
    if let Some(model_hint) = candidate.model_hint.as_ref() {
        lines.push(format!("modelHint: {model_hint}"));
    }
    if !candidate.activation_evidence.is_empty() {
        lines.push(format!(
            "activationEvidence: {}",
            candidate.activation_evidence.join(" | ")
        ));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("\nRuntime metadata:\n{}", lines.join("\n"))
    }
}

/// Canonical provider-visible tool pool after skill and work-loop policy.
#[derive(Debug, Clone)]
pub(super) struct CanonicalToolPool {
    pub definitions: Vec<ToolDefinition>,
    pub tool_names: Vec<String>,
    pub schema_hash: String,
    pub policy: String,
}

#[must_use]
pub(super) fn build_canonical_tool_pool(
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

fn stable_tool_schema_hash(definitions: &[ToolDefinition]) -> String {
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

fn tool_pool_policy_label(work_loop: &WorkLoopDecision) -> &'static str {
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

/// Return true when a work loop must produce concrete tool evidence.
#[must_use]
pub(super) fn requires_tool_execution_evidence(work_loop: &WorkLoopDecision) -> bool {
    work_loop
        .reason_codes
        .iter()
        .any(|reason| reason == TOOL_REQUIRED_WORK_INTENT_REASON)
}

/// Prompt contribution that prevents artifact-building turns from answering in prose only.
#[must_use]
pub(super) fn tool_required_prompt_contribution(
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
pub(super) fn continuation_context_prompt_contribution(
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
    if terminal_status == "provider_textual_tool_call_markup" {
        failed_items.push(
            "The provider emitted textual tool-call markup instead of a structured tool call."
                .to_string(),
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

fn skill_candidate(
    entry: &SkillIndexEntry,
    reason: impl Into<String>,
    score: u32,
) -> SkillResolutionCandidate {
    let trusted_source = source_family_can_auto_load(&entry.source);
    let reason = reason.into();
    SkillResolutionCandidate {
        skill_id: Some(entry.name.clone()),
        name: entry.name.clone(),
        source: entry.source.clone(),
        reason: reason.clone(),
        score,
        trusted_source,
        auto_load_allowed: trusted_source,
        loaded: false,
        blocked_reason: None,
        load_warning: None,
        when_to_use: None,
        allowed_tools: Vec::new(),
        model_hint: None,
        activation_evidence: vec![reason],
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
        "ls ",
        "cat ",
        "run ",
        "执行",
        "打开",
        "edit",
        "write",
        "create",
        "delete",
        "install",
        "build",
        "test",
        "complete",
        "finish",
        "finalize",
        "bash",
        "网页",
        "网站",
        "页面",
        "游戏",
        "html",
        "css",
        "javascript",
        "文件",
        "写入",
        "完成",
        "完善",
        "补完",
        "写完",
        "做完",
        "完整",
    ];
    !toolish.iter().any(|needle| lower.contains(needle))
}

fn is_tool_required_work_intent(user_message: &str) -> bool {
    let lower = user_message.to_lowercase();
    let completion_action = [
        "完整网页",
        "完整网站",
        "完整页面",
        "完整游戏",
        "完整应用",
        "完整版",
        "完整的网页",
        "完整的页面",
        "完整的游戏",
        "完整网页版",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let action = completion_action
        || [
            "create",
            "build",
            "make",
            "implement",
            "write",
            "complete",
            "finish",
            "finalize",
            "generate",
            "develop",
            "scaffold",
            "edit",
            "modify",
            "fix",
            "创建",
            "新建",
            "生成",
            "实现",
            "完成",
            "完善",
            "补完",
            "写完",
            "做完",
            "收尾",
            "开发",
            "搭建",
            "做一个",
            "写一个",
            "帮我做",
            "帮我创建",
            "帮我写",
            "修改",
            "修复",
            "优化",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
    let artifact = [
        "web app",
        "website",
        "webpage",
        "page",
        "game",
        "app",
        "component",
        "demo",
        "file",
        "html",
        "css",
        "javascript",
        "typescript",
        "网页",
        "网站",
        "页面",
        "游戏",
        "应用",
        "组件",
        "文件",
        "界面",
        "声效",
        "音效",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    action && artifact
}

fn context_requires_tool_execution(context: &WorkLoopRouteContext) -> bool {
    if context.recent_tool_required_without_tool_message.is_some() {
        return true;
    }

    if context
        .last_user_message
        .as_deref()
        .is_some_and(is_tool_required_work_intent)
    {
        return true;
    }

    if context.last_assistant_claimed_tool_execution_without_tool {
        return true;
    }

    if context
        .last_degraded_reason
        .as_deref()
        .is_some_and(is_tool_required_terminal_reason)
    {
        return true;
    }

    if context.last_resume_available == Some(true)
        && context
            .last_task_outcome
            .as_deref()
            .is_some_and(|outcome| matches!(outcome, "failed" | "partial_success"))
    {
        return true;
    }

    context.last_assistant_text.as_deref().is_some_and(|text| {
        text.contains("未执行工具")
            || text.contains("无法确认完成")
            || text.contains("no tool")
            || text.contains("no tools")
            || text.contains("unable to confirm completion")
    })
}

/// Return true when assistant prose claims it is executing/writing files without tool evidence.
#[must_use]
pub(super) fn assistant_claims_tool_execution_without_tool(text: &str) -> bool {
    let lower = text.to_lowercase();
    let tool_channel_marker = lower.contains("<function_calls")
        || lower.contains("<invoke")
        || lower.contains("tool_use")
        || lower.contains("function_call");
    let command_write_claim = (lower.contains("bash")
        || lower.contains("shell")
        || lower.contains("terminal")
        || lower.contains("命令")
        || lower.contains("工具"))
        && (lower.contains("写入")
            || lower.contains("创建文件")
            || lower.contains("保存")
            || lower.contains("write")
            || lower.contains("create file"));
    let file_write_claim = (lower.contains("index.html")
        || lower.contains(".html")
        || lower.contains(".css")
        || lower.contains(".js")
        || lower.contains("文件"))
        && (lower.contains("直接写入")
            || lower.contains("一次性写入")
            || lower.contains("完整写入")
            || lower.contains("我把")
            || lower.contains("我会")
            || lower.contains("让我用")
            || lower.contains("i will write")
            || lower.contains("i'll write"));

    tool_channel_marker || command_write_claim || file_write_claim
}

/// Return true when assistant prose says it is about to use an external action.
#[must_use]
pub(super) fn assistant_signals_tool_intent(text: &str) -> bool {
    let lower = text.to_lowercase();
    if assistant_claims_tool_execution_without_tool(text) {
        return true;
    }

    let action_prefix = [
        "let me",
        "i'll",
        "i will",
        "i am going to",
        "i'm going to",
        "让我",
        "我来",
        "我会",
        "我将",
        "先看",
        "先检查",
        "先执行",
        "开始",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let external_action = [
        "search",
        "check",
        "fetch",
        "find",
        "read",
        "write",
        "create",
        "run",
        "execute",
        "inspect",
        "open",
        "edit",
        "bash",
        "tool",
        "查找",
        "搜索",
        "检查",
        "读取",
        "写入",
        "创建",
        "运行",
        "执行",
        "打开",
        "调用工具",
        "用工具",
        "用 bash",
        "用 repl",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    action_prefix && external_action
}

/// Provider compatibility signal for models that print pseudo tool markup
/// instead of emitting structured `tool_calls` deltas.
#[must_use]
pub(super) fn detect_textual_tool_call_markup(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let family = if lower.contains("<function_calls") || lower.contains("</function_calls>") {
        Some("xml_function_calls")
    } else if lower.contains("<invoke") || lower.contains("</invoke>") {
        Some("xml_invoke")
    } else if lower.contains("<tool_use") || lower.contains("</tool_use>") {
        Some("xml_tool_use")
    } else if lower.contains("\"tool_calls\"") || lower.contains("function_call") {
        Some("json_tool_call_text")
    } else {
        None
    }?;
    Some(family.to_string())
}

/// Nudge used when provider text announces an action but emits no tool call.
#[must_use]
pub(super) fn tool_intent_nudge_message() -> String {
    "[agent_loop_control]\nYou said you would perform an action, but no tool call was emitted. Do not describe the action in prose. Use the available tool_calls mechanism now with complete JSON arguments. If the intended tool is blocked or unavailable, produce a final report that explicitly says why the work cannot proceed.".to_string()
}

fn is_tool_required_terminal_reason(reason: &str) -> bool {
    matches!(
        reason,
        "tool_required_no_tool"
            | "model_stop_no_tools"
            | "max_iterations_reached"
            | "repeated_tool_batch_no_progress"
            | "invalid_tool_args_repeated"
            | "provider_textual_tool_call_markup"
            | "provider_prepare_failed"
            | "stream_error"
    )
}

fn is_continuation_intent(user_message: &str) -> bool {
    let normalized = user_message.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }

    matches!(
        normalized.as_str(),
        "继续"
            | "接着"
            | "继续吧"
            | "继续做"
            | "接着做"
            | "继续执行"
            | "继续完成"
            | "继续刚才的"
            | "继续上一步"
            | "continue"
            | "go on"
            | "resume"
            | "keep going"
            | "carry on"
    ) || normalized.starts_with("继续")
        || normalized.starts_with("接着")
        || normalized.starts_with("continue ")
        || normalized.starts_with("resume ")
        || normalized.starts_with("keep going")
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

fn message_text(message: &ConversationMessage) -> Option<String> {
    let text = message
        .blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn recent_tool_required_without_tool(messages: &[ConversationMessage]) -> Option<String> {
    let (index, message) =
        messages
            .iter()
            .enumerate()
            .rev()
            .find(|(_, message)| match message.role {
                MessageRole::User => {
                    message_text(message).is_some_and(|text| is_tool_required_work_intent(&text))
                }
                _ => false,
            })?;
    let text = message_text(message)?;
    let mutating_tool_evidence_after = messages
        .iter()
        .skip(index + 1)
        .flat_map(|message| message.blocks.iter())
        .any(block_is_mutating_tool_evidence);
    (!mutating_tool_evidence_after && is_tool_required_work_intent(&text)).then_some(text)
}

fn assistant_claimed_tool_execution_without_tool_message(message: &ConversationMessage) -> bool {
    let text_claim = message
        .blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .any(assistant_claims_tool_execution_without_tool);
    let mutating_tool_evidence = message.blocks.iter().any(block_is_mutating_tool_evidence);

    text_claim && !mutating_tool_evidence
}

fn block_is_mutating_tool_evidence(block: &ContentBlock) -> bool {
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
        || terminal_status == "tool_required_no_tool"
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
}
