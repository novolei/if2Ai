//! Typed resume / recoverability contract (MIG-021 / T-011).
//!
//! Replaces free-form `degraded_reason` strings and the single
//! `resume_available` boolean with a typed [`ResumeReason`] enum
//! and a structured [`ResumeRecoverability`] payload carried on
//! `stream_complete` / `stream_error` events.
//!
//! ## Safe-to-retry semantics
//!
//! [`safe_to_retry_mutations`] is the canonical gate: when `true`,
//! the run executed no mutating tools (file writes, memory stores,
//! destructive shell commands) and can be safely retried without
//! risk of duplicate side-effects.

use serde::{Deserialize, Serialize};

/// Typed reason a run became resumable (mirrors the `ResumeReason`
/// frontend contract in `src/transport/contracts.ts`).
///
/// Order matters: the first variant that matches during
/// classification wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeReason {
    /// Provider network / gateway timeout before the first token.
    NetworkTimeout,
    /// Provider stream terminated with an error mid-stream.
    StreamError,
    /// Agent ran into the max-iterations guard.
    MaxIterationsReached,
    /// Agent produced consecutive tool batches with no new meaningful
    /// tool output (conversation loop detector tripped).
    RepeatedToolBatchNoProgress,
    /// Agent produced consecutive invalid tool arguments.
    InvalidToolArgsRepeated,
    /// Provider rate-limited the request (HTTP 429).  The request
    /// can be retried after a delay.
    RateLimited,
    /// Provider refused the request (bad request / validation /
    /// auth).  The user can adjust the prompt and retry.
    ProviderRejected,
    /// A tool succeeded (read-only) before the stream failed —
    /// partial progress exists.
    ReadOnlySuccessBeforeFailure,
    /// The model stopped without issuing tools (empty tool batch).
    ModelStopNoTools,
    /// Stream failed to start at all (pre-connect failure).
    FailedToStartStream,
}

impl ResumeReason {
    /// Human-readable label suitable for a frontend CTA explainer.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::NetworkTimeout => "网络超时",
            Self::StreamError => "流错误",
            Self::MaxIterationsReached => "达到最大迭代次数",
            Self::RepeatedToolBatchNoProgress => "工具批次无进展",
            Self::InvalidToolArgsRepeated => "工具参数无效",
            Self::RateLimited => "请求频率受限",
            Self::ProviderRejected => "Provider 拒绝请求",
            Self::ReadOnlySuccessBeforeFailure => "只读成功但后续失败",
            Self::ModelStopNoTools => "模型停止无工具调用",
            Self::FailedToStartStream => "未能启动流",
        }
    }
}

/// Structured recoverability payload carried on `stream_complete`
/// / `stream_error` so the frontend can render typed resume CTAs
/// without parsing free-form strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeRecoverability {
    /// Whether a resume is offered for this run.
    pub available: bool,
    /// Typed reason (absent when `available` is false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<ResumeReason>,
    /// Whether it is safe to retry without risk of duplicate
    /// side-effects (no mutating tool executed successfully).
    pub safe_to_retry_mutations: bool,
    /// Number of remaining retry attempts (from supervisor budget).
    pub retry_budget_remaining: u32,
}

impl ResumeRecoverability {
    /// Build a no-resume sentinel.
    #[must_use]
    pub fn none() -> Self {
        Self {
            available: false,
            reason: None,
            safe_to_retry_mutations: false,
            retry_budget_remaining: 0,
        }
    }

    /// Build a resumable payload.
    #[must_use]
    pub fn resumable(
        reason: ResumeReason,
        safe_to_retry_mutations: bool,
        retry_budget_remaining: u32,
    ) -> Self {
        Self {
            available: true,
            reason: Some(reason),
            safe_to_retry_mutations,
            retry_budget_remaining,
        }
    }
}

/// Whether the resume can be attempted safely — no mutating tool
/// executed successfully during the run, so a retry will not
/// duplicate side-effects.
///
/// This is the canonical gate.  MIG-021 consumers MUST call this
/// before offering a resume CTA with "safe" labelling.
#[must_use]
pub fn safe_to_retry_mutations(has_successful_mutating_tool: bool) -> bool {
    !has_successful_mutating_tool
}

