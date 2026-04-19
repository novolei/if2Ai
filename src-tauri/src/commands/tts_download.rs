//! TTS model download + status Tauri commands.
//!
//! Provides 3 commands:
//! - `tts_model_status`: Check which model files are present/missing
//! - `tts_model_download_start`: Start downloading missing model files
//! - `tts_model_download_status`: Poll download progress
//!
//! Downloads are chunked and resumable from `~/.if2ai/models/tts/`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::modules::tts::config::{AUDIO_TOKENIZER_HF_REPO, TTS_MODEL_HF_REPO};

/// Status of a single model file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFileInfo {
    pub name: String,
    /// File size in bytes (0 if missing).
    pub size: u64,
    pub present: bool,
}

/// Overall TTS model status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsModelStatusResponse {
    /// Whether all required model files are present.
    pub ready: bool,
    pub tts_files: Vec<ModelFileInfo>,
    pub tokenizer_files: Vec<ModelFileInfo>,
    /// Total size of all files (present + missing) in bytes.
    pub total_bytes: u64,
    /// Size of missing files in bytes.
    pub missing_bytes: u64,
    /// Cache directory path.
    pub cache_dir: String,
}

/// Download progress status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsDownloadStatusResponse {
    pub is_downloading: bool,
    pub percent: f32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub current_file: String,
    pub error: Option<String>,
}

/// Shared download state, managed via Tauri `.manage()` and accessed
/// by download commands and the frontend polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TtsDownloadState {
    pub is_downloading: bool,
    pub percent: f32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub current_file: String,
    pub error: Option<String>,
    pub abort: bool,
}

impl Default for TtsDownloadState {
    fn default() -> Self {
        Self {
            is_downloading: false,
            percent: 0.0,
            downloaded_bytes: 0,
            total_bytes: 0,
            current_file: String::new(),
            error: None,
            abort: false,
        }
    }
}

/// HuggingFace file metadata with download URL and size.
struct HfFile {
    /// Remote file name on HuggingFace (e.g. `moss_tts_prefill.onnx`).
    remote_name: String,
    /// Local file name in cache (e.g. `prefill.onnx`).
    local_name: String,
    size: u64,
    url: String,
    /// Whether this belongs to the TTS model (true) or audio tokenizer (false).
    is_tts: bool,
}

/// Mapping from local cache file names to HuggingFace remote file names.
///
/// The MOSS-TTS-Nano repo uses `moss_tts_` prefixes and shared `.data`
/// weight files, while our ONNX loader expects specific local names
/// (see `GlobalSessions::load` and `LocalSessions::load`).
/// TTS 主模型文件清单 —— `(local_name, remote_name)` 对，本地名必须与
/// HuggingFace `snapshot_download` 实际产物的文件名一致。
///
/// 注意：本地名 == 远程名（不再做 alias 重命名），与 [`OnnxTtsProvider`]
/// 通过 manifest 解析的 ONNX 文件名 100% 一致。
fn tts_model_file_map() -> &'static [(&'static str, &'static str)] {
    &[
        // 顶层 manifest + 各 ONNX 文件 + 共享外部权重 + tokenizer
        ("browser_poc_manifest.json", "browser_poc_manifest.json"),
        ("tts_browser_onnx_meta.json", "tts_browser_onnx_meta.json"),
        ("tokenizer.model", "tokenizer.model"),
        ("moss_tts_prefill.onnx", "moss_tts_prefill.onnx"),
        ("moss_tts_decode_step.onnx", "moss_tts_decode_step.onnx"),
        ("moss_tts_local_decoder.onnx", "moss_tts_local_decoder.onnx"),
        (
            "moss_tts_local_cached_step.onnx",
            "moss_tts_local_cached_step.onnx",
        ),
        (
            "moss_tts_local_fixed_sampled_frame.onnx",
            "moss_tts_local_fixed_sampled_frame.onnx",
        ),
    ]
}

/// Audio tokenizer 文件清单（本地名 == 远程名，与 HF 实际产物一致）。
fn tokenizer_file_map() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "codec_browser_onnx_meta.json",
            "codec_browser_onnx_meta.json",
        ),
        (
            "moss_audio_tokenizer_encode.onnx",
            "moss_audio_tokenizer_encode.onnx",
        ),
        (
            "moss_audio_tokenizer_encode.data",
            "moss_audio_tokenizer_encode.data",
        ),
        (
            "moss_audio_tokenizer_decode_full.onnx",
            "moss_audio_tokenizer_decode_full.onnx",
        ),
        (
            "moss_audio_tokenizer_decode_step.onnx",
            "moss_audio_tokenizer_decode_step.onnx",
        ),
        (
            "moss_audio_tokenizer_decode_shared.data",
            "moss_audio_tokenizer_decode_shared.data",
        ),
    ]
}

