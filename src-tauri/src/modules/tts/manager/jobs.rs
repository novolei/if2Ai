//! Streaming job manager for TTS.
//!
//! Mirrors the Python `StreamingJob` and `StreamingJobManager`
//! from `app.py`: manages concurrent streaming synthesis
//! sessions, tracks their state, and provides lifecycle
//! operations (create, get, close, delete).
//!
//! ## StreamingJob State Machine
//!
//! ```text
//! starting → streaming → done | failed | closed
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! let manager = StreamingJobManager::new();
//! let job = manager.create();
//! println!("stream_id={}", job.stream_id);
//! manager.close(&job.stream_id);
//! manager.delete(&job.stream_id);
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// A single streaming synthesis session.
///
/// Mirrors the Python `StreamingJob` dataclass from `app.py`.
#[derive(Debug)]
pub struct StreamingJob {
    /// Unique identifier for this stream (format: `stream-{timestamp}-{uuid}`).
    pub stream_id: String,
    /// Current state: "starting", "streaming", "done", "failed", "closed".
    pub state: String,
    /// Human-readable run status message.
    pub run_status: String,
    /// Error message if state is "failed".
    pub error: Option<String>,
    /// Path to prompt audio for voice clone mode.
    pub prompt_audio_path: Option<String>,
    /// Sample rate of the audio (always 48000 for MOSS-TTS-Nano).
    pub sample_rate: u32,
    /// Number of audio channels (always 2 for stereo).
    pub channels: u16,
    /// Total emitted audio seconds.
    pub emitted_audio_seconds: f32,
    /// Lead seconds (buffer ahead of playback).
    pub lead_seconds: f32,
    /// Current text chunk index being synthesized.
    pub current_chunk_index: Option<usize>,
    /// Text chunks the input was split into.
    pub text_chunks: Vec<String>,
    /// Whether the stream has been closed by the client.
    pub is_closed: bool,
    /// Monotonic timestamp when the job was created.
    pub created_at: std::time::Instant,
    /// Monotonic timestamp when synthesis started.
    pub started_at: Option<std::time::Instant>,
    /// Monotonic timestamp when first audio was emitted.
    pub first_audio_at: Option<std::time::Instant>,
    /// Monotonic timestamp when synthesis completed.
    pub completed_at: Option<std::time::Instant>,
}

impl StreamingJob {
    /// Create a new streaming job with the given ID.
    pub fn new(stream_id: String) -> Self {
        Self {
            stream_id,
            state: "starting".to_string(),
            run_status: "Starting realtime synthesis...".to_string(),
            error: None,
            prompt_audio_path: None,
            sample_rate: 48_000,
            channels: 2,
            emitted_audio_seconds: 0.0,
            lead_seconds: 0.0,
            current_chunk_index: None,
            text_chunks: Vec::new(),
            is_closed: false,
            created_at: std::time::Instant::now(),
            started_at: None,
            first_audio_at: None,
            completed_at: None,
        }
    }

    /// Calculate first audio latency in seconds.
    ///
    /// Returns `None` if either `started_at` or `first_audio_at` is not set.
    #[must_use]
    pub fn first_audio_latency(&self) -> Option<f32> {
        match (self.started_at, self.first_audio_at) {
            (Some(start), Some(first)) => {
                let latency = first.duration_since(start).as_secs_f32();
                Some(latency.max(0.0))
            }
            _ => None,
        }
    }

    /// Check if the job has completed successfully.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.state == "done"
    }

    /// Check if the job has failed.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.state == "failed"
    }

    /// Check if the job has been closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    /// Get a snapshot of the current job state as a serializable map.
    ///
    /// Mirrors `StreamingJob.snapshot()` from `app.py`.
    #[must_use]
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "stream_id": self.stream_id,
            "state": self.state,
            "run_status": self.run_status,
            "error": self.error,
            "prompt_audio_path": self.prompt_audio_path,
            "sample_rate": self.sample_rate,
            "channels": self.channels,
            "emitted_audio_seconds": self.emitted_audio_seconds,
            "lead_seconds": self.lead_seconds,
            "current_chunk_index": self.current_chunk_index,
            "text_chunks": self.text_chunks,
            "first_audio_latency_seconds": self.first_audio_latency(),
            "completed_at": self.completed_at.map(|t| t.duration_since(self.created_at).as_secs_f32()),
            "ready": self.is_done(),
            "failed": self.is_failed(),
            "closed": self.is_closed,
        })
    }
}

/// Manages concurrent streaming synthesis jobs.
///
/// Mirrors the Python `StreamingJobManager` from `app.py`.
pub struct StreamingJobManager {
    inner: Arc<Mutex<JobMap>>,
    /// Counter for generating unique stream IDs.
    counter: AtomicU64,
}

/// Internal job map protected by async mutex.
struct JobMap {
    jobs: HashMap<String, Arc<Mutex<StreamingJob>>>,
}

