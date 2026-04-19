//! STT 设置：provider 选择 + Groq API Key 持久化。
//!
//! 设置文件 `<app_data>/stt_settings.json`：
//! ```json
//! { "provider": "whisper" | "groq", "groq_api_key": "gsk_...", "groq_model": "whisper-large-v3-turbo" }
//! ```

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SttProvider {
    /// 本地 whisper.cpp（Metal 加速）
    #[default]
    Whisper,
    /// Groq Whisper API（云端，需 API Key）
    Groq,
    /// 本地 SenseVoice ONNX（vendored from open-flow，中文优势）
    #[serde(rename = "openflow")]
    OpenFlow,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct SttSettings {
    pub provider: SttProvider,
    #[serde(default)]
    pub groq_api_key: Option<String>,
    #[serde(default)]
    pub groq_model: Option<String>,
}

fn settings_path(app_data_dir: &std::path::Path) -> PathBuf {
    app_data_dir.join("stt_settings.json")
}

pub fn load(app_data_dir: &std::path::Path) -> SttSettings {
    let path = settings_path(app_data_dir);
    if !path.exists() {
        return SttSettings::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<SttSettings>(&s).ok())
        .unwrap_or_default()
}

pub fn save(app_data_dir: &std::path::Path, settings: &SttSettings) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| format!("create dir: {e}"))?;
    let path = settings_path(app_data_dir);
    let content = serde_json::to_string_pretty(settings).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&path, content).map_err(|e| format!("write: {e}"))?;
    Ok(())
}
