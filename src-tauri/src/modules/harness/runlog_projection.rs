//! DT-01 Stage 1 PR S1.1 — skeleton for deriving [`HarnessRunReport`]
//! from the canonical run-log JSONL (per ARCHITECTURE.md §6.1 and
//! `docs/superpowers/plans/2026-05-05-dt01-harness-derive-from-runlog.md`).
//!
//! Honest scope of this slice:
//!
//! - [`collect_run_entries`] is implemented end-to-end: it reads the
//!   canonical per-session/per-run JSONL file (path layout owned by
//!   [`crate::modules::runtime::event_log::RunEventLogger`]) and
//!   parses each line into a [`RunLogEntry`], preserving file order
//!   (which is the monotonic `seq` order maintained by the logger).
//! - [`fold_run_log_to_report`] is **deliberately unimplemented** in
//!   S1.1 and panics via `todo!()`. The fold itself lands in S1.2
//!   together with any run-log emit gaps surfaced by the schema audit
//!   (`docs/superpowers/plans/2026-05-05-dt01-s11-schema-audit.md`).
//! - The reconciliation test in this module's `tests` sub-module is
//!   marked `#[ignore]` and exists as an executable spec for S1.2.
//!
//! No production code path consumes [`fold_run_log_to_report`] in
//! S1.1; behaviour is therefore zero-change.

#![allow(dead_code)]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::run_report::HarnessRunReport;
use crate::modules::runtime::event_log::RunLogEntry;

/// Resolve the canonical run-log JSONL path for a `(session_id, run_id)`
/// pair under `base_dir`.
///
/// Mirrors the private layout in
/// [`crate::modules::runtime::event_log::RunEventLogger::for_base_dir`]:
/// `<base_dir>/runtime/run-log/<session_id>/<run_id>.jsonl`.
#[must_use]
pub fn run_log_path(base_dir: &Path, session_id: &str, run_id: &str) -> PathBuf {
    base_dir
        .join("runtime")
        .join("run-log")
        .join(session_id)
        .join(format!("{run_id}.jsonl"))
}

/// Read all [`RunLogEntry`] rows for a single `(session_id, run_id)`
/// from the canonical run-log JSONL on disk.
///
/// File order is preserved; this matches the monotonic `seq`
/// assignment performed by
/// [`crate::modules::runtime::event_log::RunEventLogger`]. Lines that
/// are empty (only whitespace) are skipped silently — defensive
/// against trailing newlines. Lines that fail to parse return an
/// [`std::io::ErrorKind::InvalidData`] error so callers can surface
/// corruption explicitly rather than silently dropping records.
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] when the JSONL file is
/// missing, unreadable, or contains a malformed line.
pub fn collect_run_entries(
    base_dir: &Path,
    session_id: &str,
    run_id: &str,
) -> std::io::Result<Vec<RunLogEntry>> {
    let path = run_log_path(base_dir, session_id, run_id);
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: RunLogEntry = serde_json::from_str(&line).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "failed to parse run-log entry at {}:{}: {}",
                    path.display(),
                    line_no + 1,
                    err
                ),
            )
        })?;
        entries.push(entry);
    }
    Ok(entries)
}

/// Fold an ordered slice of [`RunLogEntry`] rows into a
/// [`HarnessRunReport`].
///
/// **S1.1 status**: deliberately unimplemented. The full reconciliation
/// fold (every aggregate counter, evidence vector, and blocking
/// failure rule) is the subject of PR DT-01-S1.2; the schema audit at
/// `docs/superpowers/plans/2026-05-05-dt01-s11-schema-audit.md`
/// enumerates the required event-family mappings.
///
/// # Panics
///
/// Always panics with `todo!("DT-01 S1.2: implement reconciliation fold")`.
#[must_use]
pub fn fold_run_log_to_report(_entries: &[RunLogEntry]) -> HarnessRunReport {
    todo!("DT-01 S1.2: implement reconciliation fold")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::event_log::RunEventLogger;

    /// Unit test: `collect_run_entries` reads three appended events
    /// in `seq` order, preserving ids and event_type strings.
    #[test]
    fn collect_run_entries_reads_three_appended_lines_in_order() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let session_id = "sess-collect";
        let run_id = "run-collect";

        // Use the production logger to write the JSONL so we exercise
        // the same on-disk layout (`<base>/runtime/run-log/<sid>/<rid>.jsonl`).
        let logger = RunEventLogger::for_base_dir(tmp.path(), session_id, run_id);
        let a = logger.append_sync("conversation:run_started", serde_json::json!({"k": 1}));
        let b = logger.append_sync("tool:tool_call_queued", serde_json::json!({"k": 2}));
        let c = logger.append_sync("conversation:stream_complete", serde_json::json!({"k": 3}));

        let entries = collect_run_entries(tmp.path(), session_id, run_id)
            .expect("collect_run_entries should succeed");

        assert_eq!(entries.len(), 3, "expected 3 parsed entries");
        let seqs: Vec<u64> = entries.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3], "entries must be in seq/file order");
        let event_ids: Vec<&str> = entries.iter().map(|e| e.event_id.as_str()).collect();
        assert_eq!(event_ids, vec![&*a.event_id, &*b.event_id, &*c.event_id]);
        let types: Vec<&str> = entries.iter().map(|e| e.event_type.as_str()).collect();
        assert_eq!(
            types,
            vec![
                "conversation:run_started",
                "tool:tool_call_queued",
                "conversation:stream_complete",
            ]
        );
    }

    /// Reconciliation scenario test (DT-01-S1.1 spec).
    ///
    /// Marked `#[ignore]` because [`fold_run_log_to_report`] panics
    /// with `todo!()` in S1.1 — that panic is the executable spec
    /// reminding S1.2 to implement the fold.  Once S1.2 lands the
    /// `#[ignore]` should be removed and the body extended to
    /// (a) drive a real `TurnService` turn through `EventBus` +
    /// `RunEventLogger`, (b) call `TraceAggregator::finalize` to get
    /// `eventbus_report`, (c) call `fold_run_log_to_report(
    /// collect_run_entries(...))` to get `runlog_report`, and
    /// (d) assert deep equivalence modulo a ±1s timestamp tolerance
    /// on `started_at` / `ended_at` / `observed_at`.
    #[test]
    #[ignore = "DT-01 S1.2: fold_run_log_to_report unimplemented; reconciliation test is the spec"]
    fn reconciliation_eventbus_vs_runlog_equivalent_for_minimal_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let session_id = "sess-recon";
        let run_id = "run-recon";

        // Write a minimal run-log fixture so `collect_run_entries`
        // returns a non-empty slice; the fold is what we are gating
        // on, not the read.  S1.2 will replace this fixture with a
        // real TurnService-driven scenario.
        let logger = RunEventLogger::for_base_dir(tmp.path(), session_id, run_id);
        let _ = logger.append_sync(
            "conversation:run_started",
            serde_json::json!({ "caller": "reconciliation_test" }),
        );
        let _ = logger.append_sync(
            "conversation:stream_complete",
            serde_json::json!({ "status": "ok" }),
        );

        let entries = collect_run_entries(tmp.path(), session_id, run_id)
            .expect("collect_run_entries should succeed");
        // The next line panics today via `todo!()`; that is the
        // S1.1 contract (executable spec for S1.2).
        let _runlog_report = fold_run_log_to_report(&entries);
        // S1.2 will add the EventBus comparison + ±1s timestamp
        // tolerance assertion here.
    }
}
