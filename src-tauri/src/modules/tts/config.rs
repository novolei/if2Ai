//! TTS configuration — model paths, generation parameters, voice presets.
//!
//! Mirrors `moss_tts_nano/defaults.py` and the form field defaults in `app.py`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default model paths for the TTS ONNX models.
pub const TTS_MODEL_HF_REPO: &str = "OpenMOSS-Team/MOSS-TTS-Nano-100M-ONNX";
pub const AUDIO_TOKENIZER_HF_REPO: &str = "OpenMOSS-Team/MOSS-Audio-Tokenizer-Nano-ONNX";

/// Default model directory under `~/.if2ai/models/tts/`.
/// 默认模型缓存根目录：`~/.if2ai/models/tts/`。
///
/// 与 HuggingFace `snapshot_download(local_dir=...)` 的实际产物路径一致：
/// - `<root>/MOSS-TTS-Nano-100M-ONNX/moss_tts_prefill.onnx`
/// - `<root>/MOSS-Audio-Tokenizer-Nano-ONNX/moss_audio_tokenizer_encode.onnx`
/// - `<root>/MOSS-TTS-Nano-100M-ONNX/browser_poc_manifest.json`
///
/// 历史上曾用过 `dirs::data_local_dir()`，但和 main.rs lazy provider /
/// `OnnxTtsProvider::from_model_dir` 的检测路径不一致（导致 UI 显示"未下载"
/// 但 backend 实际能加载真模型）。统一到 home dir 下方便用户用 `~/.if2ai/`
/// 直接 ls 查看。
pub fn default_model_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("models")
        .join("tts")
}

/// MOSS-TTS-Nano 主模型在缓存根下的子目录（与 HF repo dir 一致）。
pub const TTS_MODEL_SUBDIR: &str = "MOSS-TTS-Nano-100M-ONNX";

/// MOSS-Audio-Tokenizer-Nano 在缓存根下的子目录。
pub const AUDIO_TOKENIZER_SUBDIR: &str = "MOSS-Audio-Tokenizer-Nano-ONNX";

/// Default voice preset audio directory under `~/.if2ai/models/tts/voices/`.
pub fn default_voice_dir() -> PathBuf {
    default_model_dir().join("voices")
}

/// Warmup text — short Chinese sentence for priming the model.
pub const WARMUP_TEXT: &str = "你好，欢迎使用 Nano-TTS。";

/// Maximum frames for warmup (short synthesis).
pub const WARMUP_MAX_FRAMES: u32 = 96;

/// Audio output sample rate (always 48kHz for MOSS-TTS-Nano).
pub const SAMPLE_RATE: u32 = 48_000;

/// Audio output channels (always stereo).
pub const CHANNELS: u16 = 2;

/// Generation parameters — mirrors all generation options from `app.py`.
///
/// Each field maps to a form input in the web demo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationParams {
    /// Maximum audio frames to generate. Default: 375.
    pub max_new_frames: u32,
    /// Maximum text tokens per chunk for voice clone splitting. Default: 75.
    pub voice_clone_max_text_tokens: u32,
    /// Max TTS chunk batch size (0 = auto). Default: 0.
    pub tts_max_batch_size: u32,
    /// Max codec batch size (0 = auto). Default: 0.
    pub codec_max_batch_size: u32,
    /// Enable sampling (vs greedy). Default: true.
    pub do_sample: bool,
    /// Text token sampling temperature. Default: 1.0.
    pub text_temperature: f32,
    /// Text token nucleus sampling probability. Default: 1.0.
    pub text_top_p: f32,
    /// Text token top-k filtering. Default: 50.
    pub text_top_k: u32,
    /// Audio token sampling temperature. Default: 0.8.
    pub audio_temperature: f32,
    /// Audio token nucleus sampling probability. Default: 0.95.
    pub audio_top_p: f32,
    /// Audio token top-k filtering. Default: 25.
    pub audio_top_k: u32,
    /// Audio token repetition penalty. Default: 1.2.
    pub audio_repetition_penalty: f32,
    /// Random seed for reproducibility. None = random.
    pub seed: Option<u64>,
    /// Enable robust text normalization. Default: true.
    pub enable_robust_normalization: bool,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            max_new_frames: 375,
            voice_clone_max_text_tokens: 75,
            tts_max_batch_size: 0,
            codec_max_batch_size: 0,
            do_sample: true,
            text_temperature: 1.0,
            text_top_p: 1.0,
            text_top_k: 50,
            audio_temperature: 0.8,
            audio_top_p: 0.95,
            audio_top_k: 25,
            audio_repetition_penalty: 1.2,
            seed: None,
            enable_robust_normalization: true,
        }
    }
}

/// A single audio chunk emitted during streaming synthesis.
///
/// Contains PCM16LE data ready for playback via Web Audio API
/// (frontend) or direct audio output (native).
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// PCM16LE audio bytes. Each frame is `channels * 2` bytes.
    pub pcm_data: Vec<u8>,
    /// Sample rate (always 48000).
    pub sample_rate: u32,
    /// Number of channels (always 2).
    pub channels: u16,
    /// Which text chunk this audio belongs to (0-based).
    pub chunk_index: usize,
    /// Whether this is a pause frame (chunk boundary).
    pub is_pause: bool,
    /// Total audio emitted so far in seconds.
    pub emitted_audio_seconds: f32,
    /// Lead time ahead of playback in seconds.
    pub lead_seconds: f32,
}

/// Voice preset definition — mirrors `VoicePreset` in `moss_tts_nano_runtime.py`.
///
/// Each preset contains a name, embedded audio data (WAV/MP3), and description.
#[derive(Debug, Clone)]
pub struct VoicePreset {
    /// Preset name (e.g. "Junhao", "Trump", "Sakura").
    pub name: String,
    /// Embedded audio data bytes (WAV or MP3 format).
    ///
    /// These are loaded from the `assets/audio/` directory of the
    /// MOSS-TTS-Nano reference repo and bundled at build time.
    pub audio_data: Vec<u8>,
    /// Human-readable description (e.g. "Chinese male voice A").
    pub description: String,
    /// File extension of the original audio ("wav" or "mp3").
    pub audio_ext: String,
}

impl VoicePreset {
    /// Create a new voice preset from raw bytes.
    #[must_use]
    pub fn new(name: &str, description: &str, audio_ext: &str, audio_data: Vec<u8>) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            audio_ext: audio_ext.to_string(),
            audio_data,
        }
    }

    /// Check if this voice is MP3 encoded.
    #[must_use]
    pub fn is_mp3(&self) -> bool {
        self.audio_ext == "mp3"
    }
}
