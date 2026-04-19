//! Phase TTS-E / P3：STT Tauri 命令。
//!
//! 多 provider：whisper 本地（whisper.cpp + Metal）/ groq 云端（Whisper API）。
//! 用户可在 STT 设置页选择 provider。

use std::sync::Arc;

use base64::Engine;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

use crate::modules::stt::openflow::{
    default_sensevoice_dir, download_all as openflow_download_all,
    model_is_ready as openflow_model_is_ready, OpenFlowAsrEngine, SenseVoicePreset,
};
use crate::modules::stt::settings::{
    load as load_stt_settings, save as save_stt_settings, SttProvider, SttSettings,
};

/// 全局 OpenFlow 引擎（懒加载 + 跨命令复用）。
/// `None` = 还未实例化或模型不可用；首次 transcribe 时尝试构造。
static OPENFLOW_ENGINE: Lazy<Mutex<Option<Arc<OpenFlowAsrEngine>>>> =
    Lazy::new(|| Mutex::new(None));

async fn ensure_openflow_engine() -> Result<Arc<OpenFlowAsrEngine>, String> {
    let mut guard = OPENFLOW_ENGINE.lock().await;
    if let Some(engine) = guard.as_ref() {
        return Ok(engine.clone());
    }
    let dir = default_sensevoice_dir();
    if !openflow_model_is_ready(&dir) {
        return Err(format!(
            "SenseVoice 模型未下载。请去 设置 → STT 语音输入 → 下载 SenseVoice 模型（约 230MB）。期望路径：{}",
            dir.display()
        ));
    }
    let engine = OpenFlowAsrEngine::new(dir);
    *guard = Some(engine.clone());
    Ok(engine)
}

// ── Whisper local ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SttModelStatus {
    pub ready: bool,
    pub model_path: Option<String>,
    pub model_name: Option<String>,
    pub download_hint: String,
    pub model_dir: String,
    /// SenseVoice (OpenFlow) 模型是否就绪
    pub openflow_ready: bool,
    /// SenseVoice 模型目录（即使没下载也告诉前端期望路径）
    pub openflow_model_dir: String,
}

#[tauri::command]
pub async fn stt_model_status() -> Result<SttModelStatus, String> {
    let dir = crate::modules::stt::default_whisper_model_dir();
    let path = crate::modules::stt::find_whisper_model();
    let of_dir = default_sensevoice_dir();
    let of_ready = openflow_model_is_ready(&of_dir);
    Ok(SttModelStatus {
        ready: path.is_some(),
        model_name: path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string()),
        download_hint: format!(
            "下载 ggml-base.bin (~148MB) 到 {}：\n  curl -L https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin -o {}/ggml-base.bin",
            dir.display(), dir.display()
        ),
        model_path: path.map(|p| p.to_string_lossy().to_string()),
        model_dir: dir.to_string_lossy().to_string(),
        openflow_ready: of_ready,
        openflow_model_dir: of_dir.to_string_lossy().to_string(),
    })
}

