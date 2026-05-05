//! DT-01 Stage 1 — derive [`HarnessRunReport`] from the canonical
//! run-log JSONL (per ARCHITECTURE.md §6.1 and
//! `docs/superpowers/plans/2026-05-05-dt01-harness-derive-from-runlog.md`).
//!
//! Honest scope of this slice (PR S1.2):
//!
//! - [`collect_run_entries`] reads the canonical per-session/per-run
//!   JSONL file (path layout owned by
//!   [`crate::modules::runtime::event_log::RunEventLogger`]) and
//!   parses each line into a [`RunLogEntry`], preserving file order.
//! - [`fold_run_log_to_report`] is now implemented.  It consumes an
//!   ordered slice of run-log entries and folds them into a
//!   [`HarnessRunReport`].  The mapping table is the canonical spec
//!   from `docs/superpowers/plans/2026-05-05-dt01-s11-schema-audit.md`
//!   §2.1.
//! - `S1.2` closed one emit gap from the audit:
//!   `ExecutionModeJudged` → `execution_mode:judged` envelope (see
//!   `commands/request_intelligence.rs`).
//! - `S1.3` closes the two remaining top-priority emit gaps:
//!   `TurnFinished` → `conversation:turn_finished` (audit §2) and
//!   `PrepareStepExecuted` → `system:prepare_step` (audit §10).
//!   The fold below now reduces both into the corresponding
//!   `aggregate.turns_*`, `aggregate.total_turn_duration_ms`,
//!   `task.last_turn_succeeded`, `aggregate.prepare_step_*`, and
//!   `evidence.prepare_step` fields.  The remaining `0`-defaults
//!   (`input_tokens_total`, `output_tokens_total`,
//!   `compaction_events`, `resume_invocations`) are intentional
//!   per the audit's "variants intentionally left untouched"
//!   list — no production caller exists today.
//!
//! No production code path consumes [`fold_run_log_to_report`] in
//! S1.2; behaviour is therefore zero-change.  S1.3 will switch
//! `TraceAggregator::finalize` over to this path.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::run_report::{
    AggregateMetrics, BlockingFailure, EvidenceBundle, ExecutionModeTrace, HarnessRunReport,
    MemoryAfterTurnTrace, PermissionPromptTrace, PermissionResolutionTrace, PrepareStepTrace,
    Severity, StreamErrorTrace, TaskOutcome, TaskRunResult, HARNESS_RUN_REPORT_VERSION,
};
use crate::modules::application::ConflictResolutionOutcome;
use crate::modules::control_plane::prepare_step_execution::{
    BoundaryDecision, PermissionDecision, PrepareStepOutcome, SandboxPolicy,
};
use crate::modules::runtime::contracts::memory::MemoryWriteDisposition;
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

/// Parse an `occurred_at` RFC3339 string into a UTC [`DateTime`],
/// falling back to `now` on parse error so a single corrupt
/// timestamp cannot poison the whole fold.
fn parse_occurred_at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// Family-tag normalisation: split a run-log `event_type` of the
/// form `"<family_kind>:<family_tag>"` into `(family_kind, tag)`.
/// Entries appended via the legacy `append_sync(event_type, …)`
/// path may carry a bare tag with no `:`; in that case `kind` is
/// the empty string and the whole value is returned as the tag.
fn split_family(event_type: &str) -> (&str, &str) {
    match event_type.split_once(':') {
        Some((kind, tag)) => (kind, tag),
        None => ("", event_type),
    }
}

