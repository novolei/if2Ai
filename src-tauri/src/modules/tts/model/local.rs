//! Local ONNX sessions for the TTS model.
//!
//! Mirrors `_create_sessions()` from `ort_cpu_runtime.py` for the
//! local decoder sessions: `local_decoder`, `local_cached_step`,
//! and `local_fixed_sampleed_frame`.
//!
//! These sessions handle per-frame audio token generation after
//! the global prefill/decode has produced `global_hidden`.
//!
//! ## Session Reference
//!
//! | Session                  | Python Key             | Key Inputs                                              | Key Outputs                          |
//! |--------------------------|------------------------|---------------------------------------------------------|--------------------------------------|
//! | **decoder**              | local_decoder          | global_hidden, text_token_id, audio_prefix_token_ids    | text_logits, audio_logits            |
//! | **cached_step**          | local_cached_step      | global_hidden, text_token_id, audio_token_id, channel_index, step_type, past_valid_lengths, local_past_* | text_logits, audio_logits, local_present_* |
//! | **fixed_sampled_frame**  | local_fixed_sampled_frame | global_hidden, repetition_seen_mask, assistant_random_u, audio_random_u | should_continue, frame_token_ids |
//!
//! The local sessions are optional — they may not exist in all model
//! variants. Loading returns `None` for missing sessions rather than
//! failing, matching the Python `**{...} if ... else {}` pattern.

// Justification: constants and struct fields are defined here for TTS-2.2
// and will be consumed by TTS-3.x inference slices.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use ndarray::{ArrayD, IxDyn};

use crate::modules::tts::error::TtsError;
use crate::modules::tts::manifest::ManifestBundle;
use crate::modules::tts::model::global::OnnxSession;

/// `(text_logits, audio_logits, next_local_past)` — `local_cached_step` 输出三元组。
pub type LocalCachedStepOutput = (Vec<f32>, Vec<f32>, HashMap<String, ArrayD<f32>>);

/// `(per_channel_pcm, valid_samples, elapsed_seconds)` —— 单 chunk decode_full 结果。
pub type ChunkDecodeOutput = (Vec<Vec<i32>>, Vec<Vec<f32>>, usize, f32);
use crate::modules::tts::model::ort_io::{
    extract_f32_owned, extract_scalar_i32, extract_vec_f32, extract_vec_i32, f32_scalar,
    f32_tensor, i32_scalar, i32_tensor,
};

/// ONNX file name for the local decoder.
pub const LOCAL_DECODER_ONNX: &str = "decoder";

/// ONNX file name for the local cached step.
pub const LOCAL_CACHED_STEP_ONNX: &str = "local_cached_step";

/// ONNX file name for the local fixed sampled frame.
pub const LOCAL_FIXED_SAMPLED_FRAME_ONNX: &str = "local_fixed_sampled_frame";

/// Input name for the global hidden state (shared by all local sessions).
pub const LOCAL_GLOBAL_HIDDEN: &str = "global_hidden";

/// Input name for the text token ID.
pub const LOCAL_TEXT_TOKEN_ID: &str = "text_token_id";

/// Input name for the audio prefix token IDs (decoder).
pub const LOCAL_AUDIO_PREFIX_TOKEN_IDS: &str = "audio_prefix_token_ids";

/// Output name for text logits (shared by decoder and cached_step).
pub const LOCAL_TEXT_LOGITS: &str = "text_logits";

/// Output name for audio logits (shared by decoder and cached_step).
pub const LOCAL_AUDIO_LOGITS: &str = "audio_logits";

/// Prefix for local KV cache output tensors (renamed to local_past_* for next step).
pub const LOCAL_PRESENT_PREFIX: &str = "local_present_";

/// Prefix for local KV cache input tensors (received from previous step as local_present_*).
pub const LOCAL_PAST_PREFIX: &str = "local_past_";

/// Local ONNX sessions — decoder, cached_step, and fixed_sampled_frame.
///
/// These sessions are created per synthesis request (not shared globally)
/// because they are used within a single generation loop.
///
/// Mirrors `self.sessions["local_decoder"]`, `self.sessions["local_cached_step"]`,
/// and `self.sessions["local_fixed_sampled_frame"]` from `OrtCpuRuntime`
/// in `ort_cpu_runtime.py`.
pub struct LocalSessions {
    /// Local decoder: global_hidden + text_token_id + audio_prefix → logits.
    pub decoder: Option<OnnxSession>,
    /// Local cached step: incremental decode with local KV cache.
    pub cached_step: Option<OnnxSession>,
    /// Local fixed sampled frame: batch-sampled frame generation.
    pub fixed_sampleed_frame: Option<OnnxSession>,
    /// Phase TTS-A.6 新增：local greedy frame (do_sample=false 路径)。
    pub greedy_frame: Option<OnnxSession>,
}

