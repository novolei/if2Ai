//! Core modules - integrated from /rust/crates
//! 
//! This module aggregates all the core systems:
//! - runtime: Agent loop and execution
//! - api: Provider management and routing
//! - tools: Tool system and execution
//! - commands: Command processing
//! - plugins: Plugin system

pub mod runtime;
pub mod api;
pub mod tools;
pub mod commands;
pub mod plugins;

pub use runtime::*;
pub use api::*;
pub use tools::*;
pub use commands::*;
pub use plugins::*;
