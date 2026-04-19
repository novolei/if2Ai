//! Codec 流式解码状态机（与 Python `CodecStreamingDecodeSession` 1:1）。
//!
//! 用法：
//! ```ignore
//! let mut sess = CodecStreamingDecodeSession::new(&codec_meta);
//! sess.reset(); // 每次 synth 开始前
//! for batch in frames.chunks(budget) {
//!     let (pcm, len) = sess.run_frames(batch, &mut codec_decode_step_session)?;
//!     emit(pcm[..len]);
//! }
//! ```
//!
//! 内部维护两类状态张量（每次 `run_frames` 后从输出回灌到下次输入）：
//! - **transformer offsets**：i32 张量，记录每层 transformer 当前 offset。
//! - **attention caches**：每层一组 `(offset_i32, keys_f32, values_f32, positions_i32)`。
//!
//! 实际 ONNX `.run()` 调用待 Phase TTS-A.7 接入；本骨架已完成：
//! 1. State 张量按 codec_meta 规范的 shape 在 `reset()` 时初始化为 0。
//! 2. `run_frames` 构造 feeds dict（`audio_codes` + `audio_code_lengths` + 全部 state）。
//! 3. 输出回灌：output_name → input_name 严格对照。
//!
//! 最后一公里——把 feeds 真正喂给 `ort::Session` 并解析 `audio` / `audio_lengths`
//! 输出——保留为 `unimplemented!("TTS-A.7: wire ort::Session::run")`，因为这一步
//! 必须对真实 codec_decode_step ONNX 模型的输出张量名 / dtype 做最终校验。

#![allow(dead_code)]

use std::collections::HashMap;

use ndarray::ArrayD;

use crate::modules::tts::error::TtsError;
use crate::modules::tts::manifest::{CodecMeta, StreamingDecodeMeta};

/// 流式 codec decode 状态张量（按 dtype 分两类）。
#[derive(Debug, Default, Clone)]
pub struct CodecStreamingState {
    /// 输入名 → i32 张量（transformer offsets / attention offsets / positions）。
    pub i32_feeds: HashMap<String, ArrayD<i32>>,
    /// 输入名 → f32 张量（attention cached keys / values）。
    pub f32_feeds: HashMap<String, ArrayD<f32>>,
}

/// 输出名 → 输入名 映射（每次 run_frames 后回灌使用）。
#[derive(Debug, Default, Clone)]
struct OutputToInputMap {
    /// i32 类输出 → 下次输入名
    i32_map: Vec<(String, String)>,
    /// f32 类输出 → 下次输入名
    f32_map: Vec<(String, String)>,
}

/// 流式 codec decode session。
pub struct CodecStreamingDecodeSession {
    /// streaming_decode 规范快照（来自 codec_meta）。
    streaming_meta: StreamingDecodeMeta,
    /// num_quantizers (= n_vq for codec)。
    num_quantizers: usize,
    /// 当前状态张量。
    state: CodecStreamingState,
    /// 输出 → 输入 回灌映射（在 `new` 时构建一次）。
    out_to_in: OutputToInputMap,
}

impl CodecStreamingDecodeSession {
    /// 用 codec_meta 构造新 session（未 reset）。
    pub fn new(codec_meta: &CodecMeta) -> Self {
        let streaming_meta = codec_meta.streaming_decode.clone();
        let num_quantizers = codec_meta.codec_config.num_quantizers;

        // 构建回灌映射
        let mut out_to_in = OutputToInputMap::default();
        for spec in &streaming_meta.transformer_offsets {
            out_to_in
                .i32_map
                .push((spec.output_name.clone(), spec.input_name.clone()));
        }
        for spec in &streaming_meta.attention_caches {
            out_to_in.i32_map.push((
                spec.offset_output_name.clone(),
                spec.offset_input_name.clone(),
            ));
            out_to_in.f32_map.push((
                spec.cached_keys_output_name.clone(),
                spec.cached_keys_input_name.clone(),
            ));
            out_to_in.f32_map.push((
                spec.cached_values_output_name.clone(),
                spec.cached_values_input_name.clone(),
            ));
            out_to_in.i32_map.push((
                spec.cached_positions_output_name.clone(),
                spec.cached_positions_input_name.clone(),
            ));
        }

        let mut session = Self {
            streaming_meta,
            num_quantizers,
            state: CodecStreamingState::default(),
            out_to_in,
        };
        session.reset();
        session
    }

