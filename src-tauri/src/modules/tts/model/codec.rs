//! Codec (audio tokenizer) ONNX sessions for the TTS model.
//!
//! Mirrors `_create_sessions()` from `ort_cpu_runtime.py` for the
//! audio tokenizer sessions: `codec_encode`, `codec_decode`, and
//! `codec_decode_step`.
//!
//! The audio tokenizer converts between raw waveform (PCM) and
//! discrete audio codes (token IDs). It is used for:
//! - **Voice cloning**: encoding prompt audio → audio codes
//! - **Output generation**: decoding audio codes → waveform
//!
//! ## Session Reference
//!
//! | Session          | Python Key       | Key Inputs                         | Key Outputs                       |
//! |------------------|------------------|------------------------------------|-----------------------------------|
//! | **encode**       | codec_encode     | waveform [1,2,time], input_lengths | audio_codes [1,n_frames,16], audio_code_lengths |
//! | **decode_full**  | codec_decode     | audio_codes [1,n_frames,16], audio_code_lengths | audio [1,2,samples], audio_lengths |
//! | **decode_step**  | codec_decode_step | audio_codes + stateful KV caches  | audio, audio_lengths, updated caches |
//!
//! The decode_step session is stateful — it maintains transformer
//! and attention cache state between calls via the
//! `CodecStreamingDecodeSession` wrapper.

// Justification: CodecSessions methods/constants are consumed by future
// TTS synthesis slices (audio encoding/decoding pipeline).
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use ndarray::{ArrayD, IxDyn};

use crate::modules::tts::audio::streaming_decoder::CodecStreamingDecodeSession;
use crate::modules::tts::error::TtsError;
use crate::modules::tts::manifest::{CodecMeta, ManifestBundle};
use crate::modules::tts::model::global::OnnxSession;
use crate::modules::tts::model::ort_io::{
    extract_f32_owned, extract_i32_owned, extract_scalar_i32, f32_tensor, i32_tensor,
};

/// ONNX file name for the codec encoder.
pub const CODEC_ENCODE_ONNX: &str = "encode";

/// ONNX file name for the codec decoder (full/batch).
pub const CODEC_DECODE_FULL_ONNX: &str = "decode_full";

/// ONNX file name for the codec streaming decode step.
pub const CODEC_DECODE_STEP_ONNX: &str = "decode_step";

/// Input name for the waveform tensor.
pub const CODEC_WAVEFORM: &str = "waveform";

/// Input name for the waveform lengths.
pub const CODEC_INPUT_LENGTHS: &str = "input_lengths";

/// Input name for audio codes.
pub const CODEC_AUDIO_CODES: &str = "audio_codes";

/// Input name for audio code lengths.
pub const CODEC_AUDIO_CODE_LENGTHS: &str = "audio_code_lengths";

/// Output name for audio codes (from encoder).
pub const CODEC_AUDIO_CODES_OUT: &str = "audio_codes";

/// Output name for audio code lengths (from encoder).
pub const CODEC_AUDIO_CODE_LENGTHS_OUT: &str = "audio_code_lengths";

/// Output name for decoded audio waveform.
pub const CODEC_AUDIO_OUT: &str = "audio";

/// Output name for decoded audio lengths.
pub const CODEC_AUDIO_LENGTHS_OUT: &str = "audio_lengths";

/// Codec ONNX sessions — encode, decode_full, and decode_step.
///
/// These sessions handle audio tokenization for voice cloning
/// (encode prompt audio → codes) and audio synthesis output
/// (decode codes → waveform).
///
/// Mirrors `self.sessions["codec_encode"]`, `self.sessions["codec_decode"]`,
/// and `self.sessions["codec_decode_step"]` from `OrtCpuRuntime`
/// in `ort_cpu_runtime.py`.
pub struct CodecSessions {
    /// Audio encoder: waveform → audio codes.
    pub encode: Option<OnnxSession>,
    /// Full audio decoder: audio codes → waveform (batch).
    pub decode_full: Option<OnnxSession>,
    /// Streaming audio decoder: incremental decode with state.
    pub decode_step: Option<OnnxSession>,
}

