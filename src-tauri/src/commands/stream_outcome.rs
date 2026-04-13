//! Task outcome aggregation for stream execution truths.

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
}

pub(crate) struct TaskOutcomeResolver;

impl TaskOutcomeResolver {
    pub(crate) fn resolve(
        execution: ExecutionTruth,
        conversation: &ConversationTruth,
    ) -> UserVisibleTruth {
        if conversation.terminal_status == "cancelled_by_user" {
            return UserVisibleTruth {
                task_outcome: "failed",
                degraded_reason: Some("cancelled_by_user".to_string()),
                resume_available: false,
            };
        }

        if !conversation.stream_failed {
            return UserVisibleTruth {
                task_outcome: "completed",
                degraded_reason: None,
                resume_available: false,
            };
        }

        if execution.has_successful_mutating_tool {
            return UserVisibleTruth {
                task_outcome: "partial_success",
                degraded_reason: conversation.last_stream_error_reason.clone(),
                resume_available: true,
            };
        }

        let degraded_reason = if execution.has_successful_tool {
            conversation
                .last_stream_error_reason
                .as_ref()
                .map(|reason| format!("read_only_success_before_failure:{reason}"))
        } else {
            conversation.last_stream_error_reason.clone()
        };

        UserVisibleTruth {
            task_outcome: "failed",
            degraded_reason,
            resume_available: false,
        }
    }
}
