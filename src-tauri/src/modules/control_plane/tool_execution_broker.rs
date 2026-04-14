//! Tool execution broker for control-plane dispatch.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::Value;

use crate::modules::control_plane::audit::{AuditEmitter, FailureDiagnostic};
use crate::modules::control_plane::session_context::SessionExecutionContext;
use crate::modules::tools::{ToolContext, ToolError, ToolRegistry};

/// Unified broker for session-aware tool execution.
#[derive(Clone)]
pub struct ToolExecutionBroker {
    tool_registry: Arc<ToolRegistry>,
}

impl ToolExecutionBroker {
    /// Create a new tool execution broker.
    #[must_use]
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }

    /// Build a shared tool context from a session execution context.
    #[must_use]
    pub fn to_tool_context(
        &self,
        context: &SessionExecutionContext,
    ) -> crate::modules::tools::SharedToolContext {
        let session_id = if context.session_id.is_empty() {
            None
        } else {
            Some(context.session_id.clone())
        };
        Arc::new(Mutex::new(ToolContext::new_with_session(
            session_id,
            context.workdir.clone(),
            context.permission_mode,
        )))
    }

    /// Execute with a pre-generated trace id.
    ///
    /// # Errors
    ///
    /// Returns `ToolError` when the underlying tool dispatch fails.
    pub async fn execute_with_trace(
        &self,
        context: &SessionExecutionContext,
        tool_name: &str,
        args: Value,
        trace_id: &str,
        request_id: Option<&str>,
    ) -> Result<String, ToolError> {
        let fingerprint = crate::modules::tools::context::context_fingerprint(
            &context.session_id,
            &context.workdir,
        );
        tracing::info!(
            "[tool_execution_broker] dispatch tool='{}', trace_id='{}', context_fingerprint='{}', session_id='{}', workdir='{}'",
            tool_name,
            trace_id,
            fingerprint,
            context.session_id,
            context.workdir.display()
        );
        AuditEmitter::tool_execution_started(
            trace_id,
            &context.session_id,
            tool_name,
            &context.workdir,
            context.permission_mode,
            request_id,
        );
        let started_at = Instant::now();
        let result = if let Some(reason) = strict_mode_denial_reason(&context.workdir, tool_name) {
            AuditEmitter::policy_decision_made(
                trace_id,
                &context.session_id,
                tool_name,
                &context.workdir,
                context.permission_mode,
                "deny:sandbox_strict_mode_requires_sandbox_enabled",
                request_id,
            );
            Err(ToolError::Handler(reason))
        } else {
            let execution_context = self.to_tool_context(context);
            self.tool_registry
                .dispatch_with_context(tool_name, args, execution_context)
                .await
        };
        if let Err(err) = &result {
            if resolve_boundary_enforce_mode(&context.workdir)
                == crate::modules::runtime::config::BoundaryEnforceMode::Shadow
                && is_boundary_error(err)
            {
                AuditEmitter::policy_decision_made(
                    trace_id,
                    &context.session_id,
                    tool_name,
                    &context.workdir,
                    context.permission_mode,
                    "shadow_allow_boundary_violation",
                    request_id,
                );
                tracing::warn!(
                    "[tool_execution_broker] boundary violation observed in shadow mode; preserving original error semantics"
                );
            }
        }
        match &result {
            Ok(_) => AuditEmitter::tool_execution_finished(
                trace_id,
                &context.session_id,
                tool_name,
                &context.workdir,
                context.permission_mode,
                started_at.elapsed(),
                request_id,
            ),
            Err(err) => {
                let (error_code, failure_stage, retryable, message) = summarize_tool_error(err);
                AuditEmitter::tool_execution_failed(
                    trace_id,
                    &context.session_id,
                    tool_name,
                    &context.workdir,
                    context.permission_mode,
                    started_at.elapsed(),
                    FailureDiagnostic {
                        reason: &message,
                        error_code,
                        failure_stage,
                        retryable,
                        request_id,
                    },
                )
            }
        }
        result
    }
}

fn summarize_tool_error(err: &ToolError) -> (&'static str, &'static str, bool, String) {
    match err {
        ToolError::NotFound(_) => (
            "tool_not_found",
            "pre_dispatch",
            false,
            "tool not found".to_string(),
        ),
        ToolError::Disabled(_) => (
            "tool_disabled",
            "pre_dispatch",
            false,
            "tool is disabled".to_string(),
        ),
        ToolError::Timeout(_) => (
            "tool_timeout",
            "execution",
            true,
            "tool execution timed out".to_string(),
        ),
        ToolError::OutputTooLarge { .. } => (
            "output_too_large",
            "post_execution",
            false,
            "tool output exceeded allowed size".to_string(),
        ),
        ToolError::Register(_) => (
            "tool_register_error",
            "pre_dispatch",
            false,
            "tool registration failure".to_string(),
        ),
        ToolError::Handler(_) => (
            "tool_handler_error",
            "execution",
            false,
            "tool handler returned an error".to_string(),
        ),
    }
}

fn resolve_boundary_enforce_mode(
    workdir: &std::path::Path,
) -> crate::modules::runtime::config::BoundaryEnforceMode {
    if let Ok(value) = std::env::var("IF2AI_BOUNDARY_ENFORCE_MODE") {
        return if value.eq_ignore_ascii_case("shadow") {
            crate::modules::runtime::config::BoundaryEnforceMode::Shadow
        } else {
            crate::modules::runtime::config::BoundaryEnforceMode::Enforce
        };
    }
    crate::modules::runtime::config::ConfigLoader::default_for(workdir)
        .load()
        .map(|loaded| loaded.control_plane().boundary_enforce_mode())
        .unwrap_or(crate::modules::runtime::config::BoundaryEnforceMode::Enforce)
}

fn is_boundary_error(err: &ToolError) -> bool {
    err.to_string().contains("outside allowed workdir")
        || err.to_string().contains("outside workdir boundary")
}

fn strict_mode_denial_reason(workdir: &std::path::Path, tool_name: &str) -> Option<String> {
    if !matches!(tool_name, "bash" | "powershell") {
        return None;
    }
    let strict_mode_enabled = std::env::var("IF2AI_SANDBOX_STRICT_MODE")
        .map(|value| value != "0")
        .unwrap_or_else(|_| {
            crate::modules::runtime::config::ConfigLoader::default_for(workdir)
                .load()
                .map(|loaded| loaded.control_plane().sandbox_strict_mode())
                .unwrap_or(true)
        });
    if !strict_mode_enabled {
        return None;
    }
    let sandbox_enabled = crate::modules::runtime::config::ConfigLoader::default_for(workdir)
        .load()
        .map(|loaded| loaded.sandbox().enabled.unwrap_or(true))
        .unwrap_or(true);
    if sandbox_enabled {
        None
    } else {
        Some(
            "sandboxStrictMode blocks shell tools when sandbox.enabled=false in runtime config"
                .to_string(),
        )
    }
}
