//! Tool Attempt Ledger — canonical attempt fact store (MIG-022 / T-013).
//!
//! ## Why
//!
//! Before this module, tool attempts were tracked only through ephemeral
//! `tool_call_id` + `attempt_id` UUIDs in stream payloads. There was no
//! canonical record of how many times a tool call had been attempted, what
//! status each attempt reached, or the retry history. This made it impossible
//! to answer questions like "what was the 3rd attempt's result?" or "why did
//! this tool call need 4 retries?"
//!
//! ## Scope
//!
//! The attempt ledger is a session-scoped, append-only fact store. Each
//! tool invocation attempt is recorded as a `ToolAttempt` entry with:
//!
//! - A stable `tool_call_id` (model-assigned, reused across retries)
//! - A monotonic `attempt_no` (1-based, per `tool_call_id`)
//! - A unique `attempt_id` (UUID per attempt)
//! - An 8-state `ToolAttemptStatus` lifecycle
//! - Timing, policy decision, and failure classification
//!
//! ## Persistence
//!
//! `{app_data_dir}/runtime/attempt-ledger/{session_id}.json`
//!
//! The file is a JSON array of `ToolAttempt` records, appended to on each
//! state transition. It is read and rewritten on each write (simple, works
//! for the expected record count per session).
//!
//! ## Integration points
//!
//! - `stream_tool_execution.rs`: calls `next_attempt_no()` before each tool
//!   call, then `record_attempt()` with the result.
//! - `commands/attempt_ledger.rs`: exposes query commands to the frontend.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ── Domain types ─────────────────────────────────────────────────────────────

/// Tool attempt lifecycle statuses.
///
/// The state machine: queued → authorizing → running → completed / failed / cancelled / blocked.
/// `retrying` is entered when a previous attempt failed and a retry is being initiated.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAttemptStatus {
    /// Enqueued for execution but not yet picked up by the executor.
    Queued,
    /// Permission check in progress (interactive prompt or policy evaluation).
    Authorizing,
    /// Tool is actively executing.
    Running,
    /// A previous attempt failed and a new attempt is being prepared.
    Retrying,
    /// Tool executed successfully (returned a result, even if the result indicates a tool error).
    Completed,
    /// Tool execution failed unrecoverably (process crash, sandbox error, etc.).
    Failed,
    /// Tool execution was cancelled by the user.
    Cancelled,
    /// Tool execution is blocked by a permission gate or policy.
    Blocked,
}

/// A single tool invocation attempt record.
///
/// Persisted to the session-level attempt ledger for audit,
/// retry analysis, and timeline display.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAttempt {
    /// Session this attempt belongs to.
    pub session_id: String,
    /// Run this attempt occurred in.
    pub run_id: String,
    /// Model-assigned tool call ID (reused across retries).
    pub tool_call_id: String,
    /// Unique UUID for this specific attempt.
    pub attempt_id: String,
    /// Monotonic counter per `tool_call_id` (1-based).
    pub attempt_no: u32,
    /// Tool name.
    pub tool_name: String,
    /// Current status in the lifecycle.
    pub status: ToolAttemptStatus,
    /// ISO-8601 / RFC3339 timestamp when the attempt was created.
    pub started_at: String,
    /// ISO-8601 / RFC3339 timestamp when the attempt reached a terminal status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Policy decision string (e.g. "allow", "deny", "session_allow").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_decision: Option<String>,
    /// Classification of the failure (e.g. "timeout", "invalid_args", "permission_denied").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    /// Execution duration in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

// ── Query API ────────────────────────────────────────────────────────────────

/// In-memory ledger for querying tool attempts within a session.
///
/// Not a long-lived cache — reconstructed from disk on each query.
pub struct ToolAttemptLedger;

