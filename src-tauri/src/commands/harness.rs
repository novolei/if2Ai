//! Harness Control IPC commands
//!
//! Tauri commands that allow the frontend (and harness-cli) to control the
//! agent loop observability harness: start/stop recording, query telemetry,
//! and inspect recording status.

use serde::{Deserialize, Serialize};
use tauri::State;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::commands::AppState;
use crate::modules::harness::{
    aggregate_suite_report, compare_reports, gate_evaluate_compare, gate_evaluate_suite,
    load_corpus_from_yaml, BaselineVsCandidate, GatePolicy, HarnessRunReport, Recommendation,
    RegressionCorpus, RunIndexEntry, SessionTelemetry, SuiteReport,
};

// ──────────────────────────────────────────────────────────────────────────────
// Response types
// ──────────────────────────────────────────────────────────────────────────────

/// Response payload for harness recording status queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessStatusResponse {
    /// Whether the harness is initialised (i.e., `AppState.harness` is `Some`).
    pub harness_enabled: bool,
    /// Session IDs currently being recorded.
    pub active_recordings: Vec<String>,
}

/// Response payload for telemetry queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessTelemetryResponse {
    /// `true` if telemetry data was found for the requested session.
    pub found: bool,
    /// Telemetry snapshot, or `None` when `found == false`.
    pub telemetry: Option<SessionTelemetry>,
}

// ──────────────────────────────────────────────────────────────────────────────
// IPC commands
// ──────────────────────────────────────────────────────────────────────────────

/// Return the current harness status.
///
/// Returns `harness_enabled: false` when the harness is not initialised in
/// `AppState`; no error is returned.
#[tauri::command]
pub async fn get_harness_status(
    state: State<'_, AppState>,
) -> Result<HarnessStatusResponse, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(HarnessStatusResponse {
            harness_enabled: false,
            active_recordings: vec![],
        });
    };

    let active_recordings = harness.recorder.active_sessions().await;

    Ok(HarnessStatusResponse {
        harness_enabled: true,
        active_recordings,
    })
}

/// Start recording agent events for `session_id` to a JSONL trace file.
///
/// Returns `Ok(())` if recording was started (or was already active).
/// Returns `Err` if the harness is not initialised or the trace file cannot be
/// created.
#[tauri::command]
pub async fn start_harness_recording(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let harness = state
        .harness
        .as_ref()
        .ok_or_else(|| "harness not initialised".to_string())?;

    harness
        .recorder
        .start(&session_id, &harness.event_bus)
        .await
        .map_err(|e| format!("failed to start recording for session {session_id}: {e}"))
}

/// Stop recording agent events for `session_id`.
///
/// The JSONL trace file is flushed and closed. Returns `Ok(())` regardless of
/// whether recording was active.
#[tauri::command]
pub async fn stop_harness_recording(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(());
    };

    harness.recorder.stop(&session_id).await;
    Ok(())
}

/// Return telemetry for a specific session.
///
/// `found` is `false` when the harness is not initialised or no events have
/// been recorded for the session yet.
#[tauri::command]
pub async fn get_session_telemetry(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<HarnessTelemetryResponse, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(HarnessTelemetryResponse {
            found: false,
            telemetry: None,
        });
    };

    let telemetry = harness.telemetry.snapshot(&session_id).await;
    Ok(HarnessTelemetryResponse {
        found: telemetry.is_some(),
        telemetry,
    })
}

/// Phase M4-C closeout — finalise the live `TraceAggregator` and
/// return the current [`HarnessRunReport`] snapshot.  Does NOT
/// rotate state — repeated calls return increasingly fuller
/// snapshots of the same run.  Use [`harness_begin_run`] to
/// rotate; [`harness_finalize_and_rotate_run`] does both atomically.
///
/// Returns `Ok(None)` when the harness is not initialised
/// (production default) — callers MUST treat `None` as "harness
/// disabled", not as failure.
#[tauri::command]
pub async fn harness_finalize_run(
    state: State<'_, AppState>,
) -> Result<Option<HarnessRunReport>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(None);
    };
    Ok(Some(harness.trace_aggregator.finalize().await))
}

