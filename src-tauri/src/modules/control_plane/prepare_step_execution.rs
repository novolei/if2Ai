//! `prepare_step_execution` control-plane seam (Phase M1.8).
//!
//! Single typed entry point that wraps the three pre-execution
//! decisions a tool call must clear before it runs:
//!
//! `boundary -> permission -> sandbox`
//!
//! M1.8 explicitly does NOT enforce the seam by replacing the
//! existing per-tool logic in
//! [`crate::commands::agent::ToolRegistryExecutor`] /
//! [`super::tool_execution_broker::ToolExecutionBroker`] /
//! [`crate::modules::runtime::permissions::PermissionPolicy`].
//! Doing so in a single slice would break every tool path
//! simultaneously.  Instead this slice:
//!
//! 1. Defines a typed [`PrepareStepExecutionOutput`] surface that
//!    M4 governance + harness compare can plug into.
//! 2. Provides a default implementation that wraps the existing
//!    `PermissionPolicy::authorize` decision so the seam is
//!    immediately usable in **shadow** mode (`PreflightMode::Shadow`).
//! 3. Returns one of three canonical outcomes
//!    (`Granted` / `RequiresApproval` / `Denied`) so future M4 gate
//!    rules can short-circuit.
//!
//! Hard rules:
//! 1. The output discriminator is closed in v1; adding a variant
//!    requires a contract bump.
//! 2. The seam MUST NOT take a `tauri::Window` / `AppHandle` —
//!    those are IPC-side concerns. Permission prompting belongs to
//!    the existing prompt path until M1.7+ migrates it.
//! 3. The skeleton holds zero state beyond what's passed in; M4
//!    rule plug-ins will live behind a trait inside this module.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use crate::modules::runtime::permissions::{PermissionMode, PermissionPolicy};

use super::session_context::SessionExecutionContext;

/// Inputs needed to make one preflight decision.
pub struct PrepareStepExecutionInput<'a> {
    /// Canonical tool name (`"bash"`, `"file_edit"`, ...).
    pub tool_name: &'a str,
    /// Resolved session context (workdir, session id, project id).
    pub session_context: &'a SessionExecutionContext,
    /// Raw tool arguments, post-parse.
    pub args: &'a serde_json::Value,
    /// Active permission policy (already constructed by the IPC
    /// adapter from the user's selected `PermissionMode`).
    pub permission_policy: Arc<PermissionPolicy>,
}

/// Coarse boundary decision. `WithinWorkdir` is the only canonical
/// "safe" answer in M1.8; future slices may add finer-grained
/// `WithinSandbox` / `OutsideButAllowed` variants.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryDecision {
    /// Tool args resolve inside the session workdir / project sandbox.
    WithinWorkdir,
    /// Tool args resolve outside the workdir but the active mode
    /// allows escape (`DangerFullAccess`).
    OutsideButAllowed,
    /// Tool args resolve outside the workdir and escape is denied.
    OutsideAndDenied,
    /// Boundary cannot be evaluated from args alone (tool does not
    /// declare a path argument). Treated as `WithinWorkdir` for
    /// now.
    NotApplicable,
}

/// Permission outcome family. Mirrors
/// [`crate::modules::runtime::permissions::PermissionOutcome`] but
/// owned by the control-plane seam so the seam's contract stays
/// independent of how the inner permission system evolves.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    /// Active mode covers required mode; tool may proceed.
    Allow,
    /// Active mode is below required mode but the user still has a
    /// chance to approve via the prompt path. Until the prompt
    /// path migrates here (M1.7+), the IPC adapter handles the
    /// actual prompt; this variant just signals that approval is
    /// pending.
    RequiresApproval { required_mode: PermissionMode },
    /// Active mode is below required and prompting is not allowed
    /// for this tool / session.
    Deny { reason: String },
}

/// Sandbox policy hint. Currently a placeholder — the real
/// sandbox lands with the `worker` adoption design (M1+ later
/// slice). The seam carries the field so M4 governance can record
/// "no sandbox enforcement yet" instead of guessing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPolicy {
    /// No sandbox; tool runs in-process. Default in M1.
    None,
    /// Run inside the project workdir, no network. Reserved.
    WorkdirNoNetwork,
    /// Full sandbox. Reserved.
    Strict,
}

