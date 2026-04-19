//! `browser_poc_manifest.json` 解析。
//!
//! 该文件是 MOSS-TTS-Nano 模型仓库的顶层描述：
//! - `model_files`: 指向 tts_meta / codec_meta / tokenizer.model 等次级文件。
//! - `tts_config`: n_vq、各 audio_*_token_id（assistant_slot / end / pad / start
//!   / user_slot 等）、audio_codebook_sizes 等 TTS 协议常量。
//! - `prompt_templates`: 三组 token id 列表，构造 voice clone 输入序列时使用。
//! - `builtin_voices`: 内置音色 + prebaked `prompt_audio_codes`。
//! - `generation_defaults`: max_new_frames、do_sample、各 top_k/p/temperature 默认值。
//! - `text_samples`: warmup 用文本样例。
//!
//! 镜像 Python `OrtCpuRuntime.manifest`。

#![allow(dead_code)]

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::modules::tts::error::TtsError;

/// 顶层 manifest 数据结构。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BrowserPocManifest {
    pub model_files: ModelFiles,
    pub tts_config: TtsConfig,
    pub prompt_templates: PromptTemplates,
    #[serde(default)]
    pub builtin_voices: Vec<BuiltinVoice>,
    #[serde(default)]
    pub text_samples: Vec<TextSample>,
    #[serde(default)]
    pub generation_defaults: GenerationDefaults,
}

impl BrowserPocManifest {
    /// 从 JSON 文件加载并解析。
    pub fn load_from_file(path: &Path) -> Result<Self, TtsError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| TtsError::ManifestNotFound(format!("read {}: {e}", path.display())))?;
        let parsed: Self = serde_json::from_str(&content)
            .map_err(|e| TtsError::ManifestNotFound(format!("parse {}: {e}", path.display())))?;
        Ok(parsed)
    }
}

/// `manifest.model_files`：指向次级 JSON / tokenizer 路径。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelFiles {
    pub tts_meta: String,
    pub codec_meta: String,
    #[serde(default)]
    pub tokenizer_model: Option<String>,
}

/// `manifest.tts_config`：TTS 协议层常量。
///
/// **关键点**：`n_vq` 是 16（与 MOSS-Audio-Tokenizer-Nano 的 16 RVQ codebooks 对应），
/// 而当前 Rust 代码 hardcode 的 4 是错误值——必须从此结构读取。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TtsConfig {
    pub n_vq: usize,
    pub audio_pad_token_id: i32,
    pub audio_start_token_id: i32,
    pub audio_end_token_id: i32,
    pub audio_user_slot_token_id: i32,
    pub audio_assistant_slot_token_id: i32,
    #[serde(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// `manifest.prompt_templates`：三组 token id 列表用于构造 voice clone 输入。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PromptTemplates {
    pub user_prompt_prefix_token_ids: Vec<i32>,
    pub user_prompt_after_reference_token_ids: Vec<i32>,
    pub assistant_prompt_prefix_token_ids: Vec<i32>,
}

/// `manifest.builtin_voices[]`：内置音色 + prebaked `prompt_audio_codes`。
///
/// `prompt_audio_codes` 形状：`[frames][n_vq]` 的二维 codes 列表。
/// Python 直接读这个列表喂入 `build_voice_clone_request_rows`，
/// 不需要再调用 codec_encode。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BuiltinVoice {
    pub voice: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub prompt_text: Option<String>,
    #[serde(default)]
    pub prompt_audio_codes: Vec<Vec<i32>>,
}

/// warmup / demo 用文本样例。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextSample {
    pub text: String,
    #[serde(default)]
    pub text_token_ids: Vec<i32>,
    #[serde(default)]
    pub language: Option<String>,
}

