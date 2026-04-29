//! Reference audio loader：把任意格式 / 采样率 / 声道数的输入音频
//! 归一化为 codec 期望的 PCM（默认 48 kHz / stereo / f32 [-1, 1]）。
//!
//! ## Pipeline（与 Python `torchaudio.load + resample` 等价）
//!
//! 1. **解码**：`symphonia` 探测格式（wav/mp3/flac/ogg/aac/...）→ 拿到任意
//!    采样率 & 声道数的 f32 / i16 / i32 / i24 PCM，统一归一化到 f32 [-1, 1]
//!    interleaved 缓冲区。
//! 2. **重采样**：用 [`rubato::SincFixedIn`] + `WindowFunction::BlackmanHarris2`
//!    将采样率拉到 `target_sample_rate`。该参数组合最接近 torchaudio 默认的
//!    `kaiser_window` 滤波质量。
//! 3. **声道转换**：mono→stereo 复制；stereo→mono 平均；其余 N→M 走通用
//!    平均下混 / 复制上混。
//!
//! ## 错误
//!
//! 全部包成 [`TtsError::AudioIo`]。
//!
//! 参考 Python: `onnx_tts_runtime.py:442-458` `_load_reference_audio`。

#![allow(dead_code)]

use std::fs::File;
use std::path::Path;

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::modules::tts::error::TtsError;

/// 解析后的 reference 波形。
#[derive(Debug, Clone)]
pub struct ReferenceWaveform {
    /// 交错存储的 PCM f32 样本（`samples[t * channels + c]`）。
    pub samples: Vec<f32>,
    /// 实际采样率（与 `target_sample_rate` 一致）。
    pub sample_rate: u32,
    /// 实际声道数（与 `target_channels` 一致）。
    pub channels: u16,
    /// 时长（秒）。
    pub duration_seconds: f32,
}

impl ReferenceWaveform {
    /// 转换为 channel-major 形状 `[1, channels, samples_per_channel]` 的 ndarray，
    /// 直接喂给 codec_encode ONNX session。
    pub fn into_channel_major_ndarray(self) -> ndarray::ArrayD<f32> {
        let channels = self.channels as usize;
        let samples_per_channel = self.samples.len().checked_div(channels).unwrap_or(0);
        let mut out =
            ndarray::ArrayD::<f32>::zeros(ndarray::IxDyn(&[1, channels, samples_per_channel]));
        for t in 0..samples_per_channel {
            for c in 0..channels {
                out[[0, c, t]] = self.samples[t * channels + c];
            }
        }
        out
    }
}

/// 加载 reference 音频并归一化到 `target_sample_rate` / `target_channels`。
pub fn load_reference_audio(
    path: &Path,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<ReferenceWaveform, TtsError> {
    if !path.exists() {
        return Err(TtsError::PromptAudioNotFound(path.to_path_buf()));
    }

    let (per_channel_planar, source_sample_rate) = decode_to_planar_f32(path)?;
    let _source_channels = per_channel_planar.len() as u16;

    if per_channel_planar.is_empty() {
        return Err(TtsError::AudioIo("解码后无音频数据".into()));
    }

    // Step 2: resample (如果采样率不一致)
    let resampled_planar = if source_sample_rate == target_sample_rate {
        per_channel_planar
    } else {
        resample_planar(per_channel_planar, source_sample_rate, target_sample_rate)?
    };

    // Step 3: 通道转换
    let final_planar = convert_channels(resampled_planar, target_channels)?;

    // 交错化
    let samples_per_channel = final_planar.first().map(|c| c.len()).unwrap_or(0);
    let mut interleaved = Vec::with_capacity(samples_per_channel * target_channels as usize);
    for t in 0..samples_per_channel {
        for c in 0..target_channels as usize {
            interleaved.push(final_planar[c][t]);
        }
    }

    let duration_seconds = if target_sample_rate > 0 {
        samples_per_channel as f32 / target_sample_rate as f32
    } else {
        0.0
    };

    Ok(ReferenceWaveform {
        samples: interleaved,
        sample_rate: target_sample_rate,
        channels: target_channels,
        duration_seconds,
    })
}

// ── 内部步骤 ─────────────────────────────────────────────────────────────────

/// 把任意格式的输入文件解码成 planar f32 PCM（`Vec<channel>`，每 channel 是
/// `Vec<f32>`）+ 源采样率。
fn decode_to_planar_f32(path: &Path) -> Result<(Vec<Vec<f32>>, u32), TtsError> {
    let file =
        File::open(path).map_err(|e| TtsError::AudioIo(format!("open {}: {e}", path.display())))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // 用扩展名当格式提示（symphonia 探测时会用）
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| TtsError::AudioIo(format!("probe: {e}")))?;
    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| TtsError::AudioIo("音频文件无默认 track".into()))?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| TtsError::AudioIo(format!("decoder: {e}")))?;

    let source_sample_rate = codec_params
        .sample_rate
        .ok_or_else(|| TtsError::AudioIo("源采样率缺失".into()))?;
    let source_channels = codec_params
        .channels
        .map(|c| c.count())
        .ok_or_else(|| TtsError::AudioIo("源声道数缺失".into()))?;

    let mut planar: Vec<Vec<f32>> = vec![Vec::new(); source_channels];

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(ref io))
                if io.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(TtsError::AudioIo(format!("next_packet: {e}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => append_decoded_to_planar(decoded, &mut planar)?,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(TtsError::AudioIo(format!("decode: {e}"))),
        }
    }

    Ok((planar, source_sample_rate))
}

