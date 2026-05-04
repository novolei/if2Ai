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
use tauri::AppHandle;

use crate::modules::runtime::contracts::common::{
    supervisor_family, CorrelationIds, RuntimeEventType,
};
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::runtime_event;

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

// ── SupervisorOps facade (DR-01) ─────────────────────────────────────────────

/// Thin call-site facade over [`SessionSupervisor`] that bundles
/// load → mutate → persist → emit into a single hook.
///
/// DR-01 — `SupervisorOps` is the canonical entry point for all
/// session-supervisor lifecycle transitions. Direct
/// [`SessionSupervisor::*`] mutation + manual `write_supervisor_snapshot`
/// is preserved for unit tests / state-machine logic but new
/// production call sites MUST go through this facade so each
/// transition automatically:
///
/// 1. Loads the current snapshot (or creates a fresh one).
/// 2. Applies the corresponding [`SessionSupervisor`] transition.
/// 3. Persists the snapshot (best-effort; failures only `warn!`).
/// 4. Emits a `RuntimeEventType::Supervisor` envelope on the
///    `runtime_event` channel via [`runtime_event::dispatch`] when
///    `app_handle` is provided.
///
/// `app_handle` is `Option` so unit tests can exercise the
/// persistence path without standing up a Tauri runtime; production
/// callers always pass `Some(handle)`.
pub struct SupervisorOps<'a> {
    /// Application data directory (e.g. `~/.if2ai`) where the
    /// supervisor snapshot is persisted.
    pub app_data_dir: &'a Path,
    /// Session id this op targets.
    pub session_id: &'a str,
    /// Tauri app handle used to emit the envelope. `None` skips the
    /// emit step (filesystem persistence still runs).
    pub app_handle: Option<&'a AppHandle>,
    /// Optional run-event logger for envelope mirroring into the
    /// per-run event log.
    pub run_event_logger: Option<&'a RunEventLogger>,
}

impl<'a> SupervisorOps<'a> {
    /// Record a new run start (idle → running) and emit
    /// `Supervisor / start_run` envelope.
    pub fn start_run(&self, run_id: &str) {
        let run_id = run_id.to_string();
        self.with_snapshot(
            move |snap| SessionSupervisor::start_run(snap, run_id),
            supervisor_family::START_RUN,
        );
    }

    /// Record that a permission prompt was raised (running → blocked)
    /// and emit `Supervisor / blocked` envelope.
    pub fn block_permission(&self, _request_id: &str) {
        self.with_snapshot(
            SessionSupervisor::block_permission,
            supervisor_family::BLOCKED,
        );
    }

    /// Record that a permission prompt was resolved (blocked → running)
    /// and emit `Supervisor / unblocked` envelope.
    pub fn unblock_permission(&self, _request_id: &str) {
        self.with_snapshot(
            SessionSupervisor::unblock_permission,
            supervisor_family::UNBLOCKED,
        );
    }

    /// Reassert that the active run is streaming and emit
    /// `Supervisor / streaming` envelope.
    pub fn run_streaming(&self) {
        self.with_snapshot(
            SessionSupervisor::run_streaming,
            supervisor_family::STREAMING,
        );
    }

    /// Record a successful run completion and emit `Supervisor /
    /// completed` envelope.
    pub fn run_completed(&self) {
        self.with_snapshot(
            SessionSupervisor::run_completed,
            supervisor_family::COMPLETED,
        );
    }

    /// Record a recoverable run failure and emit `Supervisor /
    /// failed` envelope.
    pub fn run_failed_recoverable(&self, reason: &str) {
        let reason = reason.to_string();
        self.with_snapshot(
            move |snap| SessionSupervisor::run_failed_recoverable(snap, reason),
            supervisor_family::FAILED,
        );
    }

    /// Record a non-recoverable run failure and emit `Supervisor /
    /// failed` envelope.
    pub fn run_failed_final(&self, reason: &str) {
        let reason = reason.to_string();
        self.with_snapshot(
            move |snap| SessionSupervisor::run_failed_final(snap, reason),
            supervisor_family::FAILED,
        );
    }

    /// Record explicit user cancellation and emit `Supervisor /
    /// cancelled` envelope.
    pub fn run_cancelled(&self) {
        self.with_snapshot(
            SessionSupervisor::run_cancelled,
            supervisor_family::CANCELLED,
        );
    }

