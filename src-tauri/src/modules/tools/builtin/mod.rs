//! Builtin tools - bash, file_read, json_parse
//!
//! Provides basic tool implementations for the ToolRegistry.

pub mod bash;
pub mod file_read;
pub mod json_parse;

#[allow(unused_imports)]
pub use bash::bash_tool_entry;
#[allow(unused_imports)]
pub use file_read::file_read_tool_entry;
#[allow(unused_imports)]
pub use json_parse::json_parse_tool_entry;
