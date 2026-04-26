//! Canonical append-only run event log (MIG-016).
//!
//! This module establishes the durable fact stream that later
//! runtime projections and recovery flows will replay. It is
//! intentionally sidecar-only in MIG-016: writes are best-effort,
//! append-only, and never block the primary turn execution path.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::contracts::common::RuntimeEventEnvelope;

/// One durable append-only run-log record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunLogEntry {
    pub event_id: String,
    pub session_id: String,
    pub run_id: String,
    pub seq: u64,
    pub event_type: String,
    pub occurred_at: String,
    pub payload: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
}

impl RunLogEntry {
    fn new(
        session_id: String,
        run_id: String,
        seq: u64,
        event_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            session_id,
            run_id,
            seq,
            event_type: event_type.into(),
            occurred_at: chrono::Utc::now().to_rfc3339(),
            payload,
            causation_id: None,
            correlation_id: None,
            tool_call_id: None,
            attempt_id: None,
        }
    }

    /// Construct a [`RunLogEntry`] from a canonical
    /// [`RuntimeEventEnvelope`].
    ///
    /// The envelope provides all correlation identifiers, event type,
    /// timestamp, and payload. This method adds the durable-log
    /// concerns (`event_id`, `seq`, session/run scoping) that the
    /// envelope deliberately does not carry.
    ///
    /// When `correlation.session_id` or `correlation.run_id` are
    /// `None`, the caller must provide explicit fallback values
    /// through the `session_id` and `run_id` parameters.
    pub fn from_envelope(
        envelope: &RuntimeEventEnvelope,
        seq: u64,
        session_id: impl Into<String>,
        run_id: impl Into<String>,
    ) -> Self {
        let corr = &envelope.correlation;
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            session_id: corr.session_id.clone().unwrap_or_else(|| session_id.into()),
            run_id: corr.run_id.clone().unwrap_or_else(|| run_id.into()),
            seq,
            event_type: format!(
                "{}:{}",
                serde_json::to_value(envelope.event_type)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "unknown".to_string()),
                envelope.payload_family.0
            ),
            occurred_at: envelope.emitted_at.clone(),
            payload: envelope.payload.clone(),
            causation_id: None,
            correlation_id: corr.stream_id.clone(),
            tool_call_id: None,
            attempt_id: corr.attempt_id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct EventLogSink {
    path: PathBuf,
    write_lock: Arc<Mutex<()>>,
}

impl EventLogSink {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            write_lock: Arc::new(Mutex::new(())),
        }
    }
}

/// Per-run append-only logger with monotonic `seq`.
///
/// Writes are best-effort: any IO failure is logged and ignored so
/// the main runtime loop keeps flowing.
#[derive(Clone, Debug)]
pub struct RunEventLogger {
    session_id: String,
    run_id: String,
    seq: Arc<AtomicU64>,
    sink: Option<EventLogSink>,
}

impl RunEventLogger {
    /// Create a durable logger rooted under `base_dir`.
    #[must_use]
    pub fn for_base_dir(
        base_dir: impl AsRef<Path>,
        session_id: impl Into<String>,
        run_id: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let run_id = run_id.into();
        let path = Self::run_log_path(base_dir.as_ref(), &session_id, &run_id);
        Self {
            session_id,
            run_id,
            seq: Arc::new(AtomicU64::new(0)),
            sink: Some(EventLogSink::new(path)),
        }
    }

