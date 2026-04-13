//! Structured audit events for control-plane tool execution.

use std::time::Duration;

use crate::modules::runtime::permissions::PermissionMode;

/// Structured tool execution audit event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditEvent {
    pub event_type: &'static str,
    pub trace_id: String,
    pub session_id: String,
    pub tool_name: String,
    pub effective_workdir: String,
    pub permission_mode: String,
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

/// Emits unified structured audit events.
pub struct AuditEmitter;

struct AuditContext<'a> {
    trace_id: &'a str,
    session_id: &'a str,
    tool_name: &'a str,
    effective_workdir: &'a std::path::Path,
    permission_mode: PermissionMode,
}

struct FailureDetail<'a> {
    error_code: &'a str,
    failure_stage: &'a str,
    retryable: bool,
}

pub struct FailureDiagnostic<'a> {
    pub reason: &'a str,
    pub error_code: &'a str,
    pub failure_stage: &'a str,
    pub retryable: bool,
}

impl AuditEmitter {
    /// Generate a new trace id for one tool execution chain.
    #[must_use]
    pub fn new_trace_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn base_event(
        event_type: &'static str,
        context: AuditContext<'_>,
        duration: Duration,
        message: Option<String>,
        failure: Option<FailureDetail<'_>>,
    ) -> AuditEvent {
        let (error_code, failure_stage, retryable) = if let Some(detail) = failure {
            (
                Some(detail.error_code.to_string()),
                Some(detail.failure_stage.to_string()),
                Some(detail.retryable),
            )
        } else {
            (None, None, None)
        };
        AuditEvent {
            event_type,
            trace_id: context.trace_id.to_string(),
            session_id: context.session_id.to_string(),
            tool_name: context.tool_name.to_string(),
            effective_workdir: context.effective_workdir.display().to_string(),
            permission_mode: context.permission_mode.as_str().to_string(),
            duration_ms: duration.as_millis(),
            message,
            error_code,
            failure_stage,
            retryable,
        }
    }

    /// Emit `tool_execution_started`.
    pub fn tool_execution_started(
        trace_id: &str,
        session_id: &str,
        tool_name: &str,
        effective_workdir: &std::path::Path,
        permission_mode: PermissionMode,
    ) {
        let event = Self::base_event(
            "tool_execution_started",
            AuditContext {
                trace_id,
                session_id,
                tool_name,
                effective_workdir,
                permission_mode,
            },
            Duration::from_millis(0),
            None,
            None,
        );
        tracing::info!(target: "if2ai.audit", event = ?event);
    }

    /// Emit `policy_decision_made`.
    pub fn policy_decision_made(
        trace_id: &str,
        session_id: &str,
        tool_name: &str,
        effective_workdir: &std::path::Path,
        permission_mode: PermissionMode,
        decision: &str,
    ) {
        let event = Self::base_event(
            "policy_decision_made",
            AuditContext {
                trace_id,
                session_id,
                tool_name,
                effective_workdir,
                permission_mode,
            },
            Duration::from_millis(0),
            Some(decision.to_string()),
            None,
        );
        tracing::info!(target: "if2ai.audit", event = ?event);
    }

    /// Emit `tool_execution_finished`.
    pub fn tool_execution_finished(
        trace_id: &str,
        session_id: &str,
        tool_name: &str,
        effective_workdir: &std::path::Path,
        permission_mode: PermissionMode,
        duration: Duration,
    ) {
        let event = Self::base_event(
            "tool_execution_finished",
            AuditContext {
                trace_id,
                session_id,
                tool_name,
                effective_workdir,
                permission_mode,
            },
            duration,
            None,
            None,
        );
        tracing::info!(target: "if2ai.audit", event = ?event);
    }

    /// Emit `tool_execution_failed`.
    pub fn tool_execution_failed(
        trace_id: &str,
        session_id: &str,
        tool_name: &str,
        effective_workdir: &std::path::Path,
        permission_mode: PermissionMode,
        duration: Duration,
        diagnostic: FailureDiagnostic<'_>,
    ) {
        let event = Self::base_event(
            "tool_execution_failed",
            AuditContext {
                trace_id,
                session_id,
                tool_name,
                effective_workdir,
                permission_mode,
            },
            duration,
            Some(diagnostic.reason.to_string()),
            Some(FailureDetail {
                error_code: diagnostic.error_code,
                failure_stage: diagnostic.failure_stage,
                retryable: diagnostic.retryable,
            }),
        );
        tracing::warn!(target: "if2ai.audit", event = ?event);
    }
}
