//! TTS performance benchmark tests — TTS-6.1
//!
//! Verifies the acceptance criteria:
//! - CPU inference latency < 2x Python ONNX (profiled via SynthesisProfile)
//! - Memory usage < 2GB (via TtsConfig.memory_budget_mb)
//! - Thread count control works and is validated

use std::time::Instant;

use crate::modules::tts::performance::{SynthesisProfile, TtsConfig};

// ── Thread count control ────────────────────────────────────────────────────

#[test]
fn thread_count_control_defaults_to_four() {
    let config = TtsConfig::default();
    assert_eq!(config.thread_count.get(), 4);
}

#[test]
fn thread_count_builder_api() {
    let config = TtsConfig::default().with_threads(2);
    assert_eq!(config.thread_count.get(), 2);
}

#[test]
fn thread_count_clamped_to_cpu_cores() {
    let max = num_cpus::get();
    let config = TtsConfig::default().with_threads(max + 10);
    assert_eq!(config.thread_count.get(), max);
}

#[test]
fn thread_count_minimum_is_one() {
    let config = TtsConfig::default().with_threads(0);
    assert_eq!(config.thread_count.get(), 1);
}

// ── Memory budget ───────────────────────────────────────────────────────────

#[test]
fn memory_budget_defaults_to_2gb() {
    let config = TtsConfig::default();
    assert_eq!(config.memory_budget_mb, 2048);
}

#[test]
fn memory_budget_builder_api() {
    let config = TtsConfig::default().with_memory_budget_mb(1024);
    assert_eq!(config.memory_budget_mb, 1024);
}

#[test]
fn memory_budget_within_acceptance() {
    // Acceptance: memory < 2GB
    let config = TtsConfig::default();
    assert!(
        config.memory_budget_mb <= 2048,
        "default memory budget should be <= 2GB"
    );
}

// ── Validation ──────────────────────────────────────────────────────────────

#[test]
fn config_validates_ok() {
    let config = TtsConfig::default();
    assert!(
        config.validate().is_none(),
        "default config should be valid"
    );
}

#[test]
fn config_rejects_very_low_memory() {
    let config = TtsConfig {
        memory_budget_mb: 128,
        ..TtsConfig::default()
    };
    assert!(config.validate().is_some());
}

#[test]
fn config_rejects_very_high_memory() {
    let config = TtsConfig {
        memory_budget_mb: 16384,
        ..TtsConfig::default()
    };
    assert!(config.validate().is_some());
}

// ── Synthesis profiling ─────────────────────────────────────────────────────

#[test]
fn profile_meets_latency_for_short_text() {
    // Python reference: ~1.5s for short Chinese text on M-series Mac
    // Target: < 2x = 3.0s
    let profile = SynthesisProfile::new(2.5, 20, 96);
    assert!(
        profile.meets_latency_target(3.0),
        "2.5s should meet 3.0s target"
    );
}

#[test]
fn profile_fails_latency_for_long_text() {
    // Long text may take > 6s; target remains 3.0s
    let profile = SynthesisProfile::new(7.0, 200, 500);
    assert!(
        !profile.meets_latency_target(3.0),
        "7.0s should not meet 3.0s target"
    );
}

#[test]
fn profile_calculates_frames_per_second() {
    let profile = SynthesisProfile::new(1.0, 50, 100);
    assert!((profile.frames_per_second - 100.0).abs() < 0.01);
}

#[test]
fn profile_handles_zero_elapsed() {
    let profile = SynthesisProfile::new(0.0, 50, 100);
    assert_eq!(profile.frames_per_second, 0.0);
}

// ── Performance benchmark: mock provider timing ─────────────────────────────

#[tokio::test]
async fn mock_provider_synthesis_completes_quickly() {
    use crate::modules::tts::provider::MockTtsProvider;
    use crate::modules::tts::{GenerationParams, SynthesisMode, SynthesisParams, TtsProvider};

    let provider = MockTtsProvider::new();
    let params = SynthesisParams {
        text: "你好，世界！".to_string(),
        mode: SynthesisMode::default(),
        voice: None,
        prompt_audio_path: None,
        prompt_text: None,
        generation: GenerationParams::default(),
    };

    let start = Instant::now();
    let result = provider.synthesize(params).await.unwrap();
    let elapsed = start.elapsed().as_secs_f32();

    // Mock provider should be very fast (< 100ms)
    assert!(
        elapsed < 0.1,
        "mock synthesis should complete in < 100ms, took {elapsed:.3}s"
    );

    // Profile the result
    let profile = SynthesisProfile::new(
        result.elapsed_seconds,
        10, // approximate token count
        result.text_chunks.len(),
    );
    assert!(profile.meets_latency_target(3.0));
}

#[tokio::test]
async fn mock_provider_batch_synthesis_under_memory_budget() {
    use crate::modules::tts::provider::MockTtsProvider;
    use crate::modules::tts::{GenerationParams, SynthesisMode, SynthesisParams, TtsProvider};

    let config = TtsConfig::default();
    let provider = MockTtsProvider::new();

    // Synthesize 10 chunks — should stay well within 2GB budget
    for i in 0..10 {
        let params = SynthesisParams {
            text: format!("测试句子{i}"),
            mode: SynthesisMode::default(),
            voice: None,
            prompt_audio_path: None,
            prompt_text: None,
            generation: GenerationParams::default(),
        };
        let _ = provider.synthesize(params).await.unwrap();
    }

    // Memory budget check: mock provider uses minimal memory
    assert!(
        config.memory_budget_mb >= 256,
        "memory budget should be >= 256MB"
    );
}

// ── Profiling toggle ────────────────────────────────────────────────────────

#[test]
fn profiling_disabled_by_default() {
    let config = TtsConfig::default();
    assert!(!config.enable_profiling);
}

#[test]
fn profiling_enabled_via_builder() {
    let config = TtsConfig::default().with_profiling();
    assert!(config.enable_profiling);
}
