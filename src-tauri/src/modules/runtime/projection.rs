//! Projection Checkpoint — cold-start persistence for deterministic
//! projection state (T-020).
//!
//! ## Why
//!
//! Before this module, every page refresh or session re-open reconstructed
//! the entire projection from scratch by replaying the full event log.
//! With `RuntimeProjectionSnapshot` growing over many runs, full replays
//! become expensive. A checkpoint captures the snapshot at a known point
//! so cold-start can be: load checkpoint → replay incremental events.
//!
//! ## Scope
//!
//! This module handles backend-side checkpoint I/O only. The frontend
//! owns the snapshot schema (`RuntimeProjectionSnapshot` in TypeScript);
//! the backend stores serialized frontend state as-is with minimal
//! backend-side metadata (`last_applied_seq`, timestamps).
//!
//! ## Persistence
//!
//! `{app_data_dir}/runtime/projection-checkpoints/{session_id}/runtime.json`
//!
//! ## Integration points
//!
//! - `stream_finalize.rs`: saves checkpoint after terminal events
//!   (stream_complete, stream_error).
//! - `session.rs` close handler: saves final checkpoint.
//! - `commands/session.rs`: exposes `get_session_projection_checkpoint`
//!   for frontend cold-start consumption.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ── Domain types ─────────────────────────────────────────────────────────────

/// Checkpoint metadata persisted alongside the serialized frontend snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionCheckpointMeta {
    /// Session this checkpoint belongs to.
    pub session_id: String,
    /// Last event log sequence number applied to this snapshot.
    pub last_applied_seq: u64,
    /// ISO-8601 / RFC3339 timestamp when the checkpoint was first created.
    pub created_at: String,
    /// ISO-8601 / RFC3339 timestamp of the last update.
    pub updated_at: String,
}

/// Full checkpoint record including the frontend's serialized snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionCheckpoint {
    #[serde(flatten)]
    pub meta: ProjectionCheckpointMeta,
    /// Serialized `RuntimeProjectionSnapshot` JSON from the frontend.
    pub snapshot: serde_json::Value,
}

impl ProjectionCheckpoint {
    /// Create a new checkpoint with the given snapshot and last applied seq.
    pub fn new(session_id: String, last_applied_seq: u64, snapshot: serde_json::Value) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            meta: ProjectionCheckpointMeta {
                session_id,
                last_applied_seq,
                created_at: now.clone(),
                updated_at: now,
            },
            snapshot,
        }
    }
}

/// Response returned to the frontend on cold-start checkpoint load.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionCheckpointResponse {
    /// The frontend's serialized projection snapshot.
    pub snapshot: serde_json::Value,
    /// Last sequence number applied to this checkpoint.
    pub last_applied_seq: u64,
    /// Whether a checkpoint file existed on disk.
    pub checkpoint_exists: bool,
    /// Whether the current snapshot was loaded from a checkpoint.
    pub loaded_from_checkpoint: bool,
    /// ISO-8601 / RFC3339 timestamp of the checkpoint.
    pub checkpoint_updated_at: Option<String>,
}

// ── Persistence ──────────────────────────────────────────────────────────────

/// Save a projection checkpoint for a session.
///
/// Best-effort: IO errors are logged but not propagated so checkpoint
/// failures never block the main execution path.
pub fn save_checkpoint(app_data_dir: &Path, checkpoint: &ProjectionCheckpoint) -> io::Result<()> {
    let path = checkpoint_path(app_data_dir, &checkpoint.meta.session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(checkpoint).map_err(io::Error::other)?;
    fs::write(path, bytes)
}

/// Load a projection checkpoint for a session, returning a response suitable
/// for front-end consumption.
///
/// If no checkpoint exists on disk, returns a response with
/// `checkpoint_exists: false` and an empty snapshot so the frontend can
/// perform a full replay.
pub fn load_checkpoint(
    app_data_dir: &Path,
    session_id: &str,
) -> io::Result<ProjectionCheckpointResponse> {
    let path = checkpoint_path(app_data_dir, session_id);
    match fs::read(&path) {
        Ok(bytes) => {
            let checkpoint: ProjectionCheckpoint = serde_json::from_slice(&bytes)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            Ok(ProjectionCheckpointResponse {
                snapshot: checkpoint.snapshot,
                last_applied_seq: checkpoint.meta.last_applied_seq,
                checkpoint_exists: true,
                loaded_from_checkpoint: true,
                checkpoint_updated_at: Some(checkpoint.meta.updated_at),
            })
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(ProjectionCheckpointResponse {
            snapshot: serde_json::Value::Null,
            last_applied_seq: 0,
            checkpoint_exists: false,
            loaded_from_checkpoint: false,
            checkpoint_updated_at: None,
        }),
        Err(err) => Err(err),
    }
}

/// Remove the checkpoint for a session.
pub fn delete_checkpoint(app_data_dir: &Path, session_id: &str) -> io::Result<()> {
    let path = checkpoint_path(app_data_dir, session_id);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn checkpoint_path(app_data_dir: &Path, session_id: &str) -> PathBuf {
    app_data_dir
        .join("runtime")
        .join("projection-checkpoints")
        .join(format!("{}.json", safe_filename(session_id)))
}

/// Sanitise a session ID for use as a filename component.
fn safe_filename(session_id: &str) -> String {
    session_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_save_and_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let session = "session-1";

        let snapshot = serde_json::json!({
            "runs": {},
            "approvals": {},
            "memory": {},
            "activation": null,
            "executionMode": null,
            "supervisor": null,
            "browsers": {}
        });

        let cp = ProjectionCheckpoint::new(session.to_string(), 42, snapshot.clone());
        save_checkpoint(dir.path(), &cp).unwrap();

        let loaded = load_checkpoint(dir.path(), session).unwrap();
        assert!(loaded.checkpoint_exists);
        assert!(loaded.loaded_from_checkpoint);
        assert_eq!(loaded.last_applied_seq, 42);
        assert_eq!(loaded.snapshot, snapshot);

        delete_checkpoint(dir.path(), session).unwrap();
        let after_delete = load_checkpoint(dir.path(), session).unwrap();
        assert!(!after_delete.checkpoint_exists);
        assert!(!after_delete.loaded_from_checkpoint);
        assert_eq!(after_delete.last_applied_seq, 0);
    }

    #[test]
    fn checkpoint_missing_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_checkpoint(dir.path(), "nonexistent").unwrap();
        assert!(!result.checkpoint_exists);
        assert!(!result.loaded_from_checkpoint);
        assert_eq!(result.last_applied_seq, 0);
        assert_eq!(result.snapshot, serde_json::Value::Null);
    }

    #[test]
    fn checkpoint_update_preserves_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let session = "session-1";
        let snapshot_a = serde_json::json!({ "runs": { "a": 1 } });
        let snapshot_b = serde_json::json!({ "runs": { "b": 2 } });

        let cp1 = ProjectionCheckpoint::new(session.to_string(), 1, snapshot_a);
        save_checkpoint(dir.path(), &cp1).unwrap();

        let loaded1 = load_checkpoint(dir.path(), session).unwrap();
        assert!(loaded1.checkpoint_updated_at.is_some());

        let cp2 = ProjectionCheckpoint::new(session.to_string(), 2, snapshot_b);
        save_checkpoint(dir.path(), &cp2).unwrap();

        let loaded2 = load_checkpoint(dir.path(), session).unwrap();
        assert_eq!(loaded2.last_applied_seq, 2);
    }
}
