// Runtime event reducer (Phase M2.3).
//
// Pure folder over `CanonicalRuntimeEvent` → `RuntimeProjectionSnapshot`.
// Produces immutable snapshots so future M2.4 stores can subscribe
// against shallow-equality.
//
// Hard rules (mirror file-level plan §5.6):
// 1. Reducer only projects state. It does not invent business
//    truth (no execution-mode inference, no activation backfill).
// 2. Reducer does not touch localStorage / IndexedDB / network.
// 3. Reducer does not depend on React.
// 4. Reducer is pure: same `(prev, event)` → same `next`.
//
// Out of scope:
// - `project / session` CRUD state (still owned by `App.tsx`
//   useState until M2.4+).
// - User preferences (font / density / model picker — owned by
//   `src/stores/*` until the M2.x preferences slice).

import type {
  AttemptTimelineProjection,
  CanonicalRuntimeEvent,
  RunProjection,
  RuntimeProjectionSnapshot,
  ToolCallProjection,
} from "./types.ts";
import { emptyProjectionSnapshot } from "./types.ts";

/** Cap on the rolling memory event ring so the snapshot never grows
 * unbounded.  Older events fall off the front. */
const MEMORY_RING_CAP = 64;

/** Phase M3.6 — cap on the rolling memory write-decision ring. */
const MEMORY_DECISION_RING_CAP = 32;

/**
 * Apply one event to the previous snapshot. Returns a new snapshot
 * (structural copy at every touched node — leaves are reference-
 * equal when unchanged).
 */
