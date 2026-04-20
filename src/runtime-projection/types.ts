// Canonical runtime-projection types (Phase M2.2 + M2.3).
//
// These are the **frontend-side** canonical event/state shapes
// produced by the translator and consumed by the reducer + future
// stores. They are intentionally distinct from:
//
// - `@/transport/contracts` legacy DTOs (snake_case wire shapes)
// - `@/transport/contracts` canonical envelope types (mirror of
//   the Rust runtime contract, also camelCase but defined for the
//   M0.3 envelope migration which is **not** wired into the agent
//   stream yet)
//
// Naming: camelCase, frontend-friendly. Field semantics preserved
// from the legacy wire payload — translator does no semantic
// rewriting, it only renames and groups.

import type {
  ContextBudgetUsage,
  MemoryContextItem,
  MemoryEventPayload,
} from '@/transport/contracts'

/** Tool-call lifecycle status, shared by every `stream_tool_call_update`. */
export type ToolCallStatus = 'queued' | 'running' | 'completed' | 'error'

/** Coarse outcome carried on `stream_complete`. */
export type TaskOutcome = 'completed' | 'partial_success' | 'failed'

/** Permission decision carried on tool-call updates. */
export type PolicyDecision = 'allow' | 'deny' | 'prompt'

/**
 * Canonical runtime event family produced by the translator.
 *
 * Discriminated union on `kind`. Adding a kind is a contract change
 * — every consumer that switches on `kind` must handle the new
 * variant. Removing a kind is a wire-level break.
 *
 * `runId` is sourced from the legacy `stream_id` field today; once
 * the Rust `RuntimeEventEnvelope` migration completes (M2.4+),
 * `runId` will come from `correlation.runId` instead.
 */
export type CanonicalRuntimeEvent =
  | StreamTextDeltaEvent
  | StreamThinkingStartEvent
  | StreamThinkingDeltaEvent
  | StreamToolCallUpdateEvent
  | StreamFinalTextOverrideEvent
  | StreamCompleteEvent
  | StreamErrorEvent
  | PermissionRequestEvent
  | MemoryLifecycleEvent
  | ActivationSnapshotEvent
  | ExecutionModeDecisionEvent

export interface StreamTextDeltaEvent {
  kind: 'stream_text_delta'
  runId: string
  text: string
  receivedAt: number
}

export interface StreamThinkingStartEvent {
  kind: 'stream_thinking_start'
  runId: string
  receivedAt: number
}

export interface StreamThinkingDeltaEvent {
  kind: 'stream_thinking_delta'
  runId: string
  thinking: string
  receivedAt: number
}

export interface StreamToolCallUpdateEvent {
  kind: 'stream_tool_call_update'
  runId: string
  toolCallId: string
  toolName: string
  status: ToolCallStatus
  toolArgs?: Record<string, unknown>
  toolResult?: string
  toolDurationMs?: number
  effectiveWorkdir?: string
  policyDecision?: PolicyDecision
  evidenceId?: string
  requestId?: string
  receivedAt: number
}

export interface StreamFinalTextOverrideEvent {
  kind: 'stream_final_text_override'
  runId: string
  text: string
  requestId?: string
  receivedAt: number
}

export interface StreamCompleteEvent {
  kind: 'stream_complete'
  runId: string
  taskOutcome?: TaskOutcome
  degradedReason?: string
  resumeAvailable?: boolean
  resumeCursor?: string
  requestId?: string
  contextBudgetUsage?: ContextBudgetUsage
  memoryItems?: MemoryContextItem[]
  receivedAt: number
}

export interface StreamErrorEvent {
  kind: 'stream_error'
  runId: string
  reason: string
  taskOutcome?: TaskOutcome
  degradedReason?: string
  resumeAvailable?: boolean
  resumeCursor?: string
  requestId?: string
  receivedAt: number
}

export interface PermissionRequestEvent {
  kind: 'permission_request'
  sessionId: string
  toolName: string
  permissionMode: string
  currentMode: string
  message: string
  receivedAt: number
}

export interface MemoryLifecycleEvent {
  kind: 'memory_event'
  /** Pass-through of the backend taxonomy.  Not mapped to a closed
   * frontend enum because the backend list is itself open (new
   * memory events are added per phase). */
  payload: MemoryEventPayload
  receivedAt: number
}

/** Phase M2.2 placeholder.  No backend source emits an activation
 * snapshot today — the `LicenseLifecycleService` is placeholder-only.
 * The variant exists so the reducer can be extended without a
 * contract bump once a real source lands. */
