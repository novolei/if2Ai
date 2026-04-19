//! 自回归 TTS 推理循环（Phase TTS-A.6 Spike #2c 主体）。
//!
//! 把以下 5 件事按 Python `OrtCpuRuntime.generate_audio_frames`
//! (`ort_cpu_runtime.py:621-786`) 1:1 串起来：
//!
//! 1. **Prefill**：[`GlobalSessions::run_prefill`] →
//!    `(global_hidden, past_by_name)`。
//! 2. **每帧**循环 `max_new_frames` 次：
//!    - 选一种 sample mode 路径生成 `frame: Vec<i32>`：
//!      - greedy（do_sample=false + 有 `local_greedy_frame`）
//!      - fixed（sample_mode=fixed + 有 `local_fixed_sampled_frame`）
//!      - full（sample_mode=full + 有 `local_cached_step`）
//!      - fallback：`local_decoder` 逐 channel 采样
//!    - 把 frame 包成 `[1, 1, n_vq+1]` int32 row 喂给
//!      [`GlobalSessions::run_decode_step`]，更新 `global_hidden` + past_by_name。
//! 3. **on_frame** callback：流式 codec 每生成一帧立即喂给 codec_decode_step。
//! 4. End-token：text 采到 audio_end_token_id 时直接 break。
//! 5. 返回 `Vec<Vec<i32>>` 全部 frames，由调用方喂 codec_decode_full / streaming。

#![allow(dead_code)]

use std::collections::HashSet;

use ndarray::{ArrayD, IxDyn};
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::modules::tts::config::GenerationParams;
use crate::modules::tts::error::TtsError;
use crate::modules::tts::inference::request_builder::VoiceCloneRequestRows;
use crate::modules::tts::inference::sampling::{
    sample_assistant_text_token, sample_audio_token, SamplingParams,
};
use crate::modules::tts::manifest::ManifestBundle;
use crate::modules::tts::model::global::GlobalSessions;
use crate::modules::tts::model::local::LocalSessions;

/// 一次合成请求的协议常量（从 manifest 抽出来供 hot loop 使用，避免每帧都查 HashMap）。
#[derive(Debug, Clone, Copy)]
struct ProtoConst {
    n_vq: usize,
    audio_pad: i32,
    audio_assistant_slot: i32,
    audio_end: i32,
    audio_codebook_size: usize,
}

impl ProtoConst {
    fn from_manifest(manifest: &ManifestBundle) -> Self {
        let cfg = manifest.tts_config();
        let codebook = manifest
            .tts_meta
            .model_config
            .audio_codebook_sizes
            .first()
            .copied()
            .unwrap_or(1024);
        Self {
            n_vq: cfg.n_vq,
            audio_pad: cfg.audio_pad_token_id,
            audio_assistant_slot: cfg.audio_assistant_slot_token_id,
            audio_end: cfg.audio_end_token_id,
            audio_codebook_size: codebook,
        }
    }
}

/// 选择一种 sample mode 执行路径（基于 generation params + 已加载的 sessions）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SamplePath {
    Greedy,
    Fixed,
    Full,
    Fallback,
}

fn choose_sample_path(
    generation: &GenerationParams,
    local: &LocalSessions,
    manifest_sample_mode: &str,
) -> SamplePath {
    let do_sample = generation.do_sample;
    if !do_sample && local.greedy_frame.is_some() {
        return SamplePath::Greedy;
    }
    if local.fixed_sampleed_frame.is_some() && manifest_sample_mode == "fixed" {
        return SamplePath::Fixed;
    }
    if local.cached_step.is_some() && manifest_sample_mode == "full" {
        return SamplePath::Full;
    }
    SamplePath::Fallback
}

/// 帧回调：流式 codec decode 在每生成一个新帧时被调用。
pub type OnFrameCallback<'a> =
    Box<dyn FnMut(&[Vec<i32>], usize, &[i32]) -> Result<(), TtsError> + 'a>;