    /// 重置状态：所有 transformer offsets / attention keys/values 清零，
    /// positions 填 -1（与 Python 一致）。
    pub fn reset(&mut self) {
        self.state.i32_feeds.clear();
        self.state.f32_feeds.clear();

        for spec in &self.streaming_meta.transformer_offsets {
            self.state
                .i32_feeds
                .insert(spec.input_name.clone(), zero_i32(&spec.shape));
        }
        for spec in &self.streaming_meta.attention_caches {
            self.state
                .i32_feeds
                .insert(spec.offset_input_name.clone(), zero_i32(&spec.offset_shape));
            self.state.f32_feeds.insert(
                spec.cached_keys_input_name.clone(),
                zero_f32(&spec.cache_shape),
            );
            self.state.f32_feeds.insert(
                spec.cached_values_input_name.clone(),
                zero_f32(&spec.cache_shape),
            );
            self.state.i32_feeds.insert(
                spec.cached_positions_input_name.clone(),
                filled_i32(&spec.positions_shape, -1),
            );
        }
    }

    /// 构造一次 run_frames 的输入 feeds。
    ///
    /// 返回 `(audio_codes_input, length_input, state_i32, state_f32)`，
    /// 调用方可以直接喂给 `ort::Session`。
    pub fn build_run_inputs(&self, frame_rows: &[Vec<i32>]) -> Result<RunFramesInputs, TtsError> {
        if frame_rows.is_empty() {
            return Err(TtsError::CodecError("frame_rows 为空".into()));
        }
        let frame_count = frame_rows.len();
        let nq = self.num_quantizers;
        let mut audio_codes = ArrayD::<i32>::zeros(ndarray::IxDyn(&[1, frame_count, nq]));
        for (f, row) in frame_rows.iter().enumerate() {
            for q in 0..nq {
                audio_codes[[0, f, q]] = row.get(q).copied().unwrap_or(0);
            }
        }
        let lengths = ArrayD::<i32>::from_shape_vec(ndarray::IxDyn(&[1]), vec![frame_count as i32])
            .map_err(|e| TtsError::CodecError(format!("audio_code_lengths shape: {e}")))?;

        Ok(RunFramesInputs {
            audio_codes,
            audio_code_lengths: lengths,
            state_i32: self.state.i32_feeds.clone(),
            state_f32: self.state.f32_feeds.clone(),
        })
    }

    /// 接收 ONNX 输出后：把指定输出名的张量回灌到对应输入名供下次使用。
    ///
    /// 调用方在 ort `session.run()` 之后，按输出顺序拿到 named_outputs，
    /// 然后调用本方法把 state 张量转移到 self.state 中。
    pub fn ingest_outputs(
        &mut self,
        i32_outputs: HashMap<String, ArrayD<i32>>,
        f32_outputs: HashMap<String, ArrayD<f32>>,
    ) -> Result<(), TtsError> {
        for (out_name, in_name) in &self.out_to_in.i32_map {
            let tensor = i32_outputs
                .get(out_name)
                .ok_or_else(|| TtsError::CodecError(format!("缺少 i32 流式输出: {out_name}")))?;
            self.state.i32_feeds.insert(in_name.clone(), tensor.clone());
        }
        for (out_name, in_name) in &self.out_to_in.f32_map {
            let tensor = f32_outputs
                .get(out_name)
                .ok_or_else(|| TtsError::CodecError(format!("缺少 f32 流式输出: {out_name}")))?;
            self.state.f32_feeds.insert(in_name.clone(), tensor.clone());
        }
        Ok(())
    }

    /// 当前所有 i32 / f32 state 输入张量的不可变引用（用于 ort::inputs! 构造）。
    pub fn current_state(&self) -> &CodecStreamingState {
        &self.state
    }

    /// 流式 transformer / attention 的总状态张量数（合 i32 + f32）。
    pub fn state_tensor_count(&self) -> usize {
        self.state.i32_feeds.len() + self.state.f32_feeds.len()
    }
}

/// 一次 `run_frames` 的全部输入张量 + 当前 state（state 已 clone 一份方便 move 到
/// ort::inputs!）。
#[derive(Debug, Clone)]
pub struct RunFramesInputs {
    pub audio_codes: ArrayD<i32>,
    pub audio_code_lengths: ArrayD<i32>,
    pub state_i32: HashMap<String, ArrayD<i32>>,
    pub state_f32: HashMap<String, ArrayD<f32>>,
}

fn zero_i32(shape: &[usize]) -> ArrayD<i32> {
    ArrayD::<i32>::zeros(ndarray::IxDyn(shape))
}

fn zero_f32(shape: &[usize]) -> ArrayD<f32> {
    ArrayD::<f32>::zeros(ndarray::IxDyn(shape))
}