export function reduceRuntimeEvent(
  prev: RuntimeProjectionSnapshot,
  event: CanonicalRuntimeEvent,
): RuntimeProjectionSnapshot {
  switch (event.kind) {
    case "stream_run_bound":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        sessionId: event.sessionId,
      }));
    case "stream_text_delta":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        text: run.text + event.text,
        status: "streaming",
      }));
    case "stream_thinking_start":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        thinkingStarted: true,
        status: "streaming",
      }));
    case "stream_thinking_delta":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        thinking: run.thinking + event.thinking,
        thinkingStarted: true,
        status: "streaming",
      }));
    case "stream_tool_call_update":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => {
        const existing = run.toolCalls[event.toolCallId];
        const next: ToolCallProjection = {
          toolCallId: event.toolCallId,
          toolName: event.toolName,
          status: event.status,
          toolArgs: event.toolArgs ?? existing?.toolArgs,
          toolResult: event.toolResult ?? existing?.toolResult,
          toolDurationMs: event.toolDurationMs ?? existing?.toolDurationMs,
          effectiveWorkdir:
            event.effectiveWorkdir ?? existing?.effectiveWorkdir,
          policyDecision: event.policyDecision ?? existing?.policyDecision,
          evidenceId: event.evidenceId ?? existing?.evidenceId,
          requestId: event.requestId ?? existing?.requestId,
          attemptId: event.attemptId ?? existing?.attemptId,
          attemptNo: existing?.attemptNo,
          failureKind: existing?.failureKind ?? null,
          attemptHistory: existing?.attemptHistory ?? [],
          firstSeenAt: existing?.firstSeenAt ?? event.receivedAt,
          lastUpdatedAt: event.receivedAt,
        };
        return {
          ...run,
          toolCalls: { ...run.toolCalls, [event.toolCallId]: next },
        };
      });
    case "stream_final_text_override":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        text: event.text,
      }));
    case "skill_resolution_snapshot":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        skillResolution: event.plan,
      }));
    case "final_run_report":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        finalRunReport: event.report,
        status:
          event.report.taskOutcome === "failed"
            ? "failed"
            : run.status === "streaming"
              ? "completed"
              : run.status,
        taskOutcome:
          event.report.taskOutcome === "completed" ||
          event.report.taskOutcome === "partial_success" ||
          event.report.taskOutcome === "failed"
            ? event.report.taskOutcome
            : run.taskOutcome,
        resumeAvailable: event.report.resumeAvailable,
        resumeCursor: event.report.resumeCursor ?? run.resumeCursor,
      }));
    case "stream_complete":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        status: deriveCompletionStatus(event.taskOutcome),
        taskOutcome: event.taskOutcome,
        degradedReason: event.degradedReason,
        resumeAvailable: event.resumeAvailable ?? false,
        resumeCursor: event.resumeCursor,
        recoverability: event.recoverability,
        contextBudgetUsage: event.contextBudgetUsage,
        turnCost: event.turnCost ?? run.turnCost,
        routing: event.routing ?? run.routing,
        sessionTotals: event.sessionTotals ?? run.sessionTotals,
        memoryItems: event.memoryItems ?? run.memoryItems,
        promptDiagnostics: event.promptDiagnostics ?? run.promptDiagnostics,
      })).pipe((snap) => {
        if (event.memoryItems && event.memoryItems.length > 0) {
          return {
            ...snap,
            memory: {
              ...snap.memory,
              lastRecallItems: event.memoryItems,
            },
          };
        }
        return snap;
      });
    case "stream_error":
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        status: "failed",
        taskOutcome: event.taskOutcome ?? "failed",
        degradedReason: event.degradedReason ?? event.reason,
        resumeAvailable: event.resumeAvailable ?? false,
        resumeCursor: event.resumeCursor,
        recoverability: event.recoverability,
      }));
    case "permission_request":
      return {
        ...prev,
        approvals: {
          ...prev.approvals,
          [event.sessionId]: {
            sessionId: event.sessionId,
            toolName: event.toolName,
            permissionMode: event.permissionMode,
            currentMode: event.currentMode,
            message: event.message,
            receivedAt: event.receivedAt,
          },
        },
      };
    case "permission_resolved": {
      // Strip the resolved approval; the dialog has handed the
      // decision back to the backend via `respondPermission`.
      if (!(event.sessionId in prev.approvals)) {
        return prev;
      }
      const nextApprovals: typeof prev.approvals = {};
      for (const [key, val] of Object.entries(prev.approvals)) {
        if (key !== event.sessionId) nextApprovals[key] = val;
      }
      return { ...prev, approvals: nextApprovals };
    }
    case "memory_event": {
      const ring = [...prev.memory.recentEvents, event.payload];
      const trimmed =
        ring.length > MEMORY_RING_CAP
          ? ring.slice(ring.length - MEMORY_RING_CAP)
          : ring;
      // Memory Audit P1 #5 — `memory_invalidated` is a meta-event
      // (carries no audit content; just a refetch hint).  Bump the
      // monotonic version + record the scope hint so React selectors
      // keyed on `memory.invalidationVersion` re-run their fetch.
      const isInvalidation = event.payload.event === "memory_invalidated";
      const nextScope = isInvalidation
        ? (typeof event.payload.extra?.scope_kind === "string"
            ? event.payload.extra.scope_kind
            : "entries")
        : prev.memory.lastInvalidationScope;
      return {
        ...prev,
        memory: {
          ...prev.memory,
          recentEvents: trimmed,
          invalidationVersion: isInvalidation
            ? prev.memory.invalidationVersion + 1
            : prev.memory.invalidationVersion,
          lastInvalidationScope: nextScope,
        },
      };
    }
    case "memory_after_turn": {
      // Phase M3-C closeout — latest-wins projection of the
      // backend batch envelope.  Fires once per turn end, even
      // when the batch is empty (closes the M3-B "empty batch is
      // unobservable" audit gap).  Preserves the full
      // `quality` / `conflicts` arrays so consumers can render
      // gate / resolver detail without re-fetching.
      const acceptedCount = event.quality.accepted.length;
      const rejectedCount = event.quality.rejected.length;
      const warningCount = event.quality.warnings.length;
      return {
        ...prev,
        memory: {
          ...prev.memory,
          lastAfterTurn: {
            traceVersion: event.traceVersion,
            caller: event.caller,
            policyVersion: event.policyVersion,
            decidedAt: event.decidedAt,
            decisionCount: event.decisions.length,
            acceptedCount,
            rejectedCount,
            warningCount,
            conflictsCount: event.conflicts.length,
            quality: event.quality,
            conflicts: event.conflicts,
            receivedAt: event.receivedAt,
          },
        },
      };
    }
    case "memory_write_decision": {
      // Phase M3.6 — rolling ring of typed write decisions.
      // Capped at MEMORY_DECISION_RING_CAP; oldest decisions fall
      // off the front so the snapshot stays bounded.  No backend
      // event source dispatches this today (M3-B+ wiring); the
      // branch exists so the seam is end-to-end.
      const projection = {
        candidateId: event.candidateId,
        decision: event.payload,
        receivedAt: event.receivedAt,
      };
      const ring = [...prev.memory.writeDecisions, projection];
      const trimmed =
        ring.length > MEMORY_DECISION_RING_CAP
          ? ring.slice(ring.length - MEMORY_DECISION_RING_CAP)
          : ring;
      return {
        ...prev,
        memory: { ...prev.memory, writeDecisions: trimmed },
      };
    }
    case "activation_snapshot":
      // Honest projection: only what the canonical event carries.
      // No backfill of license metadata that the backend has not
      // produced.  When `LicenseLifecycleService` becomes a real
      // source, this branch already accepts richer events without
      // a contract bump.
      return {
        ...prev,
        activation: {
          statusKind: event.statusKind,
          allowsMainShell: event.allowsMainShell,
          capturedAt: event.receivedAt,
        },
      };
    case "execution_mode_decision": {
      const base = event.runId && event.workLoop
        ? mergeRun(prev, event.runId, event.receivedAt, (run) => ({
            ...run,
            workLoop: event.workLoop,
          }))
        : prev;
      return {
        ...base,
        executionMode: {
          runId: event.runId,
          executionMode: event.executionMode,
          riskLevel: event.riskLevel,
          complexityLevel: event.complexityLevel,
          reasonCodes: event.reasonCodes,
          scenarioProfileHint: event.scenarioProfileHint,
          matchedRules: event.matchedRules,
          slotSummary: event.slotSummary,
          ambiguousEscalated: event.ambiguousEscalated,
          escalationSource: event.escalationSource,
          // Preserve any prior manual override the user already
          // dispatched — the classifier judgment refresh does not
          // clear the override.  Override is cleared explicitly via
          // `execution_mode_manual_override { override: null }`.
          manualOverride: prev.executionMode?.manualOverride ?? null,
          policyVersion: event.policyVersion,
          workLoop: event.workLoop,
          capturedAt: event.receivedAt,
          lastUpdatedAt: event.receivedAt,
        },
      };
    }
    case "projection_discard_session_runs": {
      const sid = event.sessionId;
      const runs = { ...prev.runs };
      for (const [runId, run] of Object.entries(runs)) {
        if (run.sessionId === sid) {
          delete runs[runId];
        }
      }
      const approvals = { ...prev.approvals };
      delete approvals[sid];
      return { ...prev, runs, approvals };
    }
    case "execution_mode_manual_override": {
      // Manual override may arrive before any classifier judgment
      // (e.g. user picks a mode pre-classification).  In that case
      // we synthesise an empty projection so the override is
      // visible immediately; classifier-driven fields stay zero
      // until the next decision arrives.
      const base = prev.executionMode;
      if (!base) {
        if (event.override === null) return prev;
        return {
          ...prev,
          executionMode: {
            executionMode: event.override,
            riskLevel: "low",
            complexityLevel: "trivial",
            reasonCodes: [],
            scenarioProfileHint: undefined,
            matchedRules: [],
            slotSummary: null,
            ambiguousEscalated: false,
            manualOverride: event.override,
            policyVersion: "",
            workLoop: undefined,
            capturedAt: event.receivedAt,
            lastUpdatedAt: event.receivedAt,
          },
        };
      }
      return {
        ...prev,
        executionMode: {
          ...base,
          manualOverride: event.override,
          lastUpdatedAt: event.receivedAt,
        },
      };
    }
    case "supervisor_snapshot":
      // T-007 — latest-wins supervisor projection. Full replacement
      // per session so consumers always see the most recent backend
      // snapshot without needing to merge per-field deltas.
      return {
        ...prev,
        supervisor: {
          sessionId: event.sessionId,
          status: event.status,
          activeRunId: event.activeRunId,
          activeRunStatus: event.activeRunStatus,
          pendingPermissionCount: event.pendingPermissionCount,
          lastErrorKind: event.lastErrorKind,
          recoverable: event.recoverable,
          retryBudgetRemaining: event.retryBudgetRemaining,
          disconnectGraceUntil: event.disconnectGraceUntil,
          lastUpdatedAt: event.lastUpdatedAt,
          capturedAt: event.receivedAt,
        },
      };
    case "smart_browser_projection":
      return {
        ...prev,
        browsers: {
          ...prev.browsers,
          [event.smartBrowserSessionId]: {
            sessionId: event.sessionId,
            smartBrowserSessionId: event.smartBrowserSessionId,
            backend: event.backend,
            running: event.running,
            url: event.url,
            title: event.title,
            thumbnail: event.thumbnail,
            takenOver: event.takenOver,
            lastAction: event.lastAction,
            diagnostics: event.diagnostics,
            escalationState: event.escalationState,
            lastUpdatedAt: event.receivedAt,
          },
        },
      };
    case "tool_attempt_timeline_snapshot": {
      const timeline: AttemptTimelineProjection = {
        sessionId: event.sessionId,
        byToolCallId: {},
        byAttemptId: {},
        stats: event.stats,
        lastFetchedAt: event.receivedAt,
      };
      for (const a of event.attempts) {
        (timeline.byToolCallId[a.toolCallId] ??= []).push(a);
        timeline.byAttemptId[a.attemptId] = a;
      }
      // Merge attempt history into existing tool-call projections
      // so consumers can render per-tool-call attempt timelines
      // without traversing the timeline index.
      const nextRuns = { ...prev.runs };
      for (const [runId, run] of Object.entries(nextRuns)) {
        const updated = { ...run.toolCalls };
        let touched = false;
        for (const [tcId, tc] of Object.entries(updated)) {
          const hist = timeline.byToolCallId[tcId] ?? [];
          if (hist.length > 0) {
            updated[tcId] = {
              ...tc,
              attemptHistory: hist,
              attemptId: hist[hist.length - 1]?.attemptId,
              attemptNo: hist[hist.length - 1]?.attemptNo,
              failureKind: hist[hist.length - 1]?.failureKind,
            };
            touched = true;
          }
        }
        if (touched) {
          nextRuns[runId] = { ...run, toolCalls: updated };
        }
      }
      return {
        ...prev,
        runs: nextRuns,
        attemptTimeline: timeline,
      };
    }
    default: {
      // Exhaustiveness assertion. TS will flag a missing case at
      // compile time when a new kind is added in `./types`.
      const _exhaustive: never = event;
      void _exhaustive;
      return prev;
    }
  }
}

