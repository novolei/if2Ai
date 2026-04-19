//! Local ONNX sessions for the TTS model.
//!
//! Mirrors `_create_sessions()` from `ort_cpu_runtime.py` for the
//! local decoder sessions: `local_decoder`, `local_cached_step`,
//! and `local_fixed_sampleed_frame`.
//!
//! These sessions handle per-frame audio token generation after
//! the global prefill/decode has produced `global_hidden`.
//!
//! ## Session Reference
//!
//! | Session                  | Python Key             | Key Inputs                                              | Key Outputs                          |
//! |--------------------------|------------------------|---------------------------------------------------------|--------------------------------------|
//! | **decoder**              | local_decoder          | global_hidden, text_token_id, audio_prefix_token_ids    | text_logits, audio_logits            |
//! | **cached_step**          | local_cached_step      | global_hidden, text_token_id, audio_token_id, channel_index, step_type, past_valid_lengths, local_past_* | text_logits, audio_logits, local_present_* |
//! | **fixed_sampled_frame**  | local_fixed_sampled_frame | global_hidden, repetition_seen_mask, assistant_random_u, audio_random_u | should_continue, frame_token_ids |
//!
//! The local sessions are optional — they may not exist in all model
//! variants. Loading returns `None` for missing sessions rather than
//! failing, matching the Python `**{...} if ... else {}` pattern.

// Justification: constants and struct fields are defined here for TTS-2.2
// and will be consumed by TTS-3.x inference slices.
#![allow(dead_code)]

use std::path::Path;

use crate::modules::tts::error::TtsError;
use crate::modules::tts::model::global::OnnxSession;

/// ONNX file name for the local decoder.
pub const LOCAL_DECODER_ONNX: &str = "decoder";

/// ONNX file name for the local cached step.
pub const LOCAL_CACHED_STEP_ONNX: &str = "local_cached_step";

/// ONNX file name for the local fixed sampled frame.
pub const LOCAL_FIXED_SAMPLED_FRAME_ONNX: &str = "local_fixed_sampled_frame";

/// Input name for the global hidden state (shared by all local sessions).
pub const LOCAL_GLOBAL_HIDDEN: &str = "global_hidden";

/// Input name for the text token ID.
pub const LOCAL_TEXT_TOKEN_ID: &str = "text_token_id";

/// Input name for the audio prefix token IDs (decoder).
pub const LOCAL_AUDIO_PREFIX_TOKEN_IDS: &str = "audio_prefix_token_ids";

/// Output name for text logits (shared by decoder and cached_step).
pub const LOCAL_TEXT_LOGITS: &str = "text_logits";

/// Output name for audio logits (shared by decoder and cached_step).
pub const LOCAL_AUDIO_LOGITS: &str = "audio_logits";

/// Prefix for local KV cache output tensors (renamed to local_past_* for next step).
pub const LOCAL_PRESENT_PREFIX: &str = "local_present_";

/// Prefix for local KV cache input tensors (received from previous step as local_present_*).
pub const LOCAL_PAST_PREFIX: &str = "local_past_";

/// Local ONNX sessions — decoder, cached_step, and fixed_sampled_frame.
///
/// These sessions are created per synthesis request (not shared globally)
/// because they are used within a single generation loop.
///
/// Mirrors `self.sessions["local_decoder"]`, `self.sessions["local_cached_step"]`,
/// and `self.sessions["local_fixed_sampled_frame"]` from `OrtCpuRuntime`
/// in `ort_cpu_runtime.py`.
pub struct LocalSessions {
    /// Local decoder: global_hidden + text_token_id + audio_prefix → logits.
    pub decoder: Option<OnnxSession>,
    /// Local cached step: incremental decode with local KV cache.
    pub cached_step: Option<OnnxSession>,
    /// Local fixed sampled frame: batch-sampled frame generation.
    pub fixed_sampleed_frame: Option<OnnxSession>,
}