/// 一键下载 ggml whisper 模型到本地缓存目录。
#[tauri::command]
pub async fn stt_download_whisper_model(model_id: Option<String>) -> Result<String, String> {
    let model_name = model_id.as_deref().unwrap_or("ggml-base.bin");
    let dir = crate::modules::stt::default_whisper_model_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("create dir: {e}"))?;
    let dest = dir.join(model_name);
    if dest.exists() {
        return Ok(dest.to_string_lossy().to_string());
    }
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{model_name}");
    tracing::info!(url = %url, dest = %dest.display(), "downloading whisper model");
    let resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .build()
        .map_err(|e| format!("client: {e}"))?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("download: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("read bytes: {e}"))?;
    let tmp = dest.with_extension("bin.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("write tmp: {e}"))?;
    std::fs::rename(&tmp, &dest).map_err(|e| format!("rename: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

// ── STT settings ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SttSettingsDto {
    pub provider: String,
    pub groq_api_key_set: bool,
    pub groq_model: String,
}

impl From<&SttSettings> for SttSettingsDto {
    fn from(s: &SttSettings) -> Self {
        Self {
            provider: match s.provider {
                SttProvider::Whisper => "whisper".to_string(),
                SttProvider::Groq => "groq".to_string(),
                SttProvider::OpenFlow => "openflow".to_string(),
            },
            groq_api_key_set: s.groq_api_key.as_ref().is_some_and(|k| !k.is_empty()),
            groq_model: s
                .groq_model
                .clone()
                .unwrap_or_else(|| crate::modules::stt::groq::GROQ_DEFAULT_MODEL.to_string()),
        }
    }
}

#[tauri::command]
pub async fn stt_get_settings(app: tauri::AppHandle) -> Result<SttSettingsDto, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    let s = load_stt_settings(&dir);
    Ok((&s).into())
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveSttSettingsRequest {
    pub provider: Option<String>,
    pub groq_api_key: Option<String>,
    pub groq_model: Option<String>,
}

#[tauri::command]
pub async fn stt_save_settings(
    app: tauri::AppHandle,
    request: SaveSttSettingsRequest,
) -> Result<SttSettingsDto, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    let mut current = load_stt_settings(&dir);
    if let Some(p) = request.provider.as_deref() {
        current.provider = match p {
            "groq" => SttProvider::Groq,
            "openflow" => SttProvider::OpenFlow,
            _ => SttProvider::Whisper,
        };
    }
    if let Some(key) = request.groq_api_key {
        current.groq_api_key = if key.trim().is_empty() {
            None
        } else {
            Some(key.trim().to_string())
        };
    }
    if let Some(m) = request.groq_model {
        current.groq_model = if m.trim().is_empty() { None } else { Some(m) };
    }
    save_stt_settings(&dir, &current)?;
    Ok((&current).into())
}

// ── Transcribe ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SttTranscribeRequest {
    /// PCM16LE base64 字节
    pub audio_bytes_base64: String,
    pub language: Option<String>,
    pub sample_rate: Option<u32>,
    /// 可选覆盖 provider；未传则用 settings.provider
    pub provider_override: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SttTranscribeResponse {
    pub text: String,
    pub language: String,
    pub elapsed_seconds: f32,
    pub provider: String,
}

#[tauri::command]
pub async fn stt_transcribe(
    app: tauri::AppHandle,
    request: SttTranscribeRequest,
) -> Result<SttTranscribeResponse, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&request.audio_bytes_base64)
        .map_err(|e| format!("base64 解码失败: {e}"))?;
    if bytes.is_empty() {
        return Err("音频数据为空".to_string());
    }
    let sample_rate = request.sample_rate.unwrap_or(16_000);
    let language = request.language.clone();

    // 解析 provider
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    let settings = load_stt_settings(&dir);
    let provider = match request.provider_override.as_deref() {
        Some("groq") => SttProvider::Groq,
        Some("whisper") => SttProvider::Whisper,
        Some("openflow") => SttProvider::OpenFlow,
        _ => settings.provider.clone(),
    };

    match provider {
        SttProvider::Whisper => {
            let model_path = crate::modules::stt::find_whisper_model().ok_or_else(|| {
                "Whisper 模型未下载。请去 设置 → STT 配置 一键下载，或切换 provider 为 Groq"
                    .to_string()
            })?;
            let result = tokio::task::spawn_blocking(move || {
                let pcm_f32 = pcm16le_to_f32(&bytes, sample_rate)?;
                crate::modules::stt::transcribe_pcm(&model_path, &pcm_f32, language.as_deref())
            })
            .await
            .map_err(|e| format!("spawn_blocking join: {e}"))?
            .map_err(|e| format!("transcribe failed: {e}"))?;
            tracing::info!(provider="whisper", text=%result.text, "STT done");
            Ok(SttTranscribeResponse {
                text: result.text,
                language: result.language,
                elapsed_seconds: result.elapsed_seconds,
                provider: "whisper".to_string(),
            })
        }
        SttProvider::OpenFlow => {
            let engine = ensure_openflow_engine().await?;
            // SenseVoice 期望 PCM f32 任意采样率（内部会重采样到 16kHz）
            let pcm_f32: Vec<f32> = bytes
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                .collect();
            let result = engine
                .transcribe(pcm_f32, sample_rate, language.as_deref())
                .await?;
            tracing::info!(provider="openflow", text=%result.text, "STT done");
            Ok(SttTranscribeResponse {
                text: result.text,
                language: result.language,
                elapsed_seconds: result.elapsed_seconds,
                provider: "openflow".to_string(),
            })
        }
        SttProvider::Groq => {
            let api_key = settings
                .groq_api_key
                .filter(|k| !k.is_empty())
                .ok_or_else(|| "未配置 Groq API Key。请去 设置 → STT 配置 填写".to_string())?;
            // 直接把 PCM16LE 包成 WAV → multipart
            let wav = crate::modules::stt::groq::pcm16le_to_wav(&bytes, sample_rate, 1);
            let model = settings.groq_model.as_deref();
            let result = crate::modules::stt::groq::transcribe_via_groq(
                wav,
                &api_key,
                language.as_deref(),
                model,
            )
            .await?;
            tracing::info!(provider="groq", text=%result.text, "STT done");
            Ok(SttTranscribeResponse {
                text: result.text,
                language: result.language,
                elapsed_seconds: result.elapsed_seconds,
                provider: "groq".to_string(),
            })
        }
    }
}

