//! Voice clone 输入序列组装。
//!
//! 把 `(prompt_audio_codes, text_token_ids)` 编排成 prefill ONNX 期望的
//! `input_ids` (`[L, n_vq+1]`, int32) + `attention_mask` (`[1, L]`, int32)。
//!
//! 协议（与 Python `OrtCpuRuntime.build_voice_clone_request_rows`
//! `ort_cpu_runtime.py:456-476` 1:1）：
//!
//! ```text
//! prefix_text_tokens = user_prompt_prefix_token_ids + [audio_start]
//! suffix_text_tokens = [audio_end]
//!                    + user_prompt_after_reference_token_ids
//!                    + text_token_ids
//!                    + assistant_prompt_prefix_token_ids
//!                    + [audio_start]
//!
//! rows = build_text_rows(prefix_text_tokens)
//!      + build_audio_prefix_rows(prompt_audio_codes)
//!      + build_text_rows(suffix_text_tokens)
//! ```
//!
//! 每行宽度 `n_vq + 1`：第 0 列放 text token id 或 audio slot id，第 1..=n_vq
//! 列放 audio code 或 pad token id。

#![allow(dead_code)]

use crate::modules::tts::manifest::ManifestBundle;

/// Prefill ONNX 的输入张量数据（已展开为 row-major 的二维列表）。
#[derive(Debug, Clone)]
pub struct VoiceCloneRequestRows {
    /// 形状 `[L, n_vq + 1]`，i32。
    pub input_ids: Vec<Vec<i32>>,
    /// 形状 `[1, L]`，i32（全 1）。
    pub attention_mask: Vec<i32>,
}

impl VoiceCloneRequestRows {
    /// 序列长度 L。
    pub fn seq_len(&self) -> usize {
        self.input_ids.len()
    }

    /// 行宽 = n_vq + 1。
    pub fn row_width(&self) -> usize {
        self.input_ids.first().map(|r| r.len()).unwrap_or(0)
    }
}

/// 把一段文本 token id 序列展开为 prefill 行（每行宽度 = n_vq + 1，第 0 列是
/// text token，1..=n_vq 列填 audio_pad）。
fn build_text_rows(text_token_ids: &[i32], n_vq: usize, audio_pad: i32) -> Vec<Vec<i32>> {
    let row_width = n_vq + 1;
    text_token_ids
        .iter()
        .map(|&tid| {
            let mut row = vec![audio_pad; row_width];
            row[0] = tid;
            row
        })
        .collect()
}

/// 把 prompt audio codes 展开为 prefill 行：第 0 列固定填 `slot_token_id`
/// （voice clone 用 user_slot），1..=n_vq 列填该帧 audio codes（不足 n_vq
/// 的尾部用 audio_pad 补）。
fn build_audio_prefix_rows(
    prompt_audio_codes: &[Vec<i32>],
    n_vq: usize,
    slot_token_id: i32,
    audio_pad: i32,
) -> Vec<Vec<i32>> {
    let row_width = n_vq + 1;
    prompt_audio_codes
        .iter()
        .map(|frame| {
            let mut row = vec![audio_pad; row_width];
            row[0] = slot_token_id;
            for (i, &code) in frame.iter().take(n_vq).enumerate() {
                row[i + 1] = code;
            }
            row
        })
        .collect()
}

