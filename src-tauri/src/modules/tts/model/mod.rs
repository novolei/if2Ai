//! TTS model loading coordinator.
//!
//! Manages model download/caching and exposes the root model directory
//! path for ONNX session loading in subsequent slices.

pub mod downloader;

pub use downloader::{ensure_models_cached, ModelPaths};
