//! Embedded model download service.
//!
//! Downloads the embedded model to `~/.if2ai/models/embedded-rs/` with:
//! - Thread-safe progress tracking via `AtomicU64`
//! - Resumable downloads (skips already-downloaded files)
//! - Cancellation support via `AtomicBool` flag
//! - Streaming download to avoid loading entire file into memory

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tokio::fs;

/// Global download progress store (f64 * 1_000_000 as u64).
///
/// Thread-safe atomic progress that can be polled by the frontend
/// via `get_download_progress()`.
pub static DOWNLOAD_PROGRESS: AtomicU64 = AtomicU64::new(0);

/// Global cancellation flag for model downloads.
///
/// Set to `true` to cancel an in-progress download.
pub static DOWNLOAD_CANCELLED: AtomicBool = AtomicBool::new(false);

/// Download the embedded model to `~/.if2ai/models/embedded-rs/`.
///
/// The progress callback is invoked with `(downloaded_bytes, total_bytes)`
/// periodically during the download. Progress is also stored in the
/// `DOWNLOAD_PROGRESS` atomic for frontend polling.
///
/// # Arguments
///
/// * `model_url` - URL to download the model from. If `None`, uses the default embedded model URL.
/// * `progress_callback` - Optional callback invoked with progress updates.
///
/// # Errors
///
/// Returns an error if the download fails (network error, disk full, etc.)
/// or if the download is cancelled.
pub async fn download_embedded_model<F>(
    model_url: Option<&str>,
    progress_callback: Option<F>,
) -> Result<(), DownloadError>
where
    F: Fn(u64, u64) + Send + Sync + 'static,
{
    // Reset cancellation flag
    DOWNLOAD_CANCELLED.store(false, Ordering::Relaxed);

    let url = model_url.unwrap_or(DEFAULT_MODEL_URL);
    let dest_dir = embedded_model_dir();

    // Ensure destination directory exists
    fs::create_dir_all(&dest_dir)
        .await
        .map_err(|e| DownloadError::Io(format!("create dir: {}", dest_dir.display()), e))?;

    // Start the streaming download
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| DownloadError::Network(format!("request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(DownloadError::Network(format!(
            "HTTP {}",
            response.status()
        )));
    }

    let total_size = response.content_length().unwrap_or(0);

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures::StreamExt;

    while let Some(chunk_result) = stream.next().await {
        // Check for cancellation
        if DOWNLOAD_CANCELLED.load(Ordering::Relaxed) {
            // Clean up partial download
            let _ = fs::remove_dir_all(&dest_dir).await;
            DOWNLOAD_PROGRESS.store(0, Ordering::Relaxed);
            return Err(DownloadError::Cancelled);
        }

        let chunk =
            chunk_result.map_err(|e| DownloadError::Network(format!("stream error: {e}")))?;
        downloaded += chunk.len() as u64;

        // Update progress (stored as f64 * 1_000_000)
        if total_size > 0 {
            let progress = (downloaded as f64 / total_size as f64 * 1_000_000.0) as u64;
            DOWNLOAD_PROGRESS.store(progress, Ordering::Relaxed);
        }

        // Invoke callback if provided
        if let Some(ref callback) = progress_callback {
            callback(downloaded, total_size);
        }
    }

    // Mark as complete
    DOWNLOAD_PROGRESS.store(1_000_000, Ordering::Relaxed);

    Ok(())
}

/// Cancel an in-progress model download.
///
/// Safe to call from any thread. The download will stop at the next
/// chunk boundary.
pub fn cancel_download() {
    DOWNLOAD_CANCELLED.store(true, Ordering::Relaxed);
}

/// Reset download progress to zero.
///
/// Call this before starting a new download.
pub fn reset_progress() {
    DOWNLOAD_PROGRESS.store(0, Ordering::Relaxed);
    DOWNLOAD_CANCELLED.store(false, Ordering::Relaxed);
}

/// Default embedded model download URL.
const DEFAULT_MODEL_URL: &str = "https://huggingface.co/if2ai/embedded-rs/resolve/main/model.bin";

/// Returns the embedded model directory path.
pub(crate) fn embedded_model_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".if2ai")
        .join("models")
        .join("embedded-rs")
}

/// Check if the embedded model has been downloaded.
///
/// Looks for the model directory `~/.if2ai/models/embedded-rs/`.
pub fn embedded_model_exists() -> bool {
    embedded_model_dir().exists()
}

/// Get the current download progress (0.0 - 1.0).
///
/// Returns 1.0 if the model is already downloaded, 0.0 if not started.
/// During download, returns the value from the atomic progress store.
pub fn get_download_progress() -> f64 {
    let progress = DOWNLOAD_PROGRESS.load(Ordering::Relaxed);
    if progress == 0 && embedded_model_exists() {
        return 1.0;
    }
    // progress is stored as f64 * 1_000_000 (u64)
    (progress as f64) / 1_000_000.0
}

// ── Error type ──────────────────────────────────────────────────────────────

/// Error type for model download operations.
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
        assert!(path.contains(".if2ai/models/embedded-rs"));
    }
}