/** Apply a list of events in order. Convenience for queue
 * subscribers and for unit-test event scripts. */
export function reduceRuntimeEventBatch(
  prev: RuntimeProjectionSnapshot,
  events: readonly CanonicalRuntimeEvent[],
): RuntimeProjectionSnapshot {
  let next = prev;
  for (const event of events) {
    if (shouldDropForSessionGuard(next, event)) {
      // ER-03 — quiet diagnostic; never warn/error so a brisk
      // session-switch storm does not flood the console.
      // eslint-disable-next-line no-console
      console.debug(
        "[runtime-event-reducer] drop mismatched-session event",
        { kind: event.kind, currentSessionId: next.currentSessionId },
      );
      continue;
    }
    next = reduceRuntimeEvent(next, event);
  }
  return next;
}

/**
 * ER-03 — predicate used by [`reduceRuntimeEventBatch`] to drop
 * events that belong to a session other than the one the snapshot
 * is currently scoped to (set via
 * [`RuntimeProjectionStore.swapSession`]).
 *
 * The runtime event family is heterogeneous: some kinds carry an
 * explicit `sessionId`, others only a `runId`.  For run-only events
 * we look up the run's bound session id (recorded by
 * `stream_run_bound`).  Events that carry no session id and no
 * known run binding fall through unchanged — there is no ground for
 * a guard decision and the legacy behavior is "accept".
 */