/// Phase M4-D — start a new run.  Discards any in-flight
/// aggregator state; callers that wanted to preserve the previous
/// run MUST have called [`harness_finalize_run`] (or
/// [`harness_finalize_and_rotate_run`]) first.
///
/// `label` is an optional human-readable tag stored on the new
/// report (corpus task id + tag).  When `run_id` is `None` the
/// backend generates a fresh UUID.
///
/// Returns the new run id (so the caller can correlate later
/// finalise calls).  Returns `Ok(None)` when harness is disabled.
#[tauri::command]
pub async fn harness_begin_run(
    state: State<'_, AppState>,
    run_id: Option<String>,
    label: Option<String>,
) -> Result<Option<String>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(None);
    };
    let id = run_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    harness.trace_aggregator.begin_run(id.clone(), label).await;
    Ok(Some(id))
}

/// Phase M4-D — atomic finalise + rotate.  Returns the snapshot
/// of the **previous** run (frozen at this call) and starts a
/// fresh run identified by `next_run_id` (or a new UUID when
/// `None`).  Implementation is locked under one write so EventBus
/// folding cannot interleave between snapshot + rotation.
///
/// Phase M4.6 — finalised report is **auto-saved** to the
/// durable report store before returning, so the M4.5 compare
/// flow / M4.8 gate / future review surface can re-load it
/// without the caller having to persist.  Save failure is
/// logged at WARN but does NOT fail the IPC (rotation still
/// happens; caller still gets the in-memory snapshot).
///
/// Returns `Ok(None)` when harness is disabled.
#[tauri::command]
pub async fn harness_finalize_and_rotate_run(
    state: State<'_, AppState>,
    next_run_id: Option<String>,
    label: Option<String>,
) -> Result<Option<HarnessRunReport>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(None);
    };
    let next = next_run_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let report = harness
        .trace_aggregator
        .finalize_and_reset_run(next, label)
        .await;
    // M4.6 auto-persist.  Best-effort: warn-and-continue on save
    // failure so the caller still receives the snapshot.
    if let Err(e) = harness.report_store.save(&report).await {
        tracing::warn!(
            target: "harness.persistence",
            run_id = %report.run_id,
            error = %e,
            "[harness_finalize_and_rotate_run] save failed (non-fatal)"
        );
    }
    Ok(Some(report))
}

/// Phase M4.6 — list every persisted run report as a lightweight
/// summary.  Sorted newest-first.  Skips corrupt files silently
/// (logged WARN by the store).  Returns `Ok(vec![])` when
/// harness is disabled or the store is empty.
#[tauri::command]
pub async fn harness_list_reports(
    state: State<'_, AppState>,
) -> Result<Vec<RunIndexEntry>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(Vec::new());
    };
    harness.report_store.list().await.map_err(|e| e.to_string())
}

/// Phase M4.6 — load one persisted report by `run_id`.  Returns
/// `Ok(None)` when the report does not exist (caller MUST
/// distinguish missing vs corrupt — corrupt parses surface as
/// `Err`).  Returns `Ok(None)` when harness is disabled.
#[tauri::command]
pub async fn harness_load_report(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<Option<HarnessRunReport>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(None);
    };
    harness
        .report_store
        .load(&run_id)
        .await
        .map_err(|e| e.to_string())
}

/// Phase M4.6 — delete one persisted report by `run_id`.  Returns
/// `Ok(false)` when the report did not exist (idempotent).
/// Returns `Ok(false)` when harness is disabled.
#[tauri::command]
pub async fn harness_delete_report(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<bool, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(false);
    };
    harness
        .report_store
        .delete(&run_id)
        .await
        .map_err(|e| e.to_string())
}

/// Phase M4.6 — explicitly save a caller-supplied report (e.g. one
/// loaded from external corpus, or a candidate report received
/// over IPC) into the durable store.  Returns the file path on
/// success; `Ok(None)` when harness is disabled.
#[tauri::command]
pub async fn harness_save_report(
    state: State<'_, AppState>,
    report: HarnessRunReport,
) -> Result<Option<String>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(None);
    };
    let path = harness
        .report_store
        .save(&report)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

/// Phase M4.7 — load a regression corpus from a YAML path.  Pure
/// IO + parse; does not store anything in `AppState`.  Caller
/// holds the returned [`RegressionCorpus`] for the duration of
/// suite aggregation (typically passed straight back to
/// [`harness_aggregate_suite_report`]).
///
/// Independent of harness initialisation — a corpus can be
/// loaded even when `harness=None` (the load is just a YAML
/// parse).
#[tauri::command]
pub async fn harness_load_corpus(yaml_path: String) -> Result<RegressionCorpus, String> {
    load_corpus_from_yaml(PathBuf::from(yaml_path))
        .await
        .map_err(|e| e.to_string())
}

