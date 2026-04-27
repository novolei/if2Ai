//! Embedded model (multilingual-e5-small) download via fastembed-rs.
//!
//! Uses the `fastembed` crate to download and cache
//! `intfloat/multilingual-e5-small` (384-dim, ~487 MB) from HuggingFace.
//! This matches the uclaw-rs reference implementation exactly.
//!
//! - Thread-safe progress tracking via `AtomicU64`
//! - Download handled by fastembed's internal ORT downloader
//! - Resumable (fastembed caches by model hash)

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

/// Display name of the embedded model (matches HuggingFace model card).
pub const MODEL_NAME: &str = "intfloat/multilingual-e5-small";
pub const MODEL_DETAIL: &str = "多语言向量化模型 · 384 维";
/// Approximate model size in MB for the current FastEmbed ONNX cache.
pub const MODEL_SIZE_MB: u64 = 487;
const MODEL_SIZE_BYTES: u64 = MODEL_SIZE_MB * 1024 * 1024;
const PROGRESS_SCALE: u64 = 1_000_000;

/// Global download progress store (f64 * 1_000_000 as u64).
pub static DOWNLOAD_PROGRESS: AtomicU64 = AtomicU64::new(0);

/// Whether a download task is currently active.
pub static DOWNLOAD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Global cancellation flag for model downloads.
pub static DOWNLOAD_CANCELLED: AtomicBool = AtomicBool::new(false);

static DOWNLOAD_LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Start the embedded model download in the background.
///
/// This command path is used by onboarding and settings. It must return quickly
/// so the UI can keep progressing while FastEmbed performs the network download
/// and ONNX initialization on a blocking worker thread.
///
/// # Errors
///
/// Returns an error if the download task cannot be started.
pub fn start_embedded_model_download(app: Option<tauri::AppHandle>) -> Result<(), DownloadError> {
    if embedded_model_exists() {
        DOWNLOAD_PROGRESS.store(PROGRESS_SCALE, Ordering::Relaxed);
        clear_last_download_error();
        emit_progress_if_possible(app.as_ref(), MODEL_SIZE_BYTES, MODEL_SIZE_BYTES);
        return Ok(());
    }

    if DOWNLOAD_IN_PROGRESS.swap(true, Ordering::Relaxed) {
        return Ok(());
    }

    let dest_dir = prepare_download_state();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_download_after_claim(app, dest_dir).await {
            set_last_download_error(error.to_string());
        }
    });

    Ok(())
}

/// Download the embedded multilingual-e5-small model via fastembed.
///
/// fastembed handles the HTTP download, caching, and extraction internally.
/// The model is cached in `~/.if2ai/models/fastembed/`.
///
/// # Errors
///
/// Returns an error if the download fails (network error, disk full, etc.)
/// or if the download is cancelled.
pub async fn download_embedded_model(app: Option<tauri::AppHandle>) -> Result<(), DownloadError> {
    if embedded_model_exists() {
        DOWNLOAD_PROGRESS.store(PROGRESS_SCALE, Ordering::Relaxed);
        clear_last_download_error();
        emit_progress_if_possible(app.as_ref(), MODEL_SIZE_BYTES, MODEL_SIZE_BYTES);
        return Ok(());
    }

    if DOWNLOAD_IN_PROGRESS.swap(true, Ordering::Relaxed) {
        return Ok(());
    }

    let dest_dir = prepare_download_state();
    run_download_after_claim(app, dest_dir).await
}

fn prepare_download_state() -> PathBuf {
    DOWNLOAD_CANCELLED.store(false, Ordering::Relaxed);
    clear_last_download_error();
    let dest_dir = embedded_model_cache_dir();
    DOWNLOAD_PROGRESS.store(0, Ordering::Relaxed);
    update_progress_from_cache_size(&dest_dir);
    dest_dir
}

