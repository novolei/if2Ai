//! Phase 4 Integration Tests
//!
//! This module contains integration tests for all Phase 4 tool implementations.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::modules::memory::default_memory_provider;
    use crate::modules::scheduler::default_scheduler;
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
        let memory = default_memory_provider();
        let scheduler = default_scheduler();
        crate::modules::tools::register_builtin_tools(&registry, memory, scheduler);

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
        let memory = default_memory_provider();

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
        let memory = default_memory_provider();
        let scheduler = default_scheduler();
        crate::modules::tools::register_builtin_tools(&registry, memory, scheduler);

        // Get registry via toolset registry
        let toolset_registry = crate::modules::tools::ToolSetRegistry::new();
        let all_tools = toolset_registry.tools_from_toolsets(&["files".to_string()]);

        // Should have file tools
        assert!(!all_tools.is_empty(), "Should have file tools");
    }
}