/// Top-level outcome of one preflight call.  Closed in v1.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareStepOutcome {
    /// All three decisions cleared; caller may dispatch.
    Granted,
    /// Permission requires user approval. Caller must invoke the
    /// existing prompt path before dispatching.
    RequiresApproval,
    /// At least one decision rejected the call.
    Denied,
}

/// Composite preflight decision.  Carries every typed sub-decision
/// so M4 harness traces can record exactly what the seam saw.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PrepareStepExecutionOutput {
    pub outcome: PrepareStepOutcome,
    pub boundary_decision: BoundaryDecision,
    pub permission_decision: PermissionDecision,
    pub sandbox_policy: SandboxPolicy,
    /// Stable preflight policy version. Pinned here so the harness
    /// gate (M4) can compare two runs.
    pub policy_version: String,
}

/// Stable policy version emitted by the M1.8 seam.
pub const PREPARE_STEP_POLICY_VERSION: &str = "prepare-step@m1.8-skeleton";

/// Run boundary → permission → sandbox in order.  Returns the
/// composite typed decision.
///
/// **Important**: the M1.8 default implementation runs in *shadow*
/// mode — it computes the typed decision but does not block the
/// caller. The IPC adapter is free to log the decision and proceed
/// with its existing per-tool path while M4 gate rules ramp up.
#[must_use]
pub fn prepare_step_execution(input: PrepareStepExecutionInput<'_>) -> PrepareStepExecutionOutput {
    let boundary_decision = evaluate_boundary(input.session_context, input.args);

    let permission_decision =
        evaluate_permission(&input.permission_policy, input.tool_name, input.args);

    let sandbox_policy = SandboxPolicy::None;

    let outcome = compose_outcome(&boundary_decision, &permission_decision);

    PrepareStepExecutionOutput {
        outcome,
        boundary_decision,
        permission_decision,
        sandbox_policy,
        policy_version: PREPARE_STEP_POLICY_VERSION.to_string(),
    }
}

fn evaluate_boundary(ctx: &SessionExecutionContext, args: &serde_json::Value) -> BoundaryDecision {
    // Best-effort: probe the canonical "path" / "file_path" / "cwd"
    // fields. Tools that do not carry an obvious path key are
    // treated as NotApplicable. Real per-tool boundary checks live
    // in the existing tool handlers; M1.8 only computes a hint.
    let candidate = path_arg(args);
    let Some(candidate) = candidate else {
        return BoundaryDecision::NotApplicable;
    };

    let candidate_path = PathBuf::from(candidate);
    if !candidate_path.is_absolute() {
        return BoundaryDecision::WithinWorkdir;
    }

    if candidate_path.starts_with(&ctx.workdir) {
        BoundaryDecision::WithinWorkdir
    } else if matches!(ctx.permission_mode, PermissionMode::DangerFullAccess) {
        BoundaryDecision::OutsideButAllowed
    } else {
        BoundaryDecision::OutsideAndDenied
    }
}

