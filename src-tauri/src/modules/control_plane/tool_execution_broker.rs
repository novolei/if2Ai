//! Tool execution broker for control-plane dispatch.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use crate::modules::control_plane::audit::{AuditEmitter, FailureDiagnostic};
use crate::modules::control_plane::session_context::SessionExecutionContext;
use crate::modules::tools::builtin::atomic_consolidation::resolve_alias as resolve_atomic_alias;
use crate::modules::tools::{ToolContext, ToolError, ToolRegistry};

/// WU-007 — env var that disables BR-003 alias rewriting.
pub const DISABLE_TOOL_ALIAS_ENV: &str = "IF2AI_DISABLE_TOOL_ALIAS";

fn alias_disabled() -> bool {
    std::env::var(DISABLE_TOOL_ALIAS_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Resolve a (possibly legacy) tool name to its atomic equivalent.
///
/// - When the kill-switch is set, returns the input unchanged.
/// - Catches panics in the alias table look-up so a corrupt table
///   never breaks dispatch — the broker logs a warning and the
///   original name is used.
#[must_use]
pub fn resolve_tool_name(tool_name: &str) -> String {
    if alias_disabled() {
        return tool_name.to_string();
    }
    let raw = tool_name.to_string();
    let lookup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        resolve_atomic_alias(tool_name).map(|s| s.to_string())
    }));
    match lookup {
        Ok(Some(atomic)) => atomic,
        Ok(None) => raw,
        Err(_) => {
            tracing::warn!(
                tool = %raw,
                "[tool_execution_broker] resolve_alias panicked; passing original tool name"
            );
            raw
        }
    }
}

/// Result of a tool execution, including retry metadata.
#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    /// The tool output string.
    pub output: String,
    /// Number of attempts made (1 = first attempt succeeded, >1 = retried).
    pub attempts: u32,
}

impl ToolExecutionResult {
    /// Create a result from a single successful attempt.
    #[must_use]
    pub fn first_attempt(output: String) -> Self {
        Self {
            output,
            attempts: 1,
        }
    }
}

/// Default maximum retry attempts for tool execution.
const DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 2;

/// Fixed retry interval in seconds.
const RETRY_INTERVAL_SECS: u64 = 1;

/// Network-oriented tools that get an extra retry attempt.
const NETWORK_TOOLS: &[&str] = &["web_fetch", "web_search", "web_research", "http_request"];

/// Determine the max retry attempts for a given tool.
fn max_attempts_for_tool(tool_name: &str) -> u32 {
    if NETWORK_TOOLS.contains(&tool_name) {
        3
    } else {
        DEFAULT_MAX_RETRY_ATTEMPTS
    }
}

/// Unified broker for session-aware tool execution.
#[derive(Clone)]
pub struct ToolExecutionBroker {
    tool_registry: Arc<ToolRegistry>,
}

#[derive(Debug, Clone, Copy)]
struct SkillTurnGuardState {
    skill_loaded_successfully: bool,
    updated_at: Instant,
}

static SKILL_TURN_GUARD: OnceLock<Mutex<HashMap<String, SkillTurnGuardState>>> = OnceLock::new();
const SKILL_TURN_GUARD_TTL_SECS: u64 = 30 * 60; // 30 minutes

fn skill_turn_guard() -> &'static Mutex<HashMap<String, SkillTurnGuardState>> {
    SKILL_TURN_GUARD.get_or_init(|| Mutex::new(HashMap::new()))
}

fn guard_request_key(request_id: Option<&str>) -> Option<String> {
    request_id.map(ToString::to_string)
}

fn cleanup_stale_guard_entries(entries: &mut HashMap<String, SkillTurnGuardState>) {
    let ttl = Duration::from_secs(SKILL_TURN_GUARD_TTL_SECS);
    entries.retain(|_, state| state.updated_at.elapsed() <= ttl);
}

fn mark_skill_loaded_for_request(request_id: Option<&str>) {
    let Some(key) = guard_request_key(request_id) else {
        return;
    };
    if let Ok(mut entries) = skill_turn_guard().lock() {
        cleanup_stale_guard_entries(&mut entries);
        entries.insert(
            key,
            SkillTurnGuardState {
                skill_loaded_successfully: true,
                updated_at: Instant::now(),
            },
        );
    }
}

fn request_already_loaded_skill(request_id: Option<&str>) -> bool {
    let Some(key) = guard_request_key(request_id) else {
        return false;
    };
    if let Ok(mut entries) = skill_turn_guard().lock() {
        cleanup_stale_guard_entries(&mut entries);
        if let Some(state) = entries.get_mut(&key) {
            state.updated_at = Instant::now();
            return state.skill_loaded_successfully;
        }
    }
    false
}

