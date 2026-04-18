//! TTS error types.
//!
//! Mirrors the error paths in `moss_tts_nano_runtime.py` and `app.py`:
//! missing models, invalid text, synthesis failures, streaming errors.

/// All error types for the TTS module.
#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    /// TTS model files not found or not downloaded.
    #[error("TTS model not found: {0}. Run warmup or download from HuggingFace.")]
    ModelNotFound(String),

    /// Audio tokenizer model files not found.
    #[error("Audio tokenizer not found: {0}. Run warmup or download from HuggingFace.")]
    AudioTokenizerNotFound(String),

    /// Synthesis failed during inference.
    #[error("Synthesis failed: {0}")]
    SynthesisFailed(String),

    /// Input text is empty or invalid.
    #[error("Text is required")]
    EmptyText,

    /// Invalid synthesis mode.
    #[error("Invalid synthesis mode: {0}. Must be 'voice_clone' or 'continuation'.")]
    InvalidMode(String),

    /// Voice preset not found.
    #[error("Voice preset not found: {0}. Available voices: {1}")]
    VoiceNotFound(String, String),

    /// Prompt audio file not found.
    #[error("Prompt audio not found: {0}")]
    PromptAudioNotFound(std::path::PathBuf),

    /// ONNX Runtime inference error.
    #[error("ONNX inference failed: {0}")]
    OnnxError(String),

    /// Text tokenization error.
    #[error("Text tokenization failed: {0}")]
    TokenizationError(String),

    /// Audio codec error (encode or decode).
    #[error("Audio codec failed: {0}")]
    CodecError(String),

    /// Streaming job not found.
    #[error("Streaming job not found: {0}")]
    StreamNotFound(String),

    /// Stream was cancelled or closed.
    #[error("Stream closed: {0}")]
    StreamClosed(String),

    /// Warmup not yet complete — model not primed.
    #[error("Warmup not complete. Please run warmup first.")]
    WarmupNotReady,

    /// WAV encoding/decoding failure.
    #[error("WAV decode failed: {0}")]
    WavDecode(String),

    /// Generic error with a message.
    #[error("TTS error: {0}")]
    Generic(String),
}
