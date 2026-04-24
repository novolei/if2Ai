//! Session Supervisor — canonical lifecycle state owner (MIG-020 / T-006).
//!
//! ## Why
//!
//! Before this module, active run / stop / retry / permission state was
//! scattered across `StreamTaskInputs`, `permission_senders` map,
//! pending-permission files, the event log, and the frontend Redux store.
//! Without a supervisor, there is no single backend snapshot that answers
//! "what is the session lifecycle state right now?"
//!
//! ## Scope
//!
//! State-truth only — no complex scheduling or multi-viewer coordination.
//! The supervisor snapshot is the canonical answer to:
//!
//! - Is a run active for this session?
//! - What is the run's terminal status?
//! - Is the session blocked on a permission prompt?
//! - Is it safe to retry after a failure?
//! - Is the session in a disconnect grace window?
//!
//! ## Persistence
//!
//! `{app_data_dir}/runtime/supervisor/{session_id}.json`
//!
//! ## Lifecycle hooks (called by TurnService)
//!
//! - `start_run()`    — idle → running, set active_run_id
//! - `block_permission()` — running → blocked, bump pending count
//! - `unblock_permission()` — blocked → running, decrement pending count
//! - `run_streaming()` — ensure running
//! - `run_completed()` — running → completed/idle
//! - `run_failed()`   — running → recoverable_failed (or idle)
//! - `close_session()` — any → closed

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ── Domain types ─────────────────────────────────────────────────────────────

/// Top-level supervisor lifecycle states.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisorStatus {
    /// No active run; the session is waiting for user input.
    Idle,
    /// A run is actively streaming.
    Running,
    /// The active run is blocked on a pending permission prompt.
    Blocked,
    /// The active run failed but may be recoverable (retry budget > 0,
    /// no mutating tool executed, etc.).
    RecoverableFailed,
    /// The active run completed successfully.
    Completed,
    /// The session has been closed (or the window disconnected).
    Closed,
}

/// Per-run status tracked inside the `SupervisorSnapshot`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Run is actively receiving tokens from the provider.
    Streaming,
    /// Run finished with `stream_complete`.
    Completed,
    /// Run terminated with `stream_error` or an unhandled fault.
    Failed,
    /// Run was explicitly cancelled by the user.
    Cancelled,
}

/// Canonical supervisor snapshot for one session.
///
/// This is the single backend-side answer to "what is the session lifecycle
/// state?"  It replaces ad-hoc scattered state reads and is persisted so
/// the frontend can recover it after a refresh.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorSnapshot {
    /// Session this snapshot belongs to.
    pub session_id: String,
    /// Top-level lifecycle status.
    pub status: SupervisorStatus,
    /// Currently active run ID (set on `start_run`, cleared on terminal).
    pub active_run_id: Option<String>,
    /// Status of the active run (set on terminal events).
    pub active_run_status: Option<RunStatus>,
    /// How many pending-permission prompts are blocking the active run.
    pub pending_permission_count: usize,
    /// The last non-recoverable error kind, if any.
    pub last_error_kind: Option<String>,
    /// Whether the session is in a recoverable state after a failure.
    pub recoverable: bool,
    /// Number of remaining retry attempts for the current run.
    pub retry_budget_remaining: u32,
    /// RFC 3339 timestamp — after this the session is considered
    /// disconnected (set on stream start + grace period; cleared on
    /// stream complete/error).
    pub disconnect_grace_until: Option<String>,
    /// RFC 3339 timestamp of the last mutation.
    pub last_updated_at: String,
}

