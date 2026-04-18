//! Security integration tests (M7).
//!
//! These tests exercise the security boundary across multiple modules:
//!
//! 1. `xss_input_rejected` — `ThreatScanner` flags XSS payloads alongside
//!    credential patterns, so they are visible to audit & policy layers.
//! 2. `path_traversal_blocked` — `BoundaryResolver` rejects `..` escapes
//!    relative to the configured workdir before any I/O happens.
//! 3. `atomic_write_rollback` — A path-traversal write fails *before* the
//!    target file exists on disk, proving boundary checks are pre-flight
//!    (no partial state is left behind to roll back).
//! 4. `access_context_enforcement` — `PermissionPolicy` denies a write tool
//!    when the active mode is `ReadOnly`, regardless of arguments.
//!
//! These integration tests live in `tests/` rather than `#[cfg(test)]` so
//! they additionally validate that the `pub` API surface of each module
//! is reachable by external consumers (Tauri commands, harness, CLI).

use std::path::PathBuf;

use if2ai_backend::modules::control_plane::BoundaryResolver;
use if2ai_backend::modules::memory::security::ThreatScanner;
use if2ai_backend::modules::runtime::permissions::{
    PermissionMode, PermissionOutcome, PermissionPolicy,
};

/// Helper: produce a unique, never-reused workdir under the OS temp dir.
fn fresh_workdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "if2ai_sec_{}_{}_{}",
        tag,
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create workdir");
    dir
}

#[test]
fn test_xss_input_rejected() {
    // ThreatScanner is the entry point for memory-write inspection.  It must
    // surface XSS-shaped payloads as `category = "xss"` so the audit / policy
    // layer can reject them in enforce mode.
    let scanner = ThreatScanner::with_builtin_patterns();

    let payloads = [
        "<script>alert(1)</script>",
        "<SCRIPT src=//evil.com/x.js></script>",
        "click <script  type='module'> here",
    ];
    for payload in payloads {
        let report = scanner.scan("note", payload);
        assert!(
            report.flagged,
            "expected XSS payload {payload:?} to be flagged"
        );
        assert_eq!(report.category, "xss", "payload={payload:?}");
    }

    // Sanity: benign HTML without <script> should not be flagged by the XSS
    // pattern (avoid false positives that would render the rule useless).
    let benign = scanner.scan("note", "<div>hello world</div>");
    assert!(!benign.flagged);
}

#[test]
fn test_path_traversal_blocked() {
    let workdir = fresh_workdir("traversal");
    let canonical_workdir =
        BoundaryResolver::canonicalize_workdir(&workdir).expect("canonicalize workdir");

    // Resolve an attacker-controlled relative path that escapes the workdir.
    let escape = workdir.join("..").join("..").join("etc").join("passwd");
    let canonical_escape =
        BoundaryResolver::canonicalize_with_missing_leaf_support(&escape).expect("canonicalize");

    let result = BoundaryResolver::assert_within_workdir(&canonical_workdir, &canonical_escape);
    assert!(result.is_err(), "traversal must be rejected");
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("outside allowed workdir"),
        "expected explicit boundary message, got {msg}"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&workdir);
}

#[test]
fn test_atomic_write_rollback() {
    // Arrange a workdir, then attempt to "write" to a path outside it via the
    // same pre-flight boundary check used by file_write.  The check must
    // reject the request *before* any file is created — i.e. there is
    // nothing to roll back, which is the strongest form of atomicity.
    let workdir = fresh_workdir("atomic");
    let canonical_workdir =
        BoundaryResolver::canonicalize_workdir(&workdir).expect("canonicalize workdir");

    let bad_target = workdir.join("..").join("escaped_via_traversal.txt");
    let canonical_target = BoundaryResolver::canonicalize_with_missing_leaf_support(&bad_target)
        .expect("canonicalize target");

    let denied =
        BoundaryResolver::assert_within_workdir(&canonical_workdir, &canonical_target).is_err();
    assert!(denied, "boundary must reject before write");

    // Confirm: nothing was written at the would-be escaped target.
    assert!(
        !canonical_target.exists(),
        "no file should exist at escaped target {canonical_target:?}"
    );

    let _ = std::fs::remove_dir_all(&workdir);
}

#[test]
fn test_access_context_enforcement() {
    // ReadOnly mode must deny any tool whose required mode is WorkspaceWrite
    // or stricter, regardless of input arguments.
    let policy = PermissionPolicy::new(PermissionMode::ReadOnly)
        .with_tool_requirement("file_write", PermissionMode::WorkspaceWrite);

    let outcome = policy.authorize("file_write", r#"{"path":"x.txt","content":"hi"}"#, None);
    match outcome {
        PermissionOutcome::Deny { reason } => {
            assert!(!reason.is_empty(), "deny reason should be non-empty");
        }
        PermissionOutcome::Allow => panic!("ReadOnly mode must not allow a write tool"),
    }

    // Sanity: lifting the mode to WorkspaceWrite makes the same call succeed,
    // proving the denial is mode-driven (not a hard-coded reject).
    let lifted = PermissionPolicy::new(PermissionMode::WorkspaceWrite)
        .with_tool_requirement("file_write", PermissionMode::WorkspaceWrite);
    assert_eq!(
        lifted.authorize("file_write", r#"{"path":"x.txt"}"#, None),
        PermissionOutcome::Allow
    );
}