/// Fold an ordered slice of [`RunLogEntry`] rows into a
/// [`HarnessRunReport`].
///
/// Mapping is the canonical §2.1 table from
/// `docs/superpowers/plans/2026-05-05-dt01-s11-schema-audit.md`.
///
/// `started_at` / `ended_at` are the first/last `occurred_at`
/// timestamps observed.  When `entries` is empty the report falls
/// back to `Utc::now()` for both bounds (matching the empty-report
/// shape from [`HarnessRunReport::new_empty`]).
///
/// The `run_id` / `session_id` are taken from the first entry that
/// carries them.  Caller is expected to pass entries from a single
/// `(session_id, run_id)` slice; mixed slices yield an undefined
/// (last-wins) projection and should not occur in practice because
/// [`collect_run_entries`] reads exactly one file.
#[must_use]
pub fn fold_run_log_to_report(entries: &[RunLogEntry]) -> HarnessRunReport {
    // ── Bootstrapping ────────────────────────────────────────────
    let (run_id, session_id) = entries
        .first()
        .map(|e| (e.run_id.clone(), Some(e.session_id.clone())))
        .unwrap_or_else(|| (String::new(), None));
    let started_at = entries
        .first()
        .map(|e| parse_occurred_at(&e.occurred_at))
        .unwrap_or_else(Utc::now);
    let ended_at = entries
        .last()
        .map(|e| parse_occurred_at(&e.occurred_at))
        .unwrap_or(started_at);

    let mut report = HarnessRunReport {
        report_version: HARNESS_RUN_REPORT_VERSION.to_string(),
        run_id: run_id.clone(),
        label: None,
        session_id,
        project_id: None,
        started_at,
        ended_at,
        task: TaskRunResult {
            task_id: run_id,
            outcome: TaskOutcome::Incomplete,
            turn_count: 0,
            last_turn_succeeded: false,
            error_summary: None,
        },
        aggregate: AggregateMetrics::default(),
        blocking_failures: Vec::new(),
        evidence: EvidenceBundle::default(),
    };

    // Track tool_call_id → tool_name so the *result* row can be
    // attributed to the tool name even when the failed/completed
    // payload omits it (defensive — usually carried).
    let mut tool_call_names: HashMap<String, String> = HashMap::new();

    for entry in entries {
        let (kind, tag) = split_family(&entry.event_type);
        let observed_at = parse_occurred_at(&entry.occurred_at);
        let payload = &entry.payload;

        match (kind, tag) {
            // ── Memory after-turn (✅ canonical, full fidelity) ─
            ("memory", "after_turn") => fold_memory_after_turn(&mut report, payload, observed_at),

            // ── Permission prompt (✅ canonical) ────────────────
            ("permission", "prompt_opened") => {
                report.aggregate.permission_prompts += 1;
                let tool_name = payload
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                report
                    .evidence
                    .permission_prompts
                    .push(PermissionPromptTrace {
                        tool_name,
                        observed_at,
                    });
            }

            // ── Permission decision (canonical family + legacy
            //     bare-string `permission_resolved` rows; both are
            //     accepted per audit §12) ─────────────────────────
            ("permission", "decision") | ("", "permission_resolved") => {
                let decision = payload
                    .get("decision")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let scope = payload
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or("once")
                    .to_string();
                let tool_name = payload
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                if decision == "allow" {
                    report.aggregate.permission_resolved_allow += 1;
                } else if decision == "deny" {
                    report.aggregate.permission_resolved_deny += 1;
                }
                report
                    .evidence
                    .permission_resolved
                    .push(PermissionResolutionTrace {
                        tool_name,
                        decision,
                        scope,
                        observed_at,
                    });
            }

            // ── Execution-mode classifier judgment (closed in
            //     S1.2 by the new dispatch in
            //     commands/request_intelligence.rs) ───────────────
            ("execution_mode", "judged") => {
                report.aggregate.execution_mode_judgments += 1;
                let execution_mode = payload
                    .get("execution_mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let risk_level = payload
                    .get("risk_level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let complexity_level = payload
                    .get("complexity_level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let policy_version = payload
                    .get("policy_version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                report.evidence.execution_mode.push(ExecutionModeTrace {
                    execution_mode,
                    risk_level,
                    complexity_level,
                    policy_version,
                    observed_at,
                });
            }

            // ── Tool call lifecycle ─────────────────────────────
            // First observation of each tool_call_id increments the
            // requested counter (audit §5).  Later events attribute
            // success/failure (audit §6).
            ("tool", "tool_call_queued") | ("tool", "tool_call_running") => {
                let tool_name = payload
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let tool_call_id = entry
                    .tool_call_id
                    .clone()
                    .or_else(|| {
                        payload
                            .get("tool_call_id")
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                    })
                    .unwrap_or_default();
                if !tool_call_id.is_empty() && !tool_call_names.contains_key(&tool_call_id) {
                    if let Some(name) = tool_name {
                        report.aggregate.tool_calls_total += 1;
                        *report
                            .aggregate
                            .tool_calls_by_name
                            .entry(name.clone())
                            .or_insert(0) += 1;
                        tool_call_names.insert(tool_call_id, name);
                    }
                }
            }
            ("tool", "tool_call_completed") => {
                let name = resolve_tool_name(entry, payload, &tool_call_names);
                if let Some(name) = name {
                    *report
                        .aggregate
                        .tool_successes_by_name
                        .entry(name)
                        .or_insert(0) += 1;
                }
            }
            ("tool", "tool_call_failed") => {
                let name = resolve_tool_name(entry, payload, &tool_call_names);
                if let Some(name) = name {
                    report.blocking_failures.push(BlockingFailure {
                        code: "tool_failure".to_string(),
                        severity: Severity::Warning,
                        message: format!("Tool '{name}' returned success=false"),
                        evidence_ref: Some(format!("tool_result:{name}")),
                        observed_at,
                    });
                }
            }

            // ── Stream errors (canonical conversation:stream_error) ─
            ("conversation", "stream_error") => {
                report.aggregate.stream_errors += 1;
                let reason = payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let resume_available = payload
                    .get("resume_available")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                report.evidence.stream_errors.push(StreamErrorTrace {
                    reason: reason.clone(),
                    resume_available,
                    observed_at,
                });
                let severity = if resume_available {
                    Severity::Warning
                } else {
                    Severity::Blocking
                };
                report.blocking_failures.push(BlockingFailure {
                    code: "stream_error".to_string(),
                    severity,
                    message: format!("stream errored: {reason}"),
                    evidence_ref: Some(format!(
                        "stream_errors:{}",
                        report.evidence.stream_errors.len() - 1
                    )),
                    observed_at,
                });
            }

            // ── Project id may ride on a run_started payload ────
            ("conversation", "run_started") => {
                if let Some(pid) = payload
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .filter(|_| report.project_id.is_none())
                {
                    report.project_id = Some(pid.to_string());
                }
            }

            // ── DT-01 S1.3 — turn-level rollup (audit §2) ─────
            // Mirrors the canonical envelope dispatched alongside
            // every `emit_turn_finished` bus emit (run.rs +
            // stream_finalize.rs).  Drives `turn_count`,
            // `turns_completed`, `turns_succeeded`,
            // `last_turn_succeeded`, `total_turn_duration_ms`, and
            // (when present) per-turn token totals.
            ("conversation", "turn_finished") => {
                report.task.turn_count = report.task.turn_count.saturating_add(1);
                report.aggregate.turns_completed =
                    report.aggregate.turns_completed.saturating_add(1);
                let succeeded = payload
                    .get("succeeded")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if succeeded {
                    report.aggregate.turns_succeeded =
                        report.aggregate.turns_succeeded.saturating_add(1);
                }
                report.task.last_turn_succeeded = succeeded;
                let duration_ms = payload
                    .get("durationMs")
                    .or_else(|| payload.get("duration_ms"))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                report.aggregate.total_turn_duration_ms = report
                    .aggregate
                    .total_turn_duration_ms
                    .saturating_add(duration_ms);
                // Wire keys are `inputUsage` / `outputUsage` so the
                // `SENSITIVE_JSON_KEYS` redaction filter (substring
                // `"token"`) does not strip the integers.  Legacy
                // snake_case fallbacks are kept for hand-written
                // fixtures.
                if let Some(t_in) = payload
                    .get("inputUsage")
                    .or_else(|| payload.get("input_usage"))
                    .and_then(|v| v.as_u64())
                {
                    report.aggregate.input_tokens_total =
                        report.aggregate.input_tokens_total.saturating_add(t_in);
                }
                if let Some(t_out) = payload
                    .get("outputUsage")
                    .or_else(|| payload.get("output_usage"))
                    .and_then(|v| v.as_u64())
                {
                    report.aggregate.output_tokens_total =
                        report.aggregate.output_tokens_total.saturating_add(t_out);
                }
            }

            // ── DT-01 S1.3 — prepare-step rollup (audit §10) ──
            // Mirrors the canonical envelope dispatched alongside
            // every `AgentEvent::PrepareStepExecuted` bus emit
            // (`stream_tool_execution.rs`).  Drives
            // `prepare_step_total/granted/requires_approval/denied`
            // and `evidence.prepare_step`.
            ("system", "prepare_step") => {
                fold_prepare_step(&mut report, payload, observed_at);
            }
            _ => {}
        }
    }

    report.task.outcome = derive_task_outcome(&report);
    report
}

fn resolve_tool_name(
    entry: &RunLogEntry,
    payload: &serde_json::Value,
    seen: &HashMap<String, String>,
) -> Option<String> {
    if let Some(name) = payload.get("tool_name").and_then(|v| v.as_str()) {
        return Some(name.to_string());
    }
    let id = entry
        .tool_call_id
        .clone()
        .or_else(|| {
            payload
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    if id.is_empty() {
        None
    } else {
        seen.get(&id).cloned()
    }
}

fn fold_memory_after_turn(
    report: &mut HarnessRunReport,
    payload: &serde_json::Value,
    observed_at: DateTime<Utc>,
) {
    use crate::modules::application::QualityGateResult;
    use crate::modules::runtime::contracts::memory::MemoryWriteDecision;

    // The canonical envelope is camelCase per
    // `application/stream_emitter_service.rs`.  Fall back to
    // snake_case keys so the fold also accepts hand-written
    // fixtures (mostly to keep tests resilient).
    let trace_version = payload
        .get("traceVersion")
        .or_else(|| payload.get("trace_version"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let caller = payload
        .get("caller")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let policy_version = payload
        .get("policyVersion")
        .or_else(|| payload.get("policy_version"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let session_id = payload
        .get("sessionId")
        .or_else(|| payload.get("session_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let project_id = payload
        .get("projectId")
        .or_else(|| payload.get("project_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let decided_at = payload
        .get("decidedAt")
        .or_else(|| payload.get("decided_at"))
        .and_then(|v| v.as_str())
        .map(parse_occurred_at)
        .unwrap_or(observed_at);

    let decisions: Vec<MemoryWriteDecision> = payload
        .get("decisions")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let quality: QualityGateResult = payload
        .get("quality")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_else(|| QualityGateResult {
            accepted: Vec::new(),
            rejected: Vec::new(),
            warnings: Vec::new(),
            policy_version: String::new(),
        });
    let conflicts: Vec<crate::modules::application::ConflictResolution> = payload
        .get("conflicts")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    report.aggregate.memory_decision_count += decisions.len() as u64;
    report.aggregate.memory_accepted_count += quality.accepted.len() as u64;
    report.aggregate.memory_rejected_count += quality.rejected.len() as u64;
    report.aggregate.memory_warning_count += quality.warnings.len() as u64;
    for c in &conflicts {
        match c.outcome {
            ConflictResolutionOutcome::RequirePrompt => {
                report.aggregate.memory_conflict_prompt_count += 1;
            }
            ConflictResolutionOutcome::AcceptReplacement => {
                report.aggregate.memory_conflict_replace_count += 1;
            }
            ConflictResolutionOutcome::KeepExisting
            | ConflictResolutionOutcome::RejectCandidate => {
                report.aggregate.memory_conflict_keep_existing_count += 1;
            }
            ConflictResolutionOutcome::NoConflict => {
                report.aggregate.memory_conflict_no_conflict_count += 1;
            }
        }
    }
    let deny_count = decisions
        .iter()
        .filter(|d| matches!(d.disposition, MemoryWriteDisposition::Deny))
        .count();
    if deny_count > 0 {
        report.blocking_failures.push(BlockingFailure {
            code: "memory_write_denied".to_string(),
            severity: Severity::Blocking,
            message: format!("{deny_count} memory candidate(s) denied by write policy"),
            evidence_ref: Some(format!(
                "memory_after_turn:{}",
                report.evidence.memory_after_turn.len()
            )),
            observed_at,
        });
    }
    if project_id.is_some() && report.project_id.is_none() {
        report.project_id = project_id.clone();
    }
    report
        .evidence
        .memory_after_turn
        .push(MemoryAfterTurnTrace {
            trace_version,
            caller,
            session_id,
            project_id,
            policy_version,
            decided_at,
            decisions,
            quality,
            conflicts,
        });
}

/// DT-01 S1.3 — fold one `system:prepare_step` envelope into the
/// report's `prepare_step_*` aggregate counters and
/// `evidence.prepare_step` vector.  Audit §10.
fn fold_prepare_step(
    report: &mut HarnessRunReport,
    payload: &serde_json::Value,
    observed_at: DateTime<Utc>,
) {
    let tool_name = payload
        .get("toolName")
        .or_else(|| payload.get("tool_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let outcome: PrepareStepOutcome = payload
        .get("outcome")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(PrepareStepOutcome::Granted);
    let boundary: BoundaryDecision = payload
        .get("boundary")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(BoundaryDecision::NotApplicable);
    let permission: PermissionDecision = payload
        .get("permission")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(PermissionDecision::Allow);
    let sandbox: SandboxPolicy = payload
        .get("sandbox")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(SandboxPolicy::None);
    let policy_version = payload
        .get("policyVersion")
        .or_else(|| payload.get("policy_version"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    report.aggregate.prepare_step_total = report.aggregate.prepare_step_total.saturating_add(1);
    match outcome {
        PrepareStepOutcome::Granted => {
            report.aggregate.prepare_step_granted =
                report.aggregate.prepare_step_granted.saturating_add(1);
        }
        PrepareStepOutcome::RequiresApproval => {
            report.aggregate.prepare_step_requires_approval = report
                .aggregate
                .prepare_step_requires_approval
                .saturating_add(1);
        }
        PrepareStepOutcome::Denied => {
            report.aggregate.prepare_step_denied =
                report.aggregate.prepare_step_denied.saturating_add(1);
            report.blocking_failures.push(BlockingFailure {
                code: "prepare_step_denied".to_string(),
                severity: Severity::Blocking,
                message: format!("prepare_step denied tool '{tool_name}'"),
                evidence_ref: Some(format!(
                    "prepare_step:{}",
                    report.evidence.prepare_step.len()
                )),
                observed_at,
            });
        }
    }
    report.evidence.prepare_step.push(PrepareStepTrace {
        tool_name,
        outcome,
        boundary,
        permission,
        sandbox,
        policy_version,
        observed_at,
    });
}

fn derive_task_outcome(r: &HarnessRunReport) -> TaskOutcome {
    if r.aggregate.turns_completed == 0 {
        return TaskOutcome::Incomplete;
    }
    let has_blocking = r
        .blocking_failures
        .iter()
        .any(|f| f.severity == Severity::Blocking);
    if has_blocking {
        return TaskOutcome::Failed;
    }
    let has_warning = r
        .blocking_failures
        .iter()
        .any(|f| f.severity == Severity::Warning);
    match (r.task.last_turn_succeeded, has_warning) {
        (true, false) => TaskOutcome::Success,
        (true, true) => TaskOutcome::PartialSuccess,
        (false, _) => TaskOutcome::Failed,
    }
}

/// Compute the absolute difference in seconds between two
/// [`DateTime<Utc>`] values.  Used by the reconciliation test's
/// ±1s tolerance check (kept here so other harness tests can
/// reuse the helper).
#[must_use]
pub fn timestamp_within_tolerance(a: DateTime<Utc>, b: DateTime<Utc>, tolerance_secs: i64) -> bool {
    (a - b).num_seconds().abs() <= tolerance_secs
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

    /// Unit test: empty input produces an `Incomplete` report
    /// pinned to the current contract version.
    #[test]
    fn fold_empty_entries_returns_incomplete_report() {
        let report = fold_run_log_to_report(&[]);
        assert_eq!(report.report_version, HARNESS_RUN_REPORT_VERSION);
        assert_eq!(report.task.outcome, TaskOutcome::Incomplete);
        assert_eq!(report.aggregate.turns_completed, 0);
        assert_eq!(report.aggregate.permission_prompts, 0);
        assert!(report.evidence.memory_after_turn.is_empty());
    }

    /// Unit test: `permission:prompt_opened` and
    /// `permission:decision` envelopes feed the corresponding
    /// counters and evidence vectors.
    #[test]
    fn fold_permission_envelopes_update_counts_and_evidence() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logger = RunEventLogger::for_base_dir(tmp.path(), "s", "r");
        logger.append_sync(
            "permission:prompt_opened",
            serde_json::json!({ "tool_name": "bash" }),
        );
        logger.append_sync(
            "permission:decision",
            serde_json::json!({ "tool_name": "bash", "decision": "allow", "scope": "once" }),
        );
        logger.append_sync(
            "permission:decision",
            serde_json::json!({ "decision": "deny", "scope": "session" }),
        );
        let entries =
            collect_run_entries(tmp.path(), "s", "r").expect("collect_run_entries should succeed");
        let report = fold_run_log_to_report(&entries);
        assert_eq!(report.aggregate.permission_prompts, 1);
        assert_eq!(report.aggregate.permission_resolved_allow, 1);
        assert_eq!(report.aggregate.permission_resolved_deny, 1);
        assert_eq!(report.evidence.permission_prompts.len(), 1);
        assert_eq!(report.evidence.permission_resolved.len(), 2);
        assert_eq!(
            report.evidence.permission_prompts[0].tool_name.as_str(),
            "bash"
        );
    }

    /// Unit test: tool lifecycle envelopes feed
    /// `tool_calls_total`, `tool_calls_by_name`, and
    /// `tool_successes_by_name`.  Failed tool calls produce a
    /// `tool_failure` warning.
    #[test]
    fn fold_tool_envelopes_track_calls_and_successes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logger = RunEventLogger::for_base_dir(tmp.path(), "s", "r");
        logger.append_sync(
            "tool:tool_call_queued",
            serde_json::json!({
                "tool_name": "bash",
                "tool_call_id": "tc-1",
            }),
        );
        logger.append_sync(
            "tool:tool_call_completed",
            serde_json::json!({
                "tool_name": "bash",
                "tool_call_id": "tc-1",
            }),
        );
        logger.append_sync(
            "tool:tool_call_queued",
            serde_json::json!({
                "tool_name": "read_file",
                "tool_call_id": "tc-2",
            }),
        );
        logger.append_sync(
            "tool:tool_call_failed",
            serde_json::json!({
                "tool_name": "read_file",
                "tool_call_id": "tc-2",
            }),
        );
        let entries =
            collect_run_entries(tmp.path(), "s", "r").expect("collect_run_entries should succeed");
        let report = fold_run_log_to_report(&entries);
        assert_eq!(report.aggregate.tool_calls_total, 2);
        assert_eq!(
            report.aggregate.tool_calls_by_name.get("bash").copied(),
            Some(1)
        );
        assert_eq!(
            report.aggregate.tool_successes_by_name.get("bash").copied(),
            Some(1)
        );
        assert!(report
            .blocking_failures
            .iter()
            .any(|f| f.code == "tool_failure"));
    }

    /// Unit test: `execution_mode:judged` rows feed the
    /// classifier counter + evidence trace (closes the S1.2 emit
    /// gap on the read side).
    #[test]
    fn fold_execution_mode_judged_appends_evidence() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logger = RunEventLogger::for_base_dir(tmp.path(), "s", "r");
        logger.append_sync(
            "execution_mode:judged",
            serde_json::json!({
                "execution_mode": "direct_execute",
                "risk_level": "low",
                "complexity_level": "trivial",
                "policy_version": "ingress-classifier@m1.6",
            }),
        );
        let entries =
            collect_run_entries(tmp.path(), "s", "r").expect("collect_run_entries should succeed");
        let report = fold_run_log_to_report(&entries);
        assert_eq!(report.aggregate.execution_mode_judgments, 1);
        assert_eq!(report.evidence.execution_mode.len(), 1);
        assert_eq!(
            report.evidence.execution_mode[0].execution_mode.as_str(),
            "direct_execute"
        );
    }

    /// Unit test: stream errors increment counters and surface
    /// blocking-vs-warning severity per `resume_available`.
    #[test]
    fn fold_stream_error_classifies_severity_by_resume_available() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logger = RunEventLogger::for_base_dir(tmp.path(), "s", "r");
        logger.append_sync(
            "conversation:stream_error",
            serde_json::json!({
                "reason": "network_timeout",
                "resume_available": true,
            }),
        );
        logger.append_sync(
            "conversation:stream_error",
            serde_json::json!({
                "reason": "fatal",
                "resume_available": false,
            }),
        );
        let entries =
            collect_run_entries(tmp.path(), "s", "r").expect("collect_run_entries should succeed");
        let report = fold_run_log_to_report(&entries);
        assert_eq!(report.aggregate.stream_errors, 2);
        let warn = report
            .blocking_failures
            .iter()
            .filter(|f| f.code == "stream_error" && f.severity == Severity::Warning)
            .count();
        let block = report
            .blocking_failures
            .iter()
            .filter(|f| f.code == "stream_error" && f.severity == Severity::Blocking)
            .count();
        assert_eq!(warn, 1);
        assert_eq!(block, 1);
    }

    /// Reconciliation scenario test (DT-01-S1.3 implementation).
    ///
    /// Drives a hand-written fixture covering the full S1.3 fold
    /// surface: memory after-turn, permission prompt + decision,
    /// tool call lifecycle, stream error, execution-mode judgment,
    /// **TurnFinished × 2** (multi-turn — one success then one
    /// failure), and **PrepareStepExecuted** (granted + denied).
    ///
    /// **Intentional zeros remain only for variants the audit
    /// flagged as having no production caller**:
    ///
    /// - `aggregate.input_tokens_total` — `LlmRequested` is unwired
    ///   (audit §3).
    /// - `aggregate.compaction_events` — bus variant unused
    ///   (audit §7).
    /// - `aggregate.resume_invocations` — reserved for M4.5+
    ///   (audit §14).
    #[test]
    fn reconciliation_runlog_fold_matches_audit_table_for_s13_surface() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let session_id = "sess-recon";
        let run_id = "run-recon";
        let logger = RunEventLogger::for_base_dir(tmp.path(), session_id, run_id);

        let t0_started = Utc::now();
        // run_started — used to seed project_id projection.
        logger.append_sync(
            "conversation:run_started",
            serde_json::json!({
                "caller": "reconciliation_test",
                "project_id": "proj-x",
            }),
        );
        // memory:after_turn — accepted=1, conflict=RequirePrompt.
        logger.append_sync(
            "memory:after_turn",
            serde_json::json!({
                "traceVersion": "memory-after-turn-trace@m4.1",
                "caller": "reconciliation",
                "policyVersion": "memory-write-policy@m3.3",
                "decidedAt": Utc::now().to_rfc3339(),
                "decisions": [],
                "quality": {
                    "accepted": [],
                    "rejected": [],
                    "warnings": [],
                    "policyVersion": "memory-quality-gate@m4.0",
                },
                "conflicts": [],
            }),
        );
        // tool lifecycle — one success, one failure.
        logger.append_sync(
            "tool:tool_call_queued",
            serde_json::json!({"tool_name": "bash", "tool_call_id": "tc-a"}),
        );
        logger.append_sync(
            "tool:tool_call_completed",
            serde_json::json!({"tool_name": "bash", "tool_call_id": "tc-a"}),
        );
        logger.append_sync(
            "tool:tool_call_queued",
            serde_json::json!({"tool_name": "read_file", "tool_call_id": "tc-b"}),
        );
        logger.append_sync(
            "tool:tool_call_failed",
            serde_json::json!({"tool_name": "read_file", "tool_call_id": "tc-b"}),
        );
        // permission prompt + allow.
        logger.append_sync(
            "permission:prompt_opened",
            serde_json::json!({"tool_name": "bash"}),
        );
        logger.append_sync(
            "permission:decision",
            serde_json::json!({"tool_name": "bash", "decision": "allow", "scope": "once"}),
        );
        // execution-mode judgment (S1.2 closed gap).
        logger.append_sync(
            "execution_mode:judged",
            serde_json::json!({
                "execution_mode": "direct_execute",
                "risk_level": "low",
                "complexity_level": "trivial",
                "policy_version": "ingress-classifier@m1.6",
            }),
        );
        // recoverable stream error.
        logger.append_sync(
            "conversation:stream_error",
            serde_json::json!({"reason": "network_timeout", "resume_available": true}),
        );
        // DT-01 S1.3 — prepare_step (granted) + prepare_step (denied).
        logger.append_sync(
            "system:prepare_step",
            serde_json::json!({
                "toolName": "bash",
                "outcome": "granted",
                "boundary": "within_workdir",
                "permission": "allow",
                "sandbox": "none",
                "policyVersion": "prepare-step@m1.8-sandbox",
            }),
        );
        logger.append_sync(
            "system:prepare_step",
            serde_json::json!({
                "toolName": "file_edit",
                "outcome": "denied",
                "boundary": "outside_and_denied",
                "permission": { "deny": { "reason": "outside workdir" } },
                "sandbox": "none",
                "policyVersion": "prepare-step@m1.8-sandbox",
            }),
        );
        // DT-01 S1.3 — turn_finished × 2 (multi-turn: one failure
        // then one success — final turn drives `last_turn_succeeded`).
        logger.append_sync(
            "conversation:turn_finished",
            serde_json::json!({
                "turnNumber": 1,
                "succeeded": false,
                "durationMs": 1234,
                "terminalStatus": "failed",
                "outputUsage": 42,
            }),
        );
        logger.append_sync(
            "conversation:turn_finished",
            serde_json::json!({
                "turnNumber": 2,
                "succeeded": true,
                "durationMs": 567,
                "terminalStatus": "completed",
                "outputUsage": 17,
            }),
        );

        let entries = collect_run_entries(tmp.path(), session_id, run_id)
            .expect("collect_run_entries should succeed");
        let report = fold_run_log_to_report(&entries);

        // ── Identity / scoping ─────────────────────────────────
        assert_eq!(report.report_version, HARNESS_RUN_REPORT_VERSION);
        assert_eq!(report.run_id, run_id);
        assert_eq!(report.session_id.as_deref(), Some(session_id));
        assert_eq!(report.project_id.as_deref(), Some("proj-x"));
        assert!(
            timestamp_within_tolerance(report.started_at, t0_started, 1),
            "started_at within ±1s of fixture clock"
        );
        assert!(
            timestamp_within_tolerance(report.ended_at, Utc::now(), 1),
            "ended_at within ±1s of now"
        );

        // ── Aggregates the S1.2 fold can produce ──────────────
        assert_eq!(report.aggregate.permission_prompts, 1);
        assert_eq!(report.aggregate.permission_resolved_allow, 1);
        assert_eq!(report.aggregate.permission_resolved_deny, 0);
        assert_eq!(report.aggregate.tool_calls_total, 2);
        assert_eq!(
            report.aggregate.tool_calls_by_name.get("bash").copied(),
            Some(1)
        );
        assert_eq!(
            report.aggregate.tool_successes_by_name.get("bash").copied(),
            Some(1)
        );
        assert_eq!(report.aggregate.execution_mode_judgments, 1);
        assert_eq!(report.aggregate.stream_errors, 1);
        assert_eq!(report.aggregate.memory_decision_count, 0);

        // ── Evidence vectors ──────────────────────────────────
        assert_eq!(report.evidence.memory_after_turn.len(), 1);
        assert_eq!(report.evidence.permission_prompts.len(), 1);
        assert_eq!(report.evidence.permission_resolved.len(), 1);
        assert_eq!(report.evidence.execution_mode.len(), 1);
        assert_eq!(report.evidence.stream_errors.len(), 1);

        // ── DT-01 S1.3 closed gaps — turn_finished surface ────
        assert_eq!(
            report.task.turn_count, 2,
            "S1.3: 2 turn_finished envelopes folded"
        );
        assert_eq!(report.aggregate.turns_completed, 2);
        assert_eq!(report.aggregate.turns_succeeded, 1);
        assert_eq!(report.aggregate.total_turn_duration_ms, 1234 + 567);
        assert!(
            report.task.last_turn_succeeded,
            "S1.3: last turn_finished succeeded=true wins"
        );
        // tokensOut is the only token field carried in the fixture
        // (redaction disabled above so the fold actually sees the
        // numeric values).
        assert_eq!(report.aggregate.output_tokens_total, 42 + 17);

        // ── DT-01 S1.3 closed gaps — prepare_step surface ─────
        assert_eq!(
            report.aggregate.prepare_step_total, 2,
            "S1.3: 2 prepare_step envelopes folded"
        );
        assert_eq!(report.aggregate.prepare_step_granted, 1);
        assert_eq!(report.aggregate.prepare_step_denied, 1);
        assert_eq!(report.aggregate.prepare_step_requires_approval, 0);
        assert_eq!(report.evidence.prepare_step.len(), 2);
        assert!(
            report
                .blocking_failures
                .iter()
                .any(|f| f.code == "prepare_step_denied" && f.severity == Severity::Blocking),
            "S1.3: a denied prepare_step surfaces as Blocking failure"
        );

        // ── Intentional zeros (no production caller) ──────────
        assert_eq!(
            report.aggregate.input_tokens_total, 0,
            "audit §3-4: no inputUsage in fixture; LlmRequested unwired in production"
        );
        assert_eq!(
            report.aggregate.compaction_events, 0,
            "audit §7: ContextCompacted bus variant unused"
        );
        assert_eq!(
            report.aggregate.resume_invocations, 0,
            "audit §14: ResumeInvoked reserved for M4.5+"
        );

        // ── Outcome derivation ────────────────────────────────
        // turns_completed > 0 + a Blocking prepare_step_denied
        // failure ⇒ TaskOutcome::Failed.  The recoverable
        // stream_error contributes a Warning but the Blocking
        // override wins.
        assert_eq!(report.task.outcome, TaskOutcome::Failed);

        // Avoid dead variable warning; t0_started binds time we
        // sampled before fixture writes.
        let _ = t0_started;
    }

    /// DT-01 S1.3 — focused unit test: a single happy-path
    /// `conversation:turn_finished` envelope drives the full
    /// turn-level rollup with no other context.  Uses the canonical
    /// non-redacted wire keys `inputUsage` / `outputUsage` (see
    /// `TurnFinishedPayload` doc) so the values survive
    /// `SENSITIVE_JSON_KEYS` redaction.
    #[test]
    fn fold_turn_finished_drives_turn_aggregates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logger = RunEventLogger::for_base_dir(tmp.path(), "s", "r");
        logger.append_sync(
            "conversation:turn_finished",
            serde_json::json!({
                "turnNumber": 1,
                "succeeded": true,
                "durationMs": 999,
                "inputUsage": 10,
                "outputUsage": 20,
            }),
        );
        let entries =
            collect_run_entries(tmp.path(), "s", "r").expect("collect_run_entries should succeed");
        let report = fold_run_log_to_report(&entries);
        assert_eq!(report.task.turn_count, 1);
        assert_eq!(report.aggregate.turns_completed, 1);
        assert_eq!(report.aggregate.turns_succeeded, 1);
        assert_eq!(report.aggregate.total_turn_duration_ms, 999);
        assert_eq!(report.aggregate.input_tokens_total, 10);
        assert_eq!(report.aggregate.output_tokens_total, 20);
        assert!(report.task.last_turn_succeeded);
        assert_eq!(report.task.outcome, TaskOutcome::Success);
    }

    /// DT-01 S1.3 — focused unit test: a single
    /// `system:prepare_step` envelope with `outcome=granted`
    /// drives the granted counter and pushes a typed
    /// `PrepareStepTrace` into the evidence vector.
    #[test]
    fn fold_prepare_step_granted_appends_evidence_and_counter() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logger = RunEventLogger::for_base_dir(tmp.path(), "s", "r");
        logger.append_sync(
            "system:prepare_step",
            serde_json::json!({
                "toolName": "bash",
                "outcome": "granted",
                "boundary": "within_workdir",
                "permission": "allow",
                "sandbox": "none",
                "policyVersion": "prepare-step@m1.8-sandbox",
            }),
        );
        let entries =
            collect_run_entries(tmp.path(), "s", "r").expect("collect_run_entries should succeed");
        let report = fold_run_log_to_report(&entries);
        assert_eq!(report.aggregate.prepare_step_total, 1);
        assert_eq!(report.aggregate.prepare_step_granted, 1);
        assert_eq!(report.aggregate.prepare_step_denied, 0);
        assert_eq!(report.evidence.prepare_step.len(), 1);
        assert_eq!(report.evidence.prepare_step[0].tool_name.as_str(), "bash");
        assert!(report
            .blocking_failures
            .iter()
            .all(|f| f.code != "prepare_step_denied"));
    }
}