impl LocalSessions {
    /// Create local sessions from the TTS model directory.
    ///
    /// Each session is loaded only if the corresponding `.onnx` file exists.
    /// Missing sessions result in `None` rather than an error, matching
    /// the Python conditional session creation pattern.
    ///
    /// Mirrors `_create_sessions()` from `ort_cpu_runtime.py:360-370`
    /// for the "local_decoder", "local_cached_step", and
    /// "local_fixed_sampled_frame" entries.
    pub fn load(tts_model_dir: &Path, thread_count: usize) -> Result<Self, TtsError> {
        let decoder_path = tts_model_dir.join("decoder.onnx");
        let cached_step_path = tts_model_dir.join("local_cached_step.onnx");
        let fixed_path = tts_model_dir.join("local_fixed_sampled_frame.onnx");

        let decoder = if decoder_path.exists() {
            Some(OnnxSession::load(&decoder_path, thread_count)?)
        } else {
            None
        };

        let cached_step = if cached_step_path.exists() {
            Some(OnnxSession::load(&cached_step_path, thread_count)?)
        } else {
            None
        };

        let fixed_sampleed_frame = if fixed_path.exists() {
            Some(OnnxSession::load(&fixed_path, thread_count)?)
        } else {
            None
        };

        Ok(Self {
            decoder,
            cached_step,
            fixed_sampleed_frame,
            greedy_frame: None,
        })
    }

    /// Phase TTS-A.2/A.6：从 manifest 给出的文件名加载（含 `local_greedy_frame`）。
    pub fn load_from_manifest(
        manifest: &ManifestBundle,
        thread_count: usize,
    ) -> Result<Self, TtsError> {
        let try_load = |key: &str| -> Result<Option<OnnxSession>, TtsError> {
            match manifest.tts_onnx_path(key) {
                Ok(path) if path.exists() => Ok(Some(OnnxSession::load(&path, thread_count)?)),
                _ => Ok(None),
            }
        };
        Ok(Self {
            decoder: try_load("local_decoder")?,
            cached_step: try_load("local_cached_step")?,
            fixed_sampleed_frame: try_load("local_fixed_sampled_frame")?,
            greedy_frame: try_load("local_greedy_frame")?,
        })
    }

    // ─────────────────────────────────────────────────────────────────────
    // Phase TTS-A.6 Spike #2b：4 条 sample-mode 真 ONNX 调用
    // ─────────────────────────────────────────────────────────────────────

    /// `local_decoder` 路径：返回 (text_logits, audio_logits)。
    ///
    /// 镜像 Python `run_local_decoder` (`ort_cpu_runtime.py:478-494`)。
    /// audio_prefix_token_ids 形状 `[1, n_vq-1]`，不足填 audio_pad。
    pub fn run_local_decoder(
        &mut self,
        global_hidden: &ArrayD<f32>,
        text_token_id: i32,
        frame_prefix: &[i32],
        n_vq: usize,
        audio_pad: i32,
    ) -> Result<(Vec<f32>, Vec<f32>), TtsError> {
        let session = self
            .decoder
            .as_mut()
            .ok_or_else(|| TtsError::OnnxError("local_decoder session 未加载".into()))?;

        let prefix_len = n_vq.saturating_sub(1);
        let mut prefix = vec![audio_pad; prefix_len];
        for (i, &token) in frame_prefix.iter().take(prefix_len).enumerate() {
            prefix[i] = token;
        }
        let prefix_arr = ArrayD::<i32>::from_shape_vec(IxDyn(&[1, prefix_len]), prefix)
            .map_err(|e| TtsError::OnnxError(format!("audio_prefix shape: {e}")))?;

        let inputs = ort::inputs![
            "global_hidden" => f32_tensor("global_hidden", global_hidden.clone())?,
            "text_token_id" => i32_scalar("text_token_id", text_token_id)?,
            "audio_prefix_token_ids" => i32_tensor("audio_prefix_token_ids", prefix_arr)?,
        ];
        let outputs = session
            .session_mut()
            .run(inputs)
            .map_err(|e| TtsError::OnnxError(format!("local_decoder.run: {e}")))?;

        let text_logits = extract_vec_f32(
            outputs
                .get("text_logits")
                .ok_or_else(|| TtsError::OnnxError("local_decoder 缺 text_logits".into()))?,
            "text_logits",
        )?;
        let audio_logits = extract_vec_f32(
            outputs
                .get("audio_logits")
                .ok_or_else(|| TtsError::OnnxError("local_decoder 缺 audio_logits".into()))?,
            "audio_logits",
        )?;
        Ok((text_logits, audio_logits))
    }

