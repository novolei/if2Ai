//! Run-event log append helpers and small guard predicates shared by
//! [`super::stream_task::run_stream_task_body`], [`super::stream_iteration`],
//! and [`super::stream_tool_execution`].
//!
//! Extracted from `stream_task.rs` (steward-align / god-file reduction) so the
//! orchestrator module stays focused on loop control flow.

use crate::modules::runtime::contracts::agent_loop::WorkLoopDecision;
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::permissions::PermissionPromptDecision;
use crate::modules::runtime::stream_emitter::StreamTokenPayload;

const DEFAULT_MAX_TOOL_LOOP_ITERATIONS: usize = 10;
const MIN_MAX_TOOL_LOOP_ITERATIONS: usize = 3;

/// Hard cap on identical tool batches before the stream loop forces a stop.
pub(super) const REPEATED_TOOL_BATCH_LIMIT: usize = 3;

/// Upper bound on outer tool-loop iterations (env `IF2AI_AGENT_MAX_ITERATIONS`, clamped).
#[must_use]
pub(crate) fn agent_max_iterations() -> usize {
    std::env::var("IF2AI_AGENT_MAX_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.max(MIN_MAX_TOOL_LOOP_ITERATIONS))
        .unwrap_or(DEFAULT_MAX_TOOL_LOOP_ITERATIONS)
}

/// Stable fingerprint of the pending tool batch for repeat-detection.
#[must_use]
pub(super) fn tool_batch_signature(pending_tool_uses: &[(String, String, String)]) -> String {
    let mut parts = pending_tool_uses
        .iter()
        .map(|(_, tool_name, input_json)| {
            let compact_input = input_json.split_whitespace().collect::<Vec<_>>().join(" ");
            let compact_input = if compact_input.chars().count() > 240 {
                format!("{}...", compact_input.chars().take(240).collect::<String>())
            } else {
                compact_input
            };
            format!("{tool_name}:{compact_input}")
        })
        .collect::<Vec<_>>();
    parts.sort();
    parts.join("|")
}

/// Append a streaming token payload to the JSONL run log (type-normalised).
pub(super) async fn append_stream_event(
    run_event_logger: &RunEventLogger,
    payload: &StreamTokenPayload,
) {
    let event_type = match payload.event_type.as_str() {
        "thinking_start" => "thinking_started",
        "tool_call_update" => match payload.tool_status.as_deref() {
            Some("queued") => "tool_call_queued",
            Some("running") => "tool_call_running",
            Some("completed") => "tool_call_completed",
            Some("error") => "tool_call_failed",
            _ => "tool_call_update",
        },
        other => other,
    };
    let _ = run_event_logger
        .append_with_correlation(event_type, payload.clone(), payload.correlation.as_ref())
        .await;
}

/// Record permission override decisions into the run log for replay.
pub(super) async fn append_remembered_permission_events(
    run_event_logger: &RunEventLogger,
    tool_name: &str,
    decision: &PermissionPromptDecision,
) {
    let (decision_label, reason) = match decision {
        PermissionPromptDecision::Allow => ("allow", None),
        PermissionPromptDecision::Deny { reason } => ("deny", Some(reason.clone())),
    };
    let payload = serde_json::json!({
        "tool_name": tool_name,
        "source": "session_override",
        "decision": decision_label,
        "reason": reason,
    });
    let _ = run_event_logger
        .append("permission_requested", payload.clone())
        .await;
    let _ = run_event_logger
        .append("permission_resolved", payload)
        .await;
}

/// When memory recall is required, force a final assistant message once evidence exists.
pub(super) fn apply_memory_recall_success_finalization_guard(
    work_loop_decision: &WorkLoopDecision,
    has_successful_tool: bool,
    force_final_response_next: &mut bool,
    finalization_reason: &mut Option<String>,
) {
    if super::work_loop::should_force_final_after_memory_recall(
        work_loop_decision,
        has_successful_tool,
    ) {
        *force_final_response_next = true;
        *finalization_reason = Some("memory_recall_evidence_observed".to_string());
    }
}

/// Whether to retry when tools are required but the model returned none.
/// Supports up to 3 progressive escalation retries.
#[must_use]
pub(super) fn should_retry_tool_required_no_tool(
    work_loop_decision: &WorkLoopDecision,
    has_successful_mutating_tool: bool,
    retry_count: usize,
    force_final_response: bool,
    available_tool_count: usize,
) -> bool {
    super::work_loop::requires_tool_execution_evidence(work_loop_decision)
        && !has_successful_mutating_tool
        && retry_count < super::stream_task::TOOL_REQUIRED_NO_TOOL_MAX_RETRIES
        && !force_final_response
        && available_tool_count > 0
}

/// Whether to retry once when the assistant text signals tool intent but sent no tool calls.
#[must_use]
pub(super) fn should_retry_announced_tool_intent_no_tool(
    accumulated_text: &str,
    retry_count: usize,
    force_final_response: bool,
    available_tool_count: usize,
) -> bool {
    retry_count == 0
        && !force_final_response
        && available_tool_count > 0
        && super::work_loop::assistant_signals_tool_intent(accumulated_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::common::CorrelationIds;
    use crate::modules::runtime::event_log::{RunEventLogger, RunLogEntry};
    use crate::modules::runtime::stream_emitter::StreamTokenPayload;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("sp-c-{label}-{nanos}"))
    }

    #[tokio::test]
    async fn append_stream_event_preserves_payload_correlation() {
        let root = unique_temp_root("corr");
        let logger = RunEventLogger::for_base_dir(&root, "sess-c", "run-c");
        let mut payload = StreamTokenPayload::skeleton("stream-1", "delta");
        payload.correlation = Some(CorrelationIds {
            stream_id: Some("stream-1".into()),
            team_id: Some("team-a".into()),
            member_id: Some("mem-1".into()),
            parent_run_id: Some("parent-7".into()),
            delegation_id: Some("deleg-9".into()),
            ..Default::default()
        });

        append_stream_event(&logger, &payload).await;

        let path = logger.file_path().expect("run log path");
        let raw = std::fs::read_to_string(path).expect("read run log");
        let line = raw
            .lines()
            .find(|line| !line.trim().is_empty())
            .expect("at least one jsonl line");
        let entry: RunLogEntry = serde_json::from_str(line).expect("parse RunLogEntry");

        assert_eq!(entry.event_type, "delta");
        assert_eq!(entry.correlation_id.as_deref(), Some("stream-1"));
        assert_eq!(entry.team_id.as_deref(), Some("team-a"));
        assert_eq!(entry.member_id.as_deref(), Some("mem-1"));
        assert_eq!(entry.parent_run_id.as_deref(), Some("parent-7"));
        assert_eq!(entry.delegation_id.as_deref(), Some("deleg-9"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn append_stream_event_normalizes_tool_call_update_running_to_tool_call_running() {
        let root = unique_temp_root("normalize");
        let logger = RunEventLogger::for_base_dir(&root, "sess-c", "run-c");
        let mut payload = StreamTokenPayload::skeleton("stream-2", "tool_call_update");
        payload.tool_status = Some("running".into());
        payload.correlation = None;

        append_stream_event(&logger, &payload).await;

        let path = logger.file_path().expect("run log path");
        let raw = std::fs::read_to_string(path).expect("read run log");
        let line = raw
            .lines()
            .find(|line| !line.trim().is_empty())
            .expect("at least one jsonl line");
        let entry: RunLogEntry = serde_json::from_str(line).expect("parse RunLogEntry");

        assert_eq!(entry.event_type, "tool_call_running");

        std::fs::remove_dir_all(&root).ok();
    }
}
