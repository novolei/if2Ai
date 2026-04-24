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
  MemoryAfterTurnPayload,
  MemoryEventPayload,
  MemoryWriteDecisionPayload,
  PermissionRequestPayload,
  StreamTokenPayload,
} from "@/transport/contracts";

import type {
  ActivationSnapshotEvent,
  CanonicalRuntimeEvent,
  ExecutionModeDecisionEvent,
  MemoryAfterTurnEvent,
  MemoryLifecycleEvent,
  MemoryWriteDecisionEvent,
  PermissionRequestEvent,
} from "./types.ts";
import { translatePromptDiagnosticsSummary } from "./types.ts";

function nowMs(): number {
  return Date.now();
}

/**
 * Translate one `agent-token` payload into a canonical event.
 * Returns `null` for unknown `event_type` values so future backend
 * additions do not crash the pipeline.
 */
export function translateAgentTokenPayload(
  payload: StreamTokenPayload,
): CanonicalRuntimeEvent | null {
  // T-002: Prefer correlation.runId when available (set by GAP-002
  // contract unification), fall back to legacy stream_id.
  const runId = payload.correlation?.runId ?? payload.stream_id;
  const receivedAt = nowMs();

  switch (payload.event_type) {
    case "text_delta":
      return {
        kind: "stream_text_delta",
        runId,
        text: payload.text ?? "",
        receivedAt,
      };
    case "thinking_start":
      return { kind: "stream_thinking_start", runId, receivedAt };
    case "thinking_delta":
      return {
        kind: "stream_thinking_delta",
        runId,
        thinking: payload.thinking ?? "",
        receivedAt,
      };
    case "tool_call_update": {
      // Defensive: backend always sets these on tool_call_update,
      // but stay null-safe so the translator does not throw.
      if (!payload.tool_call_id || !payload.tool_name || !payload.tool_status) {
        return null;
      }
      return {
        kind: "stream_tool_call_update",
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
      };
    }
    case "final_text_override":
      return {
        kind: "stream_final_text_override",
        runId,
        text: payload.text ?? "",
        requestId: payload.request_id,
        receivedAt,
      };
    case "stream_complete":
      return {
        kind: "stream_complete",
        runId,
        taskOutcome: payload.task_outcome,
        degradedReason: payload.degraded_reason,
        resumeAvailable: payload.resume_available,
        resumeCursor: payload.resume_cursor,
        requestId: payload.request_id,
        contextBudgetUsage: payload.context_budget_usage,
        memoryItems: payload.memory_context,
        promptDiagnostics: payload.prompt_diagnostics
          ? translatePromptDiagnosticsSummary(payload.prompt_diagnostics)
          : undefined,
        turnCost: payload.turn_cost
          ? {
              inputTokens: payload.turn_cost.input_tokens,
              outputTokens: payload.turn_cost.output_tokens,
              cacheCreationInputTokens:
                payload.turn_cost.cache_creation_input_tokens,
              cacheReadInputTokens: payload.turn_cost.cache_read_input_tokens,
              costUsd: payload.turn_cost.cost_usd,
              model: payload.turn_cost.model,
            }
          : undefined,
        routing: payload.routing_info
          ? {
              complexityScore: payload.routing_info.complexity_score,
              complexityLevel: payload.routing_info.complexity_level,
              executionMode: payload.routing_info.execution_mode,
              usedCheapModel: payload.routing_info.used_cheap_model,
              effectiveModel: payload.routing_info.effective_model,
            }
          : undefined,
        sessionTotals: payload.session_totals
          ? {
              inputTokens: payload.session_totals.input_tokens,
              outputTokens: payload.session_totals.output_tokens,
              cacheCreationInputTokens:
                payload.session_totals.cache_creation_input_tokens,
              cacheReadInputTokens:
                payload.session_totals.cache_read_input_tokens,
              costUsd: payload.session_totals.cost_usd,
              turns: payload.session_totals.turns,
            }
          : undefined,
        receivedAt,
      };
    case "stream_error":
      return {
        kind: "stream_error",
        runId,
        reason:
          payload.tool_result ??
          payload.degraded_reason ??
          "unknown stream error",
        taskOutcome: payload.task_outcome,
        degradedReason: payload.degraded_reason,
        resumeAvailable: payload.resume_available,
        resumeCursor: payload.resume_cursor,
        requestId: payload.request_id,
        receivedAt,
      };
    default: {
      // Exhaustiveness assertion. The cast preserves type safety
      // when a new variant is added — TS will flag the missing
      // case at compile time.
      const _exhaustive: never = payload.event_type;
      void _exhaustive;
      return null;
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
    kind: "permission_request",
    sessionId: payload.session_id,
    toolName: payload.tool_name,
    permissionMode: payload.permission_mode,
    currentMode: payload.current_mode,
    message: payload.message,
    receivedAt: nowMs(),
  };
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
    kind: "memory_event",
    payload,
    receivedAt: nowMs(),
  };
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
    kind: "activation_snapshot",
    statusKind: payload.status.kind,
    allowsMainShell: payload.allowsMainShell,
    receivedAt: nowMs(),
  };
}

/**
 * Phase M3.6 — translate one [`MemoryWriteDecisionPayload`]
 * (returned by the backend `MemoryCoordinator::after_turn`
 * write-policy gate) into a canonical
 * [`MemoryWriteDecisionEvent`].
 *
 * NOTE — this is a fetch / dispatch seam, not an event stream.
 * No backend event source emits memory write decisions to the
 * frontend today; the seam exists so M3-B+ persistence + audit
 * wiring can dispatch through the same translator without a
 * contract bump.  The caller supplies the `candidateId` so the
 * UI can correlate the decision with the originating candidate
 * card.
 */
export function translateMemoryWriteDecision(
  candidateId: string,
  payload: MemoryWriteDecisionPayload,
): MemoryWriteDecisionEvent {
  return {
    kind: "memory_write_decision",
    candidateId,
    payload,
    receivedAt: nowMs(),
  };
}

/**
 * Phase M3-C closeout — translate one [`MemoryAfterTurnPayload`]
 * (the batch envelope emitted by the backend
 * `MemoryCoordinator::after_turn` per turn) into a canonical
 * [`MemoryAfterTurnEvent`].
 *
 * The translator is field-preserving — it does NOT strip the
 * `quality` / `conflicts` arrays.  M4 governance / harness
 * consumers read the full result via the canonical event without
 * a transport seam expansion.
 */
export function translateMemoryAfterTurn(
  payload: MemoryAfterTurnPayload,
): MemoryAfterTurnEvent {
  return {
    kind: "memory_after_turn",
    traceVersion: payload.traceVersion,
    caller: payload.caller,
    policyVersion: payload.policyVersion,
    decidedAt: payload.decidedAt,
    decisions: payload.decisions,
    quality: payload.quality,
    conflicts: payload.conflicts,
    receivedAt: nowMs(),
  };
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
    kind: "execution_mode_decision",
    runId,
    executionMode: payload.executionMode,
    riskLevel: payload.riskLevel,
    complexityLevel: payload.complexityLevel,
    reasonCodes: payload.reasonCodes ?? [],
    scenarioProfileHint: payload.scenarioProfileHint,
    matchedRules: payload.classifierMatchedRuleIds ?? [],
    slotSummary: payload.classifierSlotSummary ?? null,
    ambiguousEscalated: payload.classifierAmbiguousEscalated ?? false,
    escalationSource: payload.classifierEscalationSource,
    policyVersion: payload.classifierPolicyVersion,
    receivedAt: nowMs(),
  };
}