impl LocalSessions {
    /// Create local sessions from the TTS model directory.
    ///
    /// Each session is loaded only if the corresponding `.onnx` file exists.
    /// Missing sessions result in `None` rather than an error, matching
    /// the Python conditional session creation pattern.
    ///
    /// Mirrors `_create_sessions()` from `ort_cpu_runtime.py:360-370`
    /// for the "local_decoder", "local_cached_step", and
    /// "local_fixed_sampled_frame" entries.
    pub fn load(tts_model_dir: &Path, thread_count: usize) -> Result<Self, TtsError> {
        let decoder_path = tts_model_dir.join("decoder.onnx");
        let cached_step_path = tts_model_dir.join("local_cached_step.onnx");
        let fixed_path = tts_model_dir.join("local_fixed_sampled_frame.onnx");

        let decoder = if decoder_path.exists() {
            Some(OnnxSession::load(&decoder_path, thread_count)?)
        } else {
            None
        };

        let cached_step = if cached_step_path.exists() {
            Some(OnnxSession::load(&cached_step_path, thread_count)?)
        } else {
            None
        };

        let fixed_sampleed_frame = if fixed_path.exists() {
            Some(OnnxSession::load(&fixed_path, thread_count)?)
        } else {
            None
        };

        Ok(Self {
            decoder,
            cached_step,
            fixed_sampleed_frame,
        })
    }

    /// Dump session I/O names for verification against Python reference.
    pub fn dump_io_names(&self) {
        if let Some(ref s) = self.decoder {
            tracing::info!("local_decoder inputs: {:?}", s.input_names());
            tracing::info!("local_decoder outputs: {:?}", s.output_names());
        }
        if let Some(ref s) = self.cached_step {
            tracing::info!("local_cached_step inputs: {:?}", s.input_names());
            tracing::info!("local_cached_step outputs: {:?}", s.output_names());
        }
        if let Some(ref s) = self.fixed_sampleed_frame {
            tracing::info!("local_fixed_sampled_frame inputs: {:?}", s.input_names());
            tracing::info!("local_fixed_sampled_frame outputs: {:?}", s.output_names());
        }
    }

    /// Check whether all local sessions are available.
    pub fn is_ready(&self) -> bool {
        self.decoder.is_some() && self.cached_step.is_some() && self.fixed_sampleed_frame.is_some()
    }
}

/// Rename a local cached_step `local_present_*` output name to the
/// corresponding `local_past_*` input name for the next step.
///
/// Mirrors `output_name.replace("local_present_", "local_past_")` from
/// `ort_cpu_runtime.py:532`.
pub fn local_present_to_past(name: &str) -> String {
    name.replacen("local_present_", "local_past_", 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_session_constants() {
        assert_eq!(LOCAL_GLOBAL_HIDDEN, "global_hidden");
        assert_eq!(LOCAL_TEXT_TOKEN_ID, "text_token_id");
        assert_eq!(LOCAL_AUDIO_PREFIX_TOKEN_IDS, "audio_prefix_token_ids");
        assert_eq!(LOCAL_TEXT_LOGITS, "text_logits");
        assert_eq!(LOCAL_AUDIO_LOGITS, "audio_logits");
    }

    #[test]
    fn local_name_prefixes() {
        assert!(LOCAL_PRESENT_PREFIX.starts_with("local_present_"));
        assert!(LOCAL_PAST_PREFIX.starts_with("local_past_"));
    }

    #[test]
    fn rename_local_present_to_past() {
        let key = "local_present_key_0";
        let past = local_present_to_past(key);
        assert_eq!(past, "local_past_key_0");

        let value = "local_present_value_5";
        let past = local_present_to_past(value);
        assert_eq!(past, "local_past_value_5");
    }

    #[test]
    fn load_missing_model_returns_none_sessions() {
        let result = LocalSessions::load(Path::new("/nonexistent"), 4);
        // When no model files exist, all sessions should be None.
        // The load itself should succeed (no error) since missing files are OK.
        assert!(
            result.is_ok(),
            "loading from nonexistent dir should not fail; sessions just None"
        );
        let sessions = result.unwrap();
        assert!(sessions.decoder.is_none());
        assert!(sessions.cached_step.is_none());
        assert!(sessions.fixed_sampleed_frame.is_none());
    }

    #[test]
    fn is_ready_all_missing() {
        let sessions = LocalSessions::load(Path::new("/nonexistent"), 4).unwrap();
        assert!(!sessions.is_ready());
    }
}
