//! TTS (Text-to-Speech) module — MOSS-TTS-Nano ONNX native integration.
//!
//! Provides multilingual text-to-speech via ONNX Runtime CPU inference,
//! including voice cloning, streaming audio output, and 15 built-in voice presets.
//
// #![allow(dead_code)] justification: TTS types are infrastructure for future
// slices (ONNX inference, Tauri commands, Settings UI). They will be wired into
// the agent loop and app binary once the full TTS pipeline is implemented.
#![allow(dead_code)]
//!
//! ## Architecture
//!
//! The top-level trait [`TtsProvider`] abstracts the synthesis engine.
//! The primary implementation is `OnnxTtsProvider` (future), which loads
//! the MOSS-TTS-Nano ONNX models and runs inference via the `ort` crate.
//! A `MockTtsProvider` is provided for testing.
//!
//! ## Model Files
//!
//! Models are downloaded on first use from HuggingFace and cached at
//! `~/.if2ai/models/tts/`. Two model repositories are required:
//! - `MOSS-TTS-Nano-100M-ONNX` (main TTS model, ~100M params)
//! - `MOSS-Audio-Tokenizer-Nano-ONNX` (audio tokenizer, ~20M params)

pub mod audio;
pub mod config;
pub mod error;
pub mod inference;
pub mod model;
pub mod provider;
pub mod text;
pub mod voice;

#[allow(unused_imports)]
pub use audio::{wav_decode, wav_encode};

pub use config::{AudioChunk, GenerationParams, VoicePreset};
pub use error::TtsError;
#[allow(unused_imports)]
pub use model::{ensure_models_cached, ModelPaths};
#[allow(unused_imports)]
pub use text::normalize_tts_text;
#[allow(unused_imports)]
pub use voice::DemoEntry;

use async_trait::async_trait;

/// Audio sink for streaming — receives PCM chunks during synthesis.
///
/// Mirrors the Python `synthesize_stream()` iterator that yields
/// `{"type": "audio", "waveform_numpy": ..., "sample_rate": ...}` events.
#[async_trait]
pub trait AudioSink: Send + Sync {
    /// Called for each PCM audio chunk generated during streaming synthesis.
    async fn on_audio(&self, chunk: AudioChunk);
    /// Called once when streaming synthesis completes.
    async fn on_complete(&self, result: StreamResult);
}

/// Top-level TTS service trait — mirrors `NanoTTSService` from the Python reference.
///
/// Implementations must support:
/// - Buffered synthesis (complete WAV output)
/// - Streaming synthesis (per-frame PCM chunks)
/// - Voice clone mode (prompt audio → cloned voice)
/// - Continuation mode (prompt audio + prompt text → continuation)
/// - Warmup (short test synthesis to prime the model)
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Buffered synthesis — generates complete audio and returns it as a WAV byte buffer.
    ///
    /// Mirrors `NanoTTSService.synthesize()`.
    async fn synthesize(&self, params: SynthesisParams) -> Result<SynthesisResult, TtsError>;

    /// Streaming synthesis — yields audio chunks via the provided [`AudioSink`].
    ///
    /// Mirrors `NanoTTSService.synthesize_stream()`.
    async fn synthesize_stream(
        &self,
        params: SynthesisParams,
        sink: std::sync::Arc<dyn AudioSink>,
    ) -> Result<StreamResult, TtsError>;

    /// Warmup synthesis — runs a short synthesis to prime the model.
    ///
    /// Mirrors `NanoTTSService.warmup()`.
    async fn warmup(&self) -> Result<WarmupResult, TtsError>;

    /// Split text into chunks for voice clone mode, respecting token budget.
    ///
    /// Mirrors `NanoTTSService.split_voice_clone_text()`.
    fn split_voice_clone_text(
        &self,
        text: &str,
        max_tokens: usize,
    ) -> Result<Vec<String>, TtsError>;

    /// List all available voice preset names.
    ///
    /// Mirrors `NanoTTSService.list_voice_names()`.
    fn list_voices(&self) -> Vec<String>;

    /// Get a voice preset by name.
    ///
    /// Mirrors `NanoTTSService.get_voice_preset()`.
    fn get_voice(&self, name: &str) -> Option<&VoicePreset>;

    /// Get the default voice preset.
    fn default_voice(&self) -> &VoicePreset;
}

/// Parameters for a synthesis request.
///
/// This maps 1:1 to the form fields in `app.py`'s `/api/generate` endpoint.
#[derive(Debug, Clone)]
pub struct SynthesisParams {
    /// Text to synthesize.
    pub text: String,
    /// Synthesis mode: `"voice_clone"` or `"continuation"`.
    pub mode: SynthesisMode,
    /// Voice preset name (e.g. "Junhao"). Used as fallback if no prompt audio provided.
    pub voice: Option<String>,
    /// Path to prompt/reference audio file for voice cloning.
    pub prompt_audio_path: Option<std::path::PathBuf>,
    /// Prompt text for continuation mode.
    pub prompt_text: Option<String>,
    /// Generation parameters (sampling, batch size, etc.).
    pub generation: GenerationParams,
}

/// Synthesis mode — determines how the voice is selected.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum SynthesisMode {
    /// Clone voice from a reference audio file.
    #[default]
    VoiceClone,
    /// Continue from a prompt audio + prompt text pair.
    Continuation,
}

#[derive(Debug, Clone)]
pub struct SynthesisResult {
    /// WAV-encoded audio bytes (48kHz, stereo, 16-bit PCM).
    pub audio_bytes: Vec<u8>,
    /// Sample rate of the audio (always 48000 for MOSS-TTS-Nano).
    pub sample_rate: u32,
    /// Number of channels (always 2 for stereo).
    pub channels: u16,
    /// Duration of the audio in seconds.
    pub duration_seconds: f32,
    /// Voice preset name used.
    pub voice: String,
    /// Text chunks the input was split into.
    pub text_chunks: Vec<String>,
    /// Elapsed time for synthesis in seconds.
    pub elapsed_seconds: f32,
    /// Normalized text after preprocessing.
    pub normalized_text: String,
}

/// Result from streaming [`TtsProvider::synthesize_stream`].
#[derive(Debug, Clone)]
pub struct StreamResult {
    /// Path to the saved WAV file.
    pub audio_path: Option<std::path::PathBuf>,
    /// Sample rate of the audio.
    pub sample_rate: u32,
    /// Voice preset name used.
    pub voice: String,
    /// Text chunks the input was split into.
    pub text_chunks: Vec<String>,
    /// Elapsed time for synthesis in seconds.
    pub elapsed_seconds: f32,
    /// Total emitted audio seconds.
    pub emitted_audio_seconds: f32,
    /// Lead seconds (buffer ahead of playback).
    pub lead_seconds: f32,
}

/// Result from [`TtsProvider::warmup`].
#[derive(Debug, Clone)]
pub struct WarmupResult {
    /// Elapsed time for warmup synthesis in seconds.
    pub elapsed_seconds: f32,
    /// Device used for inference.
    pub device: String,
}
