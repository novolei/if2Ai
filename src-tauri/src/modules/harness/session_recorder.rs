//! Session Recorder
//!
//! Writes agent loop events to JSONL files for trace replay and harness
//! evaluation. Each session produces one file: `{base_dir}/{session_id}.jsonl`.
//!
//! Each line is a JSON-serialised [`AgentEvent`] followed by a newline.
//! Recording is started explicitly via [`SessionRecorder::start`] and stopped
//! via [`SessionRecorder::stop`].
//!
//! # `#[allow(dead_code)]` justification
//! `trace_path`, `is_recording`, `base_dir`, and `active_sessions` are public
//! API methods consumed by harness IPC commands and harness-cli. The linter
//! does not see them as used because the IPC path is gated on `harness.is_some()`.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::event_bus::EventBus;

/// State for a single active recording.
struct Recording {
    file: File,
}

impl std::fmt::Debug for Recording {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recording").finish_non_exhaustive()
    }
}

/// Session recorder — persists agent events to JSONL trace files.
///
/// Cloning shares the same underlying state.
#[derive(Clone, Debug)]
pub struct SessionRecorder {
    /// Directory where JSONL files are written.
    base_dir: PathBuf,
    /// Active recordings keyed by session_id.
    recordings: Arc<Mutex<HashMap<String, Recording>>>,
}

impl SessionRecorder {
    /// Create a new `SessionRecorder` that writes to `base_dir`.
    ///
    /// The directory is created lazily on the first call to [`start`].
    #[must_use]
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            recordings: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start recording events for `session_id`.
    ///
    /// Creates/truncates `{base_dir}/{session_id}.jsonl`. A background task
    /// reads from `bus` and appends each event as a JSON line.
    ///
    /// If recording for this session is already active, this is a no-op.
    ///
    /// # Errors
    /// Returns `Err` if the directory cannot be created or the file cannot be
    /// opened.
    pub async fn start(
        &self,
        session_id: impl Into<String>,
        bus: &EventBus,
    ) -> Result<(), std::io::Error> {
        let session_id = session_id.into();

        let mut recordings = self.recordings.lock().await;
        if recordings.contains_key(&session_id) {
            return Ok(());
        }

        tokio::fs::create_dir_all(&self.base_dir).await?;

        let path = self.base_dir.join(format!("{session_id}.jsonl"));
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await?;

        recordings.insert(session_id.clone(), Recording { file });
        drop(recordings);

        // Spawn background writer.
        let recordings_arc = Arc::clone(&self.recordings);
        let sid = session_id.clone();
        let mut rx = bus.subscribe();

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        // Only record events for this session.
                        if event.session_id() != Some(sid.as_str()) {
                            continue;
                        }

                        let Ok(mut line) = serde_json::to_string(&event) else {
                            tracing::warn!(
                                "[SessionRecorder] failed to serialise event for session {sid}"
                            );
                            continue;
                        };
                        line.push('\n');

                        let mut guard = recordings_arc.lock().await;
                        if let Some(rec) = guard.get_mut(&sid) {
                            if let Err(e) = rec.file.write_all(line.as_bytes()).await {
                                tracing::warn!(
                                    "[SessionRecorder] write error for session {sid}: {e}"
                                );
                            }
                        } else {
                            // Recording was stopped; exit task.
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(
                            "[SessionRecorder] missed {n} events for session {sid} (channel overflow)"
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        tracing::info!("[SessionRecorder] started recording session {session_id} → {path:?}");
        Ok(())
    }

    /// Stop recording for `session_id` and flush any buffered data.
    ///
    /// After this call the JSONL file is complete and the background task will
    /// exit naturally (it detects the recording entry is gone).
    pub async fn stop(&self, session_id: &str) {
        let mut recordings = self.recordings.lock().await;
        if let Some(mut rec) = recordings.remove(session_id) {
            if let Err(e) = rec.file.flush().await {
                tracing::warn!("[SessionRecorder] flush error for session {session_id}: {e}");
            }
            tracing::info!("[SessionRecorder] stopped recording session {session_id}");
        }
    }

    /// Return the path of the JSONL file for `session_id`, whether or not
    /// recording is active.
    #[must_use]
    pub fn trace_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(format!("{session_id}.jsonl"))
    }

    /// Return whether recording is active for `session_id`.
    pub async fn is_recording(&self, session_id: &str) -> bool {
        self.recordings.lock().await.contains_key(session_id)
    }

    /// List all sessions that are currently being recorded.
    pub async fn active_sessions(&self) -> Vec<String> {
        self.recordings.lock().await.keys().cloned().collect()
    }

    /// Resolve the recorder's base directory.
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::event_bus::{AgentEvent, EventBus};
    use chrono::Utc;

    #[tokio::test]
    async fn records_events_to_jsonl() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let recorder = SessionRecorder::new(tmp.path());
        let bus = EventBus::new();

        recorder
            .start("sess1", &bus)
            .await
            .expect("start recording");

        bus.emit(AgentEvent::TurnStarted {
            turn_number: 1,
            session_id: "sess1".to_string(),
            at: Utc::now(),
        })
        .ok();

        // Give background task time to flush.
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        recorder.stop("sess1").await;

        let path = recorder.trace_path("sess1");
        let content = tokio::fs::read_to_string(&path)
            .await
            .expect("read trace file");
        assert!(!content.is_empty(), "trace file should have content");
        assert!(
            content.contains("turn_started"),
            "should contain turn_started event"
        );
    }

    #[tokio::test]
    async fn is_recording_returns_correct_state() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let recorder = SessionRecorder::new(tmp.path());
        let bus = EventBus::new();

        assert!(!recorder.is_recording("x").await);
        recorder.start("x", &bus).await.expect("start");
        assert!(recorder.is_recording("x").await);
        recorder.stop("x").await;
        assert!(!recorder.is_recording("x").await);
    }
}
