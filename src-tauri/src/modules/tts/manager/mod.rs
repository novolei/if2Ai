//! TTS service manager.
//!
//! Coordinates the TTS service lifecycle including:
//! - Warmup state machine (async model loading and priming)
//! - Streaming job management (future)

#![allow(dead_code)]

pub mod warmup;

#[allow(unused_imports)]
pub use warmup::{WarmupManager, WarmupSnapshot};
