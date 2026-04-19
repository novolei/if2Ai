//! TTS provider implementations.
//!
//! Contains the trait implementations for [`crate::modules::tts::TtsProvider`]:
//! - `MockTtsProvider` — returns synthetic audio for testing.
//! - `OnnxTtsProvider` — real ONNX Runtime inference.

pub mod mock;
pub mod onnx;

#[allow(unused_imports)]
pub use mock::MockTtsProvider;
#[allow(unused_imports)]
pub use onnx::OnnxTtsProvider;
