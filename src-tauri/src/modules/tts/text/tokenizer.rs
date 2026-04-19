//! SentencePiece text tokenizer for the TTS model.
//!
//! Mirrors `spm.SentencePieceProcessor` from the Python reference
//! (`onnx_tts_runtime.py:304,317-321`).
//!
//! The SentencePiece model (`tokenizer.model`) is bundled with the
//! TTS ONNX model files and loaded at warmup time.
//!
//! ## Usage
//!
//! ```ignore
//! let tokenizer = TtsTokenizer::load("/path/to/tokenizer.model")?;
//! let tokens: Vec<u32> = tokenizer.encode("你好，世界")?;
//! let text: String = tokenizer.decode(&tokens)?;
//! let count: usize = tokenizer.count_tokens("你好")?;
//! ```

// Justification: TtsTokenizer methods are consumed by TTS-3.x inference
// slices and TTS-4.x voice clone pipeline.
#![allow(dead_code)]

use std::path::Path;

use tokenizers::Tokenizer as TokenizerImpl;

use crate::modules::tts::error::TtsError;

/// Default filename for the SentencePiece tokenizer model.
pub const TOKENIZER_MODEL_FILE: &str = "tokenizer.model";

/// Text tokenizer — wraps a SentencePiece model for encoding/decoding.
///
/// Mirrors `spm.SentencePieceProcessor` from `onnx_tts_runtime.py:304`.
pub struct TtsTokenizer {
    tokenizer: TokenizerImpl,
}

impl TtsTokenizer {
    /// Load a SentencePiece tokenizer from a `tokenizer.model` file.
    ///
    /// Mirrors `spm.SentencePieceProcessor(model_file=str(tokenizer_path))`
    /// from `onnx_tts_runtime.py:304`.
    pub fn load(model_path: &Path) -> Result<Self, TtsError> {
        if !model_path.exists() {
            return Err(TtsError::ModelNotFound(format!(
                "tokenizer.model not found at {}",
                model_path.display()
            )));
        }

        let tokenizer = TokenizerImpl::from_file(model_path)
            .map_err(|e| TtsError::TokenizationError(e.to_string()))?;

        Ok(Self { tokenizer })
    }

    /// Load from the default path inside the TTS model directory.
    ///
    /// Looks for `tokenizer.model` in the given directory.
    pub fn from_model_dir(tts_model_dir: &Path) -> Result<Self, TtsError> {
        let model_path = tts_model_dir.join(TOKENIZER_MODEL_FILE);
        Self::load(&model_path)
    }

    /// Encode text into a sequence of token IDs.
    ///
    /// Mirrors `sp_model.encode(text, out_type=int)` from
    /// `onnx_tts_runtime.py:318`.
    pub fn encode(&self, text: &str) -> Result<Vec<u32>, TtsError> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| TtsError::TokenizationError(e.to_string()))?;

        Ok(encoding.get_ids().to_vec())
    }

    /// Decode a sequence of token IDs back into text.
    ///
    /// Mirrors `sp_model.decode(token_ids)` from the Python reference.
    pub fn decode(&self, tokens: &[u32]) -> Result<String, TtsError> {
        self.tokenizer
            .decode(tokens, true)
            .map_err(|e| TtsError::TokenizationError(e.to_string()))
    }

    /// Count the number of tokens in the given text.
    ///
    /// Mirrors `count_text_tokens()` from `onnx_tts_runtime.py:320-321`.
    pub fn count_tokens(&self, text: &str) -> Result<usize, TtsError> {
        self.encode(text).map(|ids| ids.len())
    }

    /// Get the vocabulary size of the tokenizer.
    pub fn vocab_size(&self) -> usize {
        self.tokenizer.get_vocab_size(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_model_returns_error() {
        let result = TtsTokenizer::load(Path::new("/nonexistent/tokenizer.model"));
        assert!(result.is_err());
    }

    #[test]
    fn from_model_dir_missing_dir_returns_error() {
        let result = TtsTokenizer::from_model_dir(Path::new("/nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn tokenizer_model_constant() {
        assert_eq!(TOKENIZER_MODEL_FILE, "tokenizer.model");
    }
}