impl CodecSessions {
    /// Create codec sessions from the audio tokenizer directory.
    ///
    /// Each session is loaded only if the corresponding `.onnx` file exists.
    /// Missing sessions result in `None` rather than an error.
    ///
    /// Mirrors `_create_sessions()` from `ort_cpu_runtime.py:376-378`
    /// for the "codec_encode", "codec_decode", and "codec_decode_step" entries.
    pub fn load(audio_tokenizer_dir: &Path, thread_count: usize) -> Result<Self, TtsError> {
        let encode_path = audio_tokenizer_dir.join("encode.onnx");
        let decode_full_path = audio_tokenizer_dir.join("decode_full.onnx");
        let decode_step_path = audio_tokenizer_dir.join("decode_step.onnx");

        let encode = if encode_path.exists() {
            Some(OnnxSession::load(&encode_path, thread_count)?)
        } else {
            None
        };

        let decode_full = if decode_full_path.exists() {
            Some(OnnxSession::load(&decode_full_path, thread_count)?)
        } else {
            None
        };

        let decode_step = if decode_step_path.exists() {
            Some(OnnxSession::load(&decode_step_path, thread_count)?)
        } else {
            None
        };

        Ok(Self {
            encode,
            decode_full,
            decode_step,
        })
    }

    /// Phase TTS-A.2：从 manifest 给出的文件名加载。
    pub fn load_from_manifest(
        manifest: &crate::modules::tts::manifest::ManifestBundle,
        thread_count: usize,
    ) -> Result<Self, TtsError> {
        let try_load = |key: &str| -> Result<Option<OnnxSession>, TtsError> {
            match manifest.codec_onnx_path(key) {
                Ok(path) if path.exists() => Ok(Some(OnnxSession::load(&path, thread_count)?)),
                _ => Ok(None),
            }
        };
        Ok(Self {
            encode: try_load("encode")?,
            decode_full: try_load("decode_full")?,
            decode_step: try_load("decode_step")?,
        })
    }

    /// Dump session I/O names for verification against Python reference.
    pub fn dump_io_names(&self) {
        if let Some(ref s) = self.encode {
            tracing::info!("codec_encode inputs: {:?}", s.input_names());
            tracing::info!("codec_encode outputs: {:?}", s.output_names());
        }
        if let Some(ref s) = self.decode_full {
            tracing::info!("codec_decode_full inputs: {:?}", s.input_names());
            tracing::info!("codec_decode_full outputs: {:?}", s.output_names());
        }
        if let Some(ref s) = self.decode_step {
            tracing::info!("codec_decode_step inputs: {:?}", s.input_names());
            tracing::info!("codec_decode_step outputs: {:?}", s.output_names());
        }
    }

    /// Check whether all codec sessions are available.
    pub fn is_ready(&self) -> bool {
        self.encode.is_some() && self.decode_full.is_some() && self.decode_step.is_some()
    }

    // ─────────────────────────────────────────────────────────────────────
    // Phase TTS-A.7 Spike #3a / #3b：codec encode + decode_full + streaming
    // ─────────────────────────────────────────────────────────────────────

