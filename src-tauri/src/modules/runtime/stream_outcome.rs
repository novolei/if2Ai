//! Task outcome aggregation for stream execution truths.

use super::recoverability::{
    classify_resume_reason, safe_to_retry_mutations, ResumeRecoverability,
};

/// Tool execution truth aggregated from one stream run.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExecutionTruth {
    pub(crate) has_successful_tool: bool,
    pub(crate) has_successful_mutating_tool: bool,
}

/// Conversation runtime truth aggregated from streaming lifecycle.
#[derive(Debug, Clone)]
pub(crate) struct ConversationTruth {
    pub(crate) stream_failed: bool,
    pub(crate) terminal_status: &'static str,
    pub(crate) last_stream_error_reason: Option<String>,
}

/// User-visible truth that should be surfaced to frontend.
#[derive(Debug, Clone)]
pub(crate) struct UserVisibleTruth {
    pub(crate) task_outcome: &'static str,
    pub(crate) degraded_reason: Option<String>,
    pub(crate) resume_available: bool,
    /// Structured recoverability payload (MIG-021).
    ///
    /// Replaces the free-form `resume_available` boolean for
    /// frontend resume CTA rendering. Always present — use
    /// [`ResumeRecoverability::available`] to check.
    pub(crate) recoverability: ResumeRecoverability,
}

/// Construct a [`UserVisibleTruth`] with a derived
/// [`ResumeRecoverability`] from the raw outcome fields.
fn new_truth(
    task_outcome: &'static str,
    degraded_reason: Option<String>,
    resume_available: bool,
    terminal_status: &str,
    has_successful_mutating_tool: bool,
) -> UserVisibleTruth {
    let recoverability = if resume_available {
        let reason = classify_resume_reason(terminal_status, degraded_reason.as_deref());
        let safe = safe_to_retry_mutations(has_successful_mutating_tool);
        match reason {
            Some(r) => ResumeRecoverability::resumable(r, safe, 0),
            None => ResumeRecoverability::none(),
        }
    } else {
        ResumeRecoverability::none()
    };
    UserVisibleTruth {
        task_outcome,
        degraded_reason,
        resume_available,
        recoverability,
    }
}

pub(crate) struct TaskOutcomeResolver;

impl TaskOutcomeResolver {
    pub(crate) fn resolve(
        execution: ExecutionTruth,
        conversation: &ConversationTruth,
    ) -> UserVisibleTruth {
        if conversation.terminal_status == "max_iterations_reached" {
            return new_truth(
                "partial_success",
                Some("max_iterations_reached".to_string()),
                true,
                conversation.terminal_status,
                execution.has_successful_mutating_tool,
            );
        }

        if matches!(
            conversation.terminal_status,
            "repeated_tool_batch_no_progress" | "invalid_tool_args_repeated"
        ) {
            return new_truth(
                "partial_success",
                Some(conversation.terminal_status.to_string()),
                true,
                conversation.terminal_status,
                execution.has_successful_mutating_tool,
            );
        }

        if conversation.terminal_status == "memory_recall_required_no_tool" {
            return new_truth(
                "failed",
                Some("memory_recall_required_no_tool".to_string()),
                true,
                conversation.terminal_status,
                execution.has_successful_mutating_tool,
            );
        }

        if conversation.terminal_status == "cancelled_by_user" {
            return new_truth(
                "failed",
                Some("cancelled_by_user".to_string()),
                false,
                conversation.terminal_status,
                execution.has_successful_mutating_tool,
            );
        }

        if !conversation.stream_failed {
            return new_truth(
                "completed",
                None,
                false,
                conversation.terminal_status,
                execution.has_successful_mutating_tool,
            );
        }

        let degraded_reason = if execution.has_successful_mutating_tool {
            conversation.last_stream_error_reason.clone()
        } else if execution.has_successful_tool {
            conversation
                .last_stream_error_reason
                .as_ref()
                .map(|reason| format!("read_only_success_before_failure:{reason}"))
        } else {
            conversation.last_stream_error_reason.clone()
        };

        let has_execution_evidence =
            execution.has_successful_tool || execution.has_successful_mutating_tool;
        let resumable = is_resumable_terminal_status(conversation.terminal_status);

        if has_execution_evidence {
            return new_truth(
                "partial_success",
                degraded_reason,
                true,
                conversation.terminal_status,
                execution.has_successful_mutating_tool,
            );
        }

        new_truth(
            "failed",
            degraded_reason,
            resumable,
            conversation.terminal_status,
            execution.has_successful_mutating_tool,
        )
    }
}

