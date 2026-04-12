//! Builtin tools - bash, file_read, json_parse
//!
//! Provides basic tool implementations for the ToolRegistry.

pub mod agent;
pub mod bash;
pub mod config;
pub mod content_search;
pub mod cron_add;
pub mod cron_list;
pub mod cron_remove;
pub mod cron_run;
pub mod cron_runs;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod glob_search;
pub mod grep_search;
pub mod http_request;
pub mod json_parse;
pub mod memory_export;
pub mod memory_forget;
pub mod memory_purge;
pub mod memory_recall;
pub mod memory_store;
pub mod notebook_edit;
pub mod powershell;
pub mod repl;
pub mod send_user_message;
pub mod skill;
pub mod skill_search;
pub mod sleep;
pub mod structured_output;
pub mod todo_write;
pub mod tool_search;
pub mod web_fetch;
pub mod web_search;

#[allow(unused_imports)]
pub use agent::agent_tool_entry;
#[allow(unused_imports)]
pub use bash::bash_tool_entry;
#[allow(unused_imports)]
pub use config::config_tool_entry;
#[allow(unused_imports)]
pub use content_search::entry as content_search_entry;
#[allow(unused_imports)]
pub use cron_add::entry as cron_add_entry;
#[allow(unused_imports)]
pub use cron_list::entry as cron_list_entry;
#[allow(unused_imports)]
pub use cron_remove::entry as cron_remove_entry;
#[allow(unused_imports)]
pub use cron_run::entry as cron_run_entry;
#[allow(unused_imports)]
pub use cron_runs::entry as cron_runs_entry;
#[allow(unused_imports)]
pub use file_edit::entry as file_edit_entry;
#[allow(unused_imports)]
pub use file_read::file_read_tool_entry;
#[allow(unused_imports)]
pub use file_write::entry as file_write_entry;
#[allow(unused_imports)]
pub use glob_search::entry as glob_search_entry;
#[allow(unused_imports)]
pub use grep_search::grep_search_tool_entry;
#[allow(unused_imports)]
pub use http_request::entry as http_request_entry;
#[allow(unused_imports)]
pub use json_parse::json_parse_tool_entry;
#[allow(unused_imports)]
pub use memory_export::entry as memory_export_entry;
#[allow(unused_imports)]
pub use memory_forget::entry as memory_forget_entry;
#[allow(unused_imports)]
pub use memory_purge::entry as memory_purge_entry;
#[allow(unused_imports)]
pub use memory_recall::entry as memory_recall_entry;
#[allow(unused_imports)]
pub use memory_store::entry as memory_store_entry;
#[allow(unused_imports)]
pub use notebook_edit::notebook_edit_tool_entry;
#[allow(unused_imports)]
pub use powershell::powershell_tool_entry;
#[allow(unused_imports)]
pub use repl::repl_tool_entry;
#[allow(unused_imports)]
pub use send_user_message::send_user_message_tool_entry;
#[allow(unused_imports)]
pub use skill::skill_tool_entry;
#[allow(unused_imports)]
pub use skill_search::skill_search_tool_entry;
#[allow(unused_imports)]
pub use sleep::sleep_tool_entry;
#[allow(unused_imports)]
pub use structured_output::structured_output_tool_entry;
#[allow(unused_imports)]
pub use todo_write::todo_write_tool_entry;
#[allow(unused_imports)]
pub use web_fetch::entry as web_fetch_entry;
#[allow(unused_imports)]
pub use web_search::entry as web_search_entry;
