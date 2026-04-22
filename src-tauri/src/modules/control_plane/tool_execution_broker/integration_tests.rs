//! MIG-002-d integration tests: verify three-state behavior (Granted/RequiresApproval/Denied)
//! across command and runtime layers.

#[cfg(test)]
mod mig_002_d_integration_tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use serde_json::json;

    use crate::modules::control_plane::{SessionExecutionContext, ToolExecutionBroker};
    use crate::modules::runtime::permissions::PermissionMode;
    use crate::modules::tools::builtin::{bash_tool_entry, file_read_tool_entry, file_write_entry};
    use crate::modules::tools::context::{SharedToolContext, ToolContext};
    use crate::modules::tools::registry::ToolRegistry;

    fn create_test_registry() -> (Arc<ToolRegistry>, ToolExecutionBroker) {
        let context: SharedToolContext = Arc::new(std::sync::Mutex::new(
            ToolContext::default_for_workdir(PathBuf::from("/tmp/test")),
        ));
        let registry = Arc::new(ToolRegistry::new(context));
        registry
            .register(file_read_tool_entry())
            .expect("register file_read");
        registry
            .register(file_write_entry())
            .expect("register file_write");
        registry.register(bash_tool_entry()).expect("register bash");
        let broker = ToolExecutionBroker::new(registry.clone());
        (registry, broker)
    }

    /// MIG-002-d: Verify Granted outcome allows tool execution
    #[tokio::test]
    async fn granted_outcome_allows_execution() {
        let root = std::env::temp_dir().join("mig-002-d-granted");
        tokio::fs::create_dir_all(&root).await.ok();
        let test_file = root.join("test.txt");
        tokio::fs::write(&test_file, "test content").await.ok();

        let (_registry, broker) = create_test_registry();
        let context = SessionExecutionContext::new(
            "test-session".to_string(),
            "test-project".to_string(),
            root.clone(),
            PermissionMode::DangerFullAccess,
        );

        let result = broker
            .execute_with_trace(
                &context,
                "read_file",
                json!({"path": test_file.to_string_lossy()}),
                "trace-granted",
                None,
            )
            .await;

        if let Err(ref e) = result {
            println!("Error: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "Granted outcome should allow tool execution, got: {:?}",
            result.err()
        );

        tokio::fs::remove_dir_all(root).await.ok();
    }

    /// MIG-002-d: Verify Denied outcome blocks tool execution
    #[tokio::test]
    async fn denied_outcome_blocks_execution() {
        let root = std::env::temp_dir().join("mig-002-d-denied");
        tokio::fs::create_dir_all(&root).await.ok();
        let test_file = root.join("test.txt");
        tokio::fs::write(&test_file, "test content").await.ok();

        let (_registry, broker) = create_test_registry();
        let context = SessionExecutionContext::new(
            "test-session".to_string(),
            "test-project".to_string(),
            root.clone(),
            PermissionMode::ReadOnly,
        );

        let result = broker
            .execute_with_trace(
                &context,
                "file_write",
                json!({"path": test_file.to_string_lossy(), "content": "new content"}),
                "trace-denied",
                None,
            )
            .await;

        assert!(
            result.is_err(),
            "Denied outcome should block tool execution"
        );
        let error_text = result.expect_err("should be denied").to_string();
        assert!(
            error_text.contains("below required") || error_text.contains("denied"),
            "Error should mention permission denial"
        );

        tokio::fs::remove_dir_all(root).await.ok();
    }

    /// MIG-002-d: Verify boundary denial blocks execution
    #[tokio::test]
    async fn boundary_denial_blocks_execution() {
        let root = std::env::temp_dir().join("mig-002-d-boundary");
        tokio::fs::create_dir_all(&root).await.ok();

        let (_registry, broker) = create_test_registry();
        let context = SessionExecutionContext::new(
            "test-session".to_string(),
            "test-project".to_string(),
            root.clone(),
            PermissionMode::WorkspaceWrite,
        );

        let result = broker
            .execute_with_trace(
                &context,
                "read_file",
                json!({"path": "/etc/passwd"}),
                "trace-boundary-denied",
                None,
            )
            .await;

        assert!(
            result.is_err(),
            "Boundary violation should block tool execution"
        );

        tokio::fs::remove_dir_all(root).await.ok();
    }

    /// MIG-002-d: Verify sandbox policy is set correctly for dangerous tools
    #[tokio::test]
    async fn sandbox_policy_set_for_dangerous_tools() {
        use crate::modules::control_plane::{
            prepare_step_execution, PrepareStepExecutionInput, SandboxPolicy,
        };
        use crate::modules::runtime::permissions::PermissionPolicy;

        let root = std::env::temp_dir().join("mig-002-d-sandbox");
        tokio::fs::create_dir_all(&root).await.ok();

        let context = SessionExecutionContext::new(
            "test-session".to_string(),
            "test-project".to_string(),
            root.clone(),
            PermissionMode::ReadOnly,
        );

        let policy = Arc::new(
            PermissionPolicy::new(PermissionMode::ReadOnly)
                .with_tool_requirement("bash", PermissionMode::DangerFullAccess),
        );

        let output = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "bash",
            session_context: &context,
            args: &json!({}),
            permission_policy: policy,
        });

        assert_eq!(
            output.sandbox_policy,
            SandboxPolicy::WorkdirNoNetwork,
            "Dangerous tool with insufficient permission should get WorkdirNoNetwork sandbox"
        );

        tokio::fs::remove_dir_all(root).await.ok();
    }

    /// MIG-002-d: Verify all three decisions are consumed by tool_executor
    #[tokio::test]
    async fn all_three_decisions_consumed() {
        use crate::modules::control_plane::{prepare_step_execution, PrepareStepExecutionInput};
        use crate::modules::runtime::permissions::PermissionPolicy;

        let root = std::env::temp_dir().join("mig-002-d-decisions");
        tokio::fs::create_dir_all(&root).await.ok();

        let context = SessionExecutionContext::new(
            "test-session".to_string(),
            "test-project".to_string(),
            root.clone(),
            PermissionMode::DangerFullAccess,
        );

        let policy = Arc::new(PermissionPolicy::new(PermissionMode::DangerFullAccess));

        let output = prepare_step_execution(PrepareStepExecutionInput {
            tool_name: "read_file",
            session_context: &context,
            args: &json!({"path": root.join("file.txt").to_string_lossy()}),
            permission_policy: policy,
        });

        // Verify all three sub-decisions are present
        assert!(
            matches!(
                output.boundary_decision,
                crate::modules::control_plane::BoundaryDecision::WithinWorkdir
            ),
            "Boundary decision should be evaluated"
        );
        assert!(
            matches!(
                output.permission_decision,
                crate::modules::control_plane::PermissionDecision::Allow
            ),
            "Permission decision should be evaluated"
        );
        assert!(
            matches!(
                output.sandbox_policy,
                crate::modules::control_plane::SandboxPolicy::None
            ),
            "Sandbox policy should be evaluated"
        );

        tokio::fs::remove_dir_all(root).await.ok();
    }
}
