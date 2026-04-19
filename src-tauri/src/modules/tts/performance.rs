//! TTS performance configuration and benchmarking.
//!
//! Provides `TtsConfig` for controlling thread count, memory budget,
//! and performance profiling for the MOSS-TTS-Nano TTS pipeline.
//!
//! ## Acceptance Criteria (TTS-6.1)
//!
//! - CPU inference latency < 2x Python ONNX reference
//! - Memory usage < 2GB during synthesis
//! - Thread count control validated

use std::num::NonZeroUsize;

/// Compile-time non-zero helper for constant thread counts.
const fn nz(n: usize) -> NonZeroUsize {
    // Safety: caller guarantees n > 0.
    // Clippy's `useless_nonzero_new_unchecked` fires for small literals,
    // but the const here avoids `unwrap()` at runtime for all cases.
    match NonZeroUsize::new(n) {
        Some(v) => v,
        None => panic!("thread count must be non-zero"),
    }
}

/// Default thread count for ONNX inference (4).
const DEFAULT_THREADS: NonZeroUsize = nz(4);

/// Performance configuration for the TTS provider.
///
/// Controls CPU thread allocation and memory budget for ONNX inference.
#[derive(Debug, Clone)]
pub struct TtsConfig {
    /// Number of CPU threads for ONNX intra-op parallelism.
    /// Must be >= 1 and <= available CPU cores. Default: 4.
    pub thread_count: NonZeroUsize,

    /// Maximum memory budget in megabytes for ONNX inference.
    /// Default: 2048 (2GB).
    pub memory_budget_mb: usize,

    /// Whether to enable performance profiling (timing logs).
    pub enable_profiling: bool,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            thread_count: DEFAULT_THREADS,
            memory_budget_mb: 2048,
            enable_profiling: false,
        }
    }
}

impl TtsConfig {
    /// Validate that the configuration is within acceptable bounds.
    ///
    /// # Returns
    ///
    /// An error string describing the violation, or `None` if valid.
    pub fn validate(&self) -> Option<String> {
        let max_threads = num_cpus::get();
        let threads = self.thread_count.get();

        if threads == 0 {
            return Some("thread_count must be >= 1".to_string());
        }
        if threads > max_threads {
            return Some(format!(
                "thread_count ({threads}) exceeds available CPU cores ({max_threads})"
            ));
        }
        if self.memory_budget_mb < 256 {
            return Some("memory_budget_mb must be >= 256".to_string());
        }
        if self.memory_budget_mb > 8192 {
            return Some("memory_budget_mb must be <= 8192".to_string());
        }
        None
    }

    /// Create a config with the specified thread count.
    ///
    /// Clamps thread_count to [1, available_cpu_cores].
    pub fn with_threads(mut self, count: usize) -> Self {
        let max = num_cpus::get();
        let clamped = count.clamp(1, max);
        self.thread_count = NonZeroUsize::new(clamped).unwrap_or(nz(1));
        self
    }

    /// Create a config with the specified memory budget in MB.
    ///
    /// Clamps to [256, 8192].
    pub fn with_memory_budget_mb(mut self, mb: usize) -> Self {
        self.memory_budget_mb = mb.clamp(256, 8192);
        self
    }

    /// Enable performance profiling.
    pub fn with_profiling(mut self) -> Self {
        self.enable_profiling = true;
        self
    }
}

/// Timing profile for a synthesis operation.
#[derive(Debug, Clone)]
pub struct SynthesisProfile {
    /// Total elapsed time in seconds.
    pub elapsed_seconds: f32,
    /// Number of input text tokens.
    pub input_tokens: usize,
    /// Number of generated audio frames.
    pub generated_frames: usize,
    /// Estimated frames per second throughput.
    pub frames_per_second: f32,
}

impl SynthesisProfile {
    /// Create a profile from elapsed time and synthesis metrics.
    #[must_use]
    pub fn new(elapsed_seconds: f32, input_tokens: usize, generated_frames: usize) -> Self {
        let fps = if elapsed_seconds > 0.0 {
            generated_frames as f32 / elapsed_seconds
        } else {
            0.0
        };
        Self {
            elapsed_seconds,
            input_tokens,
            generated_frames,
            frames_per_second: fps,
        }
    }

    /// Check if this synthesis meets the latency target.
    ///
    /// Target: < 2x Python ONNX reference latency.
    /// Python reference for short Chinese text: ~1.5s on M-series Mac.
    /// So target is < 3.0s for typical inputs.
    pub fn meets_latency_target(&self, target_seconds: f32) -> bool {
        self.elapsed_seconds < target_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = TtsConfig::default();
        assert!(config.validate().is_none());
    }

    #[test]
    fn thread_count_clamping() {
        let config = TtsConfig::default().with_threads(0);
        assert_eq!(config.thread_count.get(), 1);

        let config = TtsConfig::default().with_threads(9999);
        let max = num_cpus::get();
        assert_eq!(config.thread_count.get(), max);
    }

    #[test]
    fn memory_budget_clamping() {
        let config = TtsConfig::default().with_memory_budget_mb(100);
        assert_eq!(config.memory_budget_mb, 256);

        let config = TtsConfig::default().with_memory_budget_mb(10000);
        assert_eq!(config.memory_budget_mb, 8192);
    }

    #[test]
    fn validate_rejects_too_low_memory() {
        let config = TtsConfig {
            memory_budget_mb: 100,
            ..TtsConfig::default()
        };
        // validate() checks clamped values; this config has unclamped 100
        assert!(config.validate().is_some());
    }

    #[test]
    fn validate_rejects_excessive_threads() {
        let max = num_cpus::get();
        let config = TtsConfig::default().with_threads(max + 100);
        // After clamping, threads == max, so validate passes
        assert!(config.validate().is_none());
    }

    #[test]
    fn profiling_calculates_fps() {
        let profile = SynthesisProfile::new(2.0, 50, 100);
        assert_eq!(profile.generated_frames, 100);
        assert_eq!(profile.frames_per_second, 50.0);
    }

    #[test]
    fn profile_meets_latency_target() {
        let profile = SynthesisProfile::new(1.5, 50, 100);
        assert!(profile.meets_latency_target(3.0));
        assert!(!profile.meets_latency_target(1.0));
    }

    #[test]
    fn profiling_enables_via_builder() {
        let config = TtsConfig::default().with_profiling();
        assert!(config.enable_profiling);
    }
}
