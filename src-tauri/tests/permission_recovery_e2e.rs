//! DR-02 — End-to-end integration tests for the permission-prompt
//! cross-reload recovery path.
//!
//! Goal: prove that a permission prompt persisted by
//! [`if2ai_backend::modules::runtime::pending_permission::write_pending_permission`]
//! survives process boundaries and can be resolved cleanly by the
//! IPC response handler. The unit tests in `pending_permission.rs`
//! cover the in-process round-trip; this file covers the
//! cross-process semantics: the on-disk file format, multi-session
//! independence, corrupted-file recovery, and the
//! write → simulated-reload → resolve → cleanup sequence.
//!
//! Each `#[test]` runs in its own process (cargo's default), so a
//! single test that does `write` then `read` already simulates the
//! "app reloads, reopens the file" semantics — the in-process state
//! between calls is just the on-disk state.

use std::path::Path;

use if2ai_backend::modules::runtime::pending_permission::{
    clear_pending_permission, read_pending_permission, write_pending_permission,
    PendingPermissionRecord,
};
use tempfile::TempDir;

fn sample_record(session_id: &str, request_id: &str) -> PendingPermissionRecord {
    PendingPermissionRecord {
        request_id: request_id.to_string(),
        session_id: session_id.to_string(),
        tool_name: "bash".to_string(),
        permission_mode: "dangerFullAccess".to_string(),
        current_mode: "readOnly".to_string(),
        message: format!("Tool 'bash' requires dangerFullAccess (session {session_id})"),
        requested_at: "2026-05-09T12:00:00Z".to_string(),
    }
}

fn pending_path(app_data_dir: &Path, sanitized_session_id: &str) -> std::path::PathBuf {
    app_data_dir
        .join("runtime")
        .join("pending-permissions")
        .join(format!("{sanitized_session_id}.json"))
}

// ────────── Cross-reload round-trip ──────────

/// The canonical reload-recovery flow. Writing persists the record
/// to disk; the read by a "fresh" invocation must return the same
/// record. This is exactly what happens when the frontend webview
/// reloads or the backend is restarted mid-prompt: the in-process
/// channel is gone but the on-disk record is rehydratable.
#[test]
fn cross_reload_record_survives_process_boundary() {
    let dir = TempDir::new().expect("tempdir");
    let record = sample_record("sess-cross-reload", "req-1");

    // Phase 1: simulate a prompt being raised + persisted.
    write_pending_permission(dir.path(), &record).expect("write");

    // Phase 2: simulate the rehydrating frontend reading after reload.
    // Important: we deliberately don't keep any in-process handle to
    // the record between Phase 1 and Phase 2 — the disk is the only
    // source of truth.
    let recovered = read_pending_permission(dir.path(), &record.session_id)
        .expect("read")
        .expect("record should exist after reload");
    assert_eq!(recovered, record);
}

/// After the frontend posts a decision, the IPC response handler
/// (in `permission_service::respond_to_permission_prompt`) calls
/// `clear_pending_permission`. Verify the cleanup is observable
/// from a subsequent reload — i.e. a second crash/reload after
/// resolution does NOT redundantly re-prompt the user.
#[test]
fn resolve_then_reload_does_not_re_prompt() {
    let dir = TempDir::new().expect("tempdir");
    let record = sample_record("sess-resolve", "req-2");

    write_pending_permission(dir.path(), &record).expect("write");
    // User answers "allow" / "deny" — the IPC handler clears the record.
    clear_pending_permission(dir.path(), &record.session_id).expect("clear");

    // Now reload-equivalent: rehydration must NOT find a pending prompt.
    let after_resolve = read_pending_permission(dir.path(), &record.session_id).expect("read");
    assert!(
        after_resolve.is_none(),
        "resolved prompt should not be re-prompted on reload"
    );
}

// ────────── Multi-session independence ──────────

/// Two separate sessions can have concurrent pending prompts.
/// Resolving one must not affect the other — otherwise a user
/// switching between two chats could lose the prompt on the
/// background chat.
#[test]
fn multi_session_independence_preserves_other_records() {
    let dir = TempDir::new().expect("tempdir");
    let session_a = sample_record("sess-A", "req-A");
    let session_b = sample_record("sess-B", "req-B");

    write_pending_permission(dir.path(), &session_a).expect("write A");
    write_pending_permission(dir.path(), &session_b).expect("write B");

    // Resolve session A only.
    clear_pending_permission(dir.path(), "sess-A").expect("clear A");

    let a_after = read_pending_permission(dir.path(), "sess-A").expect("read A");
    let b_after = read_pending_permission(dir.path(), "sess-B").expect("read B");

    assert!(a_after.is_none(), "session A should be cleared");
    assert_eq!(
        b_after,
        Some(session_b),
        "session B must be preserved across session A's resolution"
    );
}