/// 把一个 `AudioBufferRef` 的样本追加到 planar f32 缓冲区。
fn append_decoded_to_planar(
    decoded: AudioBufferRef<'_>,
    planar: &mut [Vec<f32>],
) -> Result<(), TtsError> {
    let channel_count = planar.len();
    macro_rules! copy_typed {
        ($buf:expr, $ty:ty, $convert:expr) => {{
            let frames = $buf.frames();
            for c in 0..channel_count {
                let chan = $buf.chan(c);
                planar[c].reserve(frames);
                for s in chan.iter().take(frames) {
                    let value: f32 = $convert(*s);
                    planar[c].push(value);
                }
            }
        }};
    }
    match decoded {
        AudioBufferRef::F32(buf) => copy_typed!(buf, f32, |x: f32| x),
        AudioBufferRef::F64(buf) => copy_typed!(buf, f64, |x: f64| x as f32),
        AudioBufferRef::S8(buf) => copy_typed!(buf, i8, |x: i8| x as f32 / i8::MAX as f32),
        AudioBufferRef::S16(buf) => copy_typed!(buf, i16, |x: i16| x as f32 / i16::MAX as f32),
        AudioBufferRef::S24(buf) => copy_typed!(
            buf,
            symphonia::core::sample::i24,
            |x: symphonia::core::sample::i24| { (x.inner() as f32) / 8_388_607.0 }
        ),
        AudioBufferRef::S32(buf) => {
            copy_typed!(buf, i32, |x: i32| x as f32 / i32::MAX as f32)
        }
        AudioBufferRef::U8(buf) => {
            copy_typed!(buf, u8, |x: u8| (x as f32 - 128.0) / 128.0)
        }
        AudioBufferRef::U16(buf) => {
            copy_typed!(buf, u16, |x: u16| (x as f32 - 32_768.0) / 32_768.0)
        }
        AudioBufferRef::U24(buf) => copy_typed!(
            buf,
            symphonia::core::sample::u24,
            |x: symphonia::core::sample::u24| { (x.inner() as f32 - 8_388_608.0) / 8_388_608.0 }
        ),
        AudioBufferRef::U32(buf) => copy_typed!(buf, u32, |x: u32| (x as f32 - 2_147_483_648.0)
            / 2_147_483_648.0),
    }
    Ok(())
}

/// 用 rubato 对 planar PCM 做高质量重采样。
fn resample_planar(
    input: Vec<Vec<f32>>,
    source_sample_rate: u32,
    target_sample_rate: u32,
) -> Result<Vec<Vec<f32>>, TtsError> {
    if input.is_empty() {
        return Ok(input);
    }
    let channels = input.len();
    let frames = input[0].len();
    if frames == 0 {
        return Ok(input);
    }
    let ratio = target_sample_rate as f64 / source_sample_rate as f64;
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        oversampling_factor: 256,
        interpolation: SincInterpolationType::Cubic,
        window: WindowFunction::BlackmanHarris2,
    };
    // chunk_size 取 1024 是 rubato 推荐，单次处理一帧块
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, 1024, channels)
        .map_err(|e| TtsError::AudioIo(format!("rubato init: {e}")))?;

    let mut output: Vec<Vec<f32>> =
        vec![Vec::with_capacity((frames as f64 * ratio) as usize + 1024); channels];
    let mut input_offset = 0usize;
    while input_offset < frames {
        let need = resampler.input_frames_next();
        let mut chunk: Vec<Vec<f32>> = Vec::with_capacity(channels);
        for c in 0..channels {
            let mut col = Vec::with_capacity(need);
            let end = (input_offset + need).min(frames);
            col.extend_from_slice(&input[c][input_offset..end]);
            // 不足部分补 0（最后一块）
            col.resize(need, 0.0);
            chunk.push(col);
        }
        let processed = resampler
            .process(&chunk, None)
            .map_err(|e| TtsError::AudioIo(format!("rubato process: {e}")))?;
        for c in 0..channels {
            output[c].extend_from_slice(&processed[c]);
        }
        input_offset += need;
        if input_offset >= frames {
            break;
        }
    }

    Ok(output)
}