    /// `local_cached_step` 路径：返回 (text_logits, audio_logits, next_local_past)。
    ///
    /// 镜像 Python `run_local_cached_step` (`ort_cpu_runtime.py:506-535`)。
    #[allow(clippy::too_many_arguments)]
    pub fn run_local_cached_step(
        &mut self,
        global_hidden: &ArrayD<f32>,
        text_token_id: i32,
        audio_token_id: i32,
        channel_index: i32,
        step_type: i32,
        past_valid_lengths: i32,
        local_past: HashMap<String, ArrayD<f32>>,
    ) -> Result<LocalCachedStepOutput, TtsError> {
        let session = self
            .cached_step
            .as_mut()
            .ok_or_else(|| TtsError::OnnxError("local_cached_step session 未加载".into()))?;

        // 预先抓输出名（释放 mut 借用前）
        let kv_names: Vec<String> = session.output_names().iter().skip(2).cloned().collect();

        let mut inputs = ort::inputs![
            "global_hidden" => f32_tensor("global_hidden", global_hidden.clone())?,
            "text_token_id" => i32_scalar("text_token_id", text_token_id)?,
            "audio_token_id" => i32_scalar("audio_token_id", audio_token_id)?,
            "channel_index" => i32_scalar("channel_index", channel_index)?,
            "step_type" => i32_scalar("step_type", step_type)?,
            "past_valid_lengths" => i32_scalar("past_valid_lengths", past_valid_lengths)?,
        ];
        for (name, arr) in local_past {
            let t = f32_tensor(&name, arr)?;
            inputs.push((std::borrow::Cow::Owned(name), t.into()));
        }

        let outputs = session
            .session_mut()
            .run(inputs)
            .map_err(|e| TtsError::OnnxError(format!("local_cached_step.run: {e}")))?;

        let text_logits = extract_vec_f32(
            outputs
                .get("text_logits")
                .ok_or_else(|| TtsError::OnnxError("local_cached_step 缺 text_logits".into()))?,
            "text_logits",
        )?;
        let audio_logits = extract_vec_f32(
            outputs
                .get("audio_logits")
                .ok_or_else(|| TtsError::OnnxError("local_cached_step 缺 audio_logits".into()))?,
            "audio_logits",
        )?;

        // 收集 local_present_* → 重命名为 local_past_*
        let mut next_past: HashMap<String, ArrayD<f32>> = HashMap::with_capacity(kv_names.len());
        for name in &kv_names {
            let v = outputs
                .get(name.as_str())
                .ok_or_else(|| TtsError::OnnxError(format!("local_cached_step 缺 {name}")))?;
            let owned = extract_f32_owned(v, name)?;
            next_past.insert(local_present_to_past(name), owned);
        }
        Ok((text_logits, audio_logits, next_past))
    }