function shouldDropForSessionGuard(
  snapshot: RuntimeProjectionSnapshot,
  event: CanonicalRuntimeEvent,
): boolean {
  const current = snapshot.currentSessionId;
  if (current == null) return false;
  // Direct session-id fields — drop on mismatch.
  if ("sessionId" in event && typeof event.sessionId === "string") {
    return event.sessionId !== current;
  }
  // Run-bound events: resolve via the run projection populated by
  // `stream_run_bound`. If the run is known and bound to a different
  // session, drop. If it is bound to the current session, accept.
  // If the run is unknown (e.g. its `stream_run_bound` was wiped by
  // `swapSession`), drop as well — a known-bound run is the only
  // safe attribution path while a session guard is active.
  if ("runId" in event && typeof event.runId === "string") {
    const run = snapshot.runs[event.runId];
    if (run?.sessionId != null) return run.sessionId !== current;
    return true;
  }
  return false;
}

/** Re-export the empty-snapshot factory so consumers only need to
 * import this module. */
export { emptyProjectionSnapshot };

// ───────────────────────── Internal helpers ────────────────────────

/** Snapshot returned by mergeRun; supports a chainable .pipe()
 * for composing additional state mutations after a run-level edit
 * (e.g. mirroring `memoryItems` into `memory.lastRecallItems`). */
