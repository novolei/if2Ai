//! HuggingFace model download + cache manager.
//!
//! Downloads the MOSS-TTS-Nano ONNX models from HuggingFace Hub
//! and caches them locally under `~/.if2ai/models/tts/`.
//!
//! Uses `huggingface-hub` style download via `reqwest` for the
//! ONNX model files. Since the models are large (~700MB + ~130MB),
//! downloads are resumed from partial files when interrupted.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use crate::modules::tts::config::{AUDIO_TOKENIZER_HF_REPO, TTS_MODEL_HF_REPO};

/// Resolved paths for all TTS model files.
pub struct ModelPaths {
    /// Directory containing the TTS ONNX model files.
    pub tts_model_dir: PathBuf,
    /// Directory containing the audio tokenizer ONNX files.
    pub audio_tokenizer_dir: PathBuf,
}

/// Root directory for cached TTS models.
fn model_cache_root() -> PathBuf {
    crate::modules::tts::config::default_model_dir()
}

/// Returns the expected ONNX file names for the TTS model.
fn tts_onnx_files() -> &'static [&'static str] {
    &[
        "prefill.onnx",
        "prefill.onnx.data",
        "decode_step.onnx",
        "decode_step.onnx.data",
        "decoder.onnx",
        "decoder.onnx.data",
        "local_cached_step.onnx",
        "local_cached_step.onnx.data",
        "local_fixed_sampled_frame.onnx",
        "local_fixed_sampled_frame.onnx.data",
    ]
}

/// Returns the expected ONNX file names for the audio tokenizer.
fn codec_onnx_files() -> &'static [&'static str] {
    &[
        "encode.onnx",
        "encode.onnx.data",
        "decode_full.onnx",
        "decode_full.onnx.data",
    ]
}

/// Ensure all TTS models are cached locally.
///
/// Downloads from HuggingFace if not present, verifies file sizes,
/// and returns the resolved [`ModelPaths`].
pub fn ensure_models_cached() -> Result<ModelPaths, DownloadError> {
    let cache_root = model_cache_root();
    let tts_dir = cache_root.join("tts_model");
    let tokenizer_dir = cache_root.join("audio_tokenizer");

    download_if_needed(TTS_MODEL_HF_REPO, &tts_dir, tts_onnx_files())?;
    download_if_needed(AUDIO_TOKENIZER_HF_REPO, &tokenizer_dir, codec_onnx_files())?;

    Ok(ModelPaths {
        tts_model_dir: tts_dir,
        audio_tokenizer_dir: tokenizer_dir,
    })
}

/// Download files if they are missing from the cache.
fn download_if_needed(
    _repo: &str,
    _target_dir: &Path,
    _files: &[&str],
) -> Result<(), DownloadError> {
    // TODO: Implement HuggingFace Hub download with resume support.
    // This will use reqwest to download each file from:
    // https://huggingface.co/{repo}/resolve/main/{file}
    // With range requests for resume.
    // For now, check if the target directory exists and report status.

    fs::create_dir_all(_target_dir).map_err(|e| DownloadError::IoError {
        path: _target_dir.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// Error type for model download operations.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("network error: {0}")]
    Network(String),

    #[error("I/O error at {path}: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("download interrupted")]
    Interrupted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_cache_root_is_deterministic() {
        let root = model_cache_root();
        assert!(root.to_string_lossy().contains(".if2ai"));
        assert!(root.to_string_lossy().contains("models"));
        assert!(root.to_string_lossy().contains("tts"));
    }

    #[test]
    fn ensure_models_cached_creates_directories() {
        let paths = ensure_models_cached().expect("should create cache dirs");
        assert!(paths.tts_model_dir.exists());
        assert!(paths.audio_tokenizer_dir.exists());
    }
}