fn filled_i32(shape: &[usize], value: i32) -> ArrayD<i32> {
    let total: usize = shape.iter().product();
    ArrayD::<i32>::from_shape_vec(ndarray::IxDyn(shape), vec![value; total])
        .expect("shape × value vec 大小匹配")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tts::manifest::{AttentionCacheSpec, CodecConfig, TransformerOffsetSpec};

    fn make_meta() -> CodecMeta {
        CodecMeta {
            files: Default::default(),
            codec_config: CodecConfig {
                sample_rate: 48_000,
                channels: 2,
                num_quantizers: 16,
                extra: Default::default(),
            },
            streaming_decode: StreamingDecodeMeta {
                transformer_offsets: vec![TransformerOffsetSpec {
                    input_name: "tx_off_in_0".into(),
                    output_name: "tx_off_out_0".into(),
                    shape: vec![1, 1],
                }],
                attention_caches: vec![AttentionCacheSpec {
                    offset_input_name: "att_off_in_0".into(),
                    offset_output_name: "att_off_out_0".into(),
                    offset_shape: vec![1, 1],
                    cached_keys_input_name: "k_in_0".into(),
                    cached_keys_output_name: "k_out_0".into(),
                    cached_values_input_name: "v_in_0".into(),
                    cached_values_output_name: "v_out_0".into(),
                    cache_shape: vec![1, 8, 4, 4],
                    cached_positions_input_name: "pos_in_0".into(),
                    cached_positions_output_name: "pos_out_0".into(),
                    positions_shape: vec![1, 4],
                }],
            },
        }
    }

    #[test]
    fn reset_initializes_state_with_correct_shapes() {
        let meta = make_meta();
        let sess = CodecStreamingDecodeSession::new(&meta);
        assert_eq!(sess.state.i32_feeds.len(), 3); // tx_off + att_off + positions
        assert_eq!(sess.state.f32_feeds.len(), 2); // keys + values

        let pos = sess.state.i32_feeds.get("pos_in_0").unwrap();
        assert_eq!(pos.shape(), &[1, 4]);
        // positions 应该全 -1
        assert!(pos.iter().all(|&v| v == -1));

        let keys = sess.state.f32_feeds.get("k_in_0").unwrap();
        assert_eq!(keys.shape(), &[1, 8, 4, 4]);
        assert!(keys.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn build_run_inputs_shapes_audio_codes_correctly() {
        let meta = make_meta();
        let sess = CodecStreamingDecodeSession::new(&meta);
        let frames = vec![vec![1; 16], vec![2; 16]];
        let inputs = sess.build_run_inputs(&frames).unwrap();
        assert_eq!(inputs.audio_codes.shape(), &[1, 2, 16]);
        assert_eq!(inputs.audio_code_lengths.shape(), &[1]);
        assert_eq!(inputs.audio_code_lengths[[0]], 2);
    }

    #[test]
    fn ingest_outputs_swaps_state() {
        let meta = make_meta();
        let mut sess = CodecStreamingDecodeSession::new(&meta);

        let mut i32_out = HashMap::new();
        i32_out.insert(
            "tx_off_out_0".into(),
            ArrayD::from_elem(ndarray::IxDyn(&[1, 1]), 7i32),
        );
        i32_out.insert(
            "att_off_out_0".into(),
            ArrayD::from_elem(ndarray::IxDyn(&[1, 1]), 8i32),
        );
        i32_out.insert(
            "pos_out_0".into(),
            ArrayD::from_elem(ndarray::IxDyn(&[1, 4]), 9i32),
        );

        let mut f32_out = HashMap::new();
        f32_out.insert(
            "k_out_0".into(),
            ArrayD::from_elem(ndarray::IxDyn(&[1, 8, 4, 4]), 0.5f32),
        );
        f32_out.insert(
            "v_out_0".into(),
            ArrayD::from_elem(ndarray::IxDyn(&[1, 8, 4, 4]), 0.25f32),
        );

        sess.ingest_outputs(i32_out, f32_out).unwrap();
        assert_eq!(sess.state.i32_feeds.get("tx_off_in_0").unwrap()[[0, 0]], 7);
        assert_eq!(sess.state.i32_feeds.get("pos_in_0").unwrap()[[0, 0]], 9);
        assert!((sess.state.f32_feeds.get("k_in_0").unwrap()[[0, 0, 0, 0]] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn missing_output_returns_codec_error() {
        let meta = make_meta();
        let mut sess = CodecStreamingDecodeSession::new(&meta);
        let i32_out = HashMap::new(); // 全空，缺 tx_off_out_0
        let f32_out = HashMap::new();
        let err = sess.ingest_outputs(i32_out, f32_out).unwrap_err();
        assert!(matches!(err, TtsError::CodecError(_)));
    }
}