impl ToolAttemptLedger {
    /// Load all attempts for a session from disk.
    fn load_all(app_data_dir: &Path, session_id: &str) -> io::Result<Vec<ToolAttempt>> {
        let path = attempt_ledger_path(app_data_dir, session_id);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(err) => Err(err),
        }
    }

    /// Query all attempts for a specific run.
    pub fn by_run_id(
        app_data_dir: &Path,
        session_id: &str,
        run_id: &str,
    ) -> io::Result<Vec<ToolAttempt>> {
        Ok(Self::load_all(app_data_dir, session_id)?
            .into_iter()
            .filter(|a| a.run_id == run_id)
            .collect())
    }

    /// Query all attempts for a specific tool call within a session.
    pub fn by_tool_call_id(
        app_data_dir: &Path,
        session_id: &str,
        tool_call_id: &str,
    ) -> io::Result<Vec<ToolAttempt>> {
        Ok(Self::load_all(app_data_dir, session_id)?
            .into_iter()
            .filter(|a| a.tool_call_id == tool_call_id)
            .collect())
    }

    /// Query all attempts for a session.
    pub fn by_session_id(app_data_dir: &Path, session_id: &str) -> io::Result<Vec<ToolAttempt>> {
        Self::load_all(app_data_dir, session_id)
    }

    /// Compute the next `attempt_no` for a given `tool_call_id`.
    ///
    /// Returns 1 if this is the first attempt, otherwise returns the
    /// previous highest `attempt_no` + 1.
    pub fn next_attempt_no(
        app_data_dir: &Path,
        session_id: &str,
        tool_call_id: &str,
    ) -> io::Result<u32> {
        let attempts = Self::load_all(app_data_dir, session_id)?;
        let max_no = attempts
            .iter()
            .filter(|a| a.tool_call_id == tool_call_id)
            .map(|a| a.attempt_no)
            .max()
            .unwrap_or(0);
        Ok(max_no + 1)
    }
}

// ── Persistence ──────────────────────────────────────────────────────────────

/// Save (append or update) an attempt record to the session's ledger.
///
/// If an attempt with the same `attempt_id` already exists, it is replaced
/// (state transition update). Otherwise, the record is appended.
pub fn record_attempt(app_data_dir: &Path, attempt: &ToolAttempt) -> io::Result<()> {
    let mut records = ToolAttemptLedger::load_all(app_data_dir, &attempt.session_id)?;

    // Replace existing entry with same attempt_id, or append new.
    if let Some(existing) = records
        .iter_mut()
        .find(|a| a.attempt_id == attempt.attempt_id)
    {
        *existing = attempt.clone();
    } else {
        records.push(attempt.clone());
    }

    save_attempts(app_data_dir, &attempt.session_id, &records)
}

