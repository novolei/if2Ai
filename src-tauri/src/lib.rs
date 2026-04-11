//! If2Ai - AI Agent Desktop Application
//!
//! This is the unified, integrated codebase combining all functionality
//! from /rust/crates into a single, cohesive project.
//!
//! Unified project structure:
//! - modules/runtime   (from /rust/crates/runtime) - Agent execution engine
//! - modules/api       (from /rust/crates/api) - LLM provider integration
//! - modules/tools     (from /rust/crates/tools) - Tool system
//! - modules/commands  (from /rust/crates/commands) - Command handling
//! - modules/plugins   (from /rust/crates/plugins) - Plugin system
//!
//! This is the single, unified codebase for all If2Ai development.

pub mod modules;
pub use modules::api;
pub use modules::commands;
pub use modules::plugins;
pub use modules::runtime;
pub use modules::tools;

pub use modules::*;
