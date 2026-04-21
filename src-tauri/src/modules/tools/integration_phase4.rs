//! Phase 4 Integration Tests
//!
//! This module contains integration tests for all Phase 4 tool implementations.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use serde_json::json;
    use tokio::task::yield_now;
    use uuid::Uuid;

    use crate::modules::control_plane::{SessionExecutionContext, ToolExecutionBroker};
    use crate::modules::memory::default_memory_provider;
    use crate::modules::runtime::permissions::PermissionMode;
    use crate::modules::scheduler::default_scheduler;
    use crate::modules::tools::builtin::{
        bash_tool_entry, file_write_entry, grep_search_tool_entry,
    };
    use crate::modules::tools::context::{SharedToolContext, ToolContext};
    use crate::modules::tools::registry::ToolRegistry;

    /// Helper to create a test tool context
    fn test_context() -> SharedToolContext {
        Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            std::path::PathBuf::from("/tmp/test_phase4"),
        )))
    }

    /// Helper to extract tool names from definitions
    fn get_tool_names(defs: &[serde_json::Value]) -> Vec<String> {
        defs.iter()
            .filter_map(|v| {
                v.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(String::from)
            })
            .collect()
    }

    /// Test that all builtin tools are registered
    #[tokio::test]
    async fn test_registers_all_builtin_tools() {
        let context = test_context();
        let registry = ToolRegistry::new(context);

        // Register all tools
        let memory = default_memory_provider().await;
        let scheduler = default_scheduler();
        let test_browser = crate::modules::browser::BrowserRegistry::for_test(
            std::path::PathBuf::from("/tmp/browser-cold-state-phase4-test.json"),
        );
        crate::modules::tools::register_builtin_tools(
            &registry,
            memory,
            scheduler,
            test_browser,
            Arc::new(crate::modules::memory::NullPinnedStore::new()),
        );

        // Verify all expected tools exist
        let defs = registry.get_definitions(None);
        let tool_names = get_tool_names(&defs);

        // Core tools
        assert!(
            tool_names.contains(&"bash".to_string()),
            "bash tool should be registered"
        );
        assert!(
            tool_names.contains(&"read_file".to_string()),
            "read_file tool should be registered"
        );
        assert!(
            tool_names.contains(&"json_parse".to_string()),
            "json_parse tool should be registered"
        );

        // File tools
        assert!(
            tool_names.contains(&"file_write".to_string()),
            "file_write tool should be registered"
        );
        // file_edit is disabled (stub implementation)
        assert!(
            tool_names.contains(&"glob_search".to_string()),
            "glob_search tool should be registered"
        );
        assert!(
            tool_names.contains(&"content_search".to_string()),
            "content_search tool should be registered"
        );

        // Web tools
        assert!(
            tool_names.contains(&"web_fetch".to_string()),
            "web_fetch tool should be registered"
        );
        assert!(
            tool_names.contains(&"web_search".to_string()),
            "web_search tool should be registered"
        );
        assert!(
            tool_names.contains(&"http_request".to_string()),
            "http_request tool should be registered"
        );

        // Memory tools
        assert!(
            tool_names.contains(&"memory_store".to_string()),
            "memory_store tool should be registered"
        );
        assert!(
            tool_names.contains(&"memory_recall".to_string()),
            "memory_recall tool should be registered"
        );
        assert!(
            tool_names.contains(&"memory_forget".to_string()),
            "memory_forget tool should be registered"
        );
        assert!(
            tool_names.contains(&"memory_purge".to_string()),
            "memory_purge tool should be registered"
        );
        assert!(
            tool_names.contains(&"memory_export".to_string()),
            "memory_export tool should be registered"
        );

        // Cron tools
        assert!(
            tool_names.contains(&"cron_add".to_string()),
            "cron_add tool should be registered"
        );
        assert!(
            tool_names.contains(&"cron_list".to_string()),
            "cron_list tool should be registered"
        );
        assert!(
            tool_names.contains(&"cron_remove".to_string()),
            "cron_remove tool should be registered"
        );
        assert!(
            tool_names.contains(&"cron_run".to_string()),
            "cron_run tool should be registered"
        );
        assert!(
            tool_names.contains(&"cron_runs".to_string()),
            "cron_runs tool should be registered"
        );
    }

    /// Test memory_store and memory_recall integration
    #[tokio::test]
    async fn test_memory_store_and_recall() {
        let memory = default_memory_provider().await;

        // Store a memory
        memory
            .store(
                "test_key",
                "test content",
                crate::modules::memory::MemoryCategory::Conversation,
            )
            .await
            .unwrap();

        // Recall it
        let results = memory.recall("test", None, 10).await.unwrap();

        assert!(!results.is_empty(), "Should find at least one result");
        assert_eq!(results[0].key, "test_key");
        assert_eq!(results[0].content, "test content");

        // Clean up
        memory.delete("test_key").await.unwrap();
    }

    /// Test cron_add and cron_list integration
    #[tokio::test]
    async fn test_cron_add_and_list() {
        let scheduler = default_scheduler();

        // Add a cron job
        scheduler
            .add("test_job", "* * * * *", "echo hello", "Test job")
            .await
            .unwrap();

        // List jobs
        let jobs = scheduler.list().await.unwrap();

        assert!(!jobs.is_empty(), "Should have at least one job");
        assert_eq!(jobs[0].id, "test_job");
        assert_eq!(jobs[0].schedule, "* * * * *");

        // Clean up
        scheduler.remove("test_job").await.unwrap();
    }

    /// Test ToolContext workdir propagation
    #[tokio::test]
    async fn test_tool_context_workdir_propagation() {
        let workdir = std::path::PathBuf::from("/custom/workdir");
        let context = Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            workdir.clone(),
        )));

        let ctx = context.lock().unwrap();
        assert_eq!(ctx.workdir, workdir);
    }

    /// Test that tools use correct toolsets via ToolSetRegistry
    #[tokio::test]
    async fn test_tools_have_correct_toolsets() {
        let context = test_context();
        let registry = ToolRegistry::new(context);
        let memory = default_memory_provider().await;
        let scheduler = default_scheduler();
        let test_browser = crate::modules::browser::BrowserRegistry::for_test(
            std::path::PathBuf::from("/tmp/browser-cold-state-phase4-test2.json"),
        );
        crate::modules::tools::register_builtin_tools(
            &registry,
            memory,
            scheduler,
            test_browser,
            Arc::new(crate::modules::memory::NullPinnedStore::new()),
        );

        // Get registry via toolset registry
        let toolset_registry = crate::modules::tools::ToolSetRegistry::new();
        let all_tools = toolset_registry.tools_from_toolsets(&["files".to_string()]);

        // Should have file tools
        assert!(!all_tools.is_empty(), "Should have file tools");
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("if2ai-phase6a-{label}-{}", Uuid::new_v4()))
    }

    fn create_broker_with_file_tools() -> (Arc<ToolRegistry>, ToolExecutionBroker) {
        let context = Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            PathBuf::from("."),
        )));
        let registry = Arc::new(ToolRegistry::new(context));
        registry
            .register(file_write_entry())
            .expect("register file_write");
        registry
            .register(grep_search_tool_entry())
            .expect("register grep_search");
        registry.register(bash_tool_entry()).expect("register bash");
        let broker = ToolExecutionBroker::new(registry.clone());
        (registry, broker)
    }

    #[tokio::test]
    async fn test_concurrent_sessions_file_write_isolated_workdirs() {
        let root = unique_temp_dir("parallel-write");
        let workdir_a = root.join("project_a");
        let workdir_b = root.join("project_b");
        tokio::fs::create_dir_all(&workdir_a)
            .await
            .expect("create workdir a");
        tokio::fs::create_dir_all(&workdir_b)
            .await
            .expect("create workdir b");

        let (_registry, broker) = create_broker_with_file_tools();
        let context_a = SessionExecutionContext::new(
            "session-a".to_string(),
            "project-a".to_string(),
            workdir_a.clone(),
            PermissionMode::DangerFullAccess,
        );
        let context_b = SessionExecutionContext::new(
            "session-b".to_string(),
            "project-b".to_string(),
            workdir_b.clone(),
            PermissionMode::DangerFullAccess,
        );

        let write_a = broker.execute_with_trace(
            &context_a,
            "file_write",
            json!({"path":"result.txt","content":"from-session-a"}),
            "trace-session-a",
            None,
        );
        let write_b = broker.execute_with_trace(
            &context_b,
            "file_write",
            json!({"path":"result.txt","content":"from-session-b"}),
            "trace-session-b",
            None,
        );
        let (result_a, result_b) = tokio::join!(write_a, write_b);
        assert!(result_a.is_ok(), "session A write should succeed");
        assert!(result_b.is_ok(), "session B write should succeed");

        let text_a = tokio::fs::read_to_string(workdir_a.join("result.txt"))
            .await
            .expect("read project A file");
        let text_b = tokio::fs::read_to_string(workdir_b.join("result.txt"))
            .await
            .expect("read project B file");
        assert_eq!(text_a, "from-session-a");
        assert_eq!(text_b, "from-session-b");

        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn test_concurrent_grep_with_context_switch_has_no_cross_session_leak() {
        let root = unique_temp_dir("grep-switch");
        let workdir_a = root.join("project_a");
        let workdir_b = root.join("project_b");
        tokio::fs::create_dir_all(&workdir_a)
            .await
            .expect("create workdir a");
        tokio::fs::create_dir_all(&workdir_b)
            .await
            .expect("create workdir b");
        let mut content = String::new();
        for _ in 0..20_000 {
            content.push_str("filler_line\n");
        }
        content.push_str("needle_a\n");
        tokio::fs::write(workdir_a.join("a.txt"), content)
            .await
            .expect("seed project A file");
        tokio::fs::write(workdir_b.join("b.txt"), "needle_b\n")
            .await
            .expect("seed project B file");

        let (registry, broker) = create_broker_with_file_tools();
        let context_a = SessionExecutionContext::new(
            "session-a".to_string(),
            "project-a".to_string(),
            workdir_a.clone(),
            PermissionMode::DangerFullAccess,
        );

        let grep_task = broker.execute_with_trace(
            &context_a,
            "grep_search",
            json!({"pattern":"needle_a","path":"."}),
            "trace-grep-a",
            None,
        );
        let registry_for_switch = registry.clone();
        let switching = Arc::new(AtomicBool::new(true));
        let switching_for_task = switching.clone();
        let switch_task = tokio::spawn(async move {
            while switching_for_task.load(Ordering::Relaxed) {
                if let Ok(mut guard) = registry_for_switch.context().lock() {
                    guard.workdir = workdir_b.clone();
                }
                yield_now().await;
            }
        });

        let grep_result = grep_task.await;
        switching.store(false, Ordering::Relaxed);
        switch_task.await.expect("switch task should stop cleanly");
        let output = grep_result.expect("grep should succeed under explicit context");
        assert!(
            output.contains("a.txt"),
            "grep output should stay inside session A workdir"
        );
        assert!(
            !output.contains("b.txt"),
            "grep output must not leak session B workdir"
        );

        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn test_session_b_write_escape_into_session_a_is_blocked() {
        let root = unique_temp_dir("escape-check");
        let workdir_a = root.join("project_a");
        let workdir_b = root.join("project_b");
        tokio::fs::create_dir_all(&workdir_a)
            .await
            .expect("create workdir a");
        tokio::fs::create_dir_all(&workdir_b)
            .await
            .expect("create workdir b");
        tokio::fs::write(workdir_a.join("protected.txt"), "do-not-touch")
            .await
            .expect("seed protected file");

        let (_registry, broker) = create_broker_with_file_tools();
        let context_b = SessionExecutionContext::new(
            "session-b".to_string(),
            "project-b".to_string(),
            workdir_b.clone(),
            PermissionMode::WorkspaceWrite,
        );
        let result = broker
            .execute_with_trace(
                &context_b,
                "file_write",
                json!({"path":"../project_a/protected.txt","content":"intrusion"}),
                "trace-escape-b",
                None,
            )
            .await;

        assert!(result.is_err(), "path escape write should be denied");
        let protected = tokio::fs::read_to_string(workdir_a.join("protected.txt"))
            .await
            .expect("read protected file");
        assert_eq!(protected, "do-not-touch");

        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn test_concurrent_sessions_bash_executes_in_bound_workdirs() {
        let root = unique_temp_dir("parallel-bash");
        let workdir_a = root.join("project_a");
        let workdir_b = root.join("project_b");
        tokio::fs::create_dir_all(&workdir_a)
            .await
            .expect("create workdir a");
        tokio::fs::create_dir_all(&workdir_b)
            .await
            .expect("create workdir b");

        let (_registry, broker) = create_broker_with_file_tools();
        let context_a = SessionExecutionContext::new(
            "session-a".to_string(),
            "project-a".to_string(),
            workdir_a.clone(),
            PermissionMode::DangerFullAccess,
        );
        let context_b = SessionExecutionContext::new(
            "session-b".to_string(),
            "project-b".to_string(),
            workdir_b.clone(),
            PermissionMode::DangerFullAccess,
        );

        let run_a = broker.execute_with_trace(
            &context_a,
            "bash",
            json!({"command":"pwd","timeout":1000}),
            "trace-bash-a",
            None,
        );
        let run_b = broker.execute_with_trace(
            &context_b,
            "bash",
            json!({"command":"pwd","timeout":1000}),
            "trace-bash-b",
            None,
        );
        let (result_a, result_b) = tokio::join!(run_a, run_b);
        let output_a = result_a.expect("session A bash should succeed");
        let output_b = result_b.expect("session B bash should succeed");
        assert!(
            output_a.contains(workdir_a.to_string_lossy().as_ref()),
            "session A bash must execute inside session A workdir"
        );
        assert!(
            output_b.contains(workdir_b.to_string_lossy().as_ref()),
            "session B bash must execute inside session B workdir"
        );

        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn test_sandbox_strict_mode_blocks_bash_when_sandbox_disabled() {
        let root = unique_temp_dir("strict-sandbox-deny");
        let workdir = root.join("project");
        tokio::fs::create_dir_all(workdir.join(".claw"))
            .await
            .expect("create config dir");
        tokio::fs::write(
            workdir.join(".claw").join("settings.local.json"),
            r#"{"sandbox":{"enabled":false}}"#,
        )
        .await
        .expect("write sandbox config");

        let (_registry, broker) = create_broker_with_file_tools();
        let context = SessionExecutionContext::new(
            "session-strict".to_string(),
            "project-strict".to_string(),
            workdir.clone(),
            PermissionMode::DangerFullAccess,
        );
        let result = broker
            .execute_with_trace(
                &context,
                "bash",
                json!({"command":"pwd","timeout":1000}),
                "trace-strict-sandbox",
                None,
            )
            .await;

        assert!(
            result.is_err(),
            "strict mode should deny bash without sandbox"
        );
        let error_text = result.expect_err("bash should be denied").to_string();
        println!("Error text: {}", error_text);
        let error_lower = error_text.to_lowercase();
        assert!(
            error_lower.contains("sandboxstrictmode") || error_lower.contains("sandbox"),
            "denial reason should mention sandbox strict mode, got: {}",
            error_text
        );

        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