/// Batch-save all records for a session (used in tests and migrations).
fn save_attempts(app_data_dir: &Path, session_id: &str, records: &[ToolAttempt]) -> io::Result<()> {
    let path = attempt_ledger_path(app_data_dir, session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(records).map_err(io::Error::other)?;
    fs::write(path, bytes)
}

/// Remove the attempt ledger for a session.
pub fn delete_attempt_ledger(app_data_dir: &Path, session_id: &str) -> io::Result<()> {
    let path = attempt_ledger_path(app_data_dir, session_id);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn attempt_ledger_path(app_data_dir: &Path, session_id: &str) -> PathBuf {
    app_data_dir
        .join("runtime")
        .join("attempt-ledger")
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

// ── Convenience builders ─────────────────────────────────────────────────────

impl ToolAttempt {
    /// Create a new attempt entry with status `queued`.
    pub fn new(
        session_id: String,
        run_id: String,
        tool_call_id: String,
        attempt_id: String,
        attempt_no: u32,
        tool_name: String,
    ) -> Self {
        Self {
            session_id,
            run_id,
            tool_call_id,
            attempt_id,
            attempt_no,
            tool_name,
            status: ToolAttemptStatus::Queued,
            started_at: chrono::Utc::now().to_rfc3339(),
            ended_at: None,
            policy_decision: None,
            failure_kind: None,
            duration_ms: None,
        }
    }

    /// Transition to `authorizing` status.
    pub fn transition_authorizing(&mut self) {
        self.status = ToolAttemptStatus::Authorizing;
    }

    /// Transition to `running` status.
    pub fn transition_running(&mut self) {
        self.status = ToolAttemptStatus::Running;
    }

    /// Transition to `completed` status.
    pub fn transition_completed(&mut self, duration_ms: u64) {
        self.status = ToolAttemptStatus::Completed;
        self.ended_at = Some(chrono::Utc::now().to_rfc3339());
        self.duration_ms = Some(duration_ms);
    }

    /// Transition to `failed` status.
    pub fn transition_failed(&mut self, failure_kind: String, duration_ms: u64) {
        self.status = ToolAttemptStatus::Failed;
        self.ended_at = Some(chrono::Utc::now().to_rfc3339());
        self.failure_kind = Some(failure_kind);
        self.duration_ms = Some(duration_ms);
    }

    /// Transition to `blocked` status.
    pub fn transition_blocked(&mut self) {
        self.status = ToolAttemptStatus::Blocked;
        self.ended_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Transition to `cancelled` status.
    pub fn transition_cancelled(&mut self) {
        self.status = ToolAttemptStatus::Cancelled;
        self.ended_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

// ── Query response types (used in Tauri commands) ────────────────────────────

/// Response for a tool attempt ledger query.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAttemptLedgerResponse {
    pub session_id: String,
    pub attempts: Vec<ToolAttempt>,
    /// Attempt count statistics grouped by tool_call_id.
    pub stats: HashMap<String, AttemptStats>,
}

/// Per-tool_call_id summary statistics.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptStats {
    pub tool_name: String,
    pub total_attempts: u32,
    pub succeeded: bool,
    pub last_status: String,
    pub last_failure_kind: Option<String>,
}

impl ToolAttemptLedgerResponse {
    /// Build a response from a list of attempts, computing stats.
    pub fn from_attempts(session_id: String, attempts: Vec<ToolAttempt>) -> Self {
        let mut stats: HashMap<String, AttemptStats> = HashMap::new();
        for a in &attempts {
            let entry = stats
                .entry(a.tool_call_id.clone())
                .or_insert_with(|| AttemptStats {
                    tool_name: a.tool_name.clone(),
                    total_attempts: 0,
                    succeeded: false,
                    last_status: a.status_to_str(),
                    last_failure_kind: None,
                });
            entry.total_attempts += 1;
            entry.last_status = a.status_to_str();
            if a.status == ToolAttemptStatus::Completed {
                entry.succeeded = true;
            }
            if a.status == ToolAttemptStatus::Failed {
                entry.last_failure_kind = a.failure_kind.clone();
            }
        }
        Self {
            session_id,
            attempts,
            stats,
        }
    }
}

impl ToolAttempt {
    fn status_to_str(&self) -> String {
        match self.status {
            ToolAttemptStatus::Queued => "queued",
            ToolAttemptStatus::Authorizing => "authorizing",
            ToolAttemptStatus::Running => "running",
            ToolAttemptStatus::Retrying => "retrying",
            ToolAttemptStatus::Completed => "completed",
            ToolAttemptStatus::Failed => "failed",
            ToolAttemptStatus::Cancelled => "cancelled",
            ToolAttemptStatus::Blocked => "blocked",
        }
        .to_string()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn new_attempt(
        session_id: &str,
        run_id: &str,
        tool_call_id: &str,
        attempt_no: u32,
        tool_name: &str,
    ) -> ToolAttempt {
        ToolAttempt::new(
            session_id.to_string(),
            run_id.to_string(),
            tool_call_id.to_string(),
            format!("att-{tool_call_id}-{attempt_no}"),
            attempt_no,
            tool_name.to_string(),
        )
    }

    #[test]
    fn attempt_no_monotonic_per_tool_call() {
        let dir = tempfile::tempdir().unwrap();
        let session = "session-1";
        let tool_id = "toolu_001";

        // First attempt
        record_attempt(
            dir.path(),
            &new_attempt(session, "run-1", tool_id, 1, "read_file"),
        )
        .unwrap();

        let next = ToolAttemptLedger::next_attempt_no(dir.path(), session, tool_id).unwrap();
        assert_eq!(next, 2);

        // Second attempt (retry in another run)
        record_attempt(
            dir.path(),
            &new_attempt(session, "run-2", tool_id, 2, "read_file"),
        )
        .unwrap();

        let next = ToolAttemptLedger::next_attempt_no(dir.path(), session, tool_id).unwrap();
        assert_eq!(next, 3);

        delete_attempt_ledger(dir.path(), session).unwrap();
    }

    #[test]
    fn attempt_status_machine_complete() {
        let mut a = new_attempt("session-1", "run-1", "toolu_001", 1, "read_file");
        assert_eq!(a.status, ToolAttemptStatus::Queued);

        // queued → authorizing
        a.transition_authorizing();
        assert_eq!(a.status, ToolAttemptStatus::Authorizing);

        // authorizing → running
        a.transition_running();
        assert_eq!(a.status, ToolAttemptStatus::Running);

        // running → completed
        a.transition_completed(42);
        assert_eq!(a.status, ToolAttemptStatus::Completed);
        assert!(a.ended_at.is_some());
        assert_eq!(a.duration_ms, Some(42));
    }

    #[test]
    fn attempt_failure_transitions() {
        let mut a = new_attempt("session-1", "run-1", "toolu_001", 1, "read_file");

        a.transition_running();
        a.transition_failed("timeout".to_string(), 5000);
        assert_eq!(a.status, ToolAttemptStatus::Failed);
        assert_eq!(a.failure_kind.as_deref(), Some("timeout"));
        assert_eq!(a.duration_ms, Some(5000));
    }

    #[test]
    fn attempt_blocked_and_cancelled() {
        let mut a = new_attempt("session-1", "run-1", "toolu_001", 1, "write_file");

        a.transition_authorizing();
        a.transition_blocked();
        assert_eq!(a.status, ToolAttemptStatus::Blocked);

        let mut b = new_attempt("session-1", "run-2", "toolu_002", 1, "read_file");
        b.transition_cancelled();
        assert_eq!(b.status, ToolAttemptStatus::Cancelled);
    }

    #[test]
    fn ledger_persists_and_queries() {
        let dir = tempfile::tempdir().unwrap();
        let session = "session-42";

        let a1 = new_attempt(session, "run-1", "toolu_001", 1, "read_file");
        let mut a2 = new_attempt(session, "run-1", "toolu_002", 1, "write_file");
        a2.transition_running();
        a2.transition_completed(30);

        record_attempt(dir.path(), &a1).unwrap();
        record_attempt(dir.path(), &a2).unwrap();

        // Query by run_id
        let by_run = ToolAttemptLedger::by_run_id(dir.path(), session, "run-1").unwrap();
        assert_eq!(by_run.len(), 2);

        // Query by tool_call_id
        let by_tool = ToolAttemptLedger::by_tool_call_id(dir.path(), session, "toolu_001").unwrap();
        assert_eq!(by_tool.len(), 1);
        assert_eq!(by_tool[0].tool_name, "read_file");

        // Query by session_id
        let all = ToolAttemptLedger::by_session_id(dir.path(), session).unwrap();
        assert_eq!(all.len(), 2);

        // Verify state update replaces in-place
        let mut a1_updated = a1.clone();
        a1_updated.transition_running();
        a1_updated.transition_completed(50);
        record_attempt(dir.path(), &a1_updated).unwrap();

        let all_after = ToolAttemptLedger::by_session_id(dir.path(), session).unwrap();
        assert_eq!(all_after.len(), 2); // Still 2, replaced not appended
        let reloaded_a1 = all_after
            .iter()
            .find(|a| a.attempt_id == a1.attempt_id)
            .unwrap();
        assert_eq!(reloaded_a1.status, ToolAttemptStatus::Completed);

        delete_attempt_ledger(dir.path(), session).unwrap();
        assert!(ToolAttemptLedger::by_session_id(dir.path(), session)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn next_attempt_no_new_tool_call_starts_at_1() {
        let dir = tempfile::tempdir().unwrap();
        let session = "session-1";

        let next = ToolAttemptLedger::next_attempt_no(dir.path(), session, "unknown_tool").unwrap();
        assert_eq!(next, 1);
    }

    #[test]
    fn multiple_tool_calls_independent_counters() {
        let dir = tempfile::tempdir().unwrap();
        let session = "session-1";

        record_attempt(
            dir.path(),
            &new_attempt(session, "run-1", "toolu_001", 1, "read_file"),
        )
        .unwrap();
        record_attempt(
            dir.path(),
            &new_attempt(session, "run-1", "toolu_001", 2, "read_file"),
        )
        .unwrap();
        record_attempt(
            dir.path(),
            &new_attempt(session, "run-1", "toolu_002", 1, "write_file"),
        )
        .unwrap();

        assert_eq!(
            ToolAttemptLedger::next_attempt_no(dir.path(), session, "toolu_001").unwrap(),
            3
        );
        assert_eq!(
            ToolAttemptLedger::next_attempt_no(dir.path(), session, "toolu_002").unwrap(),
            2
        );

        delete_attempt_ledger(dir.path(), session).unwrap();
    }

    #[test]
    fn response_stats_are_correct() {
        let mut a1 = new_attempt("session-1", "run-1", "toolu_001", 1, "read_file");
        a1.transition_running();
        a1.transition_completed(30);

        let mut a2 = new_attempt("session-1", "run-2", "toolu_001", 2, "read_file");
        a2.transition_running();
        a2.transition_failed("timeout".to_string(), 5000);

        let response =
            ToolAttemptLedgerResponse::from_attempts("session-1".to_string(), vec![a1, a2]);
        assert_eq!(response.stats.len(), 1);

        let stats = response.stats.get("toolu_001").unwrap();
        assert_eq!(stats.total_attempts, 2);
        assert!(stats.succeeded); // First attempt succeeded
        assert_eq!(stats.last_status, "failed");
        assert_eq!(stats.last_failure_kind.as_deref(), Some("timeout"));
    }
}
