//! Tools Module
//! Migrated from /rust/crates/tools
//! Provides tool system framework and execution

pub mod builtin;
pub mod context;
pub mod integration_phase4;
pub mod registry;
pub mod toolset;
#[allow(unused_imports)]
pub use context::{SharedToolContext, ToolContext};
#[allow(unused_imports)]
pub use registry::{ToolEntry, ToolError, ToolRegistry};
#[allow(unused_imports)]
pub use toolset::{ToolSet, ToolSetRegistry, TOOLSETS};

use crate::modules::memory::SharedMemoryProvider;
use crate::modules::scheduler::SharedScheduler;

/// Register all builtin tools to the given registry.
///
/// This function is called during application startup to register
/// the default builtin tools: bash, read_file, json_parse.
pub fn register_builtin_tools(
    registry: &ToolRegistry,
    memory: SharedMemoryProvider,
    scheduler: SharedScheduler,
) {
    if let Err(e) = registry.register(builtin::bash::bash_tool_entry()) {
        eprintln!("Failed to register bash tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::file_read::file_read_tool_entry()) {
        eprintln!("Failed to register read_file tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::json_parse::json_parse_tool_entry()) {
        eprintln!("Failed to register json_parse tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::file_write_entry()) {
        eprintln!("Failed to register file_write tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::glob_search_entry()) {
        eprintln!("Failed to register glob_search tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::file_edit_entry()) {
        eprintln!("Failed to register file_edit tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::content_search_entry()) {
        eprintln!("Failed to register content_search tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::web_fetch_entry()) {
        eprintln!("Failed to register web_fetch tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::web_search_entry()) {
        eprintln!("Failed to register web_search tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::http_request_entry()) {
        eprintln!("Failed to register http_request tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::memory_store_entry(memory.clone())) {
        eprintln!("Failed to register memory_store tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::memory_recall_entry(memory.clone())) {
        eprintln!("Failed to register memory_recall tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::memory_forget_entry(memory.clone())) {
        eprintln!("Failed to register memory_forget tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::memory_purge_entry(memory.clone())) {
        eprintln!("Failed to register memory_purge tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::memory_export_entry(memory.clone())) {
        eprintln!("Failed to register memory_export tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::cron_add_entry(scheduler.clone())) {
        eprintln!("Failed to register cron_add tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::cron_list_entry(scheduler.clone())) {
        eprintln!("Failed to register cron_list tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::cron_remove_entry(scheduler.clone())) {
        eprintln!("Failed to register cron_remove tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::cron_run_entry(scheduler.clone())) {
        eprintln!("Failed to register cron_run tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::cron_runs_entry(scheduler.clone())) {
        eprintln!("Failed to register cron_runs tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::todo_write_tool_entry()) {
        eprintln!("Failed to register TodoWrite tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::skill_tool_entry()) {
        eprintln!("Failed to register Skill tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::skill_search_tool_entry()) {
        eprintln!("Failed to register SkillSearch tool: {}", e);
    }
}