/// 跑一次完整的 TTS 自回归生成。
///
/// 返回 `Vec<Vec<i32>>` 是所有生成的 audio frames（不含 prompt prefix）。
pub fn generate_audio_frames(
    manifest: &ManifestBundle,
    sessions: &mut GlobalSessions,
    local: &mut LocalSessions,
    generation: &GenerationParams,
    request: &VoiceCloneRequestRows,
    seed: Option<u64>,
    mut on_frame: Option<OnFrameCallback<'_>>,
) -> Result<Vec<Vec<i32>>, TtsError> {
    let proto = ProtoConst::from_manifest(manifest);
    let row_width = proto.n_vq + 1;
    let max_new_frames = generation.max_new_frames as usize;
    let manifest_sample_mode = manifest
        .manifest
        .generation_defaults
        .sample_mode
        .clone()
        .unwrap_or_else(|| "fixed".to_string());

    let sampling_text = SamplingParams::for_text(generation);
    let sampling_audio = SamplingParams::for_audio(generation);
    let mut rng: StdRng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::seed_from_u64(1234), // 与 Python `np.random.default_rng(1234)` 一致默认
    };
    let path = choose_sample_path(generation, local, &manifest_sample_mode);

    // ── Step 1: prefill ────────────────────────────────────────────────
    let input_ids_arr = build_prefill_input_ids(&request.input_ids, row_width)?;
    let attention_mask_arr = ArrayD::<i32>::from_shape_vec(
        IxDyn(&[1, request.attention_mask.len()]),
        request.attention_mask.clone(),
    )
    .map_err(|e| TtsError::OnnxError(format!("attention_mask shape: {e}")))?;

    let prefill = sessions.run_prefill(input_ids_arr, attention_mask_arr)?;
    let mut global_hidden = prefill.global_hidden;
    let mut past_by_name = prefill.past_by_name;
    let mut past_valid_length: i32 = request.attention_mask.iter().sum::<i32>().max(0);

    // ── Step 2: 自回归循环 ──────────────────────────────────────────────
    let mut generated: Vec<Vec<i32>> = Vec::new();
    let mut prev_tokens_per_channel: Vec<Vec<u32>> = vec![Vec::new(); proto.n_vq];
    let mut prev_set_per_channel: Vec<HashSet<u32>> = vec![HashSet::new(); proto.n_vq];

    for step_index in 0..max_new_frames {
        let frame_result = match path {
            SamplePath::Greedy => sample_via_greedy(
                local,
                &global_hidden,
                &prev_set_per_channel,
                &proto,
                generation.audio_repetition_penalty,
            )?,
            SamplePath::Fixed => sample_via_fixed(
                local,
                &global_hidden,
                &prev_set_per_channel,
                &proto,
                &mut rng,
            )?,
            SamplePath::Full => sample_via_full(
                local,
                manifest,
                &global_hidden,
                &prev_tokens_per_channel,
                &prev_set_per_channel,
                &proto,
                &sampling_text,
                &sampling_audio,
                generation.audio_repetition_penalty,
                &mut rng,
            )?,
            SamplePath::Fallback => sample_via_fallback(
                local,
                &global_hidden,
                &prev_tokens_per_channel,
                &prev_set_per_channel,
                &proto,
                &sampling_text,
                &sampling_audio,
                generation.audio_repetition_penalty,
                &mut rng,
            )?,
        };
        let (should_continue, frame) = match frame_result {
            Some(pair) => pair,
            None => break, // text token 不是 assistant_slot → 终止
        };
        if !should_continue {
            break;
        }
        if frame.len() != proto.n_vq {
            return Err(TtsError::OnnxError(format!(
                "采样得到的 frame 长度 {} != n_vq {}",
                frame.len(),
                proto.n_vq
            )));
        }

        // 更新 previous tokens
        for (channel_index, &token) in frame.iter().enumerate() {
            let token_u32 = token.max(0) as u32;
            prev_tokens_per_channel[channel_index].push(token_u32);
            prev_set_per_channel[channel_index].insert(token_u32);
        }
        generated.push(frame.clone());

        // ── Step 3: 把当前 frame 喂给 decode_step → 更新 global_hidden + past
        let mut next_row = ArrayD::<i32>::from_elem(IxDyn(&[1, 1, row_width]), proto.audio_pad);
        next_row[[0, 0, 0]] = proto.audio_assistant_slot;
        for (i, &token) in frame.iter().enumerate() {
            next_row[[0, 0, i + 1]] = token;
        }
        let decode_out = sessions.run_decode_step(next_row, past_valid_length, past_by_name)?;
        global_hidden = decode_out.global_hidden;
        past_by_name = decode_out.past_by_name;
        past_valid_length += 1;

        if let Some(cb) = on_frame.as_mut() {
            cb(&generated, step_index, &frame)?;
        }
    }

    Ok(generated)
}