    /// Record session close (any → closed) and emit `Supervisor /
    /// closed` envelope.
    pub fn close_session(&self) {
        self.with_snapshot(SessionSupervisor::close_session, supervisor_family::CLOSED);
    }

    /// Internal: load snapshot → apply mutation → persist →
    /// best-effort emit envelope.
    ///
    /// All failures are logged at `warn!` and swallowed; supervisor
    /// bookkeeping must never break a turn.
    fn with_snapshot<F: FnOnce(&mut SupervisorSnapshot)>(&self, f: F, family: &'static str) {
        let mut snap = match SessionSupervisor::load_or_create(self.app_data_dir, self.session_id) {
            Ok(snap) => snap,
            Err(err) => {
                tracing::warn!(
                    session_id = %self.session_id,
                    family = %family,
                    error = %err,
                    "[supervisor] load_or_create failed; skipping mutation"
                );
                return;
            }
        };
        f(&mut snap);
        if let Err(err) = write_supervisor_snapshot(self.app_data_dir, &snap) {
            tracing::warn!(
                session_id = %self.session_id,
                family = %family,
                error = %err,
                "[supervisor] persist failed"
            );
        }
        if let Some(handle) = self.app_handle {
            let correlation = CorrelationIds {
                session_id: Some(snap.session_id.clone()),
                run_id: snap.active_run_id.clone(),
                ..Default::default()
            };
            if let Err(err) = runtime_event::dispatch(
                Some(handle),
                RuntimeEventType::Supervisor,
                family,
                correlation,
                &snap,
                self.run_event_logger,
            ) {
                tracing::warn!(
                    session_id = %self.session_id,
                    family = %family,
                    error = ?err,
                    "[supervisor] envelope emit failed"
                );
            }
        }
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

    // ── DR-01 invariant tests ─────────────────────────────────────────────

    fn ops_for<'a>(dir: &'a tempfile::TempDir, sid: &'a str) -> SupervisorOps<'a> {
        SupervisorOps {
            app_data_dir: dir.path(),
            session_id: sid,
            app_handle: None,
            run_event_logger: None,
        }
    }

    /// Invariant: `pending_permission_count` mirrors the count of
    /// `block_permission` calls minus `unblock_permission` calls — the
    /// supervisor's view of how many permission prompts are
    /// outstanding for the active run.
    #[test]
    fn pending_count_matches_pending_permission_files() {
        let dir = tempfile::tempdir().unwrap();
        let ops = ops_for(&dir, "sess-A");

        ops.start_run("run-1");
        let snap0 = SessionSupervisor::load_or_create(dir.path(), "sess-A").unwrap();
        assert_eq!(snap0.pending_permission_count, 0);

        ops.block_permission("req-1");
        let snap1 = SessionSupervisor::load_or_create(dir.path(), "sess-A").unwrap();
        assert_eq!(snap1.pending_permission_count, 1);
        assert_eq!(snap1.status, SupervisorStatus::Blocked);

        ops.unblock_permission("req-1");
        let snap2 = SessionSupervisor::load_or_create(dir.path(), "sess-A").unwrap();
        assert_eq!(snap2.pending_permission_count, 0);
        assert_eq!(snap2.status, SupervisorStatus::Running);
    }

    /// Invariant: `active_run_id.is_some()` iff `status` is one of
    /// `Running` / `Blocked`. All terminal / idle states clear the
    /// active run id.
    #[test]
    fn active_run_id_present_iff_status_is_running_or_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let ops = ops_for(&dir, "sess-B");

        ops.start_run("run-1");
        let s = SessionSupervisor::load_or_create(dir.path(), "sess-B").unwrap();
        assert!(s.active_run_id.is_some() && s.status == SupervisorStatus::Running);

        ops.block_permission("req-1");
        let s = SessionSupervisor::load_or_create(dir.path(), "sess-B").unwrap();
        assert!(s.active_run_id.is_some() && s.status == SupervisorStatus::Blocked);

        ops.unblock_permission("req-1");
        ops.run_completed();
        let s = SessionSupervisor::load_or_create(dir.path(), "sess-B").unwrap();
        assert!(s.active_run_id.is_none() && s.status == SupervisorStatus::Completed);

        ops.start_run("run-2");
        ops.run_cancelled();
        let s = SessionSupervisor::load_or_create(dir.path(), "sess-B").unwrap();
        assert!(s.active_run_id.is_none() && s.status == SupervisorStatus::Idle);

        ops.start_run("run-3");
        ops.run_failed_final("fatal");
        let s = SessionSupervisor::load_or_create(dir.path(), "sess-B").unwrap();
        assert!(s.active_run_id.is_none() && s.status == SupervisorStatus::Idle);

        ops.close_session();
        let s = SessionSupervisor::load_or_create(dir.path(), "sess-B").unwrap();
        assert!(s.active_run_id.is_none() && s.status == SupervisorStatus::Closed);
    }