/// Classify a run's terminal status string into a typed
/// [`ResumeReason`] variant. Returns `None` when the status does
/// not correspond to any known resumable reason.
///
/// The `degraded_reason` string may carry compound information
/// (e.g. `"read_only_success_before_failure:network_timeout"`)
/// that refines the classification.
#[must_use]
pub fn classify_resume_reason(
    terminal_status: &str,
    degraded_reason: Option<&str>,
) -> Option<ResumeReason> {
    match terminal_status {
        "max_iterations_reached" => Some(ResumeReason::MaxIterationsReached),
        "repeated_tool_batch_no_progress" => Some(ResumeReason::RepeatedToolBatchNoProgress),
        "invalid_tool_args_repeated" | "repetitive_model_output" => {
            Some(ResumeReason::InvalidToolArgsRepeated)
        }
        "failed_to_start_stream" => Some(ResumeReason::FailedToStartStream),
        "model_stop_no_tools"
        | "memory_recall_required_no_tool"
        | "tool_required_no_tool"
        | "todo_ledger_incomplete"
        | "provider_textual_tool_call_markup" => Some(ResumeReason::ModelStopNoTools),
        "stream_error" => {
            // Degraded reasons may carry richer information
            if let Some(reason) = degraded_reason {
                if reason.starts_with("read_only_success_before_failure") {
                    return Some(ResumeReason::ReadOnlySuccessBeforeFailure);
                }
                if reason.contains("network_timeout") || reason.contains("timeout") {
                    return Some(ResumeReason::NetworkTimeout);
                }
                if reason.contains("rate_limited") {
                    return Some(ResumeReason::RateLimited);
                }
                if reason.contains("provider_rejected")
                    || reason.contains("request_validation_error")
                {
                    return Some(ResumeReason::ProviderRejected);
                }
            }
            Some(ResumeReason::StreamError)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_to_retry_false_when_mutating_tool_succeeded() {
        assert!(!safe_to_retry_mutations(true));
    }

    #[test]
    fn safe_to_retry_true_when_no_mutating_tool() {
        assert!(safe_to_retry_mutations(false));
    }

    #[test]
    fn recoverability_serde_round_trips() {
        let r = ResumeRecoverability::resumable(ResumeReason::NetworkTimeout, true, 3);
        let json = serde_json::to_string(&r).unwrap();
        let back: ResumeRecoverability = serde_json::from_str(&json).unwrap();
        assert!(back.available);
        assert_eq!(back.reason, Some(ResumeReason::NetworkTimeout));
        assert!(back.safe_to_retry_mutations);
        assert_eq!(back.retry_budget_remaining, 3);
    }

    #[test]
    fn resume_reason_serde_is_snake_case() {
        let json = serde_json::to_string(&ResumeReason::MaxIterationsReached).unwrap();
        assert_eq!(json, "\"max_iterations_reached\"");
    }

    #[test]
    fn all_resume_reasons_have_labels() {
        let reasons = [
            ResumeReason::NetworkTimeout,
            ResumeReason::StreamError,
            ResumeReason::MaxIterationsReached,
            ResumeReason::RepeatedToolBatchNoProgress,
            ResumeReason::InvalidToolArgsRepeated,
            ResumeReason::RateLimited,
            ResumeReason::ProviderRejected,
            ResumeReason::ReadOnlySuccessBeforeFailure,
            ResumeReason::ModelStopNoTools,
            ResumeReason::FailedToStartStream,
        ];
        for reason in &reasons {
            assert!(!reason.label().is_empty());
        }
    }

    #[test]
    fn classify_max_iterations_reached() {
        assert_eq!(
            classify_resume_reason("max_iterations_reached", None),
            Some(ResumeReason::MaxIterationsReached)
        );
    }

    #[test]
    fn classify_repeated_tool_batch() {
        assert_eq!(
            classify_resume_reason("repeated_tool_batch_no_progress", None),
            Some(ResumeReason::RepeatedToolBatchNoProgress)
        );
    }

    #[test]
    fn classify_invalid_tool_args() {
        assert_eq!(
            classify_resume_reason("invalid_tool_args_repeated", None),
            Some(ResumeReason::InvalidToolArgsRepeated)
        );
    }

    #[test]
    fn classify_failed_to_start_stream() {
        assert_eq!(
            classify_resume_reason("failed_to_start_stream", None),
            Some(ResumeReason::FailedToStartStream)
        );
    }

    #[test]
    fn classify_model_stop_no_tools() {
        assert_eq!(
            classify_resume_reason("model_stop_no_tools", None),
            Some(ResumeReason::ModelStopNoTools)
        );
    }

    #[test]
    fn classify_tool_required_no_tool() {
        assert_eq!(
            classify_resume_reason("tool_required_no_tool", None),
            Some(ResumeReason::ModelStopNoTools)
        );
    }

    #[test]
    fn classify_todo_ledger_incomplete() {
        assert_eq!(
            classify_resume_reason("todo_ledger_incomplete", None),
            Some(ResumeReason::ModelStopNoTools)
        );
    }

    #[test]
    fn classify_stream_error_with_network_timeout() {
        assert_eq!(
            classify_resume_reason("stream_error", Some("network_timeout")),
            Some(ResumeReason::NetworkTimeout)
        );
    }

    #[test]
    fn classify_stream_error_with_read_only_before_failure() {
        assert_eq!(
            classify_resume_reason(
                "stream_error",
                Some("read_only_success_before_failure:network_timeout")
            ),
            Some(ResumeReason::ReadOnlySuccessBeforeFailure)
        );
    }

    #[test]
    fn classify_stream_error_with_provider_rejected() {
        assert_eq!(
            classify_resume_reason("stream_error", Some("request_validation_error")),
            Some(ResumeReason::ProviderRejected)
        );
    }

    #[test]
    fn classify_unknown_status_returns_none() {
        assert_eq!(classify_resume_reason("completed", None), None);
    }
}
