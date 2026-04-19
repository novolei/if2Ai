//! TTS model loading coordinator.
//!
//! Manages model download/caching and exposes the root model directory
//! path for ONNX session loading in subsequent slices.

pub mod downloader;
pub mod global;
pub mod local;

pub use downloader::{ensure_models_cached, ModelPaths};
#[allow(unused_imports)]
pub use global::{GlobalSessions, OnnxSession};
#[allow(unused_imports)]
pub use local::LocalSessions;