/// 把 [L, n_vq+1] 的 i32 二维 vec 转成 [1, L, n_vq+1] ndarray。
fn build_prefill_input_ids(rows: &[Vec<i32>], row_width: usize) -> Result<ArrayD<i32>, TtsError> {
    let l = rows.len();
    let mut data = Vec::with_capacity(l * row_width);
    for row in rows {
        if row.len() != row_width {
            return Err(TtsError::OnnxError(format!(
                "request row 长度 {} != row_width {}",
                row.len(),
                row_width
            )));
        }
        data.extend_from_slice(row);
    }
    ArrayD::<i32>::from_shape_vec(IxDyn(&[1, l, row_width]), data)
        .map_err(|e| TtsError::OnnxError(format!("input_ids shape: {e}")))
}

// ── sample-mode 子路径 ──────────────────────────────────────────────────────

fn sample_via_greedy(
    local: &mut LocalSessions,
    global_hidden: &ArrayD<f32>,
    prev_set: &[HashSet<u32>],
    proto: &ProtoConst,
    repetition_penalty: f32,
) -> Result<Option<(bool, Vec<i32>)>, TtsError> {
    let (cont, frame) = local.run_local_greedy_frame(
        global_hidden,
        prev_set,
        repetition_penalty,
        proto.n_vq,
        proto.audio_codebook_size,
    )?;
    Ok(Some((cont, frame)))
}

fn sample_via_fixed(
    local: &mut LocalSessions,
    global_hidden: &ArrayD<f32>,
    prev_set: &[HashSet<u32>],
    proto: &ProtoConst,
    rng: &mut StdRng,
) -> Result<Option<(bool, Vec<i32>)>, TtsError> {
    let (cont, frame) = local.run_local_fixed_sampled_frame(
        global_hidden,
        prev_set,
        proto.n_vq,
        proto.audio_codebook_size,
        rng,
    )?;
    Ok(Some((cont, frame)))
}

#[allow(clippy::too_many_arguments)]
fn sample_via_full(
    local: &mut LocalSessions,
    manifest: &ManifestBundle,
    global_hidden: &ArrayD<f32>,
    prev_tokens: &[Vec<u32>],
    prev_set: &[HashSet<u32>],
    proto: &ProtoConst,
    sampling_text: &SamplingParams,
    sampling_audio: &SamplingParams,
    repetition_penalty: f32,
    rng: &mut StdRng,
) -> Result<Option<(bool, Vec<i32>)>, TtsError> {
    // step_type=0：text 步
    let mut local_past = local.create_empty_local_cached_past(manifest);
    let mut local_valid_len: i32 = 0;
    let (text_logits, _ignored, next_past) =
        local.run_local_cached_step(global_hidden, 0, 0, 0, 0, local_valid_len, local_past)?;
    local_past = next_past;
    local_valid_len += 1;

    let next_text_token = sample_assistant_text_token(
        &text_logits,
        proto.audio_assistant_slot as u32,
        proto.audio_end as u32,
        sampling_text,
        rng,
    )?;
    if next_text_token != proto.audio_assistant_slot as u32 {
        return Ok(None);
    }

    // step_type=1：audio first channel
    let (_unused, audio_logits, next_past) = local.run_local_cached_step(
        global_hidden,
        next_text_token as i32,
        0,
        0,
        1,
        local_valid_len,
        local_past,
    )?;
    local_past = next_past;
    local_valid_len += 1;

    let mut frame: Vec<i32> = Vec::with_capacity(proto.n_vq);
    let first_logits = slice_audio_channel_logits(&audio_logits, 0, proto.n_vq)?;
    let sampled = sample_audio_token(
        &first_logits,
        &prev_tokens[0],
        &prev_set[0],
        sampling_audio,
        repetition_penalty,
        rng,
    )?;
    let mut prev_token = sampled as i32;
    frame.push(prev_token);

    // step_type=2：channel 1..n_vq
    for channel_index in 1..proto.n_vq {
        let (_unused, audio_logits, next_past) = local.run_local_cached_step(
            global_hidden,
            0,
            prev_token,
            (channel_index - 1) as i32,
            2,
            local_valid_len,
            local_past,
        )?;
        local_past = next_past;
        local_valid_len += 1;

        let logits = slice_audio_channel_logits(&audio_logits, channel_index, proto.n_vq)?;
        let sampled = sample_audio_token(
            &logits,
            &prev_tokens[channel_index],
            &prev_set[channel_index],
            sampling_audio,
            repetition_penalty,
            rng,
        )?;
        prev_token = sampled as i32;
        frame.push(prev_token);
    }

    let _ = local_past; // 仅 hot loop 内复用，本次 step 结束抛弃
    Ok(Some((true, frame)))
}

