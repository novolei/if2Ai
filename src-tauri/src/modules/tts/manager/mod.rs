//! TTS service manager.
//!
//! Coordinates the TTS service lifecycle including:
//! - Warmup state machine (async model loading and priming)
//! - Streaming job management (concurrent stream lifecycle)

#![allow(dead_code)]

pub mod jobs;
pub mod warmup;

#[allow(unused_imports)]
pub use jobs::{StreamingJob, StreamingJobManager};
#[allow(unused_imports)]
pub use warmup::{WarmupManager, WarmupSnapshot};
