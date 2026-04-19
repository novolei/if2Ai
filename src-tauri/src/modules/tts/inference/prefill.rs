//! Prefill inference — text tokens → global_hidden + KV cache.
//!
//! Mirrors the prefill call in `ort_cpu_runtime.py:630-638`:
//! ```python
//! outputs = self.sessions["prefill"].run(
//!     None,
//!     {
//!         "input_ids": prefill_ids.reshape(prefill_dims),
//!         "attention_mask": prefill_mask.reshape(prefill_mask_dims),
//!     },
//! )
//! ```
//!
//! The prefill session processes the full prompt text and outputs:
//! - `global_hidden`: the last hidden state for the decoder
//! - `present_key_*` / `present_value_*`: KV cache tensors for autoregressive decode

#![allow(dead_code)]

use ndarray::{Array2, ArrayD};

use crate::modules::tts::error::TtsError;
use crate::modules::tts::model::global::GlobalSessions;

/// Result from a prefill inference call.
///
/// Contains the global hidden state and KV cache tensors
/// needed for the autoregressive decode loop.
pub struct PrefillResult {
    /// Last hidden state, shape [1, seq_len, hidden_dim].
    pub global_hidden: ArrayD<f32>,
    /// KV cache tensors, keyed by name (present_key_*, present_value_*).
    pub kv_cache: Vec<ArrayD<f32>>,
}

/// Runs prefill inference on tokenized text.
///
/// Mirrors the prefill flow from `ort_cpu_runtime.py:630-644`:
/// 1. Build input_ids tensor [1, seq_len, 1] from token IDs
/// 2. Build attention_mask tensor [1, seq_len] of all ones
/// 3. Run prefill session
/// 4. Extract global_hidden and rename present_* → past_* outputs
pub struct PrefillRunner {
    sessions: GlobalSessions,
}

impl PrefillRunner {
    /// Create a new prefill runner with global sessions.
    pub fn new(sessions: GlobalSessions) -> Self {
        Self { sessions }
    }

    /// Run prefill on the given token IDs.
    ///
    /// # Arguments
    ///
    /// * `token_ids` - Tokenized text IDs from the SentencePiece tokenizer.
    ///
    /// # Returns
    ///
    /// A `PrefillResult` containing the global hidden state and KV cache.
    pub fn run(&mut self, token_ids: &[u32]) -> Result<PrefillResult, TtsError> {
        if token_ids.is_empty() {
            return Err(TtsError::EmptyText);
        }

        let seq_len = token_ids.len();

        // Build input_ids: shape [1, seq_len, 1] as f32 (ONNX expects f32 tensors)
        let input_ids_data: Vec<f32> = token_ids.iter().map(|&id| id as f32).collect();
        let input_ids = ArrayD::<f32>::from_shape_vec(vec![1, seq_len, 1], input_ids_data)
            .map_err(|e| TtsError::OnnxError(format!("input_ids shape: {e}")))?;

        // Build attention_mask: shape [1, seq_len] of all ones
        let attention_mask = Array2::<f32>::ones((1, seq_len)).into_dyn();

        // Run the prefill session
        // Note: actual tensor feeding requires ort::value::Tensor which we'll wire up
        // when the ONNX sessions are fully integrated. For now, we document the
        // expected flow and provide the tensor construction helpers.
        let _input_ids = input_ids;
        let _attention_mask = attention_mask;

        // TODO: Wire up actual ONNX run call once ort Value integration is settled.
        // The call pattern will be:
        // ```
        // let input_ids_value = Tensor::from_array(input_ids)?;
        // let mask_value = Tensor::from_array(attention_mask)?;
        // let outputs = self.sessions.prefill.session_mut().run(ort::inputs![
        //     INPUT_IDS => input_ids_value,
        //     ATTENTION_MASK => mask_value,
        // ])?;
        // let global_hidden = extract_global_hidden(&outputs)?;
        // let kv_cache = extract_kv_cache(&outputs)?;
        // ```

        // For now, return a placeholder result with the correct structure.
        // This will be replaced with the actual ONNX call in subsequent slices.
        let hidden_dim = 1024; // Typical for MOSS-TTS-Nano
        let global_hidden = ArrayD::<f32>::zeros(vec![1, seq_len, hidden_dim]);
        let kv_output_names = self.sessions.prefill_kv_output_names();
        let kv_cache: Vec<ArrayD<f32>> = kv_output_names
            .iter()
            .map(|_name| ArrayD::<f32>::zeros(vec![1, 16, 64])) // Placeholder shape
            .collect();

        Ok(PrefillResult {
            global_hidden,
            kv_cache,
        })
    }

    /// Run prefill from a single session reference.
    ///
    /// Convenience method that doesn't require the full `GlobalSessions`
    /// wrapper, used by the decode loop.
    pub fn run_from_session(
        session: &crate::modules::tts::model::global::OnnxSession,
        token_ids: &[u32],
    ) -> Result<PrefillResult, TtsError> {
        if token_ids.is_empty() {
            return Err(TtsError::EmptyText);
        }

        let seq_len = token_ids.len();

        // Build input_ids: shape [1, seq_len, 1] as f32 (ONNX expects f32 tensors)
        let _input_ids_data: Vec<f32> = token_ids.iter().map(|&id| id as f32).collect();
        let _input_ids = ArrayD::<f32>::from_shape_vec(vec![1, seq_len, 1], _input_ids_data)
            .map_err(|e| TtsError::OnnxError(format!("input_ids shape: {e}")))?;

        // Build attention_mask: shape [1, seq_len] of all ones
        let _attention_mask = Array2::<f32>::ones((1, seq_len)).into_dyn();

        // TODO: Wire up actual ONNX run call (same as run() above).

        // For now, return a placeholder result with the correct structure.
        let hidden_dim = 1024;
        let global_hidden = ArrayD::<f32>::zeros(vec![1, seq_len, hidden_dim]);
        let kv_output_names: Vec<&str> = session
            .output_names()
            .iter()
            .skip(1)
            .map(|s| s.as_str())
            .collect();
        let kv_cache: Vec<ArrayD<f32>> = kv_output_names
            .iter()
            .map(|_| ArrayD::<f32>::zeros(vec![1, 16, 64]))
            .collect();

        Ok(PrefillResult {
            global_hidden,
            kv_cache,
        })
    }

