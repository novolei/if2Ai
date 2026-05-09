//! Routing and intent-classification helpers for the work loop.

use crate::modules::runtime::contracts::agent_loop::{WorkLoopDecision, WorkLoopKind};
use crate::modules::runtime::contracts::execution_mode::{
    ComplexityLevel, ExecutionMode, ExecutionModeDecision,
};
use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};

use super::{
    CONTINUATION_INTENT_REASON, INHERITED_TOOL_REQUIRED_WORK_INTENT_REASON,
    MEMORY_RECALL_INTENT_REASON, SIMPLE_SHELL_COMMAND_INTENT_REASON,
    TOOL_REQUIRED_WORK_INTENT_REASON, assistant_claims_tool_execution_without_tool,
    block_is_mutating_tool_evidence, is_tool_required_terminal_reason, push_unique,
};

/// Routing context built from the current session transcript and event log.
#[derive(Debug, Default, Clone)]
pub struct WorkLoopRouteContext {
    /// The last user message text, if any.
    pub last_user_message: Option<String>,
    /// The last assistant response text, if any.
    pub last_assistant_text: Option<String>,
    /// Whether the last assistant message claimed tool execution without evidence.
    pub last_assistant_claimed_tool_execution_without_tool: bool,
    /// The task outcome field from the last assistant message.
    pub last_task_outcome: Option<String>,
    /// The degraded reason field from the last assistant message.
    pub last_degraded_reason: Option<String>,
    /// Whether a resume is available from the last assistant message.
    pub last_resume_available: Option<bool>,
    /// The most recent user message that required tools, if no mutating tool evidence followed.
    pub recent_tool_required_without_tool_message: Option<String>,
    /// Resume cursor replayed from the durable run event log.
    pub event_log_resume_cursor: Option<String>,
    /// Unfinished goal text replayed from the durable run event log.
    pub event_log_unfinished_goal: Option<String>,
}

