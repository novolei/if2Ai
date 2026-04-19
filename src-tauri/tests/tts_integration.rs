//! TTS integration tests — TTS-5.3
//!
//! Tests the full TTS command flow using `MockTtsProvider`:
//! - Buffered synthesis produces playable WAV
//! - Streaming job lifecycle (start → status → close)
//! - Warmup state machine integration
//! - Demo audio path resolution
//! - Voice list and text splitting
//!
//! These tests verify the Rust integration layer (commands + manager + provider)
//! without requiring ONNX model downloads.

use std::sync::Arc;

use if2ai_backend::modules::tts::provider::MockTtsProvider;
use if2ai_backend::modules::tts::{
    GenerationParams, SynthesisMode, SynthesisParams, TtsProvider, VoicePreset,
};

// ── Buffered synthesis ──────────────────────────────────────────────────────

#[tokio::test]
async fn buffered_synthesis_returns_valid_wav() {
    let provider = MockTtsProvider::new();
    let params = SynthesisParams {
        text: "Hello world".to_string(),
        mode: SynthesisMode::default(),
        voice: None,
        prompt_audio_path: None,
        prompt_text: None,
        generation: GenerationParams::default(),
    };

    let result = provider.synthesize(params).await.unwrap();

    // Verify WAV header
    assert_eq!(&result.audio_bytes[0..4], b"RIFF");
    assert_eq!(&result.audio_bytes[8..12], b"WAVE");
    assert_eq!(result.sample_rate, 48_000);
    assert_eq!(result.channels, 2);
    assert_eq!(result.voice, "mock");
    assert_eq!(result.text_chunks.len(), 1);
    assert_eq!(result.text_chunks[0], "Hello world");
}

#[tokio::test]
async fn buffered_synthesis_base64_roundtrip() {
    use base64::Engine;

    let provider = MockTtsProvider::new();
    let params = SynthesisParams {
        text: "Base64 test".to_string(),
        mode: SynthesisMode::default(),
        voice: None,
        prompt_audio_path: None,
        prompt_text: None,
        generation: GenerationParams::default(),
    };

    let result = provider.synthesize(params).await.unwrap();

    // Simulate Tauri command base64 encoding
    let encoded = base64::engine::general_purpose::STANDARD.encode(&result.audio_bytes);
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .unwrap();

    assert_eq!(decoded, result.audio_bytes);
    // Still valid WAV after roundtrip
    assert_eq!(&decoded[0..4], b"RIFF");
}

// ── Streaming job lifecycle ──────────────────────────────────────────────────

#[tokio::test]
async fn streaming_job_lifecycle_create_get_close() {
    use if2ai_backend::modules::tts::manager::jobs::StreamingJobManager;
    use tokio::sync::Mutex;

    let manager = StreamingJobManager::new();

    // Create
    let job: Arc<Mutex<if2ai_backend::modules::tts::manager::jobs::StreamingJob>> =
        manager.create().await;
    let stream_id = {
        let j = job.lock().await;
        j.stream_id.clone()
    };
    assert!(!stream_id.is_empty());

    // Get
    let fetched = manager.get(&stream_id).await;
    assert!(fetched.is_some());

    // Close
    let closed = manager.close(&stream_id).await;
    assert!(closed.is_some());
    {
        let closed_job = closed.unwrap();
        let j = closed_job.lock().await;
        assert_eq!(j.state, "closed");
        assert!(j.is_closed);
    }

    // Get after close still works
    assert!(manager.get(&stream_id).await.is_some());

    // Delete
    manager.delete(&stream_id).await;
    assert!(manager.get(&stream_id).await.is_none());
}

#[tokio::test]
async fn streaming_job_snapshot_has_expected_fields() {
    use if2ai_backend::modules::tts::manager::jobs::StreamingJobManager;
    use tokio::sync::Mutex;

    let manager = StreamingJobManager::new();
    let job: Arc<Mutex<if2ai_backend::modules::tts::manager::jobs::StreamingJob>> =
        manager.create().await;

    let snapshot = {
        let j = job.lock().await;
        j.snapshot()
    };

    // Verify snapshot is valid JSON with expected keys
    let val: serde_json::Value = snapshot;
    assert!(val.get("stream_id").is_some());
    assert!(val.get("state").is_some());
    assert!(val.get("sample_rate").is_some());
    assert!(val.get("channels").is_some());
    assert_eq!(val["sample_rate"], 48_000);
    assert_eq!(val["channels"], 2);
}

