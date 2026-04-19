//! `codec_browser_onnx_meta.json` 解析。
//!
//! Codec（音频 tokenizer）的 ONNX 文件名 + 配置 + **流式解码状态规范**。
//!
//! `streaming_decode` 是 [`crate::modules::tts::audio::streaming_decoder::CodecStreamingDecodeSession`]
//! 的核心数据：transformer offsets 张量名 + attention caches keys/values/positions
//! 张量名 + shape，每次 `run_frames` 后将 `*_output_name` 的输出回灌到对应
//! `*_input_name` 的下一次输入。

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::modules::tts::error::TtsError;

/// `codec_browser_onnx_meta.json` 顶层结构。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodecMeta {
    /// 文件名映射：encode / decode_full / decode_step。
    pub files: HashMap<String, String>,
    pub codec_config: CodecConfig,
    /// 流式解码状态规范；非流式 codec 可能缺该字段。
    #[serde(default)]
    pub streaming_decode: StreamingDecodeMeta,
}

impl CodecMeta {
    pub fn load_from_file(path: &Path) -> Result<Self, TtsError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| TtsError::ManifestNotFound(format!("read {}: {e}", path.display())))?;
        serde_json::from_str::<Self>(&content)
            .map_err(|e| TtsError::ManifestNotFound(format!("parse {}: {e}", path.display())))
    }
}

/// `codec_meta.codec_config`：sample_rate / channels / num_quantizers 等。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodecConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub num_quantizers: usize,
    /// 余下任意字段保留。
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `codec_meta.streaming_decode`：流式 codec session 状态规范。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StreamingDecodeMeta {
    #[serde(default)]
    pub transformer_offsets: Vec<TransformerOffsetSpec>,
    #[serde(default)]
    pub attention_caches: Vec<AttentionCacheSpec>,
}

/// 一组 transformer offset 张量（int32）的 spec。
///
/// 状态机维护一对张量：上次推理输出 → 下次推理输入。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransformerOffsetSpec {
    pub input_name: String,
    pub output_name: String,
    pub shape: Vec<usize>,
}

/// 一层 attention cache 的 spec：keys/values (f32) + offset/positions (i32)。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttentionCacheSpec {
    pub offset_input_name: String,
    pub offset_output_name: String,
    pub offset_shape: Vec<usize>,
    pub cached_keys_input_name: String,
    pub cached_keys_output_name: String,
    pub cached_values_input_name: String,
    pub cached_values_output_name: String,
    pub cache_shape: Vec<usize>,
    pub cached_positions_input_name: String,
    pub cached_positions_output_name: String,
    pub positions_shape: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_codec_meta() {
        let json = r#"{
            "files": {"encode": "encode.onnx", "decode_full": "decode_full.onnx",
                      "decode_step": "decode_step.onnx"},
            "codec_config": {"sample_rate": 48000, "channels": 2, "num_quantizers": 16}
        }"#;
        let meta: CodecMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.codec_config.sample_rate, 48000);
        assert_eq!(meta.codec_config.channels, 2);
        assert_eq!(meta.codec_config.num_quantizers, 16);
        assert!(meta.streaming_decode.transformer_offsets.is_empty());
    }

    #[test]
    fn parses_streaming_decode_specs() {
        let json = r#"{
            "files": {"encode": "e.onnx", "decode_full": "d.onnx", "decode_step": "ds.onnx"},
            "codec_config": {"sample_rate": 48000, "channels": 2, "num_quantizers": 16},
            "streaming_decode": {
                "transformer_offsets": [
                    {"input_name": "tx_off_in_0", "output_name": "tx_off_out_0", "shape": [1, 1]}
                ],
                "attention_caches": [
                    {
                        "offset_input_name": "att_off_in_0", "offset_output_name": "att_off_out_0",
                        "offset_shape": [1, 1],
                        "cached_keys_input_name": "k_in_0", "cached_keys_output_name": "k_out_0",
                        "cached_values_input_name": "v_in_0", "cached_values_output_name": "v_out_0",
                        "cache_shape": [1, 8, 64, 64],
                        "cached_positions_input_name": "pos_in_0",
                        "cached_positions_output_name": "pos_out_0",
                        "positions_shape": [1, 64]
                    }
                ]
            }
        }"#;
        let meta: CodecMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.streaming_decode.transformer_offsets.len(), 1);
        assert_eq!(meta.streaming_decode.attention_caches.len(), 1);
        let att = &meta.streaming_decode.attention_caches[0];
        assert_eq!(att.cache_shape, vec![1, 8, 64, 64]);
        assert_eq!(att.positions_shape, vec![1, 64]);
    }
}
