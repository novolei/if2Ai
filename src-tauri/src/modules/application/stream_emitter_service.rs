//! Stream emitter service — orchestrates the post-turn memory write
//! pipeline and dispatches its `MemoryAfterTurn` envelope to both
//! transports:
//!   1. Frontend — `MEMORY_AFTER_TURN_EVENT` Tauri channel (drives
//!      runtime-projection store).
//!   2. Harness — `AgentEvent::MemoryAfterTurn` on the EventBus
//!      (drives M4 trace sinks / future grader components).
//!
//! Extracted from `commands/agent.rs::dispatch_after_turn` in GFR-004
//! (pure structural move, function body byte-identical).

use tauri::{AppHandle, Emitter};

use crate::modules::application::memory_injection_service::MemoryInjectionDeps;
use crate::modules::application::{AfterTurnInput, ExistingRecordRef, MemoryCoordinator};
use crate::modules::harness::{AgentEvent, EventBus};
use crate::modules::learning::reflection_note::ReflectionNote;
use crate::modules::runtime::contracts::memory::MemoryWriteCandidate;
use crate::modules::runtime::stream_emitter::MEMORY_AFTER_TURN_EVENT;

/// Frozen schema marker for the
/// [`MemoryCoordinator::after_turn`] batch envelope shape (frontend
/// transport channel + harness EventBus). Bumping this is a
/// breaking governance contract change; future graders / replay
/// MUST honor it.
pub const MEMORY_AFTER_TURN_TRACE_VERSION: &str = "memory-after-turn-trace@m4.1";

/// Phase M3-C closeout (extended in M4.1) — run the
/// [`MemoryCoordinator::after_turn`] write-policy / quality-gate /
/// conflict-resolver pipeline at the end of a turn and emit the
/// **batch envelope** through both:
///
///   1. the frontend [`MEMORY_AFTER_TURN_EVENT`] Tauri channel
///      (drives the runtime-projection store), and
///   2. the harness [`EventBus`] as
///      [`AgentEvent::MemoryAfterTurn`] (drives M4 trace sinks /
///      future grader components).
///
/// M4.1 — `candidates` is now sourced from
/// [`extract_memory_store_tool_candidates`] for `memory_store`
/// tool calls observed during the turn; `existing_records` is now
/// sourced from
/// [`lookup_existing_records_for_candidates`] so the conflict
/// resolver flips from "always NoConflict" to producing real
/// outcomes for same-key writes.  The batch envelope still fires
/// even when both arrays are empty — the empty case is the
/// explicit "no candidates this turn" signal (M3-C contract).
///
/// Emit failure on either channel is logged at TRACE — never
/// blocks the turn.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_after_turn(
    app_handle: &AppHandle,
    harness_bus: Option<&EventBus>,
    injection_deps: MemoryInjectionDeps,
    session_id: Option<String>,
    project_id: Option<String>,
    candidates: Vec<MemoryWriteCandidate>,
    existing_records: Vec<Option<ExistingRecordRef>>,
    reflection_notes: Vec<ReflectionNote>,
    caller: &'static str,
) {
    debug_assert_eq!(
        candidates.len(),
        existing_records.len(),
        "candidate / existing_record arrays must be parallel"
    );
    let coordinator = MemoryCoordinator::with_default_policy(injection_deps);
    let output = coordinator.after_turn(AfterTurnInput {
        session_id: session_id.clone(),
        project_id: project_id.clone(),
        candidates,
        existing_records,
        reflection_notes,
        caller,
    });
    // RFC3339 timestamp for the batch envelope; per-decision
    // `decidedAt` lives inside each `MemoryWriteDecision`.
    let decided_at_dt = chrono::Utc::now();
    let decided_at_rfc = decided_at_dt.to_rfc3339();

    // (1) Frontend transport channel.
    let payload = serde_json::json!({
        "traceVersion": MEMORY_AFTER_TURN_TRACE_VERSION,
        "caller": caller,
        "policyVersion": output.policy_version,
        "decidedAt": decided_at_rfc,
        "decisions": output.decisions,
        "quality": output.quality,
        "conflicts": output.conflicts,
    });
    if let Err(err) = app_handle.emit(MEMORY_AFTER_TURN_EVENT, payload) {
        tracing::trace!(
            event = MEMORY_AFTER_TURN_EVENT,
            error = %err,
            "[after_turn] memory_after_turn emit failed (non-fatal)"
        );
    }

    // (2) Harness EventBus — M4.2 ground-truth seam.  Zero
    // overhead when `harness_bus` is `None` (no bus subscribed).
    if let Some(bus) = harness_bus {
        let event = AgentEvent::MemoryAfterTurn {
            trace_version: MEMORY_AFTER_TURN_TRACE_VERSION,
            caller,
            session_id,
            project_id,
            policy_version: output.policy_version,
            decided_at: decided_at_dt,
            decisions: output.decisions,
            quality: output.quality,
            conflicts: output.conflicts,
        };
        if let Err(err) = bus.emit(event) {
            tracing::trace!(
                error = %err,
                "[after_turn] harness MemoryAfterTurn emit failed (non-fatal)"
            );
        }
    }
}
