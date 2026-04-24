#![allow(ambiguous_glob_reexports)]
//! Core modules - integrated from /rust/crates
//!
//! This module aggregates all the core systems:
//! - runtime: Agent loop and execution
//! - api: Provider management and routing
//! - tools: Tool system and execution
//! - commands: Command processing
//! - session: Session management with JSON persistence
//! - projects: Multi-project support
//! - memory: Memory storage and retrieval
//! - scheduler: Cron job scheduling
//! - browser: Chrome/Chromium automation via CDP (chromiumoxide)

pub mod api;
pub mod application;
pub mod browser;
pub mod channel;
pub mod commands;
pub mod config;
pub mod control_plane;
pub mod desktop_host;
pub mod git;
pub mod harness;
pub mod identity;
pub mod learning;
pub mod memory;
pub mod observability;
pub mod onboarding;
pub mod projects;
pub mod provider;
pub mod runtime;
pub mod scheduler;
pub mod security;
pub mod session;
pub mod skills;
pub mod stt;
pub mod system_check;
pub mod tools;
pub mod tts;
pub mod usage;
pub mod viewer_registry;

#[allow(unused_imports)]
pub use api::*;
#[allow(unused_imports)]
pub use channel::*;
#[allow(unused_imports)]
pub use commands::*;
#[allow(unused_imports)]
pub use config::{
    AppConfig, ChannelConfig, ChannelConfigRedacted, ChannelRouting, ConfigService, ModelSelection,
    ProviderConfig, CONFIG_VERSION,
};
#[allow(unused_imports)]
pub use control_plane::*;
#[allow(unused_imports)]
pub use identity::*;
#[allow(unused_imports)]
pub use learning::*;
#[allow(unused_imports)]
pub use memory::*;
#[allow(unused_imports)]
pub use onboarding::*;
#[allow(unused_imports)]
pub use projects::*;
#[allow(unused_imports)]
pub use runtime::*;
#[allow(unused_imports)]
pub use scheduler::*;
#[allow(unused_imports)]
pub use session::*;
#[allow(unused_imports)]
pub use tools::*;