interface PipeSnapshot extends RuntimeProjectionSnapshot {
  pipe(
    fn: (snap: RuntimeProjectionSnapshot) => RuntimeProjectionSnapshot,
  ): PipeSnapshot;
}

function mergeRun(
  prev: RuntimeProjectionSnapshot,
  runId: string,
  receivedAt: number,
  apply: (run: RunProjection) => RunProjection,
): PipeSnapshot {
  const existing: RunProjection =
    prev.runs[runId] ?? createEmptyRun(runId, receivedAt);
  const updated = { ...apply(existing), lastUpdatedAt: receivedAt };
  const next: RuntimeProjectionSnapshot = {
    ...prev,
    runs: { ...prev.runs, [runId]: updated },
  };
  return makePipeSnapshot(next);
}

function makePipeSnapshot(snap: RuntimeProjectionSnapshot): PipeSnapshot {
  const wrapped = snap as PipeSnapshot;
  wrapped.pipe = function pipe(
    fn: (s: RuntimeProjectionSnapshot) => RuntimeProjectionSnapshot,
  ): PipeSnapshot {
    return makePipeSnapshot(fn(wrapped));
  };
  return wrapped;
}

function createEmptyRun(runId: string, receivedAt: number): RunProjection {
  return {
    runId,
    text: "",
    thinking: "",
    thinkingStarted: false,
    status: "streaming",
    resumeAvailable: false,
    toolCalls: {},
    memoryItems: [],
    lastUpdatedAt: receivedAt,
  };
}

function deriveCompletionStatus(
  outcome: RunProjection["taskOutcome"],
): RunProjection["status"] {
  if (outcome === "failed") return "failed";
  return "completed";
}
