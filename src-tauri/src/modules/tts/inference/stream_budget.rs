//! 流式 codec decode 的自适应批量预算。
//!
//! 把"已发出多少音频秒"与"距离首音的真实墙钟时长"换算成 lead 秒数，
//! 再按阶梯映射到一次 codec_decode_step 应处理的帧数（1/2/4/8）。
//!
//! - lead < 0.20s 或还没首音：1 frame（最小延迟）
//! - lead < 0.55s：2 frames
//! - lead < 1.10s：4 frames
//! - 否则：8 frames（追求 throughput）
//!
//! 镜像 Python `_resolve_stream_decode_frame_budget` (`ort_cpu_runtime.py:216-228`)
//! 与 `_compute_stream_lead_seconds` (`ort_cpu_runtime.py:208-213`)。

#![allow(dead_code)]

use std::time::Instant;

/// 计算流式 lead 秒数：已发出音频时长 - 距首音的真实墙钟时长。
///
/// `first_audio_at` 为 `None` 时（尚未发出第一帧）返回 0.0。
pub fn compute_lead_seconds(
    emitted_samples_total: u64,
    sample_rate: u32,
    first_audio_at: Option<Instant>,
) -> f32 {
    let Some(first) = first_audio_at else {
        return 0.0;
    };
    if sample_rate == 0 {
        return 0.0;
    }
    let elapsed = first.elapsed().as_secs_f32().max(0.0);
    let emitted = emitted_samples_total as f32 / sample_rate as f32;
    emitted - elapsed
}

/// 按 lead 秒数返回下次 codec_decode_step 的帧预算。
pub fn resolve_decode_frame_budget(
    emitted_samples_total: u64,
    sample_rate: u32,
    first_audio_at: Option<Instant>,
) -> usize {
    let lead = compute_lead_seconds(emitted_samples_total, sample_rate, first_audio_at);
    if first_audio_at.is_none() || lead < 0.20 {
        return 1;
    }
    if lead < 0.55 {
        return 2;
    }
    if lead < 1.10 {
        return 4;
    }
    8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_first_audio_returns_min_budget() {
        assert_eq!(resolve_decode_frame_budget(0, 48_000, None), 1);
    }

    #[test]
    fn high_lead_returns_max_budget() {
        // 通过手动 emitted 远超 elapsed 触发 lead > 1.10s
        let now = Instant::now();
        // 1.5 秒的音频已发出，elapsed ≈ 0 → lead ≈ 1.5s
        let emitted_samples = (1.5 * 48_000.0) as u64;
        let budget = resolve_decode_frame_budget(emitted_samples, 48_000, Some(now));
        assert_eq!(budget, 8);
    }

    #[test]
    fn moderate_lead_returns_mid_budget() {
        let now = Instant::now();
        let emitted_samples = (0.30 * 48_000.0) as u64;
        let budget = resolve_decode_frame_budget(emitted_samples, 48_000, Some(now));
        assert_eq!(budget, 2);
    }
}
