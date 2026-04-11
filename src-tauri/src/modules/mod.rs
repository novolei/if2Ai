//! Core modules - integrated from /rust/crates
//!
//! This module aggregates all the core systems:
//! - runtime: Agent loop and execution
//! - api: Provider management and routing
//! - tools: Tool system and execution
//! - commands: Command processing
//! - plugins: Plugin system

pub mod api;
pub mod commands;
pub mod plugins;
pub mod runtime;
pub mod tools;

#[allow(unused_imports)]
pub use api::*;
#[allow(unused_imports)]
pub use commands::*;
#[allow(unused_imports)]
pub use plugins::*;
#[allow(unused_imports)]
pub use runtime::*;
#[allow(unused_imports)]
pub use tools::*;