/// Build routing context from the current session transcript.
#[must_use]
pub fn route_context_from_messages(messages: &[ConversationMessage]) -> WorkLoopRouteContext {
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
pub fn augment_route_context_from_run_log(
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
            "run_started" | "user_message" if context.event_log_unfinished_goal.is_none() => {
                context.event_log_unfinished_goal = entry
                    .payload
                    .get("message_preview")
                    .or_else(|| entry.payload.get("user_message"))
                    .or_else(|| entry.payload.get("content"))
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string)
                    .filter(|message| is_tool_required_work_intent(message));
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
pub fn route_work_loop(
    decision: &ExecutionModeDecision,
    user_message: &str,
) -> WorkLoopDecision {
    route_work_loop_with_context(decision, user_message, &WorkLoopRouteContext::default())
}

/// Route a classifier decision with recent session context.
#[must_use]
pub fn route_work_loop_with_context(
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
    let simple_shell_command_intent = is_direct_shell_command_intent(user_message);
    if simple_shell_command_intent {
        push_unique(
            &mut reason_codes,
            SIMPLE_SHELL_COMMAND_INTENT_REASON.to_string(),
        );
    }
    let inherited_tool_required_work_intent = continuation_intent
        && !simple_shell_command_intent
        && context_requires_tool_execution(context);
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

    let loop_kind = if simple_shell_command_intent {
        WorkLoopKind::DirectExecute
    } else {
        match decision.execution_mode {
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
        }
    };

    WorkLoopDecision {
        loop_kind,
        reason_codes,
        requires_confirmation: matches!(loop_kind, WorkLoopKind::PlanThenConfirm),
        route_hint: decision.route_hint.as_ref().map(|hint| hint.0.clone()),
    }
}

/// Return true when the user message looks like a simple direct-answer request.
pub fn looks_like_direct_answer(user_message: &str, complexity: ComplexityLevel) -> bool {
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

/// Wave A retro #4 — recognize clearly trivial single-tool requests so
/// they don't get promoted to AutonomousWork mode (which would trigger
/// TodoWrite discipline + the `todo_ledger_incomplete` safety check on
/// model_stop without formal ledger closure).
///
/// Conservative by design: only excludes when ALL conditions hold:
/// 1. Single-line input
/// 2. ≤ 80 characters total
/// 3. No multi-step or complex-artifact signals (网页/网站/系统/项目/完整/and/then/...)
/// 4. References a "primitive single-target" artifact (文件夹 / 目录 /
///    folder / directory) — these are clearly 1-tool tasks (mkdir /
///    delete / move). Plain "文件" alone is excluded because creating
///    a source code file or config can legitimately be multi-step.
///
/// False-negative tolerated: a long message that's still single-tool
/// will go through the main classifier and possibly hit the
/// downstream `terminal_status_with_todo_ledger` bypass (PR #8
/// commit 57402e4) instead. False-positive impact is high (every
/// false positive blocks the agent), false-negative impact is low
/// (just goes through the well-tested main path).
pub fn is_trivially_simple_single_tool_intent(lower: &str) -> bool {
    if lower.lines().count() != 1 {
        return false;
    }
    if lower.chars().count() > 80 {
        return false;
    }
    let multi_step_signals = [
        "完整",
        "系统",
        "项目",
        "网页",
        "网站",
        "应用",
        "页面",
        "游戏",
        "组件",
        " and ",
        " then ",
        "并且",
        "然后",
        "complete",
        "full ",
        "system",
        "project",
        "webpage",
        "website",
        " app ",
        "appli",
        "component",
    ];
    if multi_step_signals.iter().any(|n| lower.contains(n)) {
        return false;
    }
    let trivial_artifact = ["文件夹", "目录", "folder", "directory"];
    trivial_artifact.iter().any(|n| lower.contains(n))
}

/// Return true when the user message signals a tool-required work intent.
pub fn is_tool_required_work_intent(user_message: &str) -> bool {
    let lower = user_message.to_lowercase();

    // Wave A retro #4 — early-out for obvious 1-tool requests. Without
    // this, "在工作目录创建一个新的文件夹叫eee" (17 chars, 1 mkdir
    // call) gets promoted to AutonomousWork mode, which triggers
    // TodoWrite discipline, which fires `todo_ledger_incomplete` when
    // the agent forgets to formally close its 1-step plan even though
    // mkdir succeeded. The `terminal_status_with_todo_ledger` bypass
    // (PR #8 commit 57402e4) is a downstream safety net; this is the
    // upstream fix that prevents the safety net from being needed in
    // the first place.
    if is_trivially_simple_single_tool_intent(&lower) {
        return false;
    }

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
            "delete",
            "remove",
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
            "删除",
            "移除",
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
        "folder",
        "directory",
        "readme",
        ".md",
        ".html",
        ".css",
        ".js",
        ".ts",
        ".tsx",
        ".json",
        ".txt",
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
        "文件夹",
        "目录",
        "界面",
        "声效",
        "音效",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    action && artifact
}

/// Return true when the user message is a single direct shell command.
pub fn is_direct_shell_command_intent(user_message: &str) -> bool {
    let trimmed = user_message.trim();
    if trimmed.is_empty() || trimmed.lines().count() != 1 {
        return false;
    }
    let lower = trimmed.to_lowercase();
    if ["请", "帮我", "为什么", "怎么", "如何", "what", "why", "how"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        return false;
    }
    let Some(command) = lower.split_whitespace().next() else {
        return false;
    };
    matches!(
        command,
        "ls" | "pwd"
            | "date"
            | "whoami"
            | "id"
            | "uname"
            | "git"
            | "find"
            | "du"
            | "df"
            | "cat"
            | "sed"
            | "tail"
            | "head"
            | "wc"
            | "grep"
            | "rg"
            | "tree"
            | "ps"
            | "which"
            | "node"
            | "npm"
            | "pnpm"
            | "cargo"
            | "python"
            | "python3"
            | "bash"
            | "sh"
    )
}

/// Return true when the user message is a continuation intent.
pub fn is_continuation_intent(user_message: &str) -> bool {
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

/// Return true when the user message is a memory recall intent.
pub fn is_memory_recall_intent(user_message: &str) -> bool {
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

/// Return true when recent context implies tool execution is required.
pub fn context_requires_tool_execution(context: &WorkLoopRouteContext) -> bool {
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

/// Return true when recent messages contain tool-required evidence without tool completion.
pub fn recent_tool_required_without_tool(messages: &[ConversationMessage]) -> Option<String> {
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

/// Return true when the last assistant message claims tool execution without evidence.
pub fn assistant_claimed_tool_execution_without_tool_message(
    message: &ConversationMessage,
) -> bool {
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

/// Extract text from a conversation message's text blocks.
pub fn message_text(message: &ConversationMessage) -> Option<String> {
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
