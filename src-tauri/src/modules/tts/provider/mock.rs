//! Mock TTS provider — returns synthetic (silent) audio for testing.
//!
//! This implementation generates valid WAV files filled with silence (zero samples),
//! allowing the rest of the system to be tested without actual ONNX inference.
//!
//! It also tracks the last synthesis parameters for assertion in tests.

use std::sync::Arc;

use crate::modules::tts::{
    AudioChunk, AudioSink, GenerationParams, StreamResult, SynthesisMode, SynthesisParams,
    SynthesisResult, TtsError, TtsProvider, VoicePreset, WarmupResult,
};

/// Mock TTS provider that returns synthetic audio (silent WAV).
///
/// Used for testing the TTS integration layer without requiring
/// ONNX model downloads or inference.
pub struct MockTtsProvider {
    /// Available voice presets.
    voices: Vec<VoicePreset>,
    /// The last synthesis params received — for test assertions.
    last_params: std::sync::Mutex<Option<SynthesisParams>>,
}

impl MockTtsProvider {
    /// Create a new `MockTtsProvider` with no voice presets.
    #[must_use]
    pub fn new() -> Self {
        Self {
            voices: Vec::new(),
            last_params: std::sync::Mutex::new(None),
        }
    }

    /// Create a new `MockTtsProvider` with the given voice presets.
    #[must_use]
    pub fn with_voices(voices: Vec<VoicePreset>) -> Self {
        Self {
            voices,
            last_params: std::sync::Mutex::new(None),
        }
    }

    /// Generate a silent WAV file of the given duration.
    ///
    /// Returns PCM16LE bytes (16-bit signed integer, little-endian).
    fn generate_silent_wav(sample_rate: u32, channels: u16, duration_secs: f32) -> Vec<u8> {
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        let bytes_per_sample = channels as usize * 2; // 16-bit = 2 bytes
        let data_size = num_samples * bytes_per_sample;

        // Minimal WAV header (44 bytes)
        let mut wav = vec![0u8; 44 + data_size];

        // RIFF header
        wav[0..4].copy_from_slice(b"RIFF");
        let file_size = (36 + data_size as u32).to_le_bytes();
        wav[4..8].copy_from_slice(&file_size);
        wav[8..12].copy_from_slice(b"WAVE");

        // fmt chunk
        wav[12..16].copy_from_slice(b"fmt ");
        wav[16..20].copy_from_slice(&16u32.to_le_bytes()); // chunk size
        wav[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
        wav[22..24].copy_from_slice(&channels.to_le_bytes());
        wav[24..28].copy_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * channels as u32 * 2;
        wav[28..32].copy_from_slice(&byte_rate.to_le_bytes());
        wav[32..34].copy_from_slice(&(bytes_per_sample as u16).to_le_bytes()); // block align
        wav[34..36].copy_from_slice(&16u16.to_le_bytes()); // bits per sample

        // data chunk
        wav[36..40].copy_from_slice(b"data");
        wav[40..44].copy_from_slice(&(data_size as u32).to_le_bytes());
        // Audio data is already zeroed (silence)

        wav
    }

    /// Resolve voice name from params.
    fn resolve_voice(&self, voice: Option<&str>) -> &VoicePreset {
        let name = voice.unwrap_or("mock");
        self.voices
            .iter()
            .find(|v| v.name == name)
            .unwrap_or_else(|| {
                // Return a fallback if the requested voice isn't found.
                static FALLBACK: std::sync::OnceLock<VoicePreset> = std::sync::OnceLock::new();
                FALLBACK.get_or_init(|| VoicePreset::new("mock", "Mock voice", "wav", Vec::new()))
            })
    }
}

impl Default for MockTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TtsProvider for MockTtsProvider {
    async fn synthesize(&self, params: SynthesisParams) -> Result<SynthesisResult, TtsError> {
        if params.text.is_empty() {
            return Err(TtsError::EmptyText);
        }

        // Continuation mode requires prompt_text when prompt_audio is provided.
        if params.mode == SynthesisMode::Continuation
            && params.prompt_audio_path.is_some()
            && params.prompt_text.is_none()
        {
            return Err(TtsError::InvalidParam(
                "continuation mode with prompt_audio_path requires prompt_text".to_string(),
            ));
        }

        // Record params for test assertions.
        {
            let mut last = self
                .last_params
                .lock()
                .map_err(|_| TtsError::SynthesisFailed("mutex poisoned".into()))?;
            *last = Some(params.clone());
        }

        let voice = self.resolve_voice(params.voice.as_deref());

        // Generate 1 second of silent audio for mock.
        let audio_bytes = Self::generate_silent_wav(
            crate::modules::tts::config::SAMPLE_RATE,
            crate::modules::tts::config::CHANNELS,
            1.0,
        );

        let duration = 1.0f32;

        Ok(SynthesisResult {
            audio_bytes,
            sample_rate: crate::modules::tts::config::SAMPLE_RATE,
            channels: crate::modules::tts::config::CHANNELS,
            duration_seconds: duration,
            voice: voice.name.clone(),
            text_chunks: vec![params.text.clone()],
            elapsed_seconds: 0.01,
            normalized_text: params.text.clone(),
        })
    }