/// Repeat write to the same session (e.g. a follow-up prompt within
/// the same session before the first one was answered) overwrites
/// cleanly. The frontend reads the latest record after reload.
#[test]
fn second_write_overwrites_earlier_record_for_same_session() {
    let dir = TempDir::new().expect("tempdir");
    let first = sample_record("sess-overwrite", "req-first");
    let mut second = sample_record("sess-overwrite", "req-second");
    second.tool_name = "file_write".to_string();
    second.message = "Tool 'file_write' requires writeAll permission".to_string();

    write_pending_permission(dir.path(), &first).expect("write first");
    write_pending_permission(dir.path(), &second).expect("write second");

    let recovered = read_pending_permission(dir.path(), "sess-overwrite")
        .expect("read")
        .expect("record exists");
    assert_eq!(recovered, second);
    assert_ne!(recovered, first);
}

// ────────── On-disk format stability ──────────

/// The on-disk path layout is part of the public surface — anything
/// that operates on `~/.if2ai/runtime/pending-permissions/` (backups,
/// log inspection, support tooling) needs the path to be stable.
#[test]
fn on_disk_file_is_at_expected_path() {
    let dir = TempDir::new().expect("tempdir");
    let record = sample_record("sess-path", "req-path");

    write_pending_permission(dir.path(), &record).expect("write");

    let expected = pending_path(dir.path(), "sess-path");
    assert!(
        expected.exists(),
        "pending file must land at runtime/pending-permissions/<session>.json"
    );
}

/// Path traversal protection: a malicious session_id containing
/// `..`, `/`, or other shell-meaningful characters must be sanitized
/// to a flat filename so the write stays inside the configured dir.
#[test]
fn malicious_session_id_is_sanitized_inside_pending_dir() {
    let dir = TempDir::new().expect("tempdir");
    let record = sample_record("../escape/attempt", "req-attempt");

    write_pending_permission(dir.path(), &record).expect("write");

    // The expected sanitized filename collapses every special char to '_'.
    let sanitized = pending_path(dir.path(), "___escape_attempt");
    assert!(
        sanitized.exists(),
        "sanitized path '{}' should exist; sanitization must not let traversal escape",
        sanitized.display()
    );

    // No file should escape the pending-permissions directory.
    let escape_target = dir.path().join("escape");
    assert!(
        !escape_target.exists(),
        "sanitization must prevent path traversal — '{}' should not exist",
        escape_target.display()
    );

    // Round-trip: read with the original (un-sanitized) session_id MUST
    // resolve to the sanitized file. Otherwise the producer + consumer
    // would disagree on where to find the record.
    let recovered = read_pending_permission(dir.path(), "../escape/attempt").expect("read");
    assert_eq!(recovered, Some(record));
}

// ────────── Corrupted-file recovery ──────────

/// If the on-disk file is corrupted (truncated mid-write, edited by
/// hand, hit a bad sector), the read MUST return Err with
/// `InvalidData` rather than panicking. The IPC layer can then
/// surface a friendly error and let the user retry.
#[test]
fn corrupted_file_returns_invalid_data_not_panic() {
    let dir = TempDir::new().expect("tempdir");
    let session_id = "sess-corrupt";

    // Create a corrupt pending file directly.
    let path = pending_path(dir.path(), session_id);
    std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
    std::fs::write(&path, b"{not valid json").expect("write garbage");

    let result = read_pending_permission(dir.path(), session_id);
    let err = result.expect_err("corrupted file must return Err");
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::InvalidData,
        "corrupted-file read should be InvalidData, got {err:?}"
    );
}

/// Missing file is NOT an error — it's the common steady-state where
/// no prompt is currently raised. `read` returns `Ok(None)` so the
/// caller can short-circuit cleanly.
#[test]
fn missing_file_returns_ok_none() {
    let dir = TempDir::new().expect("tempdir");
    let recovered = read_pending_permission(dir.path(), "never-existed").expect("read");
    assert!(recovered.is_none());
}

/// Repeated clear of an already-cleared (or never-existed) record
/// is a no-op, NOT an error. The IPC handler may call clear
/// defensively even when it isn't sure whether a prompt was raised.
#[test]
fn clear_idempotent_when_no_record_exists() {
    let dir = TempDir::new().expect("tempdir");
    clear_pending_permission(dir.path(), "never-existed").expect("first clear should succeed");
    clear_pending_permission(dir.path(), "never-existed").expect("second clear should succeed");
}
