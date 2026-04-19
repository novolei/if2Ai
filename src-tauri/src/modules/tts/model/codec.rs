//! Codec (audio tokenizer) ONNX sessions for the TTS model.
//!
//! Mirrors `_create_sessions()` from `ort_cpu_runtime.py` for the
//! audio tokenizer sessions: `codec_encode`, `codec_decode`, and
//! `codec_decode_step`.
//!
//! The audio tokenizer converts between raw waveform (PCM) and
//! discrete audio codes (token IDs). It is used for:
//! - **Voice cloning**: encoding prompt audio → audio codes
//! - **Output generation**: decoding audio codes → waveform
//!
//! ## Session Reference
//!
//! | Session          | Python Key       | Key Inputs                         | Key Outputs                       |
//! |------------------|------------------|------------------------------------|-----------------------------------|
//! | **encode**       | codec_encode     | waveform [1,2,time], input_lengths | audio_codes [1,n_frames,16], audio_code_lengths |
//! | **decode_full**  | codec_decode     | audio_codes [1,n_frames,16], audio_code_lengths | audio [1,2,samples], audio_lengths |
//! | **decode_step**  | codec_decode_step | audio_codes + stateful KV caches  | audio, audio_lengths, updated caches |
//!
//! The decode_step session is stateful — it maintains transformer
//! and attention cache state between calls via the
//! `CodecStreamingDecodeSession` wrapper.

// Justification: CodecSessions methods/constants are consumed by future
// TTS synthesis slices (audio encoding/decoding pipeline).
#![allow(dead_code)]

use std::path::Path;

use crate::modules::tts::error::TtsError;
use crate::modules::tts::model::global::OnnxSession;

/// ONNX file name for the codec encoder.
pub const CODEC_ENCODE_ONNX: &str = "encode";

/// ONNX file name for the codec decoder (full/batch).
pub const CODEC_DECODE_FULL_ONNX: &str = "decode_full";

/// ONNX file name for the codec streaming decode step.
pub const CODEC_DECODE_STEP_ONNX: &str = "decode_step";

/// Input name for the waveform tensor.
pub const CODEC_WAVEFORM: &str = "waveform";

/// Input name for the waveform lengths.
pub const CODEC_INPUT_LENGTHS: &str = "input_lengths";

/// Input name for audio codes.
pub const CODEC_AUDIO_CODES: &str = "audio_codes";

/// Input name for audio code lengths.
pub const CODEC_AUDIO_CODE_LENGTHS: &str = "audio_code_lengths";

/// Output name for audio codes (from encoder).
pub const CODEC_AUDIO_CODES_OUT: &str = "audio_codes";

/// Output name for audio code lengths (from encoder).
pub const CODEC_AUDIO_CODE_LENGTHS_OUT: &str = "audio_code_lengths";

/// Output name for decoded audio waveform.
pub const CODEC_AUDIO_OUT: &str = "audio";

/// Output name for decoded audio lengths.
pub const CODEC_AUDIO_LENGTHS_OUT: &str = "audio_lengths";

/// Codec ONNX sessions — encode, decode_full, and decode_step.
///
/// These sessions handle audio tokenization for voice cloning
/// (encode prompt audio → codes) and audio synthesis output
/// (decode codes → waveform).
///
/// Mirrors `self.sessions["codec_encode"]`, `self.sessions["codec_decode"]`,
/// and `self.sessions["codec_decode_step"]` from `OrtCpuRuntime`
/// in `ort_cpu_runtime.py`.
pub struct CodecSessions {
    /// Audio encoder: waveform → audio codes.
    pub encode: Option<OnnxSession>,
    /// Full audio decoder: audio codes → waveform (batch).
    pub decode_full: Option<OnnxSession>,
    /// Streaming audio decoder: incremental decode with state.
    pub decode_step: Option<OnnxSession>,
}

impl CodecSessions {
    /// Create codec sessions from the audio tokenizer directory.
    ///
    /// Each session is loaded only if the corresponding `.onnx` file exists.
    /// Missing sessions result in `None` rather than an error.
    ///
    /// Mirrors `_create_sessions()` from `ort_cpu_runtime.py:376-378`
    /// for the "codec_encode", "codec_decode", and "codec_decode_step" entries.
    pub fn load(audio_tokenizer_dir: &Path, thread_count: usize) -> Result<Self, TtsError> {
        let encode_path = audio_tokenizer_dir.join("encode.onnx");
        let decode_full_path = audio_tokenizer_dir.join("decode_full.onnx");
        let decode_step_path = audio_tokenizer_dir.join("decode_step.onnx");

        let encode = if encode_path.exists() {
            Some(OnnxSession::load(&encode_path, thread_count)?)
        } else {
            None
        };

        let decode_full = if decode_full_path.exists() {
            Some(OnnxSession::load(&decode_full_path, thread_count)?)
        } else {
            None
        };

        let decode_step = if decode_step_path.exists() {
            Some(OnnxSession::load(&decode_step_path, thread_count)?)
        } else {
            None
        };

        Ok(Self {
            encode,
            decode_full,
            decode_step,
        })
    }

    /// Dump session I/O names for verification against Python reference.
    pub fn dump_io_names(&self) {
        if let Some(ref s) = self.encode {
            tracing::info!("codec_encode inputs: {:?}", s.input_names());
            tracing::info!("codec_encode outputs: {:?}", s.output_names());
        }
        if let Some(ref s) = self.decode_full {
            tracing::info!("codec_decode_full inputs: {:?}", s.input_names());
            tracing::info!("codec_decode_full outputs: {:?}", s.output_names());
        }
        if let Some(ref s) = self.decode_step {
            tracing::info!("codec_decode_step inputs: {:?}", s.input_names());
            tracing::info!("codec_decode_step outputs: {:?}", s.output_names());
        }
    }

    /// Check whether all codec sessions are available.
    pub fn is_ready(&self) -> bool {
        self.encode.is_some() && self.decode_full.is_some() && self.decode_step.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_session_constants() {
        assert_eq!(CODEC_WAVEFORM, "waveform");
        assert_eq!(CODEC_INPUT_LENGTHS, "input_lengths");
        assert_eq!(CODEC_AUDIO_CODES, "audio_codes");
        assert_eq!(CODEC_AUDIO_CODE_LENGTHS, "audio_code_lengths");
        assert_eq!(CODEC_AUDIO_OUT, "audio");
        assert_eq!(CODEC_AUDIO_LENGTHS_OUT, "audio_lengths");
    }

    #[test]
    fn load_missing_model_returns_none_sessions() {
        let result = CodecSessions::load(Path::new("/nonexistent"), 4);
        assert!(
            result.is_ok(),
            "loading from nonexistent dir should not fail; sessions just None"
        );
        let sessions = result.unwrap();
        assert!(sessions.encode.is_none());
        assert!(sessions.decode_full.is_none());
        assert!(sessions.decode_step.is_none());
    }

    #[test]
    fn is_ready_all_missing() {
        let sessions = CodecSessions::load(Path::new("/nonexistent"), 4).unwrap();
        assert!(!sessions.is_ready());
    }
}