export interface ActivationSnapshotEvent {
  kind: 'activation_snapshot'
  /** Status discriminator from
   * `runtime::contracts::activation::ActivationStatusKind`. */
  statusKind:
    | 'checking_local'
    | 'needs_activation'
    | 'requesting_activation'
    | 'pending_approval'
    | 'redeeming'
    | 'activated'
    | 'offline_grace'
    | 'expired'
    | 'revoked'
    | 'deactivated'
  allowsMainShell: boolean
  receivedAt: number
}

/** Phase M2.2 placeholder.  Backend `request_intelligence_service`
 * already produces `ExecutionModeDecision`, but it is **advisory**
 * (logged only, not yet emitted as a frontend event).  This variant
 * exists so the reducer can be extended without churn when the
 * decision becomes a real stream event. */
export interface ExecutionModeDecisionEvent {
  kind: 'execution_mode_decision'
  runId?: string
  executionMode:
    | 'direct_execute'
    | 'auto_plan_execute'
    | 'plan_then_confirm'
    | 'specialized_surface'
  riskLevel: 'low' | 'medium' | 'high'
  complexityLevel: 'trivial' | 'simple' | 'moderate' | 'complex'
  reasonCodes: string[]
  policyVersion: string
  receivedAt: number
}

// ───────────────────────── Projection state ────────────────────────

/** Per-run projection state assembled from the stream event family. */
export interface RunProjection {
  runId: string
  /** Concatenated text deltas. */
  text: string
  /** Concatenated thinking deltas. */
  thinking: string
  /** `true` after the first `stream_thinking_start`. */
  thinkingStarted: boolean
  /** Coarse status, derived from the last seen event. */
  status: 'streaming' | 'completed' | 'failed' | 'cancelled'
  taskOutcome?: TaskOutcome
  degradedReason?: string
  resumeAvailable: boolean
  resumeCursor?: string
  /** All tool-call updates indexed by `toolCallId`, last-write wins. */
  toolCalls: Record<string, ToolCallProjection>
  /** Optional context-budget snapshot from `stream_complete`. */
  contextBudgetUsage?: ContextBudgetUsage
  /** Memory items recalled this run. */
  memoryItems: MemoryContextItem[]
  /** Wall-clock `Date.now()` of the last update. */
  lastUpdatedAt: number
}

export interface ToolCallProjection {
  toolCallId: string
  toolName: string
  status: ToolCallStatus
  toolArgs?: Record<string, unknown>
  toolResult?: string
  toolDurationMs?: number
  effectiveWorkdir?: string
  policyDecision?: PolicyDecision
  evidenceId?: string
  requestId?: string
  /** First-seen wall-clock time. */
  firstSeenAt: number
  /** Last-write wall-clock time. */
  lastUpdatedAt: number
}

/** Snapshot of every pending permission prompt. */
export interface PermissionApprovalProjection {
  sessionId: string
  toolName: string
  permissionMode: string
  currentMode: string
  message: string
  receivedAt: number
}

/** Lightweight rolling memory projection for the chat surface. */
export interface MemoryRollingProjection {
  /** Latest 64 memory lifecycle events, oldest first. */
  recentEvents: MemoryEventPayload[]
  /** Items recalled by the most recent `stream_complete`. */
  lastRecallItems: MemoryContextItem[]
}

/** Activation projection — `null` until a real source emits.  Held
 * here so the reducer / future store have a stable shape. */
export interface ActivationProjection {
  statusKind: ActivationSnapshotEvent['statusKind']
  allowsMainShell: boolean
  capturedAt: number
}

/** Execution-mode projection — `null` until a real source emits. */
export interface ExecutionModeProjection {
  runId?: string
  executionMode: ExecutionModeDecisionEvent['executionMode']
  riskLevel: ExecutionModeDecisionEvent['riskLevel']
  complexityLevel: ExecutionModeDecisionEvent['complexityLevel']
  reasonCodes: string[]
  policyVersion: string
  capturedAt: number
}

/** Top-level snapshot consumed by future M2.4 stores. */
export interface RuntimeProjectionSnapshot {
  /** Runs indexed by `runId`. */
  runs: Record<string, RunProjection>
  /** Approvals indexed by `sessionId` (one prompt per session at a time). */
  approvals: Record<string, PermissionApprovalProjection>
  memory: MemoryRollingProjection
  /** `null` until backend emits an activation snapshot event. */
  activation: ActivationProjection | null
  /** `null` until backend emits an execution-mode decision event. */
  executionMode: ExecutionModeProjection | null
}

/** Build a fresh empty snapshot. Used by both the reducer module
 * and unit tests. */
export function emptyProjectionSnapshot(): RuntimeProjectionSnapshot {
  return {
    runs: {},
    approvals: {},
    memory: { recentEvents: [], lastRecallItems: [] },
    activation: null,
    executionMode: null,
  }
}