fn is_skill_file_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.contains("/.if2ai/skills/")
        || normalized.contains("/.claude/skills/")
        || normalized.starts_with(".if2ai/skills/")
        || normalized.starts_with(".claude/skills/")
}

fn post_skill_reload_denial_reason(
    request_id: Option<&str>,
    tool_name: &str,
    args: &Value,
) -> Option<String> {
    if !request_already_loaded_skill(request_id) {
        return None;
    }
    if tool_name == "skill_view" {
        return Some(
            "skill already loaded in this turn; do not call skill_view again. \
Use the existing skill tool result and continue the task."
                .to_string(),
        );
    }
    if tool_name == "read_file" {
        let target_path = args.get("path").and_then(|value| value.as_str())?;
        if is_skill_file_path(target_path) {
            return Some(
                "skill already loaded in this turn; do not read skill files via read_file. \
Use the existing skill tool result and continue the task."
                    .to_string(),
            );
        }
    }
    None
}

impl ToolExecutionBroker {
    /// Create a new tool execution broker.
    #[must_use]
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }

    /// Build a shared tool context from a session execution context.
    ///
    /// Both `session_id` and `project_id` are forwarded so that downstream
    /// scope-aware tooling (memory, future project-aware tools) can resolve a
    /// complete three-tier scope. Empty strings on the
    /// `SessionExecutionContext` are normalised to `None` to avoid spurious
    /// "empty project" matches in scoped queries.
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
        let project_id = if context.project_id.is_empty() {
            None
        } else {
            Some(context.project_id.clone())
        };
        Arc::new(Mutex::new(ToolContext::new_with_scope_evidence(
            session_id,
            project_id,
            context.workdir.clone(),
            context.permission_mode,
            Some(context.tool_success_evidence.clone()),
        )))
    }

    /// Execute with a pre-generated trace id.
    ///
    /// Wraps the underlying dispatch with automatic retry logic for
    /// transient (retryable) errors. Non-retryable errors are returned
    /// immediately. Each attempt is logged and tracked in the returned
    /// [`ToolExecutionResult::attempts`] field.
    ///
    /// # Errors
    ///
    /// Returns `ToolError` when all retry attempts are exhausted or
    /// a non-retryable error is encountered.
    pub async fn execute_with_trace(
        &self,
        context: &SessionExecutionContext,
        tool_name: &str,
        args: Value,
        trace_id: &str,
        request_id: Option<&str>,
    ) -> Result<ToolExecutionResult, ToolError> {
        let resolved_name = resolve_tool_name(tool_name);
        let tool_name: &str = &resolved_name;
        let max_attempts = max_attempts_for_tool(tool_name);
        let mut last_error: Option<ToolError> = None;

        for attempt in 1..=max_attempts {
            // Clone args for each attempt (the first iteration consumes
            // nothing extra when max_attempts == 1).
            let attempt_args = args.clone();

            match self
                .execute_single_attempt(
                    context,
                    tool_name,
                    attempt_args,
                    trace_id,
                    request_id,
                    attempt,
                )
                .await
            {
                Ok(output) => {
                    if attempt > 1 {
                        tracing::info!(
                            tool = %tool_name,
                            attempt = attempt,
                            "[tool_execution_broker] retry succeeded"
                        );
                    }
                    return Ok(ToolExecutionResult {
                        output,
                        attempts: attempt,
                    });
                }
                Err(err) => {
                    if !err.is_retryable() || attempt == max_attempts {
                        // Non-retryable or final attempt — return error.
                        if attempt > 1 {
                            tracing::warn!(
                                tool = %tool_name,
                                attempt = attempt,
                                error = %err,
                                "[tool_execution_broker] all retry attempts exhausted"
                            );
                        }
                        return Err(err);
                    }
                    tracing::warn!(
                        tool = %tool_name,
                        attempt = attempt,
                        max_attempts = max_attempts,
                        error = %err,
                        "[tool_execution_broker] retryable error, will retry after {}s",
                        RETRY_INTERVAL_SECS
                    );
                    last_error = Some(err);
                    tokio::time::sleep(Duration::from_secs(RETRY_INTERVAL_SECS)).await;
                }
            }
        }

        // Should not reach here, but handle defensively.
        Err(last_error
            .unwrap_or_else(|| ToolError::Handler("retry loop ended unexpectedly".to_string())))
    }

    /// Execute a single attempt of tool dispatch (extracted from the
    /// original `execute_with_trace` body).
    async fn execute_single_attempt(
        &self,
        context: &SessionExecutionContext,
        tool_name: &str,
        args: Value,
        trace_id: &str,
        request_id: Option<&str>,
        attempt: u32,
    ) -> Result<String, ToolError> {
        let fingerprint = crate::modules::tools::context::context_fingerprint(
            &context.session_id,
            &context.workdir,
        );
        tracing::info!(
            "[tool_execution_broker] dispatch tool='{}', trace_id='{}', attempt={}, context_fingerprint='{}', session_id='{}', workdir='{}'",
            tool_name,
            trace_id,
            attempt,
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

        // MIG-002-b — prepare_step_execution now acts as enforced preflight gate.
        let permission_policy = Arc::new(
            crate::modules::runtime::permissions::PermissionPolicy::new(context.permission_mode)
                .with_default_agent_tool_requirements(),
        );
        let preflight = crate::modules::control_plane::prepare_step_execution(
            crate::modules::control_plane::PrepareStepExecutionInput {
                tool_name,
                session_context: context,
                args: &args,
                permission_policy: permission_policy.clone(),
            },
        );

        let result = match preflight.outcome {
            crate::modules::control_plane::PrepareStepOutcome::Denied => {
                let reason = match &preflight.permission_decision {
                    crate::modules::control_plane::PermissionDecision::Deny { reason } => {
                        reason.clone()
                    }
                    _ => {
                        if matches!(
                            preflight.boundary_decision,
                            crate::modules::control_plane::BoundaryDecision::OutsideAndDenied
                        ) {
                            "Tool execution denied: path outside workdir boundary".to_string()
                        } else {
                            "Tool execution denied by preflight policy".to_string()
                        }
                    }
                };
                AuditEmitter::policy_decision_made(
                    trace_id,
                    &context.session_id,
                    tool_name,
                    &context.workdir,
                    context.permission_mode,
                    "deny:prepare_step_execution",
                    request_id,
                );
                tracing::info!(
                    "[tool_execution_broker] prepare_step_execution denied tool='{}', reason='{}'",
                    tool_name,
                    reason
                );
                Err(ToolError::Handler(reason))
            }
            crate::modules::control_plane::PrepareStepOutcome::RequiresApproval => {
                tracing::info!(
                    "[tool_execution_broker] prepare_step_execution requires approval for tool='{}'",
                    tool_name
                );
                if let Some(reason) = post_skill_reload_denial_reason(request_id, tool_name, &args)
                {
                    AuditEmitter::policy_decision_made(
                        trace_id,
                        &context.session_id,
                        tool_name,
                        &context.workdir,
                        context.permission_mode,
                        "deny:skill_reload_guard",
                        request_id,
                    );
                    Err(ToolError::Handler(reason))
                } else if let Some(reason) = strict_mode_denial_reason(&context.workdir, tool_name)
                {
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
                        .dispatch_with_context_legacy(tool_name, args, execution_context)
                        .await
                }
            }
            crate::modules::control_plane::PrepareStepOutcome::Granted => {
                tracing::info!(
                    "[tool_execution_broker] prepare_step_execution granted tool='{}'",
                    tool_name
                );
                if let Some(reason) = post_skill_reload_denial_reason(request_id, tool_name, &args)
                {
                    AuditEmitter::policy_decision_made(
                        trace_id,
                        &context.session_id,
                        tool_name,
                        &context.workdir,
                        context.permission_mode,
                        "deny:skill_reload_guard",
                        request_id,
                    );
                    Err(ToolError::Handler(reason))
                } else if let Some(reason) = strict_mode_denial_reason(&context.workdir, tool_name)
                {
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
                        .dispatch_with_context_legacy(tool_name, args, execution_context)
                        .await
                }
            }
        };
        if result.is_ok() {
            context.record_successful_tool_for_memory_gate(tool_name);
        }
        if result.is_ok() && tool_name == "skill" {
            mark_skill_loaded_for_request(request_id);
        }
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
    let retryable = err.is_retryable();
    match err {
        ToolError::NotFound(_) => (
            "tool_not_found",
            "pre_dispatch",
            retryable,
            "tool not found".to_string(),
        ),
        ToolError::Disabled(_) => (
            "tool_disabled",
            "pre_dispatch",
            retryable,
            "tool is disabled".to_string(),
        ),
        ToolError::Timeout(_) => (
            "tool_timeout",
            "execution",
            retryable,
            "tool execution timed out".to_string(),
        ),
        ToolError::OutputTooLarge { .. } => (
            "output_too_large",
            "post_execution",
            retryable,
            "tool output exceeded allowed size".to_string(),
        ),
        ToolError::Register(_) => (
            "tool_register_error",
            "pre_dispatch",
            retryable,
            "tool registration failure".to_string(),
        ),
        ToolError::Handler(_) => (
            "tool_handler_error",
            "execution",
            retryable,
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

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;
