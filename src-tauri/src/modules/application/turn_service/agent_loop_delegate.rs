//! Agent-loop delegate boundary for turn-service execution.
//!
//! AWL-005 keeps the existing streaming implementation intact while making the
//! selected loop kind execute through a small delegate seam. Future loop kinds
//! can add delegates without turning `stream_task.rs` back into the only place
//! every loop behavior must grow.

use std::future::Future;
use std::pin::Pin;

use crate::modules::runtime::contracts::agent_loop::{
    FinalRunReport, LoopOutcomeKind, PendingOperationMetadata, SkillResolutionPlan,
    WorkLoopDecision,
};
use crate::modules::runtime::resume_cursor::build_resume_cursor;
use crate::modules::runtime::stream_outcome::{
    ConversationTruth, ExecutionTruth, TaskOutcomeResolver,
};

use super::stream_task::{run_stream_task_body, StreamTaskInputs};

/// Boxed future returned by an [`AgentLoopDelegate`].
pub(super) type AgentLoopDelegateFuture<'a> =
    Pin<Box<dyn Future<Output = AgentLoopDelegateOutput> + Send + 'a>>;

/// Execution boundary for one selected work-loop implementation.
#[deprecated(note = "Superseded by the `agentic_loop::run_agentic_loop` + \
            `LoopDelegate` callback model (see Steward-Alignment S5). \
            NOT a drop-in replacement: `AgentLoopDelegate::execute` runs an \
            entire turn and returns `AgentLoopDelegateOutput`, while \
            `LoopDelegate` is a per-iteration callback bag invoked by \
            `run_agentic_loop`. Production migration is tracked in deferred \
            sub-commits 5b-2..5b-5 (stream path) and 5c-2..5c-6 (sync path).")]
pub(super) trait AgentLoopDelegate {
    /// Execute the loop and return its canonical terminal outcome summary.
    fn execute<'a>(&'a self, input: AgentLoopDelegateInput) -> AgentLoopDelegateFuture<'a>;
}

/// Input accepted by the first concrete streaming delegate.
pub(super) struct AgentLoopDelegateInput {
    pub stream_task_inputs: StreamTaskInputs,
}

/// Terminal output returned by every loop delegate.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AgentLoopDelegateOutput {
    pub outcome: LoopOutcomeKind,
    pub final_report: FinalRunReport,
    pub pending_operation: Option<PendingOperationMetadata>,
}

/// Terminal state collected from the existing streaming loop.
pub(super) struct AgentLoopTerminalState {
    pub work_loop_decision: WorkLoopDecision,
    pub skill_resolution_plan: SkillResolutionPlan,
    pub stream_id: String,
    pub provider_request_id: String,
    pub stream_failed: bool,
    pub terminal_status: Option<&'static str>,
    pub last_stream_error_reason: Option<String>,
    pub tool_loop_iterations: usize,
    pub token_count: u32,
    pub has_successful_tool: bool,
    pub has_successful_mutating_tool: bool,
    pub pending_operation: Option<PendingOperationMetadata>,
}

/// Delegate for the current provider streaming/tool-loop path.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct StreamingAgentLoopDelegate;

impl StreamingAgentLoopDelegate {
    /// Construct the default streaming delegate.
    #[must_use]
    pub(super) fn new() -> Self {
        Self
    }

    /// Build delegate output from terminal streaming state.
    #[must_use]
    pub(super) fn output_from_terminal_state(
        state: AgentLoopTerminalState,
    ) -> AgentLoopDelegateOutput {
        let terminal_status = state.terminal_status.unwrap_or("unknown");
        let user_visible_truth = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool: state.has_successful_tool,
                has_successful_mutating_tool: state.has_successful_mutating_tool,
            },
            &ConversationTruth {
                stream_failed: state.stream_failed,
                terminal_status,
                last_stream_error_reason: state.last_stream_error_reason.clone(),
            },
        );
        let resume_cursor = user_visible_truth.resume_available.then(|| {
            build_resume_cursor(
                &state.stream_id,
                state.tool_loop_iterations,
                state.token_count,
            )
        });
        let final_report = super::work_loop::build_final_run_report(
            &state.work_loop_decision,
            user_visible_truth.task_outcome.to_string(),
            terminal_status.to_string(),
            state.provider_request_id,
            state.tool_loop_iterations,
            state.has_successful_tool,
            state.has_successful_mutating_tool,
            user_visible_truth.resume_available,
            resume_cursor,
            Some(&state.skill_resolution_plan),
            Vec::new(),
        );

        AgentLoopDelegateOutput {
            outcome: final_report.outcome,
            final_report,
            pending_operation: state.pending_operation,
        }
    }
}