#[allow(clippy::too_many_arguments)]
fn sample_via_fallback(
    local: &mut LocalSessions,
    global_hidden: &ArrayD<f32>,
    prev_tokens: &[Vec<u32>],
    prev_set: &[HashSet<u32>],
    proto: &ProtoConst,
    sampling_text: &SamplingParams,
    sampling_audio: &SamplingParams,
    repetition_penalty: f32,
    rng: &mut StdRng,
) -> Result<Option<(bool, Vec<i32>)>, TtsError> {
    // 第一步：用 audio_pad text_token + 空 prefix 拿 text_logits
    let (text_logits, _) =
        local.run_local_decoder(global_hidden, 0, &[], proto.n_vq, proto.audio_pad)?;
    let next_text_token = sample_assistant_text_token(
        &text_logits,
        proto.audio_assistant_slot as u32,
        proto.audio_end as u32,
        sampling_text,
        rng,
    )?;
    if next_text_token != proto.audio_assistant_slot as u32 {
        return Ok(None);
    }

    let mut frame: Vec<i32> = Vec::with_capacity(proto.n_vq);
    for channel_index in 0..proto.n_vq {
        let (_unused, audio_logits) = local.run_local_decoder(
            global_hidden,
            next_text_token as i32,
            &frame,
            proto.n_vq,
            proto.audio_pad,
        )?;
        let logits = slice_audio_channel_logits(&audio_logits, channel_index, proto.n_vq)?;
        let sampled = sample_audio_token(
            &logits,
            &prev_tokens[channel_index],
            &prev_set[channel_index],
            sampling_audio,
            repetition_penalty,
            rng,
        )?;
        frame.push(sampled as i32);
    }
    Ok(Some((true, frame)))
}

/// 从扁平的 audio_logits 切出某个 channel 的 logits 子切片。
///
/// 镜像 Python `slice_audio_channel_logits` (`ort_cpu_runtime.py:598-603`)。
fn slice_audio_channel_logits(
    audio_logits: &[f32],
    channel_index: usize,
    n_vq: usize,
) -> Result<Vec<f32>, TtsError> {
    if audio_logits.is_empty() {
        return Err(TtsError::OnnxError("audio_logits 为空".into()));
    }
    if n_vq == 0 {
        return Err(TtsError::OnnxError("n_vq=0".into()));
    }
    let per_channel = audio_logits.len() / n_vq;
    if per_channel == 0 {
        return Err(TtsError::OnnxError(format!(
            "audio_logits 长度 {} 不能被 n_vq {} 整除",
            audio_logits.len(),
            n_vq
        )));
    }
    let start = channel_index * per_channel;
    let end = start + per_channel;
    if end > audio_logits.len() {
        return Err(TtsError::OnnxError(format!(
            "channel_index {channel_index} 越界: end={end} > {}",
            audio_logits.len()
        )));
    }
    Ok(audio_logits[start..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prefill_input_ids_shape_correct() {
        let rows = vec![vec![1, 2, 3, 4, 5], vec![6, 7, 8, 9, 10]];
        let arr = build_prefill_input_ids(&rows, 5).unwrap();
        assert_eq!(arr.shape(), &[1, 2, 5]);
        assert_eq!(arr[[0, 0, 0]], 1);
        assert_eq!(arr[[0, 1, 4]], 10);
    }

    #[test]
    fn build_prefill_rejects_mismatched_row_width() {
        let rows = vec![vec![1, 2, 3, 4, 5], vec![6, 7]];
        assert!(build_prefill_input_ids(&rows, 5).is_err());
    }

    #[test]
    fn slice_audio_channel_logits_chunks_correctly() {
        // 4 channels × 3 codebook entries each
        let logits = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ];
        let ch1 = slice_audio_channel_logits(&logits, 1, 4).unwrap();
        assert_eq!(ch1, vec![4.0, 5.0, 6.0]);
    }

    #[test]
    fn slice_audio_channel_logits_rejects_oob_channel() {
        let logits = vec![1.0; 12];
        assert!(slice_audio_channel_logits(&logits, 5, 4).is_err());
    }
}
