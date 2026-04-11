//! Core modules - integrated from /rust/crates
//!
//! This module aggregates all the core systems:
//! - runtime: Agent loop and execution
//! - api: Provider management and routing
//! - tools: Tool system and execution
//! - commands: Command processing
//! - plugins: Plugin system
//! - session: Session management with JSON persistence
//! - projects: Multi-project support

pub mod api;
pub mod commands;
pub mod plugins;
pub mod projects;
pub mod runtime;
pub mod session;
pub mod tools;

#[allow(unused_imports)]
pub use api::*;
#[allow(unused_imports)]
pub use commands::*;
#[allow(unused_imports)]
pub use plugins::*;
#[allow(unused_imports)]
pub use projects::*;
#[allow(unused_imports)]
pub use runtime::*;
#[allow(unused_imports)]
pub use session::*;
#[allow(unused_imports)]
pub use tools::*;
