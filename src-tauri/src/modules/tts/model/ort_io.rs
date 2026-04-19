//! ort 2.x 张量 I/O 辅助函数。
//!
//! Phase TTS-A.6 / A.7 共用：把 `ndarray::ArrayD<T>` ↔ `ort::Value` 来回转换的
//! 样板代码集中到一处，避免每个 session 都重复写 5 行 `Tensor::from_array`。
//!
//! 设计原则：
//! - 每个函数只负责一种 dtype (i32 / f32)，类型安全且零运行时开销。
//! - `extract_*_owned` 总是 clone 一份 owned 数据；KV cache 在循环间需要持续
//!   持有，clone 成本可接受（对单层 ~`[1, 16, seq, 64]` f32 ≈ 几 KB-几 MB）。
//! - 所有错误都包装成 [`TtsError::OnnxError`]，错误信息含张量名 / shape，便于
//!   线上 debug。

#![allow(dead_code)]

use ndarray::{ArrayD, IxDyn};
use ort::value::{DynValue, Tensor};

use crate::modules::tts::error::TtsError;

/// 把 `ArrayD<i32>` 转成 ort 的 `Tensor<i32>`，可直接放入 `ort::inputs!`。
pub fn i32_tensor(name: &str, array: ArrayD<i32>) -> Result<Tensor<i32>, TtsError> {
    Tensor::from_array(array)
        .map_err(|e| TtsError::OnnxError(format!("构造 i32 tensor `{name}` 失败: {e}")))
}

/// 把 `ArrayD<f32>` 转成 ort 的 `Tensor<f32>`。
pub fn f32_tensor(name: &str, array: ArrayD<f32>) -> Result<Tensor<f32>, TtsError> {
    Tensor::from_array(array)
        .map_err(|e| TtsError::OnnxError(format!("构造 f32 tensor `{name}` 失败: {e}")))
}

/// 从 `DynValue` 取出 owned `ArrayD<f32>`（clone 一份）。
pub fn extract_f32_owned(value: &DynValue, name: &str) -> Result<ArrayD<f32>, TtsError> {
    let view = value
        .try_extract_array::<f32>()
        .map_err(|e| TtsError::OnnxError(format!("提取 f32 tensor `{name}` 失败: {e}")))?;
    Ok(view.to_owned())
}

/// 从 `DynValue` 取出 owned `ArrayD<i32>`（clone 一份）。
pub fn extract_i32_owned(value: &DynValue, name: &str) -> Result<ArrayD<i32>, TtsError> {
    let view = value
        .try_extract_array::<i32>()
        .map_err(|e| TtsError::OnnxError(format!("提取 i32 tensor `{name}` 失败: {e}")))?;
    Ok(view.to_owned())
}

/// 取一维（标量）i32 输出（如 `should_continue` / `audio_lengths`）。
pub fn extract_scalar_i32(value: &DynValue, name: &str) -> Result<i32, TtsError> {
    let view = value
        .try_extract_array::<i32>()
        .map_err(|e| TtsError::OnnxError(format!("提取 i32 scalar `{name}` 失败: {e}")))?;
    view.iter()
        .next()
        .copied()
        .ok_or_else(|| TtsError::OnnxError(format!("scalar `{name}` 为空")))
}

/// 把一维 i32 输出展平为 `Vec<i32>`（如 `frame_token_ids`）。
pub fn extract_vec_i32(value: &DynValue, name: &str) -> Result<Vec<i32>, TtsError> {
    let view = value
        .try_extract_array::<i32>()
        .map_err(|e| TtsError::OnnxError(format!("提取 i32 vec `{name}` 失败: {e}")))?;
    Ok(view.iter().copied().collect())
}

/// 把一维 f32 输出展平为 `Vec<f32>`（如 audio_logits 单个 channel slice）。
pub fn extract_vec_f32(value: &DynValue, name: &str) -> Result<Vec<f32>, TtsError> {
    let view = value
        .try_extract_array::<f32>()
        .map_err(|e| TtsError::OnnxError(format!("提取 f32 vec `{name}` 失败: {e}")))?;
    Ok(view.iter().copied().collect())
}

/// 构造形状 `[1]` 的 i32 标量张量（许多 session 输入是 `[1]` shape）。
pub fn i32_scalar(name: &str, value: i32) -> Result<Tensor<i32>, TtsError> {
    let arr = ArrayD::<i32>::from_shape_vec(IxDyn(&[1]), vec![value])
        .map_err(|e| TtsError::OnnxError(format!("构造 i32 scalar `{name}` 失败: {e}")))?;
    i32_tensor(name, arr)
}

/// 构造形状 `[1]` 的 f32 标量张量。
pub fn f32_scalar(name: &str, value: f32) -> Result<Tensor<f32>, TtsError> {
    let arr = ArrayD::<f32>::from_shape_vec(IxDyn(&[1]), vec![value])
        .map_err(|e| TtsError::OnnxError(format!("构造 f32 scalar `{name}` 失败: {e}")))?;
    f32_tensor(name, arr)
}

/// 取最后一维 hidden state（镜像 Python `_extract_last_hidden`）。
///
/// 输入形状 `[1, seq_len, hidden]` → 输出 `[1, hidden]`；若已是 2D 则直接 owned。
pub fn extract_last_hidden(global_hidden: ArrayD<f32>) -> Result<ArrayD<f32>, TtsError> {
    let dims = global_hidden.shape().to_vec();
    match dims.as_slice() {
        [_, _hidden] => Ok(global_hidden),
        [1, seq, hidden] => {
            let last_idx = seq.saturating_sub(1);
            let mut out = ArrayD::<f32>::zeros(IxDyn(&[1, *hidden]));
            for h in 0..*hidden {
                out[[0, h]] = global_hidden[[0, last_idx, h]];
            }
            Ok(out)
        }
        other => Err(TtsError::OnnxError(format!(
            "global_hidden 形状不支持: {:?}",
            other
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_last_hidden_3d_takes_last_seq_step() {
        let mut arr = ArrayD::<f32>::zeros(IxDyn(&[1, 3, 4]));
        arr[[0, 0, 0]] = 1.0;
        arr[[0, 1, 0]] = 2.0;
        arr[[0, 2, 0]] = 3.0;
        arr[[0, 2, 3]] = 9.0;
        let last = extract_last_hidden(arr).unwrap();
        assert_eq!(last.shape(), &[1, 4]);
        assert_eq!(last[[0, 0]], 3.0);
        assert_eq!(last[[0, 3]], 9.0);
    }

    #[test]
    fn extract_last_hidden_2d_passthrough() {
        let arr = ArrayD::<f32>::zeros(IxDyn(&[1, 8]));
        let last = extract_last_hidden(arr).unwrap();
        assert_eq!(last.shape(), &[1, 8]);
    }

    #[test]
    fn extract_last_hidden_invalid_shape_returns_error() {
        let arr = ArrayD::<f32>::zeros(IxDyn(&[2, 3, 4]));
        assert!(matches!(
            extract_last_hidden(arr),
            Err(TtsError::OnnxError(_))
        ));
    }

    #[test]
    fn i32_scalar_has_shape_one() {
        let t = i32_scalar("test", 42).unwrap();
        // 通过 try_extract 验证 shape & 值
        let view = t.try_extract_array::<i32>().unwrap();
        assert_eq!(view.shape(), &[1]);
        assert_eq!(view[ndarray::IxDyn(&[0])], 42);
    }
}