/// 通道数匹配（mono↔stereo 的特化 + 通用平均/复制兜底）。
fn convert_channels(
    planar: Vec<Vec<f32>>,
    target_channels: u16,
) -> Result<Vec<Vec<f32>>, TtsError> {
    let source_channels = planar.len() as u16;
    if source_channels == 0 {
        return Err(TtsError::AudioIo("0 声道源".into()));
    }
    if source_channels == target_channels {
        return Ok(planar);
    }
    // 1 → N：复制
    if source_channels == 1 {
        let single = planar.into_iter().next().unwrap();
        return Ok(vec![single; target_channels as usize]);
    }
    // N → 1：平均
    if target_channels == 1 {
        let frames = planar[0].len();
        let mut mixed = vec![0.0f32; frames];
        let scale = 1.0 / source_channels as f32;
        for chan in &planar {
            for (i, s) in chan.iter().take(frames).enumerate() {
                mixed[i] += *s * scale;
            }
        }
        return Ok(vec![mixed]);
    }
    // 通用：N → M。M < N 则平均下混到 M（按对应 index 模 N），M > N 则循环复制。
    let frames = planar[0].len();
    let mut output: Vec<Vec<f32>> = vec![vec![0.0f32; frames]; target_channels as usize];
    if target_channels < source_channels {
        // 把每 source_chan 平均贡献到 target_chan = src % target
        let mut counts = vec![0u32; target_channels as usize];
        for (src_idx, chan) in planar.iter().enumerate() {
            let dst_idx = src_idx % target_channels as usize;
            for (i, s) in chan.iter().take(frames).enumerate() {
                output[dst_idx][i] += *s;
            }
            counts[dst_idx] += 1;
        }
        for (dst_idx, count) in counts.iter().enumerate() {
            if *count > 1 {
                let scale = 1.0 / *count as f32;
                for s in output[dst_idx].iter_mut() {
                    *s *= scale;
                }
            }
        }
    } else {
        // target > source：循环复制
        for dst_idx in 0..target_channels as usize {
            output[dst_idx] = planar[dst_idx % source_channels as usize].clone();
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_wav(path: &Path, sample_rate: u32, channels: u16, seconds: f32, freq_hz: f32) {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        let total = (sample_rate as f32 * seconds) as usize;
        for i in 0..total {
            let t = i as f32 / sample_rate as f32;
            let s = (2.0 * std::f32::consts::PI * freq_hz * t).sin() * 0.5;
            for _ in 0..channels {
                writer.write_sample((s * i16::MAX as f32) as i16).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn missing_file_returns_prompt_audio_not_found() {
        let err = load_reference_audio(Path::new("/no/such/file.wav"), 48_000, 2).unwrap_err();
        assert!(matches!(err, TtsError::PromptAudioNotFound(_)));
    }

    #[test]
    fn load_16k_mono_resample_to_48k_stereo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("in.wav");
        write_test_wav(&p, 16_000, 1, 0.5, 440.0);
        let wav = load_reference_audio(&p, 48_000, 2).unwrap();
        assert_eq!(wav.sample_rate, 48_000);
        assert_eq!(wav.channels, 2);
        // 0.5s 输入 → 48k × 0.5 = 24000 frames（容差 ±5%）
        let frames = wav.samples.len() / 2;
        let diff = (frames as i64 - 24_000_i64).abs();
        assert!(
            diff < 1_500,
            "frames out of tolerance: {frames} (diff={diff})"
        );
        assert!(wav.duration_seconds > 0.45 && wav.duration_seconds < 0.55);
    }

    #[test]
    fn load_44k_stereo_resample_to_48k_stereo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("in.wav");
        write_test_wav(&p, 44_100, 2, 0.3, 880.0);
        let wav = load_reference_audio(&p, 48_000, 2).unwrap();
        assert_eq!(wav.sample_rate, 48_000);
        assert_eq!(wav.channels, 2);
        let frames = wav.samples.len() / 2;
        let expected = (48_000_f32 * 0.3) as i32; // 14_400
        assert!(
            (frames as i32 - expected).abs() < 1_000,
            "frames {frames}, expected {expected}"
        );
    }

    #[test]
    fn load_stereo_to_mono_averages_channels() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("in.wav");
        write_test_wav(&p, 48_000, 2, 0.1, 220.0);
        let wav = load_reference_audio(&p, 48_000, 1).unwrap();
        assert_eq!(wav.channels, 1);
        // mono 帧数 ≈ stereo 一半
        assert_eq!(wav.samples.len(), wav.samples.len()); // sanity
        assert!(wav.samples.len() > 4_500 && wav.samples.len() < 5_000);
    }

    #[test]
    fn into_channel_major_ndarray_shape_correct() {
        let wav = ReferenceWaveform {
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6], // 3 frames × 2 channels interleaved
            sample_rate: 48_000,
            channels: 2,
            duration_seconds: 0.0,
        };
        let arr = wav.into_channel_major_ndarray();
        assert_eq!(arr.shape(), &[1, 2, 3]);
        assert!((arr[[0, 0, 0]] - 0.1).abs() < 1e-6);
        assert!((arr[[0, 1, 0]] - 0.2).abs() < 1e-6);
        assert!((arr[[0, 0, 1]] - 0.3).abs() < 1e-6);
        assert!((arr[[0, 1, 2]] - 0.6).abs() < 1e-6);
    }
}
