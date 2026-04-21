//! MIG-002-b test: verify tool_execution_broker enforces prepare_step_execution.

#[cfg(test)]
mod mig_002_b_tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::modules::control_plane::{
        prepare_step_execution, PrepareStepExecutionInput, PrepareStepOutcome,
        SessionExecutionContext,
    };
    use crate::modules::runtime::permissions::{PermissionMode, PermissionPolicy};

    /// MIG-002-b: verify that prepare_step_execution returns Denied
    /// when permission is insufficient.
    #[test]
    fn prepare_step_denies_insufficient_permission() {
        let ctx = SessionExecutionContext {
            session_id: "test".to_string(),
            project_id: String::new(),
            workdir: PathBuf::from("/tmp/test"),
            permission_mode: PermissionMode::ReadOnly,
        };
        let policy = Arc::new(
            PermissionPolicy::new(PermissionMode::ReadOnly)
                .with_tool_requirement("bash", PermissionMode::DangerFullAccess),
        );
        let output = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "bash",
            session_context: &ctx,
            args: &serde_json::json!({}),
            permission_policy: policy,
        });

        assert_eq!(output.outcome, PrepareStepOutcome::Denied);
    }

    /// MIG-002-b: verify that prepare_step_execution returns Granted
    /// when permission is sufficient.
    #[test]
    fn prepare_step_grants_sufficient_permission() {
        let ctx = SessionExecutionContext {
            session_id: "test".to_string(),
            project_id: String::new(),
            workdir: PathBuf::from("/tmp/test"),
            permission_mode: PermissionMode::DangerFullAccess,
        };
        let policy = Arc::new(PermissionPolicy::new(PermissionMode::DangerFullAccess));
        let output = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "read_file",
            session_context: &ctx,
            args: &serde_json::json!({ "path": "/tmp/test/file.txt" }),
            permission_policy: policy,
        });

        assert_eq!(output.outcome, PrepareStepOutcome::Granted);
    }
}