/// 构造 voice clone prefill 输入。镜像 Python `build_voice_clone_request_rows`。
///
/// # Panics
///
/// 不会 panic；若 manifest 字段缺失会构造空序列，调用方自行判断。
pub fn build_voice_clone_request_rows(
    manifest: &ManifestBundle,
    prompt_audio_codes: &[Vec<i32>],
    text_token_ids: &[i32],
) -> VoiceCloneRequestRows {
    let cfg = manifest.tts_config();
    let templates = &manifest.manifest.prompt_templates;

    let n_vq = cfg.n_vq;
    let audio_pad = cfg.audio_pad_token_id;
    let audio_start = cfg.audio_start_token_id;
    let audio_end = cfg.audio_end_token_id;
    let user_slot = cfg.audio_user_slot_token_id;

    let mut prefix_text: Vec<i32> =
        Vec::with_capacity(templates.user_prompt_prefix_token_ids.len() + 1);
    prefix_text.extend_from_slice(&templates.user_prompt_prefix_token_ids);
    prefix_text.push(audio_start);

    let mut suffix_text: Vec<i32> = Vec::with_capacity(
        1 + templates.user_prompt_after_reference_token_ids.len()
            + text_token_ids.len()
            + templates.assistant_prompt_prefix_token_ids.len()
            + 1,
    );
    suffix_text.push(audio_end);
    suffix_text.extend_from_slice(&templates.user_prompt_after_reference_token_ids);
    suffix_text.extend_from_slice(text_token_ids);
    suffix_text.extend_from_slice(&templates.assistant_prompt_prefix_token_ids);
    suffix_text.push(audio_start);

    let mut rows: Vec<Vec<i32>> = Vec::new();
    rows.extend(build_text_rows(&prefix_text, n_vq, audio_pad));
    rows.extend(build_audio_prefix_rows(
        prompt_audio_codes,
        n_vq,
        user_slot,
        audio_pad,
    ));
    rows.extend(build_text_rows(&suffix_text, n_vq, audio_pad));

    let attention_mask = vec![1i32; rows.len()];
    VoiceCloneRequestRows {
        input_ids: rows,
        attention_mask,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tts::manifest::{BrowserPocManifest, ManifestBundle};
    use std::path::PathBuf;

    fn fixture_manifest() -> ManifestBundle {
        // 直接构造 ManifestBundle（绕过磁盘 I/O）
        let json = r#"{
            "model_files": {"tts_meta": "x/tts_meta.json", "codec_meta": "x/codec_meta.json"},
            "tts_config": {
                "n_vq": 4,
                "audio_pad_token_id": 0,
                "audio_start_token_id": 100,
                "audio_end_token_id": 101,
                "audio_user_slot_token_id": 102,
                "audio_assistant_slot_token_id": 103
            },
            "prompt_templates": {
                "user_prompt_prefix_token_ids": [1, 2],
                "user_prompt_after_reference_token_ids": [3],
                "assistant_prompt_prefix_token_ids": [4, 5]
            }
        }"#;
        let manifest: BrowserPocManifest = serde_json::from_str(json).unwrap();
        ManifestBundle {
            manifest,
            tts_meta: crate::modules::tts::manifest::TtsMeta {
                files: Default::default(),
                model_config: serde_json::from_str("{}").unwrap(),
                onnx: Default::default(),
            },
            codec_meta: crate::modules::tts::manifest::CodecMeta {
                files: Default::default(),
                codec_config: serde_json::from_str(
                    r#"{"sample_rate": 48000, "channels": 2, "num_quantizers": 4}"#,
                )
                .unwrap(),
                streaming_decode: Default::default(),
            },
            manifest_dir: PathBuf::from("/tmp"),
            model_dir: PathBuf::from("/tmp"),
        }
    }

    #[test]
    fn row_width_equals_n_vq_plus_one() {
        let m = fixture_manifest();
        let rows = build_voice_clone_request_rows(&m, &[vec![10, 11, 12, 13]], &[200, 201]);
        for row in &rows.input_ids {
            assert_eq!(row.len(), 5); // n_vq=4 + 1
        }
        assert_eq!(rows.attention_mask.len(), rows.input_ids.len());
    }

    #[test]
    fn structure_matches_python_protocol() {
        let m = fixture_manifest();
        let prompt = vec![vec![20, 21, 22, 23], vec![30, 31, 32, 33]];
        let text = vec![201, 202, 203];
        let rows = build_voice_clone_request_rows(&m, &prompt, &text);

        // prefix_text = [1, 2, 100(audio_start)] → 3 rows
        // audio_prefix = 2 rows
        // suffix_text = [101(end), 3, 201, 202, 203, 4, 5, 100(start)] → 8 rows
        // total = 3 + 2 + 8 = 13
        assert_eq!(rows.input_ids.len(), 13);

        // 第一行：text token 1 在 col 0, 其余 audio_pad=0
        assert_eq!(rows.input_ids[0], vec![1, 0, 0, 0, 0]);
        // 第三行：audio_start=100
        assert_eq!(rows.input_ids[2], vec![100, 0, 0, 0, 0]);
        // 第四行：audio prefix，user_slot=102 在 col 0, audio codes 在 col 1..
        assert_eq!(rows.input_ids[3], vec![102, 20, 21, 22, 23]);
        assert_eq!(rows.input_ids[4], vec![102, 30, 31, 32, 33]);
        // 第六行：audio_end=101
        assert_eq!(rows.input_ids[5], vec![101, 0, 0, 0, 0]);
        // 最后一行：第二个 audio_start
        assert_eq!(rows.input_ids[12], vec![100, 0, 0, 0, 0]);

        // attention_mask 全 1
        assert!(rows.attention_mask.iter().all(|&v| v == 1));
    }

    #[test]
    fn empty_prompt_codes_still_valid() {
        let m = fixture_manifest();
        let rows = build_voice_clone_request_rows(&m, &[], &[200]);
        // prefix(2 user_prefix + 1 audio_start = 3) + 0 audio
        // + suffix(1 audio_end + 1 after_reference + 1 text + 2 assistant_prefix + 1 audio_start = 6)
        // = 9
        assert_eq!(rows.input_ids.len(), 9);
    }
}