impl StreamingJobManager {
    /// Create a new streaming job manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(JobMap {
                jobs: HashMap::new(),
            })),
            counter: AtomicU64::new(0),
        }
    }

    /// Create a new streaming job and return it.
    ///
    /// Generates a unique stream ID and registers the job.
    /// Mirrors `StreamingJobManager.create()` from `app.py`.
    pub async fn create(&self) -> Arc<Mutex<StreamingJob>> {
        let seq = self.counter.fetch_add(1, Ordering::Relaxed);
        let short_uuid = Uuid::new_v4()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();
        let stream_id = format!(
            "stream-{}-{seq}-{short_uuid}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        let job = Arc::new(Mutex::new(StreamingJob::new(stream_id.clone())));
        let mut map = self.inner.lock().await;
        map.jobs.insert(stream_id, Arc::clone(&job));
        job
    }

    /// Get a streaming job by ID.
    ///
    /// Returns `None` if the job doesn't exist.
    /// Mirrors `StreamingJobManager.get()` from `app.py`.
    pub async fn get(&self, stream_id: &str) -> Option<Arc<Mutex<StreamingJob>>> {
        let map = self.inner.lock().await;
        map.jobs.get(stream_id).cloned()
    }

    /// Close a streaming job by ID.
    ///
    /// Marks the job as closed and updates its state if not already done/failed.
    /// Mirrors `StreamingJobManager.close()` from `app.py`.
    ///
    /// # Returns
    ///
    /// The closed job if it existed, or `None`.
    pub async fn close(&self, stream_id: &str) -> Option<Arc<Mutex<StreamingJob>>> {
        let job = {
            let map = self.inner.lock().await;
            map.jobs.get(stream_id).cloned()
        };
        if let Some(job) = &job {
            let mut j = job.lock().await;
            j.is_closed = true;
            if j.state != "done" && j.state != "failed" {
                j.state = "closed".to_string();
            }
        }
        job
    }

    /// Delete a streaming job by ID, removing it from the map.
    ///
    /// Mirrors `StreamingJobManager.delete()` from `app.py`.
    ///
    /// # Returns
    ///
    /// The deleted job if it existed, or `None`.
    pub async fn delete(&self, stream_id: &str) -> Option<Arc<Mutex<StreamingJob>>> {
        let mut map = self.inner.lock().await;
        map.jobs.remove(stream_id)
    }

    /// Get the count of active jobs.
    #[must_use]
    pub async fn job_count(&self) -> usize {
        let map = self.inner.lock().await;
        map.jobs.len()
    }
}

impl Default for StreamingJobManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_job_new_starts_as_starting() {
        let job = StreamingJob::new("stream-test".to_string());
        assert_eq!(job.state, "starting");
        assert_eq!(job.run_status, "Starting realtime synthesis...");
        assert!(!job.is_done());
        assert!(!job.is_failed());
        assert!(!job.is_closed());
    }

    #[test]
    fn first_audio_latency_none_when_missing() {
        let job = StreamingJob::new("stream-test".to_string());
        assert!(job.first_audio_latency().is_none());
    }

    #[test]
    fn first_audio_latency_computed_when_both_timestamps_set() {
        let now = std::time::Instant::now();
        let mut job = StreamingJob::new("stream-test".to_string());
        job.started_at = Some(now);
        job.first_audio_at = Some(now + std::time::Duration::from_millis(150));
        let latency = job.first_audio_latency().unwrap();
        assert!((latency - 0.15).abs() < 0.01);
    }

    #[test]
    fn snapshot_contains_expected_fields() {
        let job = StreamingJob::new("stream-test".to_string());
        let snapshot = job.snapshot();
        assert_eq!(snapshot["stream_id"], "stream-test");
        assert_eq!(snapshot["state"], "starting");
        assert_eq!(snapshot["sample_rate"], 48000);
        assert_eq!(snapshot["channels"], 2);
    }

    #[tokio::test]
    async fn job_manager_create_and_get() {
        let manager = StreamingJobManager::new();
        assert_eq!(manager.job_count().await, 0);

        let job = manager.create().await;
        let j = job.lock().await;
        assert!(j.stream_id.starts_with("stream-"));
        drop(j);
        assert_eq!(manager.job_count().await, 1);

        let got = manager.get(&job.lock().await.stream_id).await;
        assert!(got.is_some());
    }

    #[tokio::test]
    async fn job_manager_close_and_delete() {
        let manager = StreamingJobManager::new();
        let job = manager.create().await;
        let stream_id = job.lock().await.stream_id.clone();

        // Close the job
        let closed = manager.close(&stream_id).await;
        assert!(closed.is_some());
        assert!(closed.unwrap().lock().await.is_closed);

        // Delete the job
        let deleted = manager.delete(&stream_id).await;
        assert!(deleted.is_some());
        assert_eq!(manager.job_count().await, 0);
    }

    #[tokio::test]
    async fn job_manager_get_returns_none_for_missing() {
        let manager = StreamingJobManager::new();
        let got = manager.get("nonexistent").await;
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn job_manager_close_does_not_override_done() {
        let manager = StreamingJobManager::new();
        let job = manager.create().await;
        let stream_id = job.lock().await.stream_id.clone();

        // Simulate job completed
        {
            let mut j = job.lock().await;
            j.state = "done".to_string();
        }

        // Closing should not change state from "done"
        manager.close(&stream_id).await;
        let j = job.lock().await;
        assert_eq!(j.state, "done");
        assert!(j.is_closed);
    }
}