    /// Create a logger rooted under Tauri's `app_data_dir`.
    ///
    /// Falls back to a disabled logger when the app data path is not
    /// available so runtime execution never fails only because the
    /// event log root could not be resolved.
    #[must_use]
    pub fn for_app_handle(
        app_handle: &AppHandle,
        session_id: impl Into<String>,
        run_id: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let run_id = run_id.into();
        match app_handle.path().app_data_dir() {
            Ok(base_dir) => Self::for_base_dir(base_dir, session_id, run_id),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    session_id = %session_id,
                    run_id = %run_id,
                    "[event_log] app_data_dir unavailable; disabling durable run logging"
                );
                Self::disabled(session_id, run_id)
            }
        }
    }

    /// Create a logger that tracks sequence numbers but skips disk writes.
    #[must_use]
    pub fn disabled(session_id: impl Into<String>, run_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            run_id: run_id.into(),
            seq: Arc::new(AtomicU64::new(0)),
            sink: None,
        }
    }

    /// Borrow the canonical `run_id`.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Append one event asynchronously.
    pub async fn append(
        &self,
        event_type: impl Into<String>,
        payload: impl Serialize,
    ) -> RunLogEntry {
        let entry = self.make_entry(event_type, payload);
        if let Some(sink) = &self.sink {
            if let Err(error) = append_entry_async(sink, &entry).await {
                tracing::warn!(
                    error = %error,
                    session_id = %entry.session_id,
                    run_id = %entry.run_id,
                    seq = entry.seq,
                    event_type = %entry.event_type,
                    path = %sink.path.display(),
                    "[event_log] append failed (non-fatal)"
                );
            }
        }
        entry
    }

    /// Append one event from synchronous code paths.
    pub fn append_sync(
        &self,
        event_type: impl Into<String>,
        payload: impl Serialize,
    ) -> RunLogEntry {
        let entry = self.make_entry(event_type, payload);
        if let Some(sink) = &self.sink {
            if let Err(error) = append_entry_sync(sink, &entry) {
                tracing::warn!(
                    error = %error,
                    session_id = %entry.session_id,
                    run_id = %entry.run_id,
                    seq = entry.seq,
                    event_type = %entry.event_type,
                    path = %sink.path.display(),
                    "[event_log] append failed (non-fatal)"
                );
            }
        }
        entry
    }

    /// Absolute path to this run's JSONL file, when durability is enabled.
    #[must_use]
    pub fn file_path(&self) -> Option<&Path> {
        self.sink.as_ref().map(|sink| sink.path.as_path())
    }

    /// Current monotonic sequence number for this run (T-020).
    /// Returns the last `seq` that was assigned (0 if no events have been logged yet).
    #[must_use]
    pub fn current_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    fn make_entry(&self, event_type: impl Into<String>, payload: impl Serialize) -> RunLogEntry {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let mut payload = serde_json::to_value(payload).unwrap_or_else(|error| {
            serde_json::json!({
                "serialization_error": error.to_string()
            })
        });
        if crate::modules::security::redaction::event_log_redaction_enabled() {
            crate::modules::security::redaction::redact_value_in_place(&mut payload);
        }
        RunLogEntry::new(
            self.session_id.clone(),
            self.run_id.clone(),
            seq,
            event_type,
            payload,
        )
    }

    fn run_log_path(base_dir: &Path, session_id: &str, run_id: &str) -> PathBuf {
        base_dir
            .join("runtime")
            .join("run-log")
            .join(session_id)
            .join(format!("{run_id}.jsonl"))
    }
}

async fn append_entry_async(
    sink: &EventLogSink,
    entry: &RunLogEntry,
) -> Result<(), std::io::Error> {
    let sink = sink.clone();
    let entry = entry.clone();
    tokio::task::spawn_blocking(move || append_entry_sync(&sink, &entry))
        .await
        .map_err(|error| std::io::Error::other(format!("join run log append: {error}")))?
}