// Legacy delegate retained while RunDelegate adoption (5c-2..5c-6) is queued.
#[allow(deprecated)]
impl AgentLoopDelegate for StreamingAgentLoopDelegate {
    fn execute<'a>(&'a self, input: AgentLoopDelegateInput) -> AgentLoopDelegateFuture<'a> {
        Box::pin(async move { run_stream_task_body(input.stream_task_inputs).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::agent_loop::WorkLoopKind;

    fn work_loop(loop_kind: WorkLoopKind) -> WorkLoopDecision {
        WorkLoopDecision {
            loop_kind,
            reason_codes: vec!["test".to_string()],
            requires_confirmation: matches!(loop_kind, WorkLoopKind::PlanThenConfirm),
            route_hint: None,
        }
    }

    fn empty_skill_plan() -> SkillResolutionPlan {
        SkillResolutionPlan {
            active_skill_ids: Vec::new(),
            candidates: Vec::new(),
            auto_discovery_tools: Vec::new(),
            should_load_find_skills: false,
            remote_install_policy: "quarantine_requires_user_approval".to_string(),
            loaded_skill_names: Vec::new(),
            blocked_skill_names: Vec::new(),
            load_warnings: Vec::new(),
        }
    }

    fn terminal_state(
        loop_kind: WorkLoopKind,
        terminal_status: &'static str,
    ) -> AgentLoopTerminalState {
        AgentLoopTerminalState {
            work_loop_decision: work_loop(loop_kind),
            skill_resolution_plan: empty_skill_plan(),
            stream_id: "stream-test".to_string(),
            provider_request_id: "request-test".to_string(),
            stream_failed: false,
            terminal_status: Some(terminal_status),
            last_stream_error_reason: None,
            tool_loop_iterations: 1,
            token_count: 5,
            has_successful_tool: false,
            has_successful_mutating_tool: false,
            pending_operation: None,
        }
    }

    #[test]
    fn loop_delegate_streaming_success_completed() {
        let _delegate = StreamingAgentLoopDelegate::new();
        let output = StreamingAgentLoopDelegate::output_from_terminal_state(terminal_state(
            WorkLoopKind::DirectAnswer,
            "model_stop_no_tools",
        ));

        assert_eq!(output.outcome, LoopOutcomeKind::Completed);
        assert_eq!(output.final_report.outcome, LoopOutcomeKind::Completed);
        assert_eq!(output.final_report.loop_kind, WorkLoopKind::DirectAnswer);
    }

    #[test]
    fn loop_delegate_provider_error_failed_with_plan() {
        let mut state = terminal_state(WorkLoopKind::AutonomousWork, "failed_to_start_stream");
        state.stream_failed = true;
        state.last_stream_error_reason = Some("provider unavailable".to_string());

        let output = StreamingAgentLoopDelegate::output_from_terminal_state(state);

        assert_eq!(output.outcome, LoopOutcomeKind::FailedWithPlan);
        assert_eq!(output.final_report.request_id, "request-test");
        assert!(output
            .final_report
            .failed_items
            .iter()
            .any(|item| item.contains("failed_to_start_stream")));
    }

    #[test]
    fn loop_delegate_approval_blocked() {
        let pending = PendingOperationMetadata {
            tool_call_id: "tool-1".to_string(),
            tool_name: "file_write".to_string(),
            reason: "mutating tool requires approval".to_string(),
        };
        let mut state = terminal_state(
            WorkLoopKind::PlanThenConfirm,
            "approval_required_for_mutation",
        );
        state.pending_operation = Some(pending.clone());

        let output = StreamingAgentLoopDelegate::output_from_terminal_state(state);

        assert_eq!(output.outcome, LoopOutcomeKind::NeedsApproval);
        assert_eq!(output.pending_operation, Some(pending));
        assert_eq!(output.final_report.outcome, LoopOutcomeKind::NeedsApproval);
    }

    #[test]
    fn loop_delegate_budget_exhausted() {
        let mut state = terminal_state(WorkLoopKind::AutonomousWork, "max_iterations_reached");
        state.tool_loop_iterations = 10;
        state.has_successful_tool = true;

        let output = StreamingAgentLoopDelegate::output_from_terminal_state(state);

        assert_eq!(output.outcome, LoopOutcomeKind::ExhaustedWithSummary);
        assert_eq!(
            output.final_report.outcome,
            LoopOutcomeKind::ExhaustedWithSummary
        );
        assert!(output.final_report.resume_available);
    }
}