    /// `local_fixed_sampled_frame` 路径：返回 (should_continue, frame_token_ids)。
    ///
    /// 镜像 Python `run_local_fixed_sampled_frame` (`ort_cpu_runtime.py:565-596`)。
    pub fn run_local_fixed_sampled_frame(
        &mut self,
        global_hidden: &ArrayD<f32>,
        previous_token_sets_by_channel: &[std::collections::HashSet<u32>],
        n_vq: usize,
        audio_codebook_size: usize,
        rng: &mut rand::rngs::StdRng,
    ) -> Result<(bool, Vec<i32>), TtsError> {
        use rand::Rng;
        let session = self.fixed_sampleed_frame.as_mut().ok_or_else(|| {
            TtsError::OnnxError("local_fixed_sampled_frame session 未加载".into())
        })?;

        let repetition_seen_mask =
            build_repetition_seen_mask(previous_token_sets_by_channel, n_vq, audio_codebook_size)?;

        let assistant_random_u_arr =
            ArrayD::<f32>::from_shape_vec(IxDyn(&[1]), vec![clamp_random(rng.gen::<f32>())])
                .map_err(|e| TtsError::OnnxError(format!("assistant_random_u shape: {e}")))?;

        let mut audio_random_u_data = Vec::with_capacity(n_vq);
        for _ in 0..n_vq {
            audio_random_u_data.push(clamp_random(rng.gen::<f32>()));
        }
        let audio_random_u_arr =
            ArrayD::<f32>::from_shape_vec(IxDyn(&[1, n_vq]), audio_random_u_data)
                .map_err(|e| TtsError::OnnxError(format!("audio_random_u shape: {e}")))?;

        let inputs = ort::inputs![
            "global_hidden" => f32_tensor("global_hidden", global_hidden.clone())?,
            "repetition_seen_mask" => i32_tensor("repetition_seen_mask", repetition_seen_mask)?,
            "assistant_random_u" => f32_tensor("assistant_random_u", assistant_random_u_arr)?,
            "audio_random_u" => f32_tensor("audio_random_u", audio_random_u_arr)?,
        ];

        let outputs = session
            .session_mut()
            .run(inputs)
            .map_err(|e| TtsError::OnnxError(format!("local_fixed_sampled_frame.run: {e}")))?;

        let should_continue = extract_scalar_i32(
            outputs
                .get("should_continue")
                .ok_or_else(|| TtsError::OnnxError("缺 should_continue".into()))?,
            "should_continue",
        )? != 0;
        let frame_token_ids = extract_vec_i32(
            outputs
                .get("frame_token_ids")
                .ok_or_else(|| TtsError::OnnxError("缺 frame_token_ids".into()))?,
            "frame_token_ids",
        )?;
        Ok((should_continue, frame_token_ids))
    }

    /// `local_greedy_frame` 路径：返回 (should_continue, frame_token_ids)。
    ///
    /// 镜像 Python `run_local_greedy_frame` (`ort_cpu_runtime.py:537-563`)。
    pub fn run_local_greedy_frame(
        &mut self,
        global_hidden: &ArrayD<f32>,
        previous_token_sets_by_channel: &[std::collections::HashSet<u32>],
        repetition_penalty: f32,
        n_vq: usize,
        audio_codebook_size: usize,
    ) -> Result<(bool, Vec<i32>), TtsError> {
        let session = self
            .greedy_frame
            .as_mut()
            .ok_or_else(|| TtsError::OnnxError("local_greedy_frame session 未加载".into()))?;

        let repetition_seen_mask =
            build_repetition_seen_mask(previous_token_sets_by_channel, n_vq, audio_codebook_size)?;

        let inputs = ort::inputs![
            "global_hidden" => f32_tensor("global_hidden", global_hidden.clone())?,
            "repetition_seen_mask" => i32_tensor("repetition_seen_mask", repetition_seen_mask)?,
            "repetition_penalty" => f32_scalar("repetition_penalty", repetition_penalty)?,
        ];
        let outputs = session
            .session_mut()
            .run(inputs)
            .map_err(|e| TtsError::OnnxError(format!("local_greedy_frame.run: {e}")))?;

        let should_continue = extract_scalar_i32(
            outputs
                .get("should_continue")
                .ok_or_else(|| TtsError::OnnxError("缺 should_continue".into()))?,
            "should_continue",
        )? != 0;
        let frame_token_ids = extract_vec_i32(
            outputs
                .get("frame_token_ids")
                .ok_or_else(|| TtsError::OnnxError("缺 frame_token_ids".into()))?,
            "frame_token_ids",
        )?;
        Ok((should_continue, frame_token_ids))
    }

    /// 构造空的 `local_cached_step` past 字典（全零 [1, 0, heads, head_dim]）。
    ///
    /// 镜像 `create_empty_local_cached_past` (`ort_cpu_runtime.py:496-504`)。
    pub fn create_empty_local_cached_past(
        &self,
        manifest: &ManifestBundle,
    ) -> HashMap<String, ArrayD<f32>> {
        let local_layers = manifest.tts_meta.model_config.local_layers.unwrap_or(0);
        let local_heads = manifest.tts_meta.model_config.local_heads.unwrap_or(0);
        let local_head_dim = manifest.tts_meta.model_config.local_head_dim.unwrap_or(0);
        let mut past = HashMap::with_capacity(local_layers * 2);
        for layer_index in 0..local_layers {
            for kind in &["key", "value"] {
                let name = format!("local_past_{kind}_{layer_index}");
                let arr = ArrayD::<f32>::zeros(IxDyn(&[1, 0, local_heads, local_head_dim]));
                past.insert(name, arr);
            }
        }
        past
    }