    /// Codec 编码：把 `[1, channels, time]` f32 波形 → `Vec<Vec<i32>>` audio codes
    /// (frames × num_quantizers)。
    ///
    /// 镜像 Python `OnnxTtsRuntime.encode_reference_audio`
    /// (`onnx_tts_runtime.py:460-481`)。
    pub fn encode(
        &mut self,
        waveform: ArrayD<f32>,
        num_quantizers: usize,
    ) -> Result<Vec<Vec<i32>>, TtsError> {
        let session = self
            .encode
            .as_mut()
            .ok_or_else(|| TtsError::CodecError("codec encode session 未加载".into()))?;

        // input_lengths = [waveform.shape[-1]] int32
        let waveform_len = waveform
            .shape()
            .last()
            .copied()
            .ok_or_else(|| TtsError::CodecError("waveform shape 为空".into()))?;
        let input_lengths_arr =
            ArrayD::<i32>::from_shape_vec(IxDyn(&[1]), vec![waveform_len as i32])
                .map_err(|e| TtsError::CodecError(format!("input_lengths shape: {e}")))?;

        let inputs = ort::inputs![
            "waveform" => f32_tensor("waveform", waveform)?,
            "input_lengths" => i32_tensor("input_lengths", input_lengths_arr)?,
        ];
        let outputs = session
            .session_mut()
            .run(inputs)
            .map_err(|e| TtsError::CodecError(format!("codec_encode.run: {e}")))?;

        let audio_codes_dyn = outputs
            .get("audio_codes")
            .ok_or_else(|| TtsError::CodecError("codec_encode 缺 audio_codes".into()))?;
        let audio_codes = extract_i32_owned(audio_codes_dyn, "audio_codes")?;
        // shape 应为 [1, frames, num_quantizers]
        let dims = audio_codes.shape();
        if dims.len() != 3 || dims[0] != 1 {
            return Err(TtsError::CodecError(format!(
                "audio_codes shape 不符: {:?}",
                dims
            )));
        }

        let code_lengths_dyn = outputs
            .get("audio_code_lengths")
            .ok_or_else(|| TtsError::CodecError("codec_encode 缺 audio_code_lengths".into()))?;
        let code_length = extract_scalar_i32(code_lengths_dyn, "audio_code_lengths")? as usize;
        let frames_to_take = code_length.min(dims[1]);

        let mut frames: Vec<Vec<i32>> = Vec::with_capacity(frames_to_take);
        for f in 0..frames_to_take {
            let mut row = Vec::with_capacity(num_quantizers);
            for q in 0..num_quantizers.min(dims[2]) {
                row.push(audio_codes[[0, f, q]]);
            }
            frames.push(row);
        }
        Ok(frames)
    }

    /// Codec 全量解码：`Vec<Vec<i32>>` audio codes → channel-major PCM
    /// `[channels][samples]`（每 channel 独立向量）+ 总样本长度。
    ///
    /// 镜像 Python `OrtCpuRuntime.decode_full_audio`
    /// (`ort_cpu_runtime.py:605-619`)。
    pub fn decode_full(
        &mut self,
        generated_frames: &[Vec<i32>],
        num_quantizers: usize,
    ) -> Result<(Vec<Vec<f32>>, usize), TtsError> {
        if generated_frames.is_empty() {
            return Ok((Vec::new(), 0));
        }
        let session = self
            .decode_full
            .as_mut()
            .ok_or_else(|| TtsError::CodecError("codec decode_full session 未加载".into()))?;

        let frame_count = generated_frames.len();
        let mut audio_codes_arr = ArrayD::<i32>::zeros(IxDyn(&[1, frame_count, num_quantizers]));
        for (f, row) in generated_frames.iter().enumerate() {
            for q in 0..num_quantizers {
                audio_codes_arr[[0, f, q]] = row.get(q).copied().unwrap_or(0);
            }
        }
        let lengths_arr = ArrayD::<i32>::from_shape_vec(IxDyn(&[1]), vec![frame_count as i32])
            .map_err(|e| TtsError::CodecError(format!("audio_code_lengths shape: {e}")))?;

        let inputs = ort::inputs![
            "audio_codes" => i32_tensor("audio_codes", audio_codes_arr)?,
            "audio_code_lengths" => i32_tensor("audio_code_lengths", lengths_arr)?,
        ];
        let outputs = session
            .session_mut()
            .run(inputs)
            .map_err(|e| TtsError::CodecError(format!("codec_decode_full.run: {e}")))?;

        let audio_dyn = outputs
            .get("audio")
            .ok_or_else(|| TtsError::CodecError("codec_decode 缺 audio".into()))?;
        let audio = extract_f32_owned(audio_dyn, "audio")?;
        let dims = audio.shape().to_vec();
        if dims.len() != 3 || dims[0] != 1 {
            return Err(TtsError::CodecError(format!(
                "audio shape 不符（期望 [1, channels, samples]）: {:?}",
                dims
            )));
        }
        let channels = dims[1];
        let total_samples = dims[2];

        let lengths_dyn = outputs
            .get("audio_lengths")
            .ok_or_else(|| TtsError::CodecError("codec_decode 缺 audio_lengths".into()))?;
        let valid_samples = extract_scalar_i32(lengths_dyn, "audio_lengths")? as usize;
        let take = valid_samples.min(total_samples);

        let mut per_channel: Vec<Vec<f32>> = Vec::with_capacity(channels);
        for c in 0..channels {
            let mut col = Vec::with_capacity(take);
            for s in 0..take {
                col.push(audio[[0, c, s]]);
            }
            per_channel.push(col);
        }
        Ok((per_channel, take))
    }