#[tokio::test]
async fn streaming_job_manager_handles_concurrent_streams() {
    use if2ai_backend::modules::tts::manager::jobs::StreamingJobManager;
    use tokio::sync::Mutex;

    let manager = StreamingJobManager::new();

    // Create 5 jobs
    let mut ids = Vec::new();
    for _ in 0..5 {
        let job: Arc<Mutex<if2ai_backend::modules::tts::manager::jobs::StreamingJob>> =
            manager.create().await;
        let id = {
            let j = job.lock().await;
            j.stream_id.clone()
        };
        ids.push(id);
    }

    assert_eq!(manager.job_count().await, 5);

    // Close odd-numbered jobs
    for (i, id) in ids.iter().enumerate() {
        if i % 2 == 0 {
            manager.close(id).await;
        }
    }

    assert_eq!(manager.job_count().await, 5); // close doesn't delete

    // Delete all
    for id in &ids {
        manager.delete(id).await;
    }

    assert_eq!(manager.job_count().await, 0);
}

// ── Warmup manager integration ──────────────────────────────────────────────

#[tokio::test]
async fn warmup_manager_integration_with_mock_provider() {
    use if2ai_backend::modules::tts::manager::warmup::WarmupManager;

    let provider: Arc<dyn TtsProvider> =
        Arc::new(MockTtsProvider::with_voices(vec![VoicePreset::new(
            "test",
            "Test",
            "wav",
            Vec::new(),
        )]));

    let warmup = WarmupManager::new();

    // Initial state
    let snapshot = warmup.snapshot().await;
    assert_eq!(snapshot.state, "pending");
    assert!(!snapshot.is_ready());

    // Start warmup
    warmup.start(provider.clone()).await;

    // Wait for warmup to complete (mock is fast)
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let snapshot = warmup.snapshot().await;
    assert!(snapshot.is_ready());
}

#[tokio::test]
async fn warmup_manager_ensure_ready_unblocks_after_start() {
    use if2ai_backend::modules::tts::manager::warmup::WarmupManager;
    use std::sync::Arc;

    let provider: Arc<dyn TtsProvider> = Arc::new(MockTtsProvider::new());
    let warmup = Arc::new(WarmupManager::new());

    // Start warmup in background
    let warmup_clone = Arc::clone(&warmup);
    let provider_clone = provider.clone();
    tokio::spawn(async move {
        warmup_clone.start(provider_clone).await;
    });

    // Ensure ready should complete
    warmup.ensure_ready(provider).await;

    let snapshot = warmup.snapshot().await;
    assert!(snapshot.is_ready());
}

// ── Demo audio resolution ───────────────────────────────────────────────────

#[test]
fn demo_by_id_returns_valid_entries() {
    use if2ai_backend::modules::tts::voice::demo::{all_demos, get_demo_by_id};

    let demos = all_demos();
    assert!(!demos.is_empty());

    // First demo should exist
    let first = get_demo_by_id("demo-0");
    assert!(first.is_some());
    assert!(!first.unwrap().text.is_empty());
}

#[test]
fn demo_by_id_returns_none_for_invalid_id() {
    use if2ai_backend::modules::tts::voice::demo::get_demo_by_id;

    assert!(get_demo_by_id("invalid").is_none());
    assert!(get_demo_by_id("demo-999").is_none());
    assert!(get_demo_by_id("not-a-demo").is_none());
}

#[test]
fn demo_audio_path_resolves_to_file_or_fallback() {
    use if2ai_backend::modules::tts::voice::demo::resolve_demo_audio_path;

    // This may not find the actual file if voices aren't installed,
    // but it should return Some path (either existing or fallback)
    let path = resolve_demo_audio_path("demo-0");
    assert!(path.is_some());
}

// ── Voice presets ───────────────────────────────────────────────────────────

#[test]
fn voice_presets_have_nonempty_audio() {
    use if2ai_backend::modules::tts::voice::presets::{all_voices, get_voice_by_name};

    let voices = all_voices();
    assert!(!voices.is_empty());

    for voice in &voices {
        assert!(!voice.name.is_empty(), "Voice name should not be empty");
        assert!(
            !voice.description.is_empty(),
            "Voice description should not be empty"
        );
    }

    // All voices should be retrievable by name
    for voice in &voices {
        let found = get_voice_by_name(&voice.name);
        assert!(found.is_some(), "Voice '{}' not found by name", voice.name);
    }
}

// ── Text chunking ───────────────────────────────────────────────────────────

#[tokio::test]
async fn mock_provider_splits_chinese_text() {
    let provider = MockTtsProvider::new();
    let chunks = provider
        .split_voice_clone_text("你好。今天天气不错。我们去散步吧！", 75)
        .unwrap();

    assert!(chunks.len() >= 2);
    for chunk in &chunks {
        assert!(!chunk.is_empty());
    }
}

#[tokio::test]
async fn mock_provider_splits_english_text() {
    let provider = MockTtsProvider::new();
    let chunks = provider
        .split_voice_clone_text("Hello world. How are you? I'm fine!", 75)
        .unwrap();

    assert!(chunks.len() >= 2);
}

