//! Global ONNX sessions for the TTS model.
//!
//! Mirrors `_create_sessions()` from `ort_cpu_runtime.py` for the
//! `prefill` and `decode` (decode_step) sessions.
//!
//! These two sessions share the same ONNX file directory and are
//! loaded with identical session options (CPUExecutionProvider,
//! graph optimization, thread count).
//!
//! ## Session Reference
//!
//! | Session      | Python Key | Input Names                                    | Output Names                                    |
//! |--------------|------------|------------------------------------------------|-------------------------------------------------|
//! | **prefill**  | prefill    | input_ids [1,seq_len,1], attention_mask [1,seq_len] | global_hidden, present_key_*, present_value_* |
//! | **decode**   | decode     | input_ids [1,1,1], past_valid_lengths, past_key_*, past_value_* | global_hidden, present_key_*, present_value_* |
//!
//! KV cache tensors flow from prefill outputs → decode inputs,
//! renamed from `present_*` to `past_*` between passes.

#![allow(dead_code)]

use std::path::Path;

use ort::session::{builder::GraphOptimizationLevel, Session};

use crate::modules::tts::error::TtsError;

/// Name of the prefill ONNX file (without extension).
pub const PREFILL_ONNX: &str = "prefill";

/// Name of the decode_step ONNX file (without extension).
pub const DECODE_STEP_ONNX: &str = "decode_step";

/// Input name for token IDs (shared by prefill and decode).
pub const INPUT_IDS: &str = "input_ids";

/// Input name for attention mask (prefill).
pub const ATTENTION_MASK: &str = "attention_mask";

/// Input name for past valid lengths (decode_step).
pub const PAST_VALID_LENGTHS: &str = "past_valid_lengths";

/// Output name for the last hidden state (shared by prefill and decode).
pub const GLOBAL_HIDDEN: &str = "global_hidden";

/// Prefix for KV cache output tensors (renamed to past_* for next decode step).
pub const PRESENT_KEY_PREFIX: &str = "present_key_";
pub const PRESENT_VALUE_PREFIX: &str = "present_value_";
pub const PAST_KEY_PREFIX: &str = "past_key_";
pub const PAST_VALUE_PREFIX: &str = "past_value_";

/// Default thread count for ONNX CPU inference.
pub const DEFAULT_THREAD_COUNT: usize = 4;

/// Renames a prefill/decode `present_*` output name to the corresponding `past_*` decode input name.
///
/// Mirrors `output_name.replace("present_", "past_")` from `ort_cpu_runtime.py:642,781`.
pub fn present_to_past(name: &str) -> String {
    name.replacen("present_", "past_", 1)
}

/// Resolved ONNX session with its I/O metadata.
///
/// Mirrors an `ort.InferenceSession` from the Python runtime,
/// plus the cached input/output names for tensor feed construction.
pub struct OnnxSession {
    session: Session,
    /// Input tensor names in order (cached from session.inputs).
    input_names: Vec<String>,
    /// Output tensor names in order (cached from session.outputs).
    output_names: Vec<String>,
}