/// `manifest.generation_defaults`：默认生成参数。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenerationDefaults {
    #[serde(default = "default_max_new_frames")]
    pub max_new_frames: u32,
    #[serde(default)]
    pub do_sample: bool,
    #[serde(default)]
    pub sample_mode: Option<String>,
    #[serde(default = "default_text_temperature")]
    pub text_temperature: f32,
    #[serde(default = "default_text_top_p")]
    pub text_top_p: f32,
    #[serde(default = "default_text_top_k")]
    pub text_top_k: u32,
    #[serde(default = "default_audio_temperature")]
    pub audio_temperature: f32,
    #[serde(default = "default_audio_top_p")]
    pub audio_top_p: f32,
    #[serde(default = "default_audio_top_k")]
    pub audio_top_k: u32,
    #[serde(default = "default_audio_repetition_penalty")]
    pub audio_repetition_penalty: f32,
}

impl Default for GenerationDefaults {
    fn default() -> Self {
        Self {
            max_new_frames: default_max_new_frames(),
            do_sample: true,
            sample_mode: Some("fixed".to_string()),
            text_temperature: default_text_temperature(),
            text_top_p: default_text_top_p(),
            text_top_k: default_text_top_k(),
            audio_temperature: default_audio_temperature(),
            audio_top_p: default_audio_top_p(),
            audio_top_k: default_audio_top_k(),
            audio_repetition_penalty: default_audio_repetition_penalty(),
        }
    }
}

fn default_max_new_frames() -> u32 {
    375
}
fn default_text_temperature() -> f32 {
    1.0
}
fn default_text_top_p() -> f32 {
    1.0
}
fn default_text_top_k() -> u32 {
    50
}
fn default_audio_temperature() -> f32 {
    0.8
}
fn default_audio_top_p() -> f32 {
    0.95
}
fn default_audio_top_k() -> u32 {
    25
}
fn default_audio_repetition_penalty() -> f32 {
    1.2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> &'static str {
        r#"{
            "model_files": {
                "tts_meta": "MOSS-TTS-Nano-100M-ONNX/tts_browser_onnx_meta.json",
                "codec_meta": "MOSS-Audio-Tokenizer-Nano-ONNX/codec_browser_onnx_meta.json",
                "tokenizer_model": "MOSS-TTS-Nano-100M-ONNX/tokenizer.model"
            },
            "tts_config": {
                "n_vq": 16,
                "audio_pad_token_id": 0,
                "audio_start_token_id": 1024,
                "audio_end_token_id": 1025,
                "audio_user_slot_token_id": 1026,
                "audio_assistant_slot_token_id": 1027
            },
            "prompt_templates": {
                "user_prompt_prefix_token_ids": [1, 2, 3],
                "user_prompt_after_reference_token_ids": [4, 5],
                "assistant_prompt_prefix_token_ids": [6, 7]
            },
            "builtin_voices": [
                {"voice": "Junhao", "description": "zh A", "prompt_audio_codes": [[1, 2, 3]]}
            ],
            "text_samples": [{"text": "你好"}],
            "generation_defaults": {
                "max_new_frames": 375,
                "do_sample": true,
                "sample_mode": "fixed",
                "audio_temperature": 0.8
            }
        }"#
    }

    #[test]
    fn parses_full_fixture() {
        let parsed: BrowserPocManifest = serde_json::from_str(fixture()).unwrap();
        assert_eq!(parsed.tts_config.n_vq, 16);
        assert_eq!(parsed.tts_config.audio_end_token_id, 1025);
        assert_eq!(parsed.builtin_voices.len(), 1);
        assert_eq!(parsed.builtin_voices[0].voice, "Junhao");
        assert_eq!(parsed.generation_defaults.audio_temperature, 0.8);
    }

    #[test]
    fn defaults_apply_when_generation_section_missing() {
        let json = r#"{
            "model_files": {"tts_meta": "a", "codec_meta": "b"},
            "tts_config": {
                "n_vq": 16, "audio_pad_token_id": 0, "audio_start_token_id": 1024,
                "audio_end_token_id": 1025, "audio_user_slot_token_id": 1026,
                "audio_assistant_slot_token_id": 1027
            },
            "prompt_templates": {
                "user_prompt_prefix_token_ids": [],
                "user_prompt_after_reference_token_ids": [],
                "assistant_prompt_prefix_token_ids": []
            }
        }"#;
        let parsed: BrowserPocManifest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.generation_defaults.max_new_frames, 375);
        assert_eq!(parsed.generation_defaults.audio_top_k, 25);
    }
}