fn append_entry_sync(sink: &EventLogSink, entry: &RunLogEntry) -> Result<(), std::io::Error> {
    let _guard = sink
        .write_lock
        .lock()
        .map_err(|_| std::io::Error::other("run log write lock poisoned"))?;
    if let Some(parent) = sink.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&sink.path)?;
    let line = serde_json::to_vec(entry)
        .map_err(|error| std::io::Error::other(format!("serialize run log entry: {error}")))?;
    file.write_all(&line)?;
    file.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("if2ai-{label}-{nanos}"))
    }

    fn read_entries(path: &Path) -> Vec<RunLogEntry> {
        std::fs::read_to_string(path)
            .expect("read run log")
            .lines()
            .map(|line| serde_json::from_str::<RunLogEntry>(line).expect("parse run log entry"))
            .collect()
    }

    #[tokio::test]
    async fn event_log_seq_is_monotonic_with_mixed_append_modes() {
        let root = unique_temp_root("event-log-seq");
        let logger = RunEventLogger::for_base_dir(&root, "session-1", "run-1");

        let first = logger.append("run_started", serde_json::json!({})).await;
        let second = logger.append_sync("text_delta", serde_json::json!({ "text": "hi" }));
        let third = logger
            .append("stream_complete", serde_json::json!({ "status": "ok" }))
            .await;

        assert_eq!(first.seq, 1);
        assert_eq!(second.seq, 2);
        assert_eq!(third.seq, 3);

        let path = logger.file_path().expect("run log path");
        let entries = read_entries(path);
        let seqs: Vec<u64> = entries.iter().map(|entry| entry.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn event_log_persists_stream_terminal_event() {
        let root = unique_temp_root("event-log-terminal");
        let logger = RunEventLogger::for_base_dir(&root, "session-2", "run-2");

        logger
            .append(
                "stream_error",
                serde_json::json!({
                    "reason": "network timeout",
                    "resume_available": true
                }),
            )
            .await;

        let path = logger.file_path().expect("run log path");
        let entries = read_entries(path);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, "stream_error");
        assert_eq!(entries[0].run_id, "run-2");
    }

    #[test]
    fn run_log_entry_from_envelope_round_trips() {
        use crate::modules::runtime::contracts::common::{
            CorrelationIds, RuntimeEventEnvelope, RuntimeEventType,
        };

        let envelope = RuntimeEventEnvelope::new(
            RuntimeEventType::Conversation,
            "text_delta",
            CorrelationIds {
                session_id: Some("sess-1".into()),
                run_id: Some("run-1".into()),
                stream_id: Some("s1".into()),
                project_id: None,
                turn_index: Some(0),
                attempt_id: Some("att-3".into()),
            },
            serde_json::json!({ "text": "hello" }),
        );

        let entry = RunLogEntry::from_envelope(&envelope, 1, "fallback-sess", "fallback-run");

        assert_eq!(entry.session_id, "sess-1");
        assert_eq!(entry.run_id, "run-1");
        assert_eq!(entry.seq, 1);
        assert!(entry.event_type.contains("conversation"));
        assert!(entry.event_type.contains("text_delta"));
        assert_eq!(entry.attempt_id.as_deref(), Some("att-3"));
        assert_eq!(entry.correlation_id.as_deref(), Some("s1"));

        // Round-trip through JSON
        let json = serde_json::to_string(&entry).unwrap();
        let back: RunLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "sess-1");
        assert_eq!(back.run_id, "run-1");
        assert_eq!(back.seq, 1);
        assert_eq!(back.attempt_id.as_deref(), Some("att-3"));
    }

    #[test]
    fn correlation_run_id_is_set_for_run_scoped_events() {
        use crate::modules::runtime::contracts::common::{
            CorrelationIds, RuntimeEventEnvelope, RuntimeEventType,
        };

        // Run-scoped events: stream_complete, stream_error, run_started
        // must have correlation.run_id set.
        let run_scoped_families = ["stream_complete", "stream_error", "run_started"];

        for family in &run_scoped_families {
            let envelope = RuntimeEventEnvelope::new(
                RuntimeEventType::Conversation,
                *family,
                CorrelationIds {
                    session_id: Some("sess-1".into()),
                    run_id: Some("run-1".into()),
                    ..CorrelationIds::default()
                },
                serde_json::json!({}),
            );
            let entry = RunLogEntry::from_envelope(&envelope, 1, "fallback-sess", "fallback-run");
            assert_eq!(
                entry.run_id, "run-1",
                "run_scoped event '{family}' must have run_id set"
            );
        }
    }

    #[test]
    fn from_envelope_uses_fallback_when_correlation_missing() {
        use crate::modules::runtime::contracts::common::{
            CorrelationIds, RuntimeEventEnvelope, RuntimeEventType,
        };

        let envelope = RuntimeEventEnvelope::new(
            RuntimeEventType::Tool,
            "tool_call_update",
            CorrelationIds::default(), // no session_id / run_id
            serde_json::json!({ "tool": "read_file" }),
        );

        let entry = RunLogEntry::from_envelope(&envelope, 5, "fb-sess", "fb-run");
        assert_eq!(entry.session_id, "fb-sess");
        assert_eq!(entry.run_id, "fb-run");
        assert_eq!(entry.seq, 5);
    }
}