    async fn synthesize_stream(
        &self,
        params: SynthesisParams,
        sink: Arc<dyn AudioSink>,
    ) -> Result<StreamResult, TtsError> {
        if params.text.is_empty() {
            return Err(TtsError::EmptyText);
        }

        if params.mode == SynthesisMode::Continuation
            && params.prompt_audio_path.is_some()
            && params.prompt_text.is_none()
        {
            return Err(TtsError::InvalidParam(
                "continuation mode with prompt_audio_path requires prompt_text".to_string(),
            ));
        }

        // Record params for test assertions.
        {
            let mut last = self
                .last_params
                .lock()
                .map_err(|_| TtsError::SynthesisFailed("mutex poisoned".into()))?;
            *last = Some(params.clone());
        }

        let voice = self.resolve_voice(params.voice.as_deref());

        // Emit a few mock audio chunks.
        let total_frames = 10;
        let samples_per_frame = 4800; // 100ms at 48kHz
        let channels = crate::modules::tts::config::CHANNELS;

        for i in 0..total_frames {
            let pcm_size = samples_per_frame * channels as usize * 2;
            let chunk = AudioChunk {
                pcm_data: vec![0u8; pcm_size],
                sample_rate: crate::modules::tts::config::SAMPLE_RATE,
                channels,
                chunk_index: 0,
                is_pause: false,
                emitted_audio_seconds: (i + 1) as f32 * 0.1,
                lead_seconds: (i as f32) * 0.05,
            };
            sink.on_audio(chunk).await;
        }

        let emitted_seconds = total_frames as f32 * 0.1;
        let elapsed_seconds = 0.05f32;
        let result = StreamResult {
            audio_path: None,
            sample_rate: crate::modules::tts::config::SAMPLE_RATE,
            channels: 2,
            voice: voice.name.clone(),
            text_chunks: vec![params.text.clone()],
            elapsed_seconds,
            emitted_audio_seconds: emitted_seconds,
            lead_seconds: (total_frames - 1) as f32 * 0.05,
            first_audio_latency_seconds: 0.01,
            realtime_factor: emitted_seconds / elapsed_seconds,
        };

        sink.on_complete(result.clone()).await;

        Ok(result)
    }

    async fn warmup(&self) -> Result<WarmupResult, TtsError> {
        let params = SynthesisParams {
            text: crate::modules::tts::config::WARMUP_TEXT.to_string(),
            mode: SynthesisMode::VoiceClone,
            voice: Some(self.default_voice().name.clone()),
            prompt_audio_path: None,
            prompt_text: None,
            generation: GenerationParams {
                max_new_frames: crate::modules::tts::config::WARMUP_MAX_FRAMES,
                ..Default::default()
            },
        };

        let result = self.synthesize(params).await?;

        Ok(WarmupResult {
            elapsed_seconds: result.elapsed_seconds,
            device: "cpu".to_string(),
        })
    }