    /// 流式 codec 单批运行：把 `frame_rows` 喂给 codec_decode_step，回灌
    /// state，返回 (channel-major PCM, valid_samples)。
    ///
    /// 镜像 Python `CodecStreamingDecodeSession.run_frames`
    /// (`ort_cpu_runtime.py:253-280`)。
    ///
    /// 内部委托给 [`CodecStreamingDecodeSession`] 处理 state 张量构造与回灌；
    /// 本函数只负责把 ort `Session::run` 串起来。
    pub fn run_streaming_frames(
        &mut self,
        streaming: &mut CodecStreamingDecodeSession,
        frame_rows: &[Vec<i32>],
        codec_meta: &CodecMeta,
    ) -> Result<Option<(Vec<Vec<f32>>, usize)>, TtsError> {
        if frame_rows.is_empty() {
            return Ok(None);
        }
        let session = self
            .decode_step
            .as_mut()
            .ok_or_else(|| TtsError::CodecError("codec decode_step session 未加载".into()))?;

        let run_inputs = streaming.build_run_inputs(frame_rows)?;

        // 构造 ort inputs：audio_codes + audio_code_lengths + 全部 state（i32 + f32）
        let mut inputs = ort::inputs![
            "audio_codes" => i32_tensor("audio_codes", run_inputs.audio_codes)?,
            "audio_code_lengths" => i32_tensor("audio_code_lengths", run_inputs.audio_code_lengths)?,
        ];
        for (name, arr) in run_inputs.state_i32 {
            let t = i32_tensor(&name, arr)?;
            inputs.push((std::borrow::Cow::Owned(name), t.into()));
        }
        for (name, arr) in run_inputs.state_f32 {
            let t = f32_tensor(&name, arr)?;
            inputs.push((std::borrow::Cow::Owned(name), t.into()));
        }

        let outputs = session
            .session_mut()
            .run(inputs)
            .map_err(|e| TtsError::CodecError(format!("codec_decode_step.run: {e}")))?;

        // 把流式 state 张量回灌到 streaming session
        let mut i32_outs: HashMap<String, ArrayD<i32>> = HashMap::new();
        let mut f32_outs: HashMap<String, ArrayD<f32>> = HashMap::new();

        for spec in &codec_meta.streaming_decode.transformer_offsets {
            let v = outputs.get(spec.output_name.as_str()).ok_or_else(|| {
                TtsError::CodecError(format!("streaming 缺 {}", spec.output_name))
            })?;
            i32_outs.insert(
                spec.output_name.clone(),
                extract_i32_owned(v, &spec.output_name)?,
            );
        }
        for spec in &codec_meta.streaming_decode.attention_caches {
            let v = outputs
                .get(spec.offset_output_name.as_str())
                .ok_or_else(|| TtsError::CodecError(format!("缺 {}", spec.offset_output_name)))?;
            i32_outs.insert(
                spec.offset_output_name.clone(),
                extract_i32_owned(v, &spec.offset_output_name)?,
            );
            let v = outputs
                .get(spec.cached_keys_output_name.as_str())
                .ok_or_else(|| {
                    TtsError::CodecError(format!("缺 {}", spec.cached_keys_output_name))
                })?;
            f32_outs.insert(
                spec.cached_keys_output_name.clone(),
                extract_f32_owned(v, &spec.cached_keys_output_name)?,
            );
            let v = outputs
                .get(spec.cached_values_output_name.as_str())
                .ok_or_else(|| {
                    TtsError::CodecError(format!("缺 {}", spec.cached_values_output_name))
                })?;
            f32_outs.insert(
                spec.cached_values_output_name.clone(),
                extract_f32_owned(v, &spec.cached_values_output_name)?,
            );
            let v = outputs
                .get(spec.cached_positions_output_name.as_str())
                .ok_or_else(|| {
                    TtsError::CodecError(format!("缺 {}", spec.cached_positions_output_name))
                })?;
            i32_outs.insert(
                spec.cached_positions_output_name.clone(),
                extract_i32_owned(v, &spec.cached_positions_output_name)?,
            );
        }

        // 提取 audio + audio_lengths
        let audio_dyn = outputs
            .get("audio")
            .ok_or_else(|| TtsError::CodecError("streaming 缺 audio".into()))?;
        let audio = extract_f32_owned(audio_dyn, "audio")?;
        let lengths_dyn = outputs
            .get("audio_lengths")
            .ok_or_else(|| TtsError::CodecError("streaming 缺 audio_lengths".into()))?;
        let valid = extract_scalar_i32(lengths_dyn, "audio_lengths")? as usize;

        // 此时 outputs 借用结束，可以 mut borrow streaming 回灌
        streaming.ingest_outputs(i32_outs, f32_outs)?;

        let dims = audio.shape();
        if dims.len() != 3 || dims[0] != 1 {
            return Err(TtsError::CodecError(format!(
                "streaming audio shape 不符: {:?}",
                dims
            )));
        }
        let channels = dims[1];
        let take = valid.min(dims[2]);
        let mut per_channel: Vec<Vec<f32>> = Vec::with_capacity(channels);
        for c in 0..channels {
            let mut col = Vec::with_capacity(take);
            for s in 0..take {
                col.push(audio[[0, c, s]]);
            }
            per_channel.push(col);
        }
        Ok(Some((per_channel, take)))
    }
}

