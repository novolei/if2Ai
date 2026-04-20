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
  CanonicalRuntimeEvent,
  RunProjection,
  RuntimeProjectionSnapshot,
  ToolCallProjection,
} from './types'
import { emptyProjectionSnapshot } from './types'

/** Cap on the rolling memory event ring so the snapshot never grows
 * unbounded.  Older events fall off the front. */
const MEMORY_RING_CAP = 64

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
    case 'stream_text_delta':
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        text: run.text + event.text,
        status: 'streaming',
      }))
    case 'stream_thinking_start':
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        thinkingStarted: true,
        status: 'streaming',
      }))
    case 'stream_thinking_delta':
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        thinking: run.thinking + event.thinking,
        thinkingStarted: true,
        status: 'streaming',
      }))
    case 'stream_tool_call_update':
      return mergeRun(prev, event.runId, event.receivedAt, (run) => {
        const existing = run.toolCalls[event.toolCallId]
        const next: ToolCallProjection = {
          toolCallId: event.toolCallId,
          toolName: event.toolName,
          status: event.status,
          toolArgs: event.toolArgs ?? existing?.toolArgs,
          toolResult: event.toolResult ?? existing?.toolResult,
          toolDurationMs: event.toolDurationMs ?? existing?.toolDurationMs,
          effectiveWorkdir: event.effectiveWorkdir ?? existing?.effectiveWorkdir,
          policyDecision: event.policyDecision ?? existing?.policyDecision,
          evidenceId: event.evidenceId ?? existing?.evidenceId,
          requestId: event.requestId ?? existing?.requestId,
          firstSeenAt: existing?.firstSeenAt ?? event.receivedAt,
          lastUpdatedAt: event.receivedAt,
        }
        return {
          ...run,
          toolCalls: { ...run.toolCalls, [event.toolCallId]: next },
        }
      })
    case 'stream_final_text_override':
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        text: event.text,
      }))
    case 'stream_complete':
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        status: deriveCompletionStatus(event.taskOutcome),
        taskOutcome: event.taskOutcome,
        degradedReason: event.degradedReason,
        resumeAvailable: event.resumeAvailable ?? false,
        resumeCursor: event.resumeCursor,
        contextBudgetUsage: event.contextBudgetUsage,
        memoryItems: event.memoryItems ?? run.memoryItems,
      })).pipe((snap) => {
        if (event.memoryItems && event.memoryItems.length > 0) {
          return {
            ...snap,
            memory: {
              ...snap.memory,
              lastRecallItems: event.memoryItems,
            },
          }
        }
        return snap
      })
    case 'stream_error':
      return mergeRun(prev, event.runId, event.receivedAt, (run) => ({
        ...run,
        status: 'failed',
        taskOutcome: event.taskOutcome ?? 'failed',
        degradedReason: event.degradedReason ?? event.reason,
        resumeAvailable: event.resumeAvailable ?? false,
        resumeCursor: event.resumeCursor,
      }))
    case 'permission_request':
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
      }
    case 'memory_event': {
      const ring = [...prev.memory.recentEvents, event.payload]
      const trimmed =
        ring.length > MEMORY_RING_CAP
          ? ring.slice(ring.length - MEMORY_RING_CAP)
          : ring
      return {
        ...prev,
        memory: { ...prev.memory, recentEvents: trimmed },
      }
    }
    case 'activation_snapshot':
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
      }
    case 'execution_mode_decision':
      return {
        ...prev,
        executionMode: {
          runId: event.runId,
          executionMode: event.executionMode,
          riskLevel: event.riskLevel,
          complexityLevel: event.complexityLevel,
          reasonCodes: event.reasonCodes,
          policyVersion: event.policyVersion,
          capturedAt: event.receivedAt,
        },
      }
    default: {
      // Exhaustiveness assertion. TS will flag a missing case at
      // compile time when a new kind is added in `./types`.
      const _exhaustive: never = event
      void _exhaustive
      return prev
    }
  }
}

/** Apply a list of events in order. Convenience for queue
 * subscribers and for unit-test event scripts. */
export function reduceRuntimeEventBatch(
  prev: RuntimeProjectionSnapshot,
  events: readonly CanonicalRuntimeEvent[],
): RuntimeProjectionSnapshot {
  let next = prev
  for (const event of events) {
    next = reduceRuntimeEvent(next, event)
  }
  return next
}

/** Re-export the empty-snapshot factory so consumers only need to
 * import this module. */
export { emptyProjectionSnapshot }

// ───────────────────────── Internal helpers ────────────────────────

/** Snapshot returned by mergeRun; supports a chainable .pipe()
 * for composing additional state mutations after a run-level edit
 * (e.g. mirroring `memoryItems` into `memory.lastRecallItems`). */
interface PipeSnapshot extends RuntimeProjectionSnapshot {
  pipe(fn: (snap: RuntimeProjectionSnapshot) => RuntimeProjectionSnapshot): PipeSnapshot
}

function mergeRun(
  prev: RuntimeProjectionSnapshot,
  runId: string,
  receivedAt: number,
  apply: (run: RunProjection) => RunProjection,
): PipeSnapshot {
  const existing: RunProjection =
    prev.runs[runId] ?? createEmptyRun(runId, receivedAt)
  const updated = { ...apply(existing), lastUpdatedAt: receivedAt }
  const next: RuntimeProjectionSnapshot = {
    ...prev,
    runs: { ...prev.runs, [runId]: updated },
  }
  return makePipeSnapshot(next)
}

function makePipeSnapshot(snap: RuntimeProjectionSnapshot): PipeSnapshot {
  const wrapped = snap as PipeSnapshot
  wrapped.pipe = function pipe(
    fn: (s: RuntimeProjectionSnapshot) => RuntimeProjectionSnapshot,
  ): PipeSnapshot {
    return makePipeSnapshot(fn(wrapped))
  }
  return wrapped
}

function createEmptyRun(runId: string, receivedAt: number): RunProjection {
  return {
    runId,
    text: '',
    thinking: '',
    thinkingStarted: false,
    status: 'streaming',
    resumeAvailable: false,
    toolCalls: {},
    memoryItems: [],
    lastUpdatedAt: receivedAt,
  }
}

function deriveCompletionStatus(
  outcome: CanonicalRuntimeEvent extends { taskOutcome?: infer T } ? T : never,
): RunProjection['status'] {
  if (outcome === 'failed') return 'failed'
  return 'completed'
}
