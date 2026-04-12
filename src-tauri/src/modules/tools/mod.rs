//! Tools Module
//! Migrated from /rust/crates/tools
//! Provides tool system framework and execution

pub mod builtin;
pub mod context;
pub mod registry;
#[allow(unused_imports)]
pub use context::{SharedToolContext, ToolContext};
#[allow(unused_imports)]
pub use registry::{ToolEntry, ToolError, ToolRegistry};

/// Register all builtin tools to the given registry.
///
/// This function is called during application startup to register
/// the default builtin tools: bash, read_file, json_parse.
pub fn register_builtin_tools(registry: &ToolRegistry) {
    if let Err(e) = registry.register(builtin::bash::bash_tool_entry()) {
        eprintln!("Failed to register bash tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::file_read::file_read_tool_entry()) {
        eprintln!("Failed to register read_file tool: {}", e);
    }
    if let Err(e) = registry.register(builtin::json_parse::json_parse_tool_entry()) {
        eprintln!("Failed to register json_parse tool: {}", e);
    }
}