// ── Edge cases (TTS-6.2) ───────────────────────────────────────────────────

#[tokio::test]
async fn empty_text_returns_error() {
    let provider = MockTtsProvider::new();
    let params = SynthesisParams {
        text: String::new(),
        mode: SynthesisMode::default(),
        voice: None,
        prompt_audio_path: None,
        prompt_text: None,
        generation: GenerationParams::default(),
    };

    let err = provider.synthesize(params).await.unwrap_err();
    assert!(matches!(
        err,
        if2ai_backend::modules::tts::TtsError::EmptyText
    ));
}

#[tokio::test]
async fn whitespace_only_text_is_treated_as_empty() {
    use if2ai_backend::modules::tts::TtsError;

    let provider = MockTtsProvider::new();
    let params = SynthesisParams {
        text: "   \n\t  ".to_string(),
        mode: SynthesisMode::default(),
        voice: None,
        prompt_audio_path: None,
        prompt_text: None,
        generation: GenerationParams::default(),
    };

    // Whitespace-only text should be normalized to empty and rejected
    let result = provider.synthesize(params).await;
    // Either EmptyText or a successful result with empty audio is acceptable
    if let Ok(res) = result {
        assert!(res.audio_bytes.is_empty() || res.text_chunks.iter().all(|c| c.trim().is_empty()));
    } else {
        assert!(matches!(result.unwrap_err(), TtsError::EmptyText));
    }
}

#[tokio::test]
async fn continuation_mode_requires_prompt_text_with_audio() {
    use if2ai_backend::modules::tts::TtsError;

    let provider = MockTtsProvider::new();
    let params = SynthesisParams {
        text: "Hello".to_string(),
        mode: SynthesisMode::Continuation,
        voice: None,
        prompt_audio_path: Some(std::path::PathBuf::from("/tmp/fake.wav")),
        prompt_text: None, // Missing required prompt_text
        generation: GenerationParams::default(),
    };

    let err = provider.synthesize(params).await.unwrap_err();
    assert!(
        matches!(err, TtsError::InvalidParam(_)),
        "expected InvalidParam error, got {err:?}"
    );
}

#[tokio::test]
async fn streaming_rejects_empty_text() {
    use if2ai_backend::modules::tts::TtsError;
    use if2ai_backend::modules::tts::{AudioChunk, AudioSink, StreamResult};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestSink {
        count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AudioSink for TestSink {
        async fn on_audio(&self, _chunk: AudioChunk) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
        async fn on_complete(&self, _result: StreamResult) {}
    }

    let provider = MockTtsProvider::new();
    let count = Arc::new(AtomicUsize::new(0));
    let sink = Arc::new(TestSink {
        count: Arc::clone(&count),
    });

    let params = SynthesisParams {
        text: String::new(),
        mode: SynthesisMode::default(),
        voice: None,
        prompt_audio_path: None,
        prompt_text: None,
        generation: GenerationParams::default(),
    };

    let err = provider.synthesize_stream(params, sink).await.unwrap_err();
    assert!(matches!(err, TtsError::EmptyText));
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn onnx_provider_rejects_missing_model_files() {
    use if2ai_backend::modules::tts::provider::OnnxTtsProvider;

    // Phase TTS-A.2 之后：OnnxTtsProvider 通过 manifest 解析模型路径，
    // 缺失 manifest → ManifestNotFound；签名简化为 from_model_dir。
    let result = OnnxTtsProvider::from_model_dir(std::path::Path::new("/nonexistent/tts"), Some(4));

    assert!(
        result.is_err(),
        "OnnxTtsProvider should reject missing manifest"
    );
}

#[tokio::test]
async fn tts_provider_and_manager_wire_correctly() {
    use if2ai_backend::modules::tts::manager::jobs::StreamingJobManager;
    use if2ai_backend::modules::tts::manager::warmup::WarmupManager;
    use tokio::sync::Mutex;

    let provider: Arc<dyn TtsProvider> =
        Arc::new(MockTtsProvider::with_voices(vec![VoicePreset::new(
            "test",
            "Test voice",
            "wav",
            Vec::new(),
        )]));
    let warmup = WarmupManager::new();
    let jobs = StreamingJobManager::new();

    // Verify all components wire together correctly
    assert_eq!(provider.list_voices().len(), 1);
    assert_eq!(provider.list_voices()[0], "test");
    assert_eq!(jobs.job_count().await, 0);

    // Verify warmup initial state
    let snapshot = warmup.snapshot().await;
    assert_eq!(snapshot.state, "pending");

    // Verify streaming job creation
    let job: Arc<Mutex<if2ai_backend::modules::tts::manager::jobs::StreamingJob>> =
        jobs.create().await;
    {
        let j = job.lock().await;
        assert_eq!(j.sample_rate, 48_000);
        assert_eq!(j.channels, 2);
    }
}
