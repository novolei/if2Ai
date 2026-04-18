//! WAV encoding/decoding utilities for TTS.
//!
//! Mirrors `_audio_to_wav_bytes()` from the Python `app.py` reference.
//! The MOSS-TTS-Nano model outputs f32 samples at 48kHz, stereo.

#![allow(dead_code)]

use crate::modules::tts::error::TtsError;

/// Default sample rate for TTS output (48kHz).
pub const SAMPLE_RATE: u32 = 48_000;

/// Default number of channels (stereo).
pub const CHANNELS: u16 = 2;

/// Encode f32 PCM samples into WAV bytes (16-bit PCM, 48kHz, stereo).
///
/// Input: interleaved f32 samples in range [-1.0, 1.0].
/// Output: valid WAV file bytes.
///
/// Mirrors `_audio_to_wav_bytes()` from Python app.py.
pub fn wav_encode(samples: &[f32], sample_rate: u32, channels: u16) -> Result<Vec<u8>, TtsError> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buffer = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buffer);
        let mut writer =
            hound::WavWriter::new(cursor, spec).map_err(|e| TtsError::WavDecode(e.to_string()))?;
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let i16_sample = (clamped * i16::MAX as f32) as i16;
            writer
                .write_sample(i16_sample)
                .map_err(|e| TtsError::WavDecode(e.to_string()))?;
        }
        writer
            .finalize()
            .map_err(|e| TtsError::WavDecode(e.to_string()))?;
    }
    Ok(buffer)
}

/// Decode WAV bytes into f32 PCM samples (interleaved, normalized to [-1.0, 1.0]).
///
/// Returns (samples, sample_rate, channels).
pub fn wav_decode(bytes: &[u8]) -> Result<(Vec<f32>, u32, u16), TtsError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut reader =
        hound::WavReader::new(cursor).map_err(|e| TtsError::WavDecode(e.to_string()))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;

    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|r| {
            r.map(|s| s as f32 / i16::MAX as f32)
                .map_err(|e| TtsError::WavDecode(e.to_string()))
        })
        .collect::<Result<Vec<_>, TtsError>>()?;

    Ok((samples, sample_rate, channels))
}

/// Encode stereo interleaved f32 samples with proper channel interleaving.
///
/// Takes separate left and right channel buffers and produces interleaved WAV output.
pub fn wav_encode_stereo(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
) -> Result<Vec<u8>, TtsError> {
    let len = left.len().min(right.len());
    let mut interleaved = Vec::with_capacity(len * 2);
    for i in 0..len {
        interleaved.push(left[i]);
        interleaved.push(right[i]);
    }
    wav_encode(&interleaved, sample_rate, 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip_mono() {
        let original: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0) * 2.0 - 1.0).collect();
        let wav_bytes = wav_encode(&original, SAMPLE_RATE, 1).expect("encode should succeed");
        let (decoded, sr, ch) = wav_decode(&wav_bytes).expect("decode should succeed");
        assert_eq!(sr, SAMPLE_RATE);
        assert_eq!(ch, 1);
        assert_eq!(decoded.len(), original.len());
        // 16-bit quantization introduces small errors; allow 1 LSB tolerance
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert!(
                (a - b).abs() < 1.0 / i16::MAX as f32,
                "sample mismatch: {a} vs {b}"
            );
        }
    }

    #[test]
    fn encode_decode_roundtrip_stereo() {
        let original: Vec<f32> = (0..2000).map(|i| (i as f32 / 2000.0) * 2.0 - 1.0).collect();
        let wav_bytes = wav_encode(&original, SAMPLE_RATE, 2).expect("encode should succeed");
        let (decoded, sr, ch) = wav_decode(&wav_bytes).expect("decode should succeed");
        assert_eq!(sr, SAMPLE_RATE);
        assert_eq!(ch, 2);
        assert_eq!(decoded.len(), original.len());
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert!(
                (a - b).abs() < 1.0 / i16::MAX as f32,
                "sample mismatch: {a} vs {b}"
            );
        }
    }

    #[test]
    fn encode_stereo_helper() {
        let left: Vec<f32> = vec![0.5, 0.3, 0.1];
        let right: Vec<f32> = vec![-0.5, -0.3, -0.1];
        let wav_bytes =
            wav_encode_stereo(&left, &right, SAMPLE_RATE).expect("encode should succeed");
        assert!(!wav_bytes.is_empty());
        // WAV header is 44 bytes + 6 samples * 2 bytes/sample = 56 bytes
        assert_eq!(wav_bytes.len(), 44 + 6 * 2);
    }

    #[test]
    fn encode_clamps_out_of_range() {
        let samples: Vec<f32> = vec![2.0, -3.0, 0.0];
        let wav_bytes = wav_encode(&samples, SAMPLE_RATE, 1).expect("encode should succeed");
        let (decoded, _, _) = wav_decode(&wav_bytes).expect("decode should succeed");
        assert!(decoded[0] <= 1.0, "clipped sample should be <= 1.0");
        assert!(decoded[1] >= -1.0, "clipped sample should be >= -1.0");
    }

    #[test]
    fn decode_invalid_wav_returns_error() {
        let result = wav_decode(b"not a wav file");
        assert!(result.is_err());
    }

    #[test]
    fn encode_produces_valid_wav_header() {
        let samples: Vec<f32> = vec![0.0; 100];
        let wav_bytes = wav_encode(&samples, SAMPLE_RATE, 2).expect("encode should succeed");
        // WAV header starts with "RIFF"
        assert_eq!(&wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav_bytes[8..12], b"WAVE");
    }
}