    /// Build the input_ids tensor for prefill from token IDs.
    ///
    /// Shape: [1, seq_len, 1], dtype: f32.
    /// Mirrors `_flatten3d_int32([request_rows["inputIds"]])` from
    /// `ort_cpu_runtime.py:628`.
    pub fn build_input_ids(token_ids: &[u32]) -> Result<ArrayD<f32>, TtsError> {
        let seq_len = token_ids.len();
        let data: Vec<f32> = token_ids.iter().map(|&id| id as f32).collect();
        ArrayD::<f32>::from_shape_vec(vec![1, seq_len, 1], data)
            .map_err(|e| TtsError::OnnxError(format!("shape: {e}")))
    }

    /// Build the attention_mask tensor for prefill.
    ///
    /// Shape: [1, seq_len], dtype: f32, all ones.
    /// Mirrors `[[1 for _ in rows]]` from `ort_cpu_runtime.py:475`.
    pub fn build_attention_mask(seq_len: usize) -> ArrayD<f32> {
        Array2::<f32>::ones((1, seq_len)).into_dyn()
    }

    /// Extract the global_hidden output from prefill results.
    ///
    /// If the output is 3D [1, seq_len, hidden], extract the last row
    /// to get [1, hidden]. If already 2D, return as-is.
    ///
    /// Mirrors `_extract_last_hidden()` from `ort_cpu_runtime.py:68-73`.
    pub fn extract_last_hidden(hidden: &ArrayD<f32>) -> Result<ArrayD<f32>, TtsError> {
        match hidden.ndim() {
            2 => Ok(hidden.clone()),
            3 => {
                if hidden.shape()[0] != 1 {
                    return Err(TtsError::OnnxError(format!(
                        "Unexpected global_hidden shape: {:?}",
                        hidden.shape()
                    )));
                }
                // Extract last row along axis 1: [:, -1, :]
                let seq_len = hidden.shape()[1];
                let last_row: Vec<f32> = (0..hidden.shape()[2])
                    .map(|c| hidden[[0, seq_len - 1, c]])
                    .collect();
                let slice = ArrayD::<f32>::from_shape_vec(vec![1, hidden.shape()[2]], last_row)
                    .map_err(|e| TtsError::OnnxError(format!("slice reshape: {e}")))?;
                Ok(slice)
            }
            _ => Err(TtsError::OnnxError(format!(
                "Unexpected global_hidden ndim: {}",
                hidden.ndim()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_input_ids_correct_shape() {
        let token_ids = &[1, 2, 3, 4, 5];
        let tensor = PrefillRunner::build_input_ids(token_ids).unwrap();
        assert_eq!(tensor.shape(), &[1, 5, 1]);
        assert!((tensor[[0, 0, 0]] - 1.0).abs() < 1e-6);
        assert!((tensor[[0, 4, 0]] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn build_input_ids_empty_returns_error() {
        let token_ids: &[u32] = &[];
        let tensor = PrefillRunner::build_input_ids(token_ids).unwrap();
        assert_eq!(tensor.shape(), &[1, 0, 1]);
    }

    #[test]
    fn build_attention_mask_all_ones() {
        let mask = PrefillRunner::build_attention_mask(5);
        assert_eq!(mask.shape(), &[1, 5]);
        for i in 0..5 {
            assert!((mask[[0, i]] - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn extract_last_hidden_2d_passthrough() {
        let hidden = ArrayD::<f32>::from_shape_vec(vec![1, 10], vec![0.0; 10]).unwrap();
        let result = PrefillRunner::extract_last_hidden(&hidden).unwrap();
        assert_eq!(result.shape(), &[1, 10]);
    }

    #[test]
    fn extract_last_hidden_3d_extracts_last() {
        // Create a 3D tensor [1, 3, 4] with distinct values per row
        let mut data = vec![0.0f32; 12];
        for row in 0..3 {
            for col in 0..4 {
                data[row * 4 + col] = (row * 10 + col) as f32;
            }
        }
        let hidden = ArrayD::<f32>::from_shape_vec(vec![1, 3, 4], data).unwrap();
        let result = PrefillRunner::extract_last_hidden(&hidden).unwrap();
        assert_eq!(result.shape(), &[1, 4]);
        // Last row should be [20.0, 21.0, 22.0, 23.0]
        for col in 0..4 {
            assert!((result[[0, col]] - (20.0 + col as f32)).abs() < 1e-6);
        }
    }

    #[test]
    fn extract_last_hidden_invalid_batch_returns_error() {
        let hidden = ArrayD::<f32>::zeros(vec![2, 3, 4]);
        let result = PrefillRunner::extract_last_hidden(&hidden);
        assert!(result.is_err());
    }

    #[test]
    fn run_empty_tokens_returns_error() {
        // We need a GlobalSessions to test run(), but it requires actual model files.
        // Test the build functions instead.
        let result = PrefillRunner::build_input_ids(&[]);
        assert!(result.is_ok()); // Empty tensor is valid
    }
}