fn path_arg(args: &serde_json::Value) -> Option<&str> {
    let obj = args.as_object()?;
    for key in ["path", "file_path", "filepath", "cwd", "workdir"] {
        if let Some(v) = obj.get(key).and_then(|v| v.as_str()) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn evaluate_permission(
    policy: &PermissionPolicy,
    tool_name: &str,
    args: &serde_json::Value,
) -> PermissionDecision {
    let active = policy.active_mode();
    let required = policy.required_mode_for(tool_name);
    if active == PermissionMode::Allow || active >= required {
        return PermissionDecision::Allow;
    }
    if matches!(active, PermissionMode::Prompt) {
        return PermissionDecision::RequiresApproval {
            required_mode: required,
        };
    }
    let _ = args; // M1.8 doesn't peek into args for permission; reserved for M4.
    PermissionDecision::Deny {
        reason: format!(
            "active mode '{}' below required '{}' for tool '{}'",
            active.as_str(),
            required.as_str(),
            tool_name
        ),
    }
}

fn compose_outcome(
    boundary: &BoundaryDecision,
    permission: &PermissionDecision,
) -> PrepareStepOutcome {
    if matches!(boundary, BoundaryDecision::OutsideAndDenied) {
        return PrepareStepOutcome::Denied;
    }
    match permission {
        PermissionDecision::Allow => PrepareStepOutcome::Granted,
        PermissionDecision::RequiresApproval { .. } => PrepareStepOutcome::RequiresApproval,
        PermissionDecision::Deny { .. } => PrepareStepOutcome::Denied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx(workdir: &str, mode: PermissionMode) -> SessionExecutionContext {
        SessionExecutionContext {
            session_id: "s".into(),
            project_id: String::new(),
            workdir: PathBuf::from(workdir),
            permission_mode: mode,
        }
    }

    #[test]
    fn allow_mode_grants_immediately() {
        let policy = Arc::new(PermissionPolicy::new(PermissionMode::Allow));
        let out = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "bash",
            session_context: &ctx("/tmp", PermissionMode::Allow),
            args: &serde_json::json!({}),
            permission_policy: policy,
        });
        assert_eq!(out.outcome, PrepareStepOutcome::Granted);
        assert_eq!(out.permission_decision, PermissionDecision::Allow);
        assert_eq!(out.sandbox_policy, SandboxPolicy::None);
    }

    #[test]
    fn read_only_below_full_access_denies() {
        let policy = Arc::new(
            PermissionPolicy::new(PermissionMode::ReadOnly)
                .with_tool_requirement("bash", PermissionMode::DangerFullAccess),
        );
        let out = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "bash",
            session_context: &ctx("/tmp", PermissionMode::ReadOnly),
            args: &serde_json::json!({}),
            permission_policy: policy,
        });
        assert_eq!(out.outcome, PrepareStepOutcome::Denied);
        assert!(matches!(
            out.permission_decision,
            PermissionDecision::Deny { .. }
        ));
    }

    #[test]
    fn prompt_mode_requires_approval() {
        let policy = Arc::new(
            PermissionPolicy::new(PermissionMode::Prompt)
                .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite),
        );
        let out = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "write_file",
            session_context: &ctx("/tmp", PermissionMode::Prompt),
            args: &serde_json::json!({}),
            permission_policy: policy,
        });
        assert_eq!(out.outcome, PrepareStepOutcome::RequiresApproval);
    }

    #[test]
    fn outside_workdir_with_read_only_denies_boundary() {
        let policy = Arc::new(PermissionPolicy::new(PermissionMode::ReadOnly));
        let out = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "read_file",
            session_context: &ctx("/tmp/work", PermissionMode::ReadOnly),
            args: &serde_json::json!({ "path": "/etc/passwd" }),
            permission_policy: policy,
        });
        // Permission Allow (read_file requires DangerFullAccess by
        // default per `PermissionPolicy::required_mode_for`, but
        // ReadOnly < DangerFullAccess → Deny). The boundary check
        // also fires because the path leaves the workdir under a
        // non-DangerFullAccess mode.
        assert_eq!(out.boundary_decision, BoundaryDecision::OutsideAndDenied);
        assert_eq!(out.outcome, PrepareStepOutcome::Denied);
    }

    #[test]
    fn within_workdir_with_full_access_allows() {
        let policy = Arc::new(PermissionPolicy::new(PermissionMode::DangerFullAccess));
        let out = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "read_file",
            session_context: &ctx("/tmp/work", PermissionMode::DangerFullAccess),
            args: &serde_json::json!({ "path": "/tmp/work/x.md" }),
            permission_policy: policy,
        });
        assert_eq!(out.boundary_decision, BoundaryDecision::WithinWorkdir);
        assert_eq!(out.outcome, PrepareStepOutcome::Granted);
        assert_eq!(out.policy_version, PREPARE_STEP_POLICY_VERSION);
    }
}
