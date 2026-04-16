//! Onboarding configuration module.
//!
//! Implements the configuration platform defined in ADR-014:
//! a unified ConfigService that manages dual-mode persistence:
//! - Layer 1: `~/.if2ai/config.json` (shortcut format for onboarding UI)
//! - Layer 2: `~/.if2ai/providers.yaml` + `auth.json` + `models.json` (runtime format)
//! - Bridge: `~/.claude/settings.json` (Claude Code compatibility layer)
//!
//! ## Submodules
//! - `types`: Core types (`AppConfig`, `ProviderConfig`, `ChannelConfig`, `ModelSelection`)
//! - `store`: File I/O (JSON/YAML read/write with atomic writes)
//! - `service`: ConfigService trait + implementation
//! - `bridge`: Bridge logic for `~/.claude/settings.json`
//! - `triple_files`: Layer 2 triple-file synchronization

#![allow(dead_code)]

pub mod bridge;
pub mod service;
pub mod store;
pub mod triple_files;
pub mod types;

// Re-export key types for convenience
#[allow(unused_imports)]
pub use service::ConfigService;
#[allow(unused_imports)]
pub use types::{
    AppConfig, AuthEntry, AuthJson, ChannelConfig, ChannelConfigRedacted, ChannelRouting,
    ModelEntry, ModelSelection, ModelsJson, ModelsProviderEntry, ProviderConfig, ProviderYamlEntry,
    ProvidersYaml, CONFIG_VERSION,
};
