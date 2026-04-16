//! Channel module — channel registry, adapter trait, and lifecycle management.
//!
//! Supports the onboarding Step 5 (Channel Setup):
//! - `list_channels()`: Returns all 13 builtin channels
//! - `ChannelAdapter` trait: Protocol abstraction for each platform
//! - `ChannelManager`: Dynamic start/stop of channel polling tasks
//!
//! Channel configuration types (`ChannelConfig`, `ChannelConfigRedacted`,
//! `ChannelRouting`) are defined in `crate::modules::config`.

// allow(dead_code): this module is a service layer; commands/ will use it in a later slice.
// allow(unused_imports): re-exports are for future Tauri command consumers.
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod adapter;
pub mod manager;
pub mod registry;
pub mod types;

// Re-export commonly used types
pub use adapter::{ChannelAdapter, MessageHandler};
pub use manager::ChannelManager;
pub use registry::{builtin_channels, count_by_category, find_channel};
pub use types::{
    Channel, ChannelCategory, ChannelEnvelope, ConnectMode, PlatformConfig, SenderInfo,
};
