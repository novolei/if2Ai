//! TTS provider implementations.
//!
//! Contains the trait implementations for [`crate::modules::tts::TtsProvider`]:
//! - `MockTtsProvider` — returns synthetic audio for testing.
//! - `OnnxTtsProvider` — (future) real ONNX Runtime inference.

pub mod mock;

#[allow(unused_imports)]
pub use mock::MockTtsProvider;