    /// Dump session I/O names for verification against Python reference.
    pub fn dump_io_names(&self) {
        if let Some(s) = self.decoder.as_ref() {
            tracing::info!("local_decoder inputs: {:?}", s.input_names());
            tracing::info!("local_decoder outputs: {:?}", s.output_names());
        }
        if let Some(s) = self.cached_step.as_ref() {
            tracing::info!("local_cached_step inputs: {:?}", s.input_names());
            tracing::info!("local_cached_step outputs: {:?}", s.output_names());
        }
        if let Some(s) = self.fixed_sampleed_frame.as_ref() {
            tracing::info!("local_fixed_sampled_frame inputs: {:?}", s.input_names());
            tracing::info!("local_fixed_sampled_frame outputs: {:?}", s.output_names());
        }
        if let Some(s) = self.greedy_frame.as_ref() {
            tracing::info!("local_greedy_frame inputs: {:?}", s.input_names());
            tracing::info!("local_greedy_frame outputs: {:?}", s.output_names());
        }
    }

    /// Check whether the core local sessions are available (decoder always required).
    pub fn is_ready(&self) -> bool {
        self.decoder.is_some()
    }
}

/// 构造 `repetition_seen_mask` 张量 `[1, n_vq, audio_codebook_size]` int32。
fn build_repetition_seen_mask(
    previous_token_sets_by_channel: &[std::collections::HashSet<u32>],
    n_vq: usize,
    audio_codebook_size: usize,
) -> Result<ArrayD<i32>, TtsError> {
    let mut mask = ArrayD::<i32>::zeros(IxDyn(&[1, n_vq, audio_codebook_size]));
    for (channel_index, token_set) in previous_token_sets_by_channel.iter().enumerate().take(n_vq) {
        for &token_id in token_set {
            let token_idx = token_id as usize;
            if token_idx < audio_codebook_size {
                mask[[0, channel_index, token_idx]] = 1;
            }
        }
    }
    Ok(mask)
}

fn clamp_random(value: f32) -> f32 {
    value.clamp(0.0, 0.999_999_94)
}

/// Rename a local cached_step `local_present_*` output name to the
/// corresponding `local_past_*` input name for the next step.
///
/// Mirrors `output_name.replace("local_present_", "local_past_")` from
/// `ort_cpu_runtime.py:532`.
pub fn local_present_to_past(name: &str) -> String {
    name.replacen("local_present_", "local_past_", 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_session_constants() {
        assert_eq!(LOCAL_GLOBAL_HIDDEN, "global_hidden");
        assert_eq!(LOCAL_TEXT_TOKEN_ID, "text_token_id");
        assert_eq!(LOCAL_AUDIO_PREFIX_TOKEN_IDS, "audio_prefix_token_ids");
        assert_eq!(LOCAL_TEXT_LOGITS, "text_logits");
        assert_eq!(LOCAL_AUDIO_LOGITS, "audio_logits");
    }

    #[test]
    fn local_name_prefixes() {
        assert!(LOCAL_PRESENT_PREFIX.starts_with("local_present_"));
        assert!(LOCAL_PAST_PREFIX.starts_with("local_past_"));
    }

    #[test]
    fn rename_local_present_to_past() {
        let key = "local_present_key_0";
        let past = local_present_to_past(key);
        assert_eq!(past, "local_past_key_0");

        let value = "local_present_value_5";
        let past = local_present_to_past(value);
        assert_eq!(past, "local_past_value_5");
    }

    #[test]
    fn load_missing_model_returns_none_sessions() {
        let result = LocalSessions::load(Path::new("/nonexistent"), 4);
        // When no model files exist, all sessions should be None.
        // The load itself should succeed (no error) since missing files are OK.
        assert!(
            result.is_ok(),
            "loading from nonexistent dir should not fail; sessions just None"
        );
        let sessions = result.unwrap();
        assert!(sessions.decoder.is_none());
        assert!(sessions.cached_step.is_none());
        assert!(sessions.fixed_sampleed_frame.is_none());
    }

    #[test]
    fn is_ready_all_missing() {
        let sessions = LocalSessions::load(Path::new("/nonexistent"), 4).unwrap();
        assert!(!sessions.is_ready());
    }
}