/// Unique remote files to download (deduplicated, preserving first local_name).
fn unique_remote_files(map: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut seen = std::collections::HashSet::new();
    map.iter()
        .filter(|(_, remote)| seen.insert(*remote))
        .copied()
        .collect()
}

/// Build the HuggingFace download URL for a file.
///
/// Uses the configured mirror URL if set, otherwise defaults to
/// `hf-mirror.com` (accessible from China).
fn hf_file_url(repo: &str, file: &str) -> String {
    let base = crate::commands::system_check::get_hf_mirror_url()
        .unwrap_or_else(|| "https://hf-mirror.com".to_string());
    // Strip trailing slash from base so the path is clean.
    let base = base.strip_suffix('/').unwrap_or(&base);
    format!("{base}/{repo}/resolve/main/{file}")
}

/// Check TTS model file status.
///
/// Returns which files are present/missing and total size info.
#[tauri::command]
pub async fn tts_model_status() -> Result<TtsModelStatusResponse, String> {
    let cache_root = crate::modules::tts::config::default_model_dir();
    let tts_dir = cache_root.join(crate::modules::tts::config::TTS_MODEL_SUBDIR);
    let tokenizer_dir = cache_root.join(crate::modules::tts::config::AUDIO_TOKENIZER_SUBDIR);

    let tts_files: Vec<ModelFileInfo> = tts_model_file_map()
        .iter()
        .map(|(local, _)| file_info(&tts_dir, local))
        .collect();

    let tokenizer_files: Vec<ModelFileInfo> = tokenizer_file_map()
        .iter()
        .map(|(local, _)| file_info(&tokenizer_dir, local))
        .collect();

    let total_bytes: u64 = tts_files
        .iter()
        .chain(tokenizer_files.iter())
        .map(|f| f.size)
        .sum();

    let missing_bytes: u64 = tts_files
        .iter()
        .chain(tokenizer_files.iter())
        .filter(|f| !f.present)
        .map(|f| f.size)
        .sum();

    let ready = tts_files.iter().all(|f| f.present) && tokenizer_files.iter().all(|f| f.present);

    Ok(TtsModelStatusResponse {
        ready,
        tts_files,
        tokenizer_files,
        total_bytes,
        missing_bytes,
        cache_dir: cache_root.to_string_lossy().to_string(),
    })
}

/// Get info for a single model file.
fn file_info(dir: &std::path::Path, name: &str) -> ModelFileInfo {
    let path = dir.join(name);
    if path.exists() {
        if let Ok(meta) = std::fs::metadata(&path) {
            return ModelFileInfo {
                name: name.to_string(),
                size: meta.len(),
                present: true,
            };
        }
    }
    ModelFileInfo {
        name: name.to_string(),
        size: 0,
        present: false,
    }
}

/// Start downloading missing TTS model files.
///
/// Downloads run in the background. Use `tts_model_download_status` to poll progress.
#[tauri::command]
pub async fn tts_model_download_start(
    state: tauri::State<'_, Arc<Mutex<TtsDownloadState>>>,
) -> Result<(), String> {
    let mut s = state.lock().await;
    if s.is_downloading {
        return Err("Download already in progress".to_string());
    }
    s.is_downloading = true;
    s.percent = 0.0;
    s.downloaded_bytes = 0;
    s.total_bytes = 0;
    s.current_file = String::new();
    s.error = None;
    s.abort = false;

    // Spawn the download task
    spawn_tts_download(state.inner().clone());
    Ok(())
}

/// Get current download progress.
#[tauri::command]
pub async fn tts_model_download_status(
    state: tauri::State<'_, Arc<Mutex<TtsDownloadState>>>,
) -> Result<TtsDownloadStatusResponse, String> {
    let s = state.lock().await;
    Ok(TtsDownloadStatusResponse {
        is_downloading: s.is_downloading,
        percent: s.percent,
        downloaded_bytes: s.downloaded_bytes,
        total_bytes: s.total_bytes,
        current_file: s.current_file.clone(),
        error: s.error.clone(),
    })
}

/// Start the background download task.
///
/// This is called internally after `tts_model_download_start` enables the flag.
pub(crate) fn spawn_tts_download(download_state: Arc<Mutex<TtsDownloadState>>) {
    tokio::spawn(async move {
        if let Err(e) = download_models_impl(&download_state).await {
            let mut s = download_state.lock().await;
            s.error = Some(e);
            s.is_downloading = false;
        }
    });
}