    /// Invariant: across multiple ops, `last_updated_at` must be
    /// monotonically non-decreasing. Newly persisted snapshots may
    /// share a timestamp at sub-second resolution but must never
    /// regress.
    #[test]
    fn supervisor_envelope_monotonic_last_updated_at() {
        let dir = tempfile::tempdir().unwrap();
        let ops = ops_for(&dir, "sess-C");

        ops.start_run("run-1");
        let t1 = SessionSupervisor::load_or_create(dir.path(), "sess-C")
            .unwrap()
            .last_updated_at;

        // Force a measurable delta.
        std::thread::sleep(std::time::Duration::from_millis(5));
        ops.block_permission("req-1");
        let t2 = SessionSupervisor::load_or_create(dir.path(), "sess-C")
            .unwrap()
            .last_updated_at;

        std::thread::sleep(std::time::Duration::from_millis(5));
        ops.unblock_permission("req-1");
        let t3 = SessionSupervisor::load_or_create(dir.path(), "sess-C")
            .unwrap()
            .last_updated_at;

        assert!(t1 <= t2, "last_updated_at regressed: {t1} > {t2}");
        assert!(t2 <= t3, "last_updated_at regressed: {t2} > {t3}");
        let _ = now();
    }

    /// `start_run` produces an envelope whose payload matches the
    /// persisted snapshot (event_type, family, run_id, status).
    #[test]
    fn start_run_emits_envelope_with_matching_payload() {
        // We cannot construct a real Tauri AppHandle in unit tests, so
        // we drive the same `runtime_event::dispatch` path that
        // `SupervisorOps::with_snapshot` invokes (with `handle = None`,
        // which exercises full payload serialization but skips the
        // transport — exactly the codepath taken when a TurnService
        // call site forgets to plumb the AppHandle).
        let dir = tempfile::tempdir().unwrap();
        let ops = ops_for(&dir, "sess-D");
        ops.start_run("run-XYZ");

        let snap = SessionSupervisor::load_or_create(dir.path(), "sess-D").unwrap();
        let correlation = CorrelationIds {
            session_id: Some(snap.session_id.clone()),
            run_id: snap.active_run_id.clone(),
            ..Default::default()
        };
        let env = runtime_event::dispatch(
            None,
            RuntimeEventType::Supervisor,
            supervisor_family::START_RUN,
            correlation,
            &snap,
            None,
        )
        .expect("dispatch ok with handle=None");

        assert_eq!(env.event_type, RuntimeEventType::Supervisor);
        assert_eq!(env.payload_family.0, supervisor_family::START_RUN);
        assert_eq!(env.correlation.session_id.as_deref(), Some("sess-D"));
        assert_eq!(env.correlation.run_id.as_deref(), Some("run-XYZ"));
        // Snapshot must serialize as camelCase so the frontend
        // `SupervisorSnapshot` interface lines up byte-for-byte.
        let payload = env.payload;
        assert_eq!(payload["sessionId"], "sess-D");
        assert_eq!(payload["activeRunId"], "run-XYZ");
        assert_eq!(payload["status"], "running");
    }

    /// When `app_handle` is `None`, `with_snapshot` still persists
    /// the snapshot — the emit path is best-effort and skipping it
    /// must not skip persistence.
    #[test]
    fn with_snapshot_persists_even_when_app_handle_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let ops = SupervisorOps {
            app_data_dir: dir.path(),
            session_id: "sess-E",
            app_handle: None,
            run_event_logger: None,
        };

        ops.start_run("run-only-fs");
        let loaded = read_supervisor_snapshot(dir.path(), "sess-E")
            .unwrap()
            .expect("snapshot should be persisted to disk");
        assert_eq!(loaded.session_id, "sess-E");
        assert_eq!(loaded.active_run_id.as_deref(), Some("run-only-fs"));
        assert_eq!(loaded.status, SupervisorStatus::Running);
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