// ── OpenFlow（SenseVoice）模型下载 ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowDownloadProgress {
    /// 当前文件名
    pub file: String,
    /// 已下载字节
    pub downloaded: u64,
    /// 总字节（None = HTTP 未返回 Content-Length）
    pub total: Option<u64>,
    /// 0-100 整数百分比（total 已知时）；total None 时 = -1
    pub percent: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadOpenflowRequest {
    /// "quantized" / "fp16"，None = quantized
    pub preset: Option<String>,
    /// 强制重下（覆盖已存在文件）
    pub force: Option<bool>,
}

/// 下载 SenseVoice 模型；过程中通过 `stt:openflow-download-progress` 事件向前端推进度。
/// 完成后引擎缓存被清空，下次 transcribe 时重新加载。
#[tauri::command]
pub async fn stt_download_openflow_model(
    app: tauri::AppHandle,
    request: DownloadOpenflowRequest,
) -> Result<String, String> {
    let preset = match request.preset.as_deref() {
        Some("fp16") => SenseVoicePreset::Fp16,
        _ => SenseVoicePreset::Quantized,
    };
    let force = request.force.unwrap_or(false);
    let dest = default_sensevoice_dir();

    // 进度回调 → Tauri 事件
    let app_clone = app.clone();
    let cb: crate::modules::stt::openflow::downloader::ProgressCallback =
        Box::new(move |file: &str, downloaded: u64, total: Option<u64>| {
            let percent = match total {
                Some(t) if t > 0 => ((downloaded * 100) / t) as i32,
                _ => -1,
            };
            let _ = app_clone.emit(
                "stt:openflow-download-progress",
                OpenFlowDownloadProgress {
                    file: file.to_string(),
                    downloaded,
                    total,
                    percent,
                },
            );
        });

    tracing::info!(dest = %dest.display(), "下载 SenseVoice 模型开始");
    let result_dir = openflow_download_all(&dest, preset, force, Some(cb)).await?;

    // 重置引擎缓存，下次 transcribe 重新加载新文件
    {
        let mut guard = OPENFLOW_ENGINE.lock().await;
        *guard = None;
    }

    Ok(result_dir.to_string_lossy().to_string())
}

// PCM16LE → 16kHz mono f32（whisper 输入要求）
fn pcm16le_to_f32(bytes: &[u8], source_sample_rate: u32) -> Result<Vec<f32>, String> {
    if bytes.len() % 2 != 0 {
        return Err("PCM16LE 字节数必须是偶数".to_string());
    }
    let samples: Vec<f32> = bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect();
    if source_sample_rate == 16_000 {
        return Ok(samples);
    }
    let ratio = 16_000.0 / source_sample_rate as f64;
    let output_len = (samples.len() as f64 * ratio).ceil() as usize;
    let mut resampled = Vec::with_capacity(output_len);
    for i in 0..output_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = src_pos - idx as f64;
        let s0 = samples.get(idx).copied().unwrap_or(0.0);
        let s1 = samples.get(idx + 1).copied().unwrap_or(0.0);
        resampled.push(s0 + (s1 - s0) * frac as f32);
    }
    Ok(resampled)
}