/// Download TTS models from HuggingFace with progress tracking.
async fn download_models_impl(state: &Arc<Mutex<TtsDownloadState>>) -> Result<(), String> {
    let cache_root = crate::modules::tts::config::default_model_dir();
    let tts_dir = cache_root.join(crate::modules::tts::config::TTS_MODEL_SUBDIR);
    let tokenizer_dir = cache_root.join(crate::modules::tts::config::AUDIO_TOKENIZER_SUBDIR);

    // Create directories
    std::fs::create_dir_all(&tts_dir)
        .map_err(|e| format!("Failed to create TTS model dir: {e}"))?;
    std::fs::create_dir_all(&tokenizer_dir)
        .map_err(|e| format!("Failed to create tokenizer dir: {e}"))?;

    // Collect all unique remote files to download, mapped to local names
    let mut all_files: Vec<HfFile> = Vec::new();

    for (local_name, remote_name) in unique_remote_files(tts_model_file_map()) {
        let url = hf_file_url(TTS_MODEL_HF_REPO, remote_name);
        let size = get_file_size(&url).await?;
        all_files.push(HfFile {
            remote_name: remote_name.to_string(),
            local_name: local_name.to_string(),
            size,
            url,
            is_tts: true,
        });
    }

    for (local_name, remote_name) in unique_remote_files(tokenizer_file_map()) {
        let url = hf_file_url(AUDIO_TOKENIZER_HF_REPO, remote_name);
        let size = get_file_size(&url).await?;
        all_files.push(HfFile {
            remote_name: remote_name.to_string(),
            local_name: local_name.to_string(),
            size,
            url,
            is_tts: false,
        });
    }

    let total_bytes: u64 = all_files.iter().map(|f| f.size).sum();

    {
        let mut s = state.lock().await;
        s.total_bytes = total_bytes;
    }

    let mut downloaded: u64 = 0;

    for hf_file in &all_files {
        let target = tts_dir.join(&hf_file.local_name);

        if target.exists() {
            if let Ok(meta) = std::fs::metadata(&target) {
                if meta.len() >= hf_file.size {
                    downloaded += hf_file.size;
                    {
                        let mut s = state.lock().await;
                        s.downloaded_bytes = downloaded;
                        s.percent = downloaded as f32 / total_bytes as f32;
                    }
                    continue;
                }
            }
        }

        // Download file with resume support
        {
            let mut s = state.lock().await;
            s.current_file = hf_file.local_name.clone();
        }

        download_file(
            &hf_file.url,
            &target,
            hf_file.size,
            downloaded,
            total_bytes,
            state,
        )
        .await?;

        // Copy to other local names that map to the same remote file.
        // e.g. moss_tts_global_shared.data → prefill.onnx.data + decode_step.onnx.data
        let (target_dir, file_map) = if hf_file.is_tts {
            (&tts_dir, tts_model_file_map())
        } else {
            (&tokenizer_dir, tokenizer_file_map())
        };
        let other_locals = file_map
            .iter()
            .filter(|(_, remote)| *remote == hf_file.remote_name)
            .map(|(local, _)| *local)
            .filter(|local| *local != hf_file.local_name);
        for other in other_locals {
            let other_target = target_dir.join(other);
            std::fs::copy(&target, &other_target)
                .map_err(|e| format!("Failed to copy to {other_target:?}: {e}"))?;
        }

        downloaded += hf_file.size;
    }

    {
        let mut s = state.lock().await;
        s.is_downloading = false;
        s.percent = 1.0;
        s.downloaded_bytes = total_bytes;
        s.current_file = "Complete".to_string();
    }

    Ok(())
}

/// Get file size from HuggingFace via HEAD request.
async fn get_file_size(url: &str) -> Result<u64, String> {
    // Use HuggingFace API to get file size
    // Fallback: return estimated sizes if API fails
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let resp = client
        .head(url)
        .send()
        .await
        .map_err(|e| format!("HEAD request failed for {url}: {e}"))?;

    resp.content_length()
        .ok_or_else(|| format!("No Content-Length header for {url}"))
}

/// Download a single file with streaming and progress updates.
async fn download_file(
    url: &str,
    target: &std::path::Path,
    _file_size: u64,
    base_downloaded: u64,
    total_bytes: u64,
    state: &Arc<Mutex<TtsDownloadState>>,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed for {url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} for {url}", resp.status()));
    }

    let mut file =
        std::fs::File::create(target).map_err(|e| format!("Failed to create {target:?}: {e}"))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {e}"))?;

        // Check abort flag
        {
            let s = state.lock().await;
            if s.abort {
                std::fs::remove_file(target).ok();
                return Err("Download aborted".to_string());
            }
        }

        std::io::Write::write_all(&mut file, &bytes).map_err(|e| format!("Write failed: {e}"))?;

        downloaded += bytes.len() as u64;

        // Update progress every ~1MB
        if downloaded % (1024 * 1024) < bytes.len() as u64 {
            let mut s = state.lock().await;
            s.downloaded_bytes = base_downloaded + downloaded;
            s.percent = s.downloaded_bytes as f32 / total_bytes as f32;
        }
    }

    Ok(())
}