fn is_resumable_terminal_status(status: &str) -> bool {
    matches!(
        status,
        "stream_error"
            | "failed_to_start_stream"
            | "model_stop_no_tools"
            | "repeated_tool_batch_no_progress"
            | "invalid_tool_args_repeated"
    )
}

#[cfg(test)]
mod tests {
    use super::super::recoverability::ResumeReason;
    use super::{ConversationTruth, ExecutionTruth, TaskOutcomeResolver};

    #[test]
    fn failed_to_start_without_tool_evidence_is_failed() {
        let outcome = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool: false,
                has_successful_mutating_tool: false,
            },
            &ConversationTruth {
                stream_failed: true,
                terminal_status: "failed_to_start_stream",
                last_stream_error_reason: Some("request_validation_error".to_string()),
            },
        );
        assert_eq!(outcome.task_outcome, "failed");
        assert!(outcome.resume_available);
        assert!(outcome.recoverability.available);
        assert_eq!(
            outcome.recoverability.reason,
            Some(ResumeReason::FailedToStartStream)
        );
        assert!(outcome.recoverability.safe_to_retry_mutations);
    }

    #[test]
    fn stream_error_with_tool_evidence_is_partial_success() {
        let outcome = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool: true,
                has_successful_mutating_tool: false,
            },
            &ConversationTruth {
                stream_failed: true,
                terminal_status: "stream_error",
                last_stream_error_reason: Some("network_timeout".to_string()),
            },
        );
        assert_eq!(outcome.task_outcome, "partial_success");
        assert!(outcome.resume_available);
        assert!(outcome.recoverability.available);
        assert_eq!(
            outcome.recoverability.reason,
            Some(ResumeReason::ReadOnlySuccessBeforeFailure)
        );
        assert!(outcome.recoverability.safe_to_retry_mutations);
    }

    #[test]
    fn max_iterations_is_partial_success_and_resumable() {
        let outcome = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool: true,
                has_successful_mutating_tool: true,
            },
            &ConversationTruth {
                stream_failed: false,
                terminal_status: "max_iterations_reached",
                last_stream_error_reason: None,
            },
        );
        assert_eq!(outcome.task_outcome, "partial_success");
        assert!(outcome.resume_available);
        assert_eq!(
            outcome.degraded_reason.as_deref(),
            Some("max_iterations_reached")
        );
        assert!(outcome.recoverability.available);
        assert_eq!(
            outcome.recoverability.reason,
            Some(ResumeReason::MaxIterationsReached)
        );
        assert!(!outcome.recoverability.safe_to_retry_mutations);
    }

    #[test]
    fn memory_recall_intent_no_tool_is_failed() {
        let outcome = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool: false,
                has_successful_mutating_tool: false,
            },
            &ConversationTruth {
                stream_failed: false,
                terminal_status: "memory_recall_required_no_tool",
                last_stream_error_reason: None,
            },
        );
        assert_eq!(outcome.task_outcome, "failed");
        assert_eq!(
            outcome.degraded_reason.as_deref(),
            Some("memory_recall_required_no_tool")
        );
        assert!(outcome.resume_available);
        assert!(outcome.recoverability.available);
        assert_eq!(
            outcome.recoverability.reason,
            Some(ResumeReason::ModelStopNoTools)
        );
        assert!(outcome.recoverability.safe_to_retry_mutations);
    }

    #[test]
    fn repeated_tool_batch_is_partial_success_not_completed() {
        let outcome = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool: true,
                has_successful_mutating_tool: false,
            },
            &ConversationTruth {
                stream_failed: false,
                terminal_status: "repeated_tool_batch_no_progress",
                last_stream_error_reason: None,
            },
        );
        assert_eq!(outcome.task_outcome, "partial_success");
        assert!(outcome.resume_available);
        assert_eq!(
            outcome.degraded_reason.as_deref(),
            Some("repeated_tool_batch_no_progress")
        );
        assert!(outcome.recoverability.available);
        assert_eq!(
            outcome.recoverability.reason,
            Some(ResumeReason::RepeatedToolBatchNoProgress)
        );
        assert!(outcome.recoverability.safe_to_retry_mutations);
    }
}