async fn run_download_after_claim(
    app: Option<tauri::AppHandle>,
    dest_dir: PathBuf,
) -> Result<(), DownloadError> {
    let monitor_dir = dest_dir.clone();
    let monitor_app = app.clone();
    let monitor = tokio::spawn(async move {
        while is_download_in_progress() {
            let downloaded = update_progress_from_cache_size(&monitor_dir);
            emit_progress_if_possible(monitor_app.as_ref(), downloaded, MODEL_SIZE_BYTES);
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    let result = tokio::task::spawn_blocking(move || {
        // fastembed handles the download + caching internally
        let _model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::MultilingualE5Small)
                .with_cache_dir(dest_dir)
                .with_show_download_progress(true),
        )
        .map_err(|e| DownloadError::Network(format!("fastembed download failed: {e}")))?;

        // Report completion
        DOWNLOAD_PROGRESS.store(PROGRESS_SCALE, Ordering::Relaxed);

        Ok::<(), DownloadError>(())
    })
    .await
    .map_err(|e| DownloadError::Io("spawn_blocking failed".to_string(), e.into()));

    DOWNLOAD_IN_PROGRESS.store(false, Ordering::Relaxed);
    monitor.abort();

    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            set_last_download_error(error.to_string());
            return Err(error);
        }
        Err(error) => {
            set_last_download_error(error.to_string());
            return Err(error);
        }
    }

    DOWNLOAD_PROGRESS.store(PROGRESS_SCALE, Ordering::Relaxed);
    clear_last_download_error();
    emit_progress_if_possible(app.as_ref(), MODEL_SIZE_BYTES, MODEL_SIZE_BYTES);

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
    DOWNLOAD_IN_PROGRESS.store(false, Ordering::Relaxed);
    clear_last_download_error();
}

/// Returns whether a model download is actively running.
pub fn is_download_in_progress() -> bool {
    DOWNLOAD_IN_PROGRESS.load(Ordering::Relaxed)
}

/// Return the last terminal download error, if any.
pub fn last_download_error() -> Option<String> {
    DOWNLOAD_LAST_ERROR
        .lock()
        .map(|error| error.clone())
        .unwrap_or(None)
}

fn set_last_download_error(message: String) {
    if let Ok(mut error) = DOWNLOAD_LAST_ERROR.lock() {
        *error = Some(message);
    }
}

fn clear_last_download_error() {
    if let Ok(mut error) = DOWNLOAD_LAST_ERROR.lock() {
        *error = None;
    }
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

/// Return the FastEmbed cache directory the app should use at runtime.
///
/// New installs use the stable If2Ai-owned cache directory. Existing dev/user
/// machines may already have a complete FastEmbed cache under `.fastembed_cache`
/// because older memory code used fastembed's process-working-directory default;
/// in that case, reuse it instead of forcing a duplicate 487 MB download.
pub(crate) fn embedded_model_cache_dir() -> PathBuf {
    let canonical = embedded_model_dir();
    if embedded_model_exists_in(&canonical) {
        return canonical;
    }

    legacy_fastembed_cache_dirs()
        .into_iter()
        .find(|dir| embedded_model_exists_in(dir))
        .unwrap_or(canonical)
}

/// Check if the embedded model has been downloaded and cached.
///
/// fastembed creates a directory with the model hash name
/// (e.g. `BAAI-bge-small-en-v1.5` or `intfloat-multilingual-e5-small`)
/// inside the cache dir. We check if the cache dir itself has any content.
pub fn embedded_model_exists() -> bool {
    embedded_model_exists_in(&embedded_model_dir())
        || legacy_fastembed_cache_dirs()
            .iter()
            .any(|dir| embedded_model_exists_in(dir))
}

fn legacy_fastembed_cache_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join(".fastembed_cache"));
        dirs.push(cwd.join("src-tauri").join(".fastembed_cache"));
        if let Some(parent) = cwd.parent() {
            dirs.push(parent.join(".fastembed_cache"));
        }
    }
    dirs
}

fn embedded_model_exists_in(dir: &Path) -> bool {
    if !dir.exists() {
        return false;
    }

    fastembed_snapshot_complete(dir) || legacy_flat_cache_complete(dir)
}

fn fastembed_snapshot_complete(cache_dir: &Path) -> bool {
    let repo_dir = cache_dir.join("models--intfloat--multilingual-e5-small");
    let refs_main = repo_dir.join("refs").join("main");
    let Ok(revision) = std::fs::read_to_string(refs_main) else {
        return false;
    };
    let snapshot = repo_dir.join("snapshots").join(revision.trim());
    required_snapshot_artifacts()
        .iter()
        .all(|relative| is_non_empty_file(&snapshot.join(relative)))
}

fn required_snapshot_artifacts() -> [&'static str; 5] {
    [
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
        "onnx/model.onnx",
    ]
}

fn legacy_flat_cache_complete(dir: &Path) -> bool {
    has_artifact_recursive(dir, "model.onnx", 0)
        && has_artifact_recursive(dir, "tokenizer.json", 0)
        && has_artifact_recursive(dir, "config.json", 0)
}