/// 把 channel-major PCM `[channels][samples]` 拼接成交错存储的 `Vec<f32>`。
pub fn interleave_channels(per_channel: &[Vec<f32>]) -> Vec<f32> {
    if per_channel.is_empty() {
        return Vec::new();
    }
    let frames = per_channel.iter().map(Vec::len).min().unwrap_or(0);
    let channels = per_channel.len();
    let mut out = Vec::with_capacity(frames * channels);
    for t in 0..frames {
        for c in 0..channels {
            out.push(per_channel[c][t]);
        }
    }
    out
}

/// 用 manifest 分别构造 [`ManifestBundle`] 关联的 num_quantizers 简便路径
/// （避免 provider 直接接触 manifest 内部）。
pub fn manifest_num_quantizers(manifest: &ManifestBundle) -> usize {
    manifest.codec_meta.codec_config.num_quantizers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_session_constants() {
        assert_eq!(CODEC_WAVEFORM, "waveform");
        assert_eq!(CODEC_INPUT_LENGTHS, "input_lengths");
        assert_eq!(CODEC_AUDIO_CODES, "audio_codes");
        assert_eq!(CODEC_AUDIO_CODE_LENGTHS, "audio_code_lengths");
        assert_eq!(CODEC_AUDIO_OUT, "audio");
        assert_eq!(CODEC_AUDIO_LENGTHS_OUT, "audio_lengths");
    }

    #[test]
    fn load_missing_model_returns_none_sessions() {
        let result = CodecSessions::load(Path::new("/nonexistent"), 4);
        assert!(
            result.is_ok(),
            "loading from nonexistent dir should not fail; sessions just None"
        );
        let sessions = result.unwrap();
        assert!(sessions.encode.is_none());
        assert!(sessions.decode_full.is_none());
        assert!(sessions.decode_step.is_none());
    }

    #[test]
    fn is_ready_all_missing() {
        let sessions = CodecSessions::load(Path::new("/nonexistent"), 4).unwrap();
        assert!(!sessions.is_ready());
    }
}