impl SupervisorSnapshot {
    /// Create a fresh snapshot for a session with no active run.
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            status: SupervisorStatus::Idle,
            active_run_id: None,
            active_run_status: None,
            pending_permission_count: 0,
            last_error_kind: None,
            recoverable: false,
            retry_budget_remaining: 0,
            disconnect_grace_until: None,
            last_updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// ── Persistence ──────────────────────────────────────────────────────────────

/// Persist the supervisor snapshot for a session.
pub fn write_supervisor_snapshot(
    app_data_dir: &Path,
    snapshot: &SupervisorSnapshot,
) -> io::Result<()> {
    let path = supervisor_path(app_data_dir, &snapshot.session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(snapshot).map_err(io::Error::other)?;
    fs::write(path, bytes)
}

/// Read the supervisor snapshot for a session, if one exists.
pub fn read_supervisor_snapshot(
    app_data_dir: &Path,
    session_id: &str,
) -> io::Result<Option<SupervisorSnapshot>> {
    let path = supervisor_path(app_data_dir, session_id);
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Remove the supervisor snapshot for a session.
pub fn delete_supervisor_snapshot(app_data_dir: &Path, session_id: &str) -> io::Result<()> {
    let path = supervisor_path(app_data_dir, session_id);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn supervisor_path(app_data_dir: &Path, session_id: &str) -> PathBuf {
    app_data_dir
        .join("runtime")
        .join("supervisor")
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

// ── Lifecycle ────────────────────────────────────────────────────────────────

/// Convenience builder for in-process supervisor state mutations.
///
/// Callers (TurnService lifecycle hooks) construct a `SupervisorSnapshot`,
/// call the relevant transition method, then call [`write_supervisor_snapshot`]
/// to persist.
///
/// # State-machine invariants
///
/// - Only one active run at a time (`start_run` requires idle/closed).
/// - `block_permission` is only valid while running.
/// - Terminal states (completed/failed) set `active_run_id = None`.
pub struct SessionSupervisor;

impl SessionSupervisor {
    /// Load or create a supervisor snapshot for the given session.
    pub fn load_or_create(app_data_dir: &Path, session_id: &str) -> io::Result<SupervisorSnapshot> {
        match read_supervisor_snapshot(app_data_dir, session_id)? {
            Some(snapshot) => Ok(snapshot),
            None => Ok(SupervisorSnapshot::new(session_id.to_string())),
        }
    }

    /// Transition: idle → running.  Records the new active run ID.
    pub fn start_run(snapshot: &mut SupervisorSnapshot, run_id: String) {
        snapshot.status = SupervisorStatus::Running;
        snapshot.active_run_id = Some(run_id);
        snapshot.active_run_status = Some(RunStatus::Streaming);
        snapshot.last_error_kind = None;
        snapshot.recoverable = false;
        snapshot.retry_budget_remaining = 3;
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: running → blocked (permission prompt raised).
    pub fn block_permission(snapshot: &mut SupervisorSnapshot) {
        snapshot.status = SupervisorStatus::Blocked;
        snapshot.pending_permission_count = snapshot.pending_permission_count.saturating_add(1);
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: blocked → running (permission prompt resolved).
    pub fn unblock_permission(snapshot: &mut SupervisorSnapshot) {
        if snapshot.pending_permission_count > 0 {
            snapshot.pending_permission_count -= 1;
        }
        if snapshot.pending_permission_count == 0 {
            snapshot.status = SupervisorStatus::Running;
        }
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Ensure the snapshot reflects a streaming run.
    pub fn run_streaming(snapshot: &mut SupervisorSnapshot) {
        snapshot.status = SupervisorStatus::Running;
        if snapshot.active_run_status.is_none() {
            snapshot.active_run_status = Some(RunStatus::Streaming);
        }
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: running → completed (terminal success).
    pub fn run_completed(snapshot: &mut SupervisorSnapshot) {
        snapshot.status = SupervisorStatus::Completed;
        snapshot.active_run_id = None;
        snapshot.active_run_status = Some(RunStatus::Completed);
        snapshot.pending_permission_count = 0;
        snapshot.recoverable = false;
        snapshot.retry_budget_remaining = 0;
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: running → recoverable_failed (with retry budget remaining).
    pub fn run_failed_recoverable(snapshot: &mut SupervisorSnapshot, error_kind: String) {
        snapshot.status = SupervisorStatus::RecoverableFailed;
        snapshot.active_run_id = None;
        snapshot.active_run_status = Some(RunStatus::Failed);
        snapshot.last_error_kind = Some(error_kind);
        snapshot.recoverable = true;
        snapshot.retry_budget_remaining = snapshot.retry_budget_remaining.saturating_sub(1);
        if snapshot.retry_budget_remaining == 0 {
            snapshot.recoverable = false;
        }
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: running → idle (non-recoverable failure with no budget).
    pub fn run_failed_final(snapshot: &mut SupervisorSnapshot, error_kind: String) {
        snapshot.status = SupervisorStatus::Idle;
        snapshot.active_run_id = None;
        snapshot.active_run_status = Some(RunStatus::Failed);
        snapshot.last_error_kind = Some(error_kind);
        snapshot.recoverable = false;
        snapshot.retry_budget_remaining = 0;
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: running → idle (explicit user cancellation).
    pub fn run_cancelled(snapshot: &mut SupervisorSnapshot) {
        snapshot.status = SupervisorStatus::Idle;
        snapshot.active_run_id = None;
        snapshot.active_run_status = Some(RunStatus::Cancelled);
        snapshot.last_error_kind = None;
        snapshot.recoverable = false;
        snapshot.retry_budget_remaining = 0;
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Transition: any → closed (session terminated or disconnected).
    pub fn close_session(snapshot: &mut SupervisorSnapshot) {
        snapshot.status = SupervisorStatus::Closed;
        snapshot.active_run_id = None;
        snapshot.active_run_status = None;
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    // ── State machine ─────────────────────────────────────────────────────

    #[test]
    fn supervisor_status_transitions_are_valid() {
        let mut snap = SupervisorSnapshot::new("session-1".to_string());
        assert_eq!(snap.status, SupervisorStatus::Idle);
        assert!(snap.active_run_id.is_none());

        // idle → running
        SessionSupervisor::start_run(&mut snap, "run-1".to_string());
        assert_eq!(snap.status, SupervisorStatus::Running);
        assert_eq!(snap.active_run_id.as_deref(), Some("run-1"));
        assert_eq!(snap.active_run_status, Some(RunStatus::Streaming));

        // running → blocked
        SessionSupervisor::block_permission(&mut snap);
        assert_eq!(snap.status, SupervisorStatus::Blocked);
        assert_eq!(snap.pending_permission_count, 1);

        // blocked → running (unblock)
        SessionSupervisor::unblock_permission(&mut snap);
        assert_eq!(snap.status, SupervisorStatus::Running);
        assert_eq!(snap.pending_permission_count, 0);

        // running → completed
        SessionSupervisor::run_completed(&mut snap);
        assert_eq!(snap.status, SupervisorStatus::Completed);
        assert!(snap.active_run_id.is_none());
        assert_eq!(snap.active_run_status, Some(RunStatus::Completed));
        assert!(!snap.recoverable);
    }

    #[test]
    fn recoverable_to_final_failure() {
        let mut snap = SupervisorSnapshot::new("session-1".to_string());
        SessionSupervisor::start_run(&mut snap, "run-1".to_string());
        snap.retry_budget_remaining = 2;

        // First failure → recoverable
        SessionSupervisor::run_failed_recoverable(&mut snap, "timeout".to_string());
        assert_eq!(snap.status, SupervisorStatus::RecoverableFailed);
        assert_eq!(snap.retry_budget_remaining, 1);
        assert!(snap.recoverable);
        assert_eq!(snap.last_error_kind.as_deref(), Some("timeout"));

        // Second failure → still recoverable (budget = 0, but recoverable stays)
        SessionSupervisor::run_failed_recoverable(&mut snap, "timeout".to_string());
        assert_eq!(snap.retry_budget_remaining, 0);
        // After budget hits 0, recoverable is set false
        assert!(!snap.recoverable);

        // Final failure → idle
        SessionSupervisor::run_failed_final(&mut snap, "fatal".to_string());
        assert_eq!(snap.status, SupervisorStatus::Idle);
        assert!(!snap.recoverable);
    }

    #[test]
    fn supervisor_snapshot_persists_and_loads() {
        let dir = tempfile::tempdir().unwrap();
        let mut snap = SupervisorSnapshot::new("session-42".to_string());
        SessionSupervisor::start_run(&mut snap, "run-42".to_string());

        write_supervisor_snapshot(dir.path(), &snap).unwrap();
        let loaded = read_supervisor_snapshot(dir.path(), "session-42")
            .unwrap()
            .expect("snapshot should exist");
        assert_eq!(loaded.session_id, "session-42");
        assert_eq!(loaded.status, SupervisorStatus::Running);
        assert_eq!(loaded.active_run_id.as_deref(), Some("run-42"));
        assert_eq!(loaded.active_run_status, Some(RunStatus::Streaming));

        delete_supervisor_snapshot(dir.path(), "session-42").unwrap();
        assert!(read_supervisor_snapshot(dir.path(), "session-42")
            .unwrap()
            .is_none());
    }

    #[test]
    fn active_blocked_recoverable_distinguishable() {
        let dir = tempfile::tempdir().unwrap();
        let mut snap = SupervisorSnapshot::new("session-1".to_string());

        // Active
        SessionSupervisor::start_run(&mut snap, "run-a".to_string());
        write_supervisor_snapshot(dir.path(), &snap).unwrap();
        let active = read_supervisor_snapshot(dir.path(), "session-1")
            .unwrap()
            .unwrap();
        assert_eq!(active.status, SupervisorStatus::Running);
        assert_eq!(active.active_run_id.as_deref(), Some("run-a"));

        // Blocked
        SessionSupervisor::block_permission(&mut snap);
        write_supervisor_snapshot(dir.path(), &snap).unwrap();
        let blocked = read_supervisor_snapshot(dir.path(), "session-1")
            .unwrap()
            .unwrap();
        assert_eq!(blocked.status, SupervisorStatus::Blocked);
        assert_eq!(blocked.pending_permission_count, 1);

        // Recoverable failed
        SessionSupervisor::run_failed_recoverable(&mut snap, "timeout".to_string());
        write_supervisor_snapshot(dir.path(), &snap).unwrap();
        let recoverable = read_supervisor_snapshot(dir.path(), "session-1")
            .unwrap()
            .unwrap();
        assert_eq!(recoverable.status, SupervisorStatus::RecoverableFailed);
        assert!(recoverable.recoverable);
        assert_eq!(recoverable.last_error_kind.as_deref(), Some("timeout"));
    }

    #[test]
    fn multiple_permission_blocks() {
        let mut snap = SupervisorSnapshot::new("session-1".to_string());
        SessionSupervisor::start_run(&mut snap, "run-1".to_string());

        // Block twice
        SessionSupervisor::block_permission(&mut snap);
        SessionSupervisor::block_permission(&mut snap);
        assert_eq!(snap.pending_permission_count, 2);
        assert_eq!(snap.status, SupervisorStatus::Blocked);

        // Unblock once — still blocked
        SessionSupervisor::unblock_permission(&mut snap);
        assert_eq!(snap.pending_permission_count, 1);
        assert_eq!(snap.status, SupervisorStatus::Blocked);

        // Unblock again — back to running
        SessionSupervisor::unblock_permission(&mut snap);
        assert_eq!(snap.pending_permission_count, 0);
        assert_eq!(snap.status, SupervisorStatus::Running);
    }
}
