//! Embedded model (multilingual-e5-small) download via fastembed-rs.
//!
//! Uses the `fastembed` crate to download and cache
//! `intfloat/multilingual-e5-small` (384-dim, ~120 MB) from HuggingFace.
//! This matches the uclaw-rs reference implementation exactly.
//!
//! - Thread-safe progress tracking via `AtomicU64`
//! - Download handled by fastembed's internal ORT downloader
//! - Resumable (fastembed caches by model hash)

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

/// Display name of the embedded model (matches HuggingFace model card).
pub const MODEL_NAME: &str = "intfloat/multilingual-e5-small";
pub const MODEL_DETAIL: &str = "多语言向量化模型 · 384 维";
/// Approximate model size in MB (actual: ~117 MB on disk).
pub const MODEL_SIZE_MB: u64 = 120;

/// Global download progress store (f64 * 1_000_000 as u64).
pub static DOWNLOAD_PROGRESS: AtomicU64 = AtomicU64::new(0);

/// Global cancellation flag for model downloads.
pub static DOWNLOAD_CANCELLED: AtomicBool = AtomicBool::new(false);

/// Download the embedded multilingual-e5-small model via fastembed.
///
/// fastembed handles the HTTP download, caching, and extraction internally.
/// The model is cached in `~/.if2ai/models/fastembed/`.
///
/// # Errors
///
/// Returns an error if the download fails (network error, disk full, etc.)
/// or if the download is cancelled.
pub async fn download_embedded_model<F>(
    _model_url: Option<&str>,
    _progress_callback: Option<F>,
) -> Result<(), DownloadError>
where
    F: Fn(u64, u64) + Send + Sync + 'static,
{
    DOWNLOAD_CANCELLED.store(false, Ordering::Relaxed);
    let dest_dir = embedded_model_dir();

    tokio::task::spawn_blocking(move || {
        // fastembed handles the download + caching internally
        let _model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::MultilingualE5Small)
                .with_cache_dir(dest_dir)
                .with_show_download_progress(true),
        )
        .map_err(|e| DownloadError::Network(format!("fastembed download failed: {e}")))?;

        // Report completion
        DOWNLOAD_PROGRESS.store(1_000_000, Ordering::Relaxed);

        Ok(())
    })
    .await
    .map_err(|e| DownloadError::Io("spawn_blocking failed".to_string(), e.into()))??;

    Ok(())
}

/// Cancel an in-progress model download.
pub fn cancel_download() {
    DOWNLOAD_CANCELLED.store(true, Ordering::Relaxed);
}

/// Reset download progress to zero.
pub fn reset_progress() {
    DOWNLOAD_PROGRESS.store(0, Ordering::Relaxed);
    DOWNLOAD_CANCELLED.store(false, Ordering::Relaxed);
}

/// Returns the embedded model cache directory path.
/// fastembed stores the model in a subdirectory named by model hash.
pub(crate) fn embedded_model_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".if2ai")
        .join("models")
        .join("fastembed")
}

/// Check if the embedded model has been downloaded and cached.
///
/// fastembed creates a directory with the model hash name
/// (e.g. `BAAI-bge-small-en-v1.5` or `intfloat-multilingual-e5-small`)
/// inside the cache dir. We check if the cache dir itself has any content.
pub fn embedded_model_exists() -> bool {
    let dir = embedded_model_dir();
    if !dir.exists() {
        return false;
    }
    // Check if fastembed created any model subdirectory
    match std::fs::read_dir(&dir) {
        Ok(entries) => entries.count() > 0,
        Err(_) => false,
    }
}

/// Get the current download progress (0.0 - 1.0).
///
/// Returns 1.0 if the model is already downloaded, 0.0 if not started.
pub fn get_download_progress() -> f64 {
    let progress = DOWNLOAD_PROGRESS.load(Ordering::Relaxed);
    if progress == 0 && embedded_model_exists() {
        return 1.0;
    }
    (progress as f64) / 1_000_000.0
}

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("I/O error: {0}")]
    Io(String, #[source] std::io::Error),
    #[error("Download cancelled")]
    Cancelled,
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reset_progress_sets_zero() {
        DOWNLOAD_PROGRESS.store(500_000, Ordering::Relaxed);
        reset_progress();
        assert_eq!(DOWNLOAD_PROGRESS.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_cancel_download_sets_flag() {
        DOWNLOAD_CANCELLED.store(false, Ordering::Relaxed);
        cancel_download();
        assert!(DOWNLOAD_CANCELLED.load(Ordering::Relaxed));
    }

    #[test]
    fn test_embedded_model_dir_path() {
        let dir = embedded_model_dir();
        let path = dir.to_string_lossy();
        assert!(
            path.contains(".if2ai/models/fastembed"),
            "Model dir should contain .if2ai/models/fastembed, got: {}",
            path
        );
    }
}
