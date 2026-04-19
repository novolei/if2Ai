//! `tts_browser_onnx_meta.json` 解析。
//!
//! 该文件给出 TTS 主模型 ONNX 文件名映射 + 模型超参（hidden / kv_heads /
//! head_dim / local_layers ...）+ 各 session 的输出名列表（用于把
//! `present_*` 改名为 `past_*` 喂回 decode_step）。

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::modules::tts::error::TtsError;

/// `tts_browser_onnx_meta.json` 顶层结构。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TtsMeta {
    /// 文件名映射：prefill / decode_step / local_decoder / local_cached_step
    /// / local_fixed_sampled_frame / local_greedy_frame 等键。
    pub files: HashMap<String, String>,
    /// 模型结构超参。
    pub model_config: ModelConfig,
    /// 各 session I/O 名称列表。
    #[serde(default)]
    pub onnx: OnnxNames,
}

impl TtsMeta {
    /// 从 JSON 文件加载。
    pub fn load_from_file(path: &Path) -> Result<Self, TtsError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| TtsError::ManifestNotFound(format!("read {}: {e}", path.display())))?;
        serde_json::from_str::<Self>(&content)
            .map_err(|e| TtsError::ManifestNotFound(format!("parse {}: {e}", path.display())))
    }
}

/// `tts_meta.model_config`：模型超参（用于构造空 KV cache）。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfig {
    #[serde(default)]
    pub hidden: Option<usize>,
    #[serde(default)]
    pub kv_heads: Option<usize>,
    #[serde(default)]
    pub head_dim: Option<usize>,
    #[serde(default)]
    pub global_layers: Option<usize>,
    #[serde(default)]
    pub local_layers: Option<usize>,
    #[serde(default)]
    pub local_heads: Option<usize>,
    #[serde(default)]
    pub local_head_dim: Option<usize>,
    #[serde(default)]
    pub audio_codebook_sizes: Vec<usize>,
    /// 余下任意字段保留，便于未来扩展不破坏反序列化。
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `tts_meta.onnx`：各 session 输入/输出名称（用于 KV cache 改名匹配）。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OnnxNames {
    #[serde(default)]
    pub prefill_input_names: Vec<String>,
    #[serde(default)]
    pub prefill_output_names: Vec<String>,
    #[serde(default)]
    pub decode_input_names: Vec<String>,
    #[serde(default)]
    pub decode_output_names: Vec<String>,
    #[serde(default)]
    pub local_cached_input_names: Vec<String>,
    #[serde(default)]
    pub local_cached_output_names: Vec<String>,
}

/// `local_cached_step` 的输入/输出 KV cache 子集，仅命名 helper。
#[derive(Debug, Clone, Default)]
pub struct LocalCachedOnnx<'a> {
    pub past_names: Vec<&'a str>,
    pub present_names: Vec<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_meta() {
        let json = r#"{
            "files": {"prefill": "prefill.onnx", "decode_step": "decode_step.onnx"},
            "model_config": {
                "hidden": 1024, "kv_heads": 16, "head_dim": 64,
                "local_layers": 6, "local_heads": 8, "local_head_dim": 64,
                "audio_codebook_sizes": [1024, 1024, 1024, 1024]
            },
            "onnx": {
                "prefill_output_names": ["global_hidden", "present_key_0", "present_value_0"],
                "decode_output_names": ["global_hidden", "present_key_0", "present_value_0"]
            }
        }"#;
        let meta: TtsMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.files.get("prefill").unwrap(), "prefill.onnx");
        assert_eq!(meta.model_config.hidden, Some(1024));
        assert_eq!(meta.model_config.local_layers, Some(6));
        assert_eq!(meta.onnx.prefill_output_names.len(), 3);
    }

    #[test]
    fn extra_fields_preserved_in_model_config() {
        let json = r#"{
            "files": {"prefill": "p.onnx"},
            "model_config": {"hidden": 1024, "future_field": 42}
        }"#;
        let meta: TtsMeta = serde_json::from_str(json).unwrap();
        assert!(meta.model_config.extra.contains_key("future_field"));
    }
}
