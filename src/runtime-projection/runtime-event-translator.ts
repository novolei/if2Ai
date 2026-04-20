// Runtime event translator (Phase M2.2).
//
// Single canonical entry point for normalising backend wire payloads
// into the frontend-side `CanonicalRuntimeEvent` family defined in
// `./types.ts`.  Page components MUST NOT translate payloads
// themselves; if a new event source needs to feed the projection
// pipeline, add a `translate*` function here.
//
// Hard rules:
// 1. Translator does not touch React state.
// 2. Translator does not run reducer logic.
// 3. Translator returns `null` on inputs that are not yet wired
//    (rather than throwing) so wiring slices can land progressively.
// 4. Translator preserves field semantics; renaming + grouping only.

import type {
  ActivationSnapshot,
  ExecutionModeDecision,
  MemoryEventPayload,
  PermissionRequestPayload,
  StreamTokenPayload,
} from '@/transport/contracts'

import type {
  ActivationSnapshotEvent,
  CanonicalRuntimeEvent,
  ExecutionModeDecisionEvent,
  MemoryLifecycleEvent,
  PermissionRequestEvent,
} from './types'

function nowMs(): number {
  return Date.now()
}

/**
 * Translate one `agent-token` payload into a canonical event.
 * Returns `null` for unknown `event_type` values so future backend
 * additions do not crash the pipeline.
 */
export function translateAgentTokenPayload(
  payload: StreamTokenPayload,
): CanonicalRuntimeEvent | null {
  const runId = payload.stream_id
  const receivedAt = nowMs()

  switch (payload.event_type) {
    case 'text_delta':
      return {
        kind: 'stream_text_delta',
        runId,
        text: payload.text ?? '',
        receivedAt,
      }
    case 'thinking_start':
      return { kind: 'stream_thinking_start', runId, receivedAt }
    case 'thinking_delta':
      return {
        kind: 'stream_thinking_delta',
        runId,
        thinking: payload.thinking ?? '',
        receivedAt,
      }
    case 'tool_call_update': {
      // Defensive: backend always sets these on tool_call_update,
      // but stay null-safe so the translator does not throw.
      if (!payload.tool_call_id || !payload.tool_name || !payload.tool_status) {
        return null
      }
      return {
        kind: 'stream_tool_call_update',
        runId,
        toolCallId: payload.tool_call_id,
        toolName: payload.tool_name,
        status: payload.tool_status,
        toolArgs: payload.tool_args,
        toolResult: payload.tool_result,
        toolDurationMs: payload.tool_duration_ms,
        effectiveWorkdir: payload.effective_workdir,
        policyDecision: payload.policy_decision,
        evidenceId: payload.evidence_id,
        requestId: payload.request_id,
        receivedAt,
      }
    }
    case 'final_text_override':
      return {
        kind: 'stream_final_text_override',
        runId,
        text: payload.text ?? '',
        requestId: payload.request_id,
        receivedAt,
      }
    case 'stream_complete':
      return {
        kind: 'stream_complete',
        runId,
        taskOutcome: payload.task_outcome,
        degradedReason: payload.degraded_reason,
        resumeAvailable: payload.resume_available,
        resumeCursor: payload.resume_cursor,
        requestId: payload.request_id,
        contextBudgetUsage: payload.context_budget_usage,
        memoryItems: payload.memory_context,
        receivedAt,
      }
    case 'stream_error':
      return {
        kind: 'stream_error',
        runId,
        reason: payload.tool_result ?? payload.degraded_reason ?? 'unknown stream error',
        taskOutcome: payload.task_outcome,
        degradedReason: payload.degraded_reason,
        resumeAvailable: payload.resume_available,
        resumeCursor: payload.resume_cursor,
        requestId: payload.request_id,
        receivedAt,
      }
    default: {
      // Exhaustiveness assertion. The cast preserves type safety
      // when a new variant is added — TS will flag the missing
      // case at compile time.
      const _exhaustive: never = payload.event_type
      void _exhaustive
      return null
    }
  }
}

/**
 * Translate one `permission-request` payload into a canonical
 * `PermissionRequestEvent`.
 */
export function translatePermissionRequestPayload(
  payload: PermissionRequestPayload,
): PermissionRequestEvent {
  return {
    kind: 'permission_request',
    sessionId: payload.session_id,
    toolName: payload.tool_name,
    permissionMode: payload.permission_mode,
    currentMode: payload.current_mode,
    message: payload.message,
    receivedAt: nowMs(),
  }
}

/**
 * Wrap one `memory_event` payload as a canonical
 * `MemoryLifecycleEvent`.  The backend taxonomy is preserved
 * verbatim because the list itself is open per phase.
 */
export function translateMemoryEventPayload(
  payload: MemoryEventPayload,
): MemoryLifecycleEvent {
  return {
    kind: 'memory_event',
    payload,
    receivedAt: nowMs(),
  }
}

/**
 * Phase M2.5 — translate one [`ActivationSnapshot`] (returned by
 * the new `activation_get_status` IPC command) into a canonical
 * [`ActivationSnapshotEvent`].
 *
 * NOTE — this is currently consumed via a **fetch seam**, not an
 * event stream: the bridge calls `activation_get_status()` on
 * boot (and on demand) and dispatches the translated event into
 * the projection store.  When the backend grows a real
 * `activation_status_changed` Tauri event, the bridge can swap the
 * fetch for a `listen(...)` call without touching the translator
 * or the reducer.
 */
export function translateActivationSnapshot(
  payload: ActivationSnapshot,
): ActivationSnapshotEvent {
  return {
    kind: 'activation_snapshot',
    statusKind: payload.status.kind,
    allowsMainShell: payload.allowsMainShell,
    receivedAt: nowMs(),
  }
}

/**
 * Phase M2.6 — translate one [`ExecutionModeDecision`] (returned by
 * the new `request_intelligence_classify` IPC command) into a
 * canonical [`ExecutionModeDecisionEvent`].
 *
 * NOTE — this is currently consumed via a **fetch seam**, not an
 * event stream: the bridge calls `requestIntelligenceClassify(...)`
 * on demand (e.g. when the user types in the chat input) and
 * dispatches the translated event into the projection store.  When
 * the backend grows a real `execution_mode_decision_emitted` Tauri
 * event tied to actual `start_agent_stream` calls, the bridge can
 * swap the fetch for a `listen(...)` call without touching the
 * translator or the reducer.
 *
 * `runId` is optional because the standalone classifier IPC does
 * not yet correlate to a `RuntimeEventEnvelope.run_id` (M2.7+).
 */
export function translateExecutionModeDecision(
  payload: ExecutionModeDecision,
  runId?: string,
): ExecutionModeDecisionEvent {
  return {
    kind: 'execution_mode_decision',
    runId,
    executionMode: payload.executionMode,
    riskLevel: payload.riskLevel,
    complexityLevel: payload.complexityLevel,
    reasonCodes: payload.reasonCodes ?? [],
    policyVersion: payload.classifierPolicyVersion,
    receivedAt: nowMs(),
  }
}
