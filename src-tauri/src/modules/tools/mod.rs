//! Tools Module
//! Migrated from /rust/crates/tools
//! Provides tool system framework and execution

use std::sync::Arc;

pub mod builtin;
pub mod context;
pub mod integration_phase4;
pub mod output;
pub mod registry;
pub mod toolset;
#[allow(unused_imports)]
pub use context::{SharedToolContext, ToolContext};
#[allow(unused_imports)]
pub use output::{ToolOutput, ToolResultPart};
#[allow(unused_imports)]
pub use registry::{ToolEntry, ToolError, ToolHandlerMultimodal, ToolRegistry};
#[allow(unused_imports)]
pub use toolset::{ToolSet, ToolSetRegistry, TOOLSETS};

use crate::modules::browser::BrowserRegistry;
use crate::modules::memory::{PinnedStore, SharedMemoryProvider};
use crate::modules::scheduler::SharedScheduler;

/// Register all builtin tools to the given registry.
///
/// Called during application startup. Registers the complete set of
/// built-in tools including: bash, file operations, web search/fetch,
/// browser automation, memory, scheduler/cron, skills management, and
/// utility tools (json_parse, todo_write, sleep, config, etc.).
///
/// Phase 8A.10 / T-F2 — `pinned` is the shared [`PinnedStore`] used by
/// the new `pin_memory` / `unpin_memory` tools.  Callers without access
/// to a real store (legacy tests) can pass
/// `Arc::new(crate::modules::memory::NullPinnedStore::new())`.
pub fn register_builtin_tools(
    registry: &ToolRegistry,
    memory: SharedMemoryProvider,
    scheduler: SharedScheduler,
    browser: Arc<BrowserRegistry>,
    pinned: Arc<dyn PinnedStore>,
) {
    if let Err(e) = registry.register(builtin::browser_tool_entry(browser)) {
        eprintln!("Failed to register browser tool: {}", e);
    }
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
    if let Err(e) = registry.register(builtin::pin_memory_entry(pinned.clone())) {
        eprintln!("Failed to register pin_memory tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::unpin_memory_entry(pinned.clone())) {
        eprintln!("Failed to register unpin_memory tool: {}", e);
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
    if let Err(e) = registry.register(builtin::skill_find_tool_entry()) {
        eprintln!("Failed to register skill_find tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::skill_view_tool_entry()) {
        eprintln!("Failed to register skill_view tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::skills_list_tool_entry()) {
        eprintln!("Failed to register skills_list tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::skills_categories_tool_entry()) {
        eprintln!("Failed to register skills_categories tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::skill_manage_tool_entry()) {
        eprintln!("Failed to register skill_manage tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::tool_search::tool_search_tool_entry(Arc::new(
        registry.clone(),
    ))) {
        eprintln!("Failed to register tool_search tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::agent::agent_tool_entry()) {
        eprintln!("Failed to register agent tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::grep_search_tool_entry()) {
        eprintln!("Failed to register grep_search tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::sleep_tool_entry()) {
        eprintln!("Failed to register Sleep tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::config_tool_entry()) {
        eprintln!("Failed to register Config tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::send_user_message_tool_entry()) {
        eprintln!("Failed to register SendUserMessage tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::notebook_edit_tool_entry()) {
        eprintln!("Failed to register NotebookEdit tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::structured_output_tool_entry()) {
        eprintln!("Failed to register StructuredOutput tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::repl_tool_entry()) {
        eprintln!("Failed to register REPL tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::powershell_tool_entry()) {
        eprintln!("Failed to register PowerShell tool: {}", e);
    }
}