/// Phase M4.7 — aggregate a suite report from a corpus + the
/// per-task `run_id`s the caller has run.  Run reports are
/// loaded from the durable report store; missing reports
/// classify their task as `Missing` (not an error).
///
/// `task_to_run_id` maps `task_id → run_id`.  Tasks not in the
/// map are classified `Missing`.  Run ids that don't exist on
/// disk are also classified `Missing` (load failure surfaces
/// inside the report, not as an IPC error).
///
/// Returns `Err` when harness is disabled (no store to load
/// from); use [`harness_aggregate_suite_report_inline`] (future
/// IPC) when the caller wants to supply reports directly.
#[tauri::command]
pub async fn harness_aggregate_suite_report(
    state: State<'_, AppState>,
    suite_id: String,
    corpus: RegressionCorpus,
    task_to_run_id: BTreeMap<String, String>,
) -> Result<SuiteReport, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Err("harness not initialised".to_string());
    };
    let started_at = chrono::Utc::now();
    let mut task_to_report: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
    for (task_id, run_id) in task_to_run_id {
        match harness.report_store.load(&run_id).await {
            Ok(Some(report)) => {
                task_to_report.insert(task_id, report);
            }
            Ok(None) => {
                tracing::warn!(
                    target: "harness.suite",
                    task_id = %task_id,
                    run_id = %run_id,
                    "[suite_aggregate] run report not found; task will be classified Missing"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "harness.suite",
                    task_id = %task_id,
                    run_id = %run_id,
                    error = %e,
                    "[suite_aggregate] run report load failed; task will be classified Missing"
                );
            }
        }
    }
    let ended_at = chrono::Utc::now();
    Ok(aggregate_suite_report(
        suite_id,
        &corpus,
        &task_to_report,
        started_at,
        ended_at,
    ))
}

/// Phase M4.8 — render an upgrade gate recommendation for one
/// baseline-vs-candidate compare result.  Pure compute, no
/// AppState side-effects.  Caller supplies the
/// [`BaselineVsCandidate`] (typically from
/// [`harness_compare_reports`]) and an optional [`GatePolicy`]
/// (defaults to [`GatePolicy::default_conservative`] when
/// `policy` is `None`).
///
/// Returns a typed [`Recommendation`] with `decision ∈ {Promote,
/// Hold, Reject}` and stable reason codes.  Per the M4 hard rule
/// "no harness recommendation = no promote", consumers MUST
/// treat any decision other than `Promote` as a refusal.
#[tauri::command]
pub async fn harness_evaluate_compare(
    diff: BaselineVsCandidate,
    policy: Option<GatePolicy>,
) -> Result<Recommendation, String> {
    let policy = policy.unwrap_or_default();
    Ok(gate_evaluate_compare(&diff, &policy))
}

/// Phase M4.8 — render an upgrade gate recommendation for a
/// suite-level aggregation.  Pure compute.  Same defaulting +
/// refusal semantics as [`harness_evaluate_compare`].
#[tauri::command]
pub async fn harness_evaluate_suite(
    suite: SuiteReport,
    policy: Option<GatePolicy>,
) -> Result<Recommendation, String> {
    let policy = policy.unwrap_or_default();
    Ok(gate_evaluate_suite(&suite, &policy))
}

/// Phase M4-D — read the current run id without rotating /
/// snapshotting.  Useful for callers that want to correlate
/// out-of-band events with the in-flight run.
#[tauri::command]
pub async fn harness_current_run_id(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(None);
    };
    Ok(Some(harness.trace_aggregator.current_run_id().await))
}

/// Phase M4-D — run the M4.5 baseline-vs-candidate compare on two
/// caller-supplied [`HarnessRunReport`] snapshots.  Pure compute
/// — no AppState side-effects, no harness lookup.  Caller is
/// responsible for sourcing the two reports (e.g. one from disk,
/// one from `harness_finalize_run`).
///
/// Returns `Err(String)` when version compatibility refuses the
/// compare; otherwise returns the typed [`BaselineVsCandidate`]
/// diff (camelCase wire shape via serde).
#[tauri::command]
pub async fn harness_compare_reports(
    baseline: HarnessRunReport,
    candidate: HarnessRunReport,
) -> Result<BaselineVsCandidate, String> {
    compare_reports(&baseline, &candidate).map_err(|e| e.to_string())
}

/// Return telemetry snapshots for all sessions seen since app start.
///
/// Returns an empty list when the harness is not initialised.
#[tauri::command]
pub async fn get_all_session_telemetry(
    state: State<'_, AppState>,
) -> Result<Vec<SessionTelemetry>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(vec![]);
    };

    Ok(harness.telemetry.all_snapshots().await)
}