fn has_artifact_recursive(dir: &Path, target_name: &str, depth: usize) -> bool {
    if depth > 6 {
        return false;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };

    entries.filter_map(Result::ok).any(|entry| {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name.starts_with('.') || name.ends_with(".lock") {
            return false;
        }

        let path = entry.path();
        if path.is_file() {
            return name == target_name && is_non_empty_file(&path);
        }
        path.is_dir() && has_artifact_recursive(&path, target_name, depth + 1)
    })
}

#[cfg(test)]
fn is_model_artifact_name(name: &str) -> bool {
    name.ends_with(".onnx")
        || name.ends_with(".json")
        || name.ends_with(".txt")
        || name.ends_with(".safetensors")
        || name.ends_with(".bin")
        || name == "model.onnx"
        || name == "tokenizer.json"
        || name == "config.json"
}

fn is_non_empty_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn update_progress_from_cache_size(dir: &Path) -> u64 {
    let downloaded = cache_size_bytes(dir).min(MODEL_SIZE_BYTES);
    let scaled = downloaded.saturating_mul(PROGRESS_SCALE) / MODEL_SIZE_BYTES;
    DOWNLOAD_PROGRESS.fetch_max(scaled, Ordering::Relaxed);
    downloaded
}

fn cache_size_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name.ends_with(".lock") {
                return 0;
            }

            let path = entry.path();
            if path.is_dir() {
                cache_size_bytes(&path)
            } else {
                path.metadata().map(|metadata| metadata.len()).unwrap_or(0)
            }
        })
        .sum()
}

fn emit_progress_if_possible(
    app: Option<&tauri::AppHandle>,
    downloaded_bytes: u64,
    total_bytes: u64,
) {
    if let Some(app) = app {
        crate::modules::onboarding::events::emit_download_progress(
            app,
            MODEL_NAME,
            downloaded_bytes.min(total_bytes),
            total_bytes,
        );
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
    (progress as f64) / (PROGRESS_SCALE as f64)
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
    use std::sync::{Mutex, MutexGuard};

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn test_lock() -> MutexGuard<'static, ()> {
        TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn test_reset_progress_sets_zero() {
        let _guard = test_lock();
        DOWNLOAD_PROGRESS.store(500_000, Ordering::Relaxed);
        reset_progress();
        assert_eq!(DOWNLOAD_PROGRESS.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_cancel_download_sets_flag() {
        let _guard = test_lock();
        DOWNLOAD_CANCELLED.store(false, Ordering::Relaxed);
        cancel_download();
        assert!(DOWNLOAD_CANCELLED.load(Ordering::Relaxed));
    }

    #[test]
    fn test_reset_progress_clears_in_progress_flag() {
        let _guard = test_lock();
        DOWNLOAD_IN_PROGRESS.store(true, Ordering::Relaxed);
        reset_progress();
        assert!(!is_download_in_progress());
    }

    #[test]
    fn test_reset_progress_clears_last_error() {
        let _guard = test_lock();
        set_last_download_error("network failed".to_string());
        assert!(last_download_error().is_some());
        reset_progress();
        assert!(last_download_error().is_none());
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

    #[test]
    fn test_model_artifact_filter_ignores_finder_metadata() {
        assert!(!is_model_artifact_name(".DS_Store"));
        assert!(is_model_artifact_name("model.onnx"));
        assert!(is_model_artifact_name("tokenizer.json"));
    }

    #[test]
    fn test_fastembed_snapshot_layout_is_detected() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("models--intfloat--multilingual-e5-small");
        let snapshot = repo.join("snapshots").join("abc123");
        std::fs::create_dir_all(snapshot.join("onnx")).unwrap();
        std::fs::create_dir_all(repo.join("refs")).unwrap();
        std::fs::write(repo.join("refs").join("main"), "abc123").unwrap();

        for artifact in required_snapshot_artifacts() {
            std::fs::write(snapshot.join(artifact), b"x").unwrap();
        }

        assert!(embedded_model_exists_in(temp.path()));
    }

    #[test]
    fn test_fastembed_snapshot_layout_requires_onnx_model() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("models--intfloat--multilingual-e5-small");
        let snapshot = repo.join("snapshots").join("abc123");
        std::fs::create_dir_all(snapshot.join("onnx")).unwrap();
        std::fs::create_dir_all(repo.join("refs")).unwrap();
        std::fs::write(repo.join("refs").join("main"), "abc123").unwrap();

        for artifact in required_snapshot_artifacts()
            .into_iter()
            .filter(|artifact| *artifact != "onnx/model.onnx")
        {
            std::fs::write(snapshot.join(artifact), b"x").unwrap();
        }

        assert!(!embedded_model_exists_in(temp.path()));
    }
}