impl OnnxSession {
    /// Load an ONNX session from a file path with the standard CPU config.
    ///
    /// Session options mirror `_session()` from `ort_cpu_runtime.py`:
    /// - CPUExecutionProvider (default, no explicit registration needed)
    /// - ORT_ENABLE_ALL graph optimization
    /// - intra_op_num_threads = thread_count
    /// - inter_op_num_threads = 1
    pub fn load(model_path: &Path, thread_count: usize) -> Result<Self, TtsError> {
        let session = Session::builder()
            .map_err(|e| TtsError::OnnxError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| TtsError::OnnxError(e.to_string()))?
            .with_inter_threads(1)
            .map_err(|e| TtsError::OnnxError(e.to_string()))?
            .with_intra_threads(thread_count)
            .map_err(|e| TtsError::OnnxError(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e| TtsError::OnnxError(e.to_string()))?;

        let input_names = session.inputs.iter().map(|i| i.name.clone()).collect();
        let output_names = session.outputs.iter().map(|o| o.name.clone()).collect();

        Ok(Self {
            session,
            input_names,
            output_names,
        })
    }

    /// Get a mutable reference to the inner session for running inference.
    ///
    /// Callers should use the `ort::inputs!` macro to construct inputs:
    /// ```ignore
    /// let outputs = session.session_mut().run(ort::inputs![
    ///     "input_ids" => Tensor::from_array(([1, 3], vec![1, 2, 3]))?,
    /// ])?;
    /// ```
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Get input tensor names (cached from model loading).
    pub fn input_names(&self) -> &[String] {
        &self.input_names
    }

    /// Get output tensor names (cached from model loading).
    pub fn output_names(&self) -> &[String] {
        &self.output_names
    }
}

/// Global ONNX sessions — prefill and decode_step.
///
/// These sessions are shared across all synthesis requests and
/// are initialized once at warmup time.
///
/// Mirrors `self.sessions["prefill"]` and `self.sessions["decode"]`
/// from `OrtCpuRuntime` in `ort_cpu_runtime.py`.
pub struct GlobalSessions {
    /// Text-to-speech prefill session: input_ids → global_hidden + KV cache.
    pub prefill: OnnxSession,
    /// Autoregressive decode step: single token → logits + updated KV cache.
    pub decode: OnnxSession,
}

impl GlobalSessions {
    /// Create global sessions from the TTS model directory.
    ///
    /// Loads `prefill.onnx` and `decode_step.onnx` (or their `.onnx.data`
    /// external-weight variants) with the specified thread count.
    ///
    /// Mirrors `_create_sessions()` from `ort_cpu_runtime.py:354-379`
    /// for the "prefill" and "decode" entries.
    pub fn load(tts_model_dir: &Path, thread_count: usize) -> Result<Self, TtsError> {
        let prefill_path = tts_model_dir.join("prefill.onnx");
        let decode_path = tts_model_dir.join("decode_step.onnx");

        if !prefill_path.exists() {
            return Err(TtsError::ModelNotFound(format!(
                "prefill.onnx not found at {}",
                prefill_path.display()
            )));
        }
        if !decode_path.exists() {
            return Err(TtsError::ModelNotFound(format!(
                "decode_step.onnx not found at {}",
                decode_path.display()
            )));
        }

        let prefill = OnnxSession::load(&prefill_path, thread_count)?;
        let decode = OnnxSession::load(&decode_path, thread_count)?;

        Ok(Self { prefill, decode })
    }

    /// Dump session I/O names for verification against Python reference.
    ///
    /// Used to verify that input/output names match the Python
    /// `ort_cpu_runtime.py` expectations.
    pub fn dump_io_names(&self) {
        tracing::info!("prefill inputs: {:?}", self.prefill.input_names());
        tracing::info!("prefill outputs: {:?}", self.prefill.output_names());
        tracing::info!("decode inputs: {:?}", self.decode.input_names());
        tracing::info!("decode outputs: {:?}", self.decode.output_names());
    }

    /// Get the KV cache output names from the prefill session.
    ///
    /// These are all output names after `global_hidden` (index 0).
    /// Each `present_key_*` / `present_value_*` becomes a `past_key_*` /
    /// `past_value_*` input for the decode session.
    pub fn prefill_kv_output_names(&self) -> Vec<&str> {
        self.prefill
            .output_names()
            .iter()
            .skip(1)
            .map(|s| s.as_str())
            .collect()
    }

    /// Get the KV cache input names for the decode session.
    ///
    /// These are all input names after `input_ids` and `past_valid_lengths`
    /// (indices 0 and 1).
    pub fn decode_kv_input_names(&self) -> Vec<&str> {
        self.decode
            .input_names()
            .iter()
            .skip(2)
            .map(|s| s.as_str())
            .collect()
    }

    /// Get the KV cache output names from the decode session.
    ///
    /// These are all output names after `global_hidden` (index 0).
    pub fn decode_kv_output_names(&self) -> Vec<&str> {
        self.decode
            .output_names()
            .iter()
            .skip(1)
            .map(|s| s.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onnx_session_constants() {
        assert_eq!(INPUT_IDS, "input_ids");
        assert_eq!(ATTENTION_MASK, "attention_mask");
        assert_eq!(PAST_VALID_LENGTHS, "past_valid_lengths");
        assert_eq!(GLOBAL_HIDDEN, "global_hidden");
    }

    #[test]
    fn kv_name_prefixes() {
        assert!(PRESENT_KEY_PREFIX.starts_with("present_key_"));
        assert!(PRESENT_VALUE_PREFIX.starts_with("present_value_"));
        assert!(PAST_KEY_PREFIX.starts_with("past_key_"));
        assert!(PAST_VALUE_PREFIX.starts_with("past_value_"));
    }

    #[test]
    fn rename_present_to_past() {
        let key = "present_key_0";
        let past = present_to_past(key);
        assert_eq!(past, "past_key_0");

        let value = "present_value_12";
        let past = present_to_past(value);
        assert_eq!(past, "past_value_12");
    }

    #[test]
    fn load_missing_model_returns_error() {
        let result = GlobalSessions::load(Path::new("/nonexistent"), DEFAULT_THREAD_COUNT);
        assert!(
            result.is_err(),
            "loading from nonexistent dir should fail with ModelNotFound"
        );
    }
}