    fn split_voice_clone_text(
        &self,
        text: &str,
        _max_tokens: usize,
    ) -> Result<Vec<String>, TtsError> {
        if text.is_empty() {
            return Ok(Vec::new());
        }
        // Simple split by sentence-ending punctuation.
        let chunks: Vec<String> = text
            .split(['。', '！', '？', '.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        if chunks.is_empty() {
            Ok(vec![text.to_string()])
        } else {
            Ok(chunks)
        }
    }

    fn list_voices(&self) -> Vec<String> {
        self.voices.iter().map(|v| v.name.clone()).collect()
    }

    fn get_voice(&self, name: &str) -> Option<&VoicePreset> {
        self.voices.iter().find(|v| v.name == name)
    }

    fn default_voice(&self) -> &VoicePreset {
        self.voices.first().unwrap_or_else(|| {
            static FALLBACK: std::sync::OnceLock<VoicePreset> = std::sync::OnceLock::new();
            FALLBACK.get_or_init(|| VoicePreset::new("mock", "Mock voice", "wav", Vec::new()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tts::config::CHANNELS;
    use std::sync::Arc;

    struct TestSink {
        inner: Arc<TestSinkInner>,
    }

    struct TestSinkInner {
        chunks: std::sync::Mutex<Vec<AudioChunk>>,
        completed: std::sync::Mutex<Option<StreamResult>>,
    }

    #[async_trait::async_trait]
    impl AudioSink for TestSink {
        async fn on_audio(&self, chunk: AudioChunk) {
            self.inner.chunks.lock().unwrap().push(chunk);
        }
        async fn on_complete(&self, result: StreamResult) {
            *self.inner.completed.lock().unwrap() = Some(result);
        }
    }

    #[tokio::test]
    async fn mock_synthesize_returns_valid_wav() {
        let provider = MockTtsProvider::new();
        let params = SynthesisParams {
            text: "Hello world".to_string(),
            mode: SynthesisMode::VoiceClone,
            voice: None,
            prompt_audio_path: None,
            prompt_text: None,
            generation: GenerationParams::default(),
        };

        let result = provider.synthesize(params).await.unwrap();

        assert_eq!(result.sample_rate, 48_000);
        assert_eq!(result.channels, 2);
        assert!(!result.audio_bytes.is_empty());
        assert_eq!(&result.audio_bytes[0..4], b"RIFF");
        assert_eq!(&result.audio_bytes[8..12], b"WAVE");
    }

    #[tokio::test]
    async fn mock_synthesize_rejects_empty_text() {
        let provider = MockTtsProvider::new();
        let params = SynthesisParams {
            text: String::new(),
            mode: SynthesisMode::VoiceClone,
            voice: None,
            prompt_audio_path: None,
            prompt_text: None,
            generation: GenerationParams::default(),
        };

        let err = provider.synthesize(params).await.unwrap_err();
        assert!(matches!(err, TtsError::EmptyText));
    }

    #[tokio::test]
    async fn mock_synthesize_stream_emits_chunks() {
        let provider = MockTtsProvider::new();
        let shared = Arc::new(TestSinkInner {
            chunks: std::sync::Mutex::new(Vec::new()),
            completed: std::sync::Mutex::new(None),
        });
        let sink: Arc<dyn AudioSink> = Arc::new(TestSink {
            inner: Arc::clone(&shared),
        });

        let params = SynthesisParams {
            text: "Stream test".to_string(),
            mode: SynthesisMode::VoiceClone,
            voice: None,
            prompt_audio_path: None,
            prompt_text: None,
            generation: GenerationParams::default(),
        };

        let result = provider.synthesize_stream(params, sink).await.unwrap();

        let chunks = shared.chunks.lock().unwrap();
        assert_eq!(chunks.len(), 10);
        assert_eq!(chunks[0].sample_rate, 48_000);
        assert_eq!(chunks[0].channels, CHANNELS);
        assert_eq!(result.voice, "mock");
        assert_eq!(result.emitted_audio_seconds, 1.0);

        let completed = shared.completed.lock().unwrap();
        assert!(completed.is_some());
    }

    #[tokio::test]
    async fn mock_synthesize_stream_rejects_empty_text() {
        let provider = MockTtsProvider::new();
        let shared = Arc::new(TestSinkInner {
            chunks: std::sync::Mutex::new(Vec::new()),
            completed: std::sync::Mutex::new(None),
        });
        let sink: Arc<dyn AudioSink> = Arc::new(TestSink {
            inner: Arc::clone(&shared),
        });

        let params = SynthesisParams {
            text: String::new(),
            mode: SynthesisMode::VoiceClone,
            voice: None,
            prompt_audio_path: None,
            prompt_text: None,
            generation: GenerationParams::default(),
        };

        let err = provider.synthesize_stream(params, sink).await.unwrap_err();
        assert!(matches!(err, TtsError::EmptyText));
    }

    #[tokio::test]
    async fn mock_warmup_succeeds() {
        let provider = MockTtsProvider::with_voices(vec![VoicePreset::new(
            "test",
            "Test voice",
            "wav",
            Vec::new(),
        )]);

        let result = provider.warmup().await.unwrap();
        assert_eq!(result.device, "cpu");
        assert!(result.elapsed_seconds > 0.0);
    }

    #[test]
    fn mock_split_voice_clone_text_splits_on_punctuation() {
        let provider = MockTtsProvider::new();
        let chunks = provider
            .split_voice_clone_text("你好。世界！今天天气很好？", 75)
            .unwrap();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "你好");
        assert_eq!(chunks[1], "世界");
        assert_eq!(chunks[2], "今天天气很好");
    }

    #[test]
    fn mock_split_voice_clone_text_returns_single_for_no_punctuation() {
        let provider = MockTtsProvider::new();
        let chunks = provider.split_voice_clone_text("hello world", 75).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world");
    }

    #[test]
    fn mock_split_voice_clone_text_empty() {
        let provider = MockTtsProvider::new();
        let chunks = provider.split_voice_clone_text("", 75).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn mock_list_voices_returns_registered() {
        let voices = vec![
            VoicePreset::new("Alice", "Voice A", "wav", Vec::new()),
            VoicePreset::new("Bob", "Voice B", "wav", Vec::new()),
        ];
        let provider = MockTtsProvider::with_voices(voices);
        let names = provider.list_voices();
        assert_eq!(names, vec!["Alice", "Bob"]);
    }

    #[test]
    fn mock_get_voice_returns_by_name() {
        let voices = vec![VoicePreset::new("Alice", "Voice A", "wav", Vec::new())];
        let provider = MockTtsProvider::with_voices(voices);
        assert!(provider.get_voice("Alice").is_some());
        assert!(provider.get_voice("Unknown").is_none());
    }

    #[test]
    fn mock_default_voice_returns_first_or_fallback() {
        let provider = MockTtsProvider::new();
        let voice = provider.default_voice();
        assert_eq!(voice.name, "mock");

        let voices = vec![VoicePreset::new("Alice", "Voice A", "wav", Vec::new())];
        let provider = MockTtsProvider::with_voices(voices);
        assert_eq!(provider.default_voice().name, "Alice");
    }

    #[test]
    fn generate_silent_wav_has_valid_header() {
        let wav = MockTtsProvider::generate_silent_wav(48_000, 2, 0.5);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        // 0.5s * 48000 * 2ch * 2bytes = 96000 bytes of audio data
        let data_size: u32 = u32::from_le_bytes(wav[40..44].try_into().unwrap());
        assert_eq!(data_size, 96_000);
    }

    #[tokio::test]
    async fn mock_synthesize_records_params() {
        let provider = MockTtsProvider::new();
        let params = SynthesisParams {
            text: "Record me".to_string(),
            mode: SynthesisMode::VoiceClone,
            voice: Some("test_voice".to_string()),
            prompt_audio_path: None,
            prompt_text: None,
            generation: GenerationParams::default(),
        };

        let _ = provider.synthesize(params.clone()).await.unwrap();

        let last = provider.last_params.lock().unwrap();
        assert!(last.is_some());
        let recorded = last.as_ref().unwrap();
        assert_eq!(recorded.text, "Record me");
        assert_eq!(recorded.voice, Some("test_voice".to_string()));
    }
}
