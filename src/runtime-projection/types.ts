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
  ConflictResolutionPayload,
  ContextBudgetUsage,
  MemoryContextItem,
  MemoryEventPayload,
  MemoryWriteDecisionPayload,
  PromptDiagnosticsSummary as WirePromptDiagnosticsSummary,
  QualityGateResultPayload,
} from "@/transport/contracts";

/** Tool-call lifecycle status, shared by every `stream_tool_call_update`. */
export type ToolCallStatus = "queued" | "running" | "completed" | "error";

/** Coarse outcome carried on `stream_complete`. */
export type TaskOutcome = "completed" | "partial_success" | "failed";

/** Permission decision carried on tool-call updates. */
export type PolicyDecision = "allow" | "deny" | "prompt";

export interface PromptDiagnosticsLaneSummary {
  lane: string;
  status: "active" | "suppressed";
  entryCount: number;
}

export interface PromptDiagnosticsActivatedEntry {
  entryId: string;
  lane: string;
  source: string;
}

export interface PromptDiagnosticsSuppressedEntry {
  entryId: string;
  lane: string;
  reasonCode: string;
}

export interface PromptDiagnosticsActivationReason {
  entryId: string;
  lane: string;
  reasonCode: string;
  detail: string;
}

export interface PromptDiagnosticsSummary {
  traceId: string;
  blockCount: number;
  activeLaneCount: number;
  laneSummaries: PromptDiagnosticsLaneSummary[];
  activatedEntryIds: string[];
  activatedEntries: PromptDiagnosticsActivatedEntry[];
  suppressedEntryIds: string[];
  suppressedEntries: PromptDiagnosticsSuppressedEntry[];
  activationReasonCodes: string[];
  activationReasons: PromptDiagnosticsActivationReason[];
}

export function translatePromptDiagnosticsSummary(
  summary: WirePromptDiagnosticsSummary,
): PromptDiagnosticsSummary {
  return {
    traceId: summary.trace_id,
    blockCount: summary.block_count,
    activeLaneCount: summary.active_lane_count,
    laneSummaries: summary.lane_summaries.map((lane) => ({
      lane: lane.lane,
      status: lane.status,
      entryCount: lane.entry_count,
    })),
    activatedEntryIds: summary.activated_entry_ids,
    activatedEntries: summary.activated_entries.map((entry) => ({
      entryId: entry.entry_id,
      lane: entry.lane,
      source: entry.source,
    })),
    suppressedEntryIds: summary.suppressed_entry_ids,
    suppressedEntries: summary.suppressed_entries.map((entry) => ({
      entryId: entry.entry_id,
      lane: entry.lane,
      reasonCode: entry.reason_code,
    })),
    activationReasonCodes: summary.activation_reason_codes,
    activationReasons: summary.activation_reasons.map((reason) => ({
      entryId: reason.entry_id,
      lane: reason.lane,
      reasonCode: reason.reason_code,
      detail: reason.detail,
    })),
  };
}

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
  | PermissionResolvedEvent
  | MemoryLifecycleEvent
  | MemoryWriteDecisionEvent
  | MemoryAfterTurnEvent
  | ActivationSnapshotEvent
  | ExecutionModeDecisionEvent
  | ExecutionModeManualOverrideEvent;

export interface StreamTextDeltaEvent {
  kind: "stream_text_delta";
  runId: string;
  text: string;
  receivedAt: number;
}

export interface StreamThinkingStartEvent {
  kind: "stream_thinking_start";
  runId: string;
  receivedAt: number;
}

export interface StreamThinkingDeltaEvent {
  kind: "stream_thinking_delta";
  runId: string;
  thinking: string;
  receivedAt: number;
}

export interface StreamToolCallUpdateEvent {
  kind: "stream_tool_call_update";
  runId: string;
  toolCallId: string;
  toolName: string;
  status: ToolCallStatus;
  toolArgs?: Record<string, unknown>;
  toolResult?: string;
  toolDurationMs?: number;
  effectiveWorkdir?: string;
  policyDecision?: PolicyDecision;
  evidenceId?: string;
  requestId?: string;
  receivedAt: number;
}

export interface StreamFinalTextOverrideEvent {
  kind: "stream_final_text_override";
  runId: string;
  text: string;
  requestId?: string;
  receivedAt: number;
}

export interface StreamCompleteEvent {
  kind: "stream_complete";
  runId: string;
  taskOutcome?: TaskOutcome;
  degradedReason?: string;
  resumeAvailable?: boolean;
  resumeCursor?: string;
  requestId?: string;
  contextBudgetUsage?: ContextBudgetUsage;
  memoryItems?: MemoryContextItem[];
  promptDiagnostics?: PromptDiagnosticsSummary;
  receivedAt: number;
}

export interface StreamErrorEvent {
  kind: "stream_error";
  runId: string;
  reason: string;
  taskOutcome?: TaskOutcome;
  degradedReason?: string;
  resumeAvailable?: boolean;
  resumeCursor?: string;
  requestId?: string;
  receivedAt: number;
}

export interface PermissionRequestEvent {
  kind: "permission_request";
  sessionId: string;
  toolName: string;
  permissionMode: string;
  currentMode: string;
  message: string;
  receivedAt: number;
}

/**
 * Phase M2.8 — emitted by the IPC adapter (App.tsx permission
 * dialog handler) **after** `respondPermission` returns. Drives
 * the reducer to clear `snapshot.approvals[sessionId]` so the
 * approval projection doesn't linger after the user decided.
 *
 * Decision payload is informational (recorded for harness traces /
 * future telemetry surface); the reducer only uses `sessionId` to
 * know what to clear.
 */
export interface PermissionResolvedEvent {
  kind: "permission_resolved";
  sessionId: string;
  decision: "allow" | "deny";
  scope: "once" | "session";
  receivedAt: number;
}

export interface MemoryLifecycleEvent {
  kind: "memory_event";
  /** Pass-through of the backend taxonomy.  Not mapped to a closed
   * frontend enum because the backend list is itself open (new
   * memory events are added per phase). */
  payload: MemoryEventPayload;
  receivedAt: number;
}

/**
 * Phase M3.3 + M3.6 — typed pre-write decision rendered by the
 * backend `MemoryCoordinator::after_turn` write-policy gate.
 *
 * Today no backend event source dispatches this — the seam exists
 * so M3-B+ persistence + audit wiring can plug in via the
 * translator without a contract bump.  Reducer accumulates a
 * rolling ring under `snapshot.memory.writeDecisions` ready for
 * UI consumption (M3.6 R2 surfaces).
 */
export interface MemoryWriteDecisionEvent {
  kind: "memory_write_decision";
  /** Caller-supplied id linking to the originating candidate. */
  candidateId: string;
  payload: MemoryWriteDecisionPayload;
  receivedAt: number;
}

/**
 * Phase M3-C closeout — batch envelope dispatched once per backend
 * `MemoryCoordinator::after_turn` invocation (i.e. once per real
 * turn end on either `run_agent_turn` or `start_agent_stream`).
 *
 * Fires even when the batch is empty.  The reducer projects this
 * into `snapshot.memory.lastAfterTurn` so consumers (M4 harness,
 * future governance UIs) can observe:
 *
 *   1. **That `after_turn` ran at all** (closes the M3-B "empty
 *      batch is unobservable" audit gap).
 *   2. **What stage 2 / stage 3 produced** — the `quality` and
 *      `conflicts` arrays carry the gate + resolver outputs that
 *      the per-decision `MemoryWriteDecisionEvent` does not
 *      surface.
 *
 * Per-decision `MemoryWriteDecisionEvent`s are still dispatched
 * by the bridge for non-empty batches so the existing rolling
 * `writeDecisions` ring keeps populating.
 */
export interface MemoryAfterTurnEvent {
  kind: "memory_after_turn";
  /** Phase M4.1 — governance trace contract version pinned by
   * the backend.  Translator forwards verbatim; `undefined` means
   * a pre-M4.1 emitter (legacy traces). */
  traceVersion?: string;
  caller: string;
  policyVersion: string;
  decidedAt: string;
  decisions: MemoryWriteDecisionPayload[];
  quality: QualityGateResultPayload;
  conflicts: ConflictResolutionPayload[];
  receivedAt: number;
}

/** Phase M2.2 placeholder.  No backend source emits an activation
 * snapshot today — the `LicenseLifecycleService` is placeholder-only.
 * The variant exists so the reducer can be extended without a
 * contract bump once a real source lands. */
export interface ActivationSnapshotEvent {
  kind: "activation_snapshot";
  /** Status discriminator from
   * `runtime::contracts::activation::ActivationStatusKind`. */
  statusKind:
    | "checking_local"
    | "needs_activation"
    | "requesting_activation"
    | "pending_approval"
    | "redeeming"
    | "activated"
    | "offline_grace"
    | "expired"
    | "revoked"
    | "deactivated";
  allowsMainShell: boolean;
  receivedAt: number;
}

/** Phase M2.2 placeholder.  Backend `request_intelligence_service`
 * already produces `ExecutionModeDecision`, but it is **advisory**
 * (logged only, not yet emitted as a frontend event).  This variant
 * exists so the reducer can be extended without churn when the
 * decision becomes a real stream event. */
export interface ExecutionModeDecisionEvent {
  kind: "execution_mode_decision";
  runId?: string;
  executionMode:
    | "direct_execute"
    | "auto_plan_execute"
    | "plan_then_confirm"
    | "specialized_surface";
  riskLevel: "low" | "medium" | "high";
  complexityLevel: "trivial" | "simple" | "moderate" | "complex";
  reasonCodes: string[];
  scenarioProfileHint?: string;
  /** Phase M2 audit fix — `classifierMatchedRuleIds` from the M0.5
   * `ExecutionModeDecision`. Empty array if backend didn't surface
   * any rule ids (e.g. classifier escalated to LLM fallback). */
  matchedRules: string[];
  /** Free-form classifier evidence summary. Preserved as an object
   * projection so diagnostics surfaces can render what the
   * classifier actually observed without recomputing locally. */
  slotSummary: unknown;
  /** Whether the classifier judged the request ambiguous enough to
   * escalate its evidence path. */
  ambiguousEscalated: boolean;
  /** Optional escalation source when ambiguity handling fired. */
  escalationSource?: string;
  policyVersion: string;
  receivedAt: number;
}

/**
 * Phase M2 audit fix — UI-driven manual override of the canonical
 * execution-mode judgment.  Dispatched by the manual-override
 * control (future M2.9 explainability surface).  Stored in
 * `ExecutionModeProjection.manualOverride`; pill / surface logic
 * may render the override side-by-side with the classifier
 * judgment but MUST NOT mutate `executionMode` itself.
 *
 * `null` clears the override.
 */
export interface ExecutionModeManualOverrideEvent {
  kind: "execution_mode_manual_override";
  override: ExecutionModeDecisionEvent["executionMode"] | null;
  receivedAt: number;
}

// ───────────────────────── Projection state ────────────────────────

/** Per-run projection state assembled from the stream event family. */
export interface RunProjection {
  runId: string;
  /** Concatenated text deltas. */
  text: string;
  /** Concatenated thinking deltas. */
  thinking: string;
  /** `true` after the first `stream_thinking_start`. */
  thinkingStarted: boolean;
  /** Coarse status, derived from the last seen event. */
  status: "streaming" | "completed" | "failed" | "cancelled";
  taskOutcome?: TaskOutcome;
  degradedReason?: string;
  resumeAvailable: boolean;
  resumeCursor?: string;
  /** All tool-call updates indexed by `toolCallId`, last-write wins. */
  toolCalls: Record<string, ToolCallProjection>;
  /** Optional context-budget snapshot from `stream_complete`. */
  contextBudgetUsage?: ContextBudgetUsage;
  /** Memory items recalled this run. */
  memoryItems: MemoryContextItem[];
  /** Prompt assembly diagnostics from the most recent completion. */
  promptDiagnostics?: PromptDiagnosticsSummary;
  /** Wall-clock `Date.now()` of the last update. */
  lastUpdatedAt: number;
}

export interface ToolCallProjection {
  toolCallId: string;
  toolName: string;
  status: ToolCallStatus;
  toolArgs?: Record<string, unknown>;
  toolResult?: string;
  toolDurationMs?: number;
  effectiveWorkdir?: string;
  policyDecision?: PolicyDecision;
  evidenceId?: string;
  requestId?: string;
  /** First-seen wall-clock time. */
  firstSeenAt: number;
  /** Last-write wall-clock time. */
  lastUpdatedAt: number;
}

/** Snapshot of every pending permission prompt. */
export interface PermissionApprovalProjection {
  sessionId: string;
  toolName: string;
  permissionMode: string;
  currentMode: string;
  message: string;
  receivedAt: number;
}

/** Lightweight rolling memory projection for the chat surface. */
export interface MemoryRollingProjection {
  /** Latest 64 memory lifecycle events, oldest first. */
  recentEvents: MemoryEventPayload[];
  /** Items recalled by the most recent `stream_complete`. */
  lastRecallItems: MemoryContextItem[];
  /** Phase M3.6 — rolling ring of typed memory write decisions
   * (oldest first, capped at 32).  Empty until a backend event
   * source dispatches `MemoryWriteDecisionEvent`s through the
   * bridge. */
  writeDecisions: MemoryWriteDecisionProjection[];
  /** Phase M3-C closeout — projection of the most recent
   * `after_turn` batch envelope.  `null` until the first
   * `MemoryAfterTurnEvent` arrives.  Carries `quality` /
   * `conflicts` so M4 governance consumers can read them without
   * a new transport seam.  Note: this is a *latest-wins* slot,
   * not a ring — the per-decision ring lives in
   * `writeDecisions`. */
  lastAfterTurn: MemoryAfterTurnProjection | null;
  /** Memory Audit P1 #5 — monotonic counter bumped whenever the
   * backend emits a `memory_invalidated` event.  Frontend views
   * (Browser, CompiledViewer, NarrativeViewer, PinnedEditor) key
   * their refetch effects off this so a write in one surface
   * propagates to all open views without per-component polling.
   *
   * Optional companion `lastInvalidationScope` carries the coarse
   * scope hint (`"entries" | "pinned" | "compiled" | "summaries" |
   * "all"`) so a view can decide whether the invalidation is
   * relevant to it. */
  invalidationVersion: number;
  lastInvalidationScope: string | null;
}

/** Phase M3-C closeout — per-batch projection consumed by future
 * M4 harness inspectors and governance surfaces. */
export interface MemoryAfterTurnProjection {
  /** Phase M4.1 — governance trace contract version (forwarded
   * from `MemoryAfterTurnEvent.traceVersion`). */
  traceVersion?: string;
  caller: string;
  policyVersion: string;
  decidedAt: string;
  decisionCount: number;
  acceptedCount: number;
  rejectedCount: number;
  warningCount: number;
  conflictsCount: number;
  /** Full quality result preserved verbatim so consumers can
   * render rejected / warning details without a re-fetch. */
  quality: QualityGateResultPayload;
  /** Full per-candidate conflict outcomes (parallel to the
   * decisions in the originating batch). */
  conflicts: ConflictResolutionPayload[];
  /** Wall-clock time the bridge dispatched this projection. */
  receivedAt: number;
}

/** Per-decision projection consumed by the future M3.6 R2
 * MemoryWriteCard / MemoryChip / TelemetryDrawer surfaces. */
export interface MemoryWriteDecisionProjection {
  candidateId: string;
  decision: MemoryWriteDecisionPayload;
  receivedAt: number;
}

/** Activation projection — `null` until a real source emits.  Held
 * here so the reducer / future store have a stable shape. */
export interface ActivationProjection {
  statusKind: ActivationSnapshotEvent["statusKind"];
  allowsMainShell: boolean;
  capturedAt: number;
}

/** Execution-mode projection — `null` until a real source emits. */
export interface ExecutionModeProjection {
  runId?: string;
  executionMode: ExecutionModeDecisionEvent["executionMode"];
  riskLevel: ExecutionModeDecisionEvent["riskLevel"];
  complexityLevel: ExecutionModeDecisionEvent["complexityLevel"];
  reasonCodes: string[];
  scenarioProfileHint?: string;
  /** Phase M2 audit fix — rule ids that fired in the classifier.
   * Mirrors `ExecutionModeDecisionEvent.matchedRules` so M2.9
   * explainability surfaces (popover / inspector) can render rule
   * traces without re-querying the backend. */
  matchedRules: string[];
  slotSummary: unknown;
  ambiguousEscalated: boolean;
  escalationSource?: string;
  /** Phase M2 audit fix — UI-driven manual override of the
   * classifier judgment.  `null` when the user has not overridden.
   * The pill SHOULD render the override prominently when set, but
   * `executionMode` (the classifier judgment) is NEVER mutated. */
  manualOverride: ExecutionModeProjection["executionMode"] | null;
  policyVersion: string;
  capturedAt: number;
  /** Phase M2 audit fix — alias of `capturedAt` retained for the
   * YAML m2.5 spec field name. Both refer to the wall-clock time
   * the projection was last refreshed. */
  lastUpdatedAt: number;
}

/** Top-level snapshot consumed by future M2.4 stores. */
export interface RuntimeProjectionSnapshot {
  /** Runs indexed by `runId`. */
  runs: Record<string, RunProjection>;
  /** Approvals indexed by `sessionId` (one prompt per session at a time). */
  approvals: Record<string, PermissionApprovalProjection>;
  memory: MemoryRollingProjection;
  /** `null` until backend emits an activation snapshot event. */
  activation: ActivationProjection | null;
  /** `null` until backend emits an execution-mode decision event. */
  executionMode: ExecutionModeProjection | null;
}

/** Build a fresh empty snapshot. Used by both the reducer module
 * and unit tests. */
export function emptyProjectionSnapshot(): RuntimeProjectionSnapshot {
  return {
    runs: {},
    approvals: {},
    memory: {
      recentEvents: [],
      invalidationVersion: 0,
      lastInvalidationScope: null,
      lastRecallItems: [],
      writeDecisions: [],
      lastAfterTurn: null,
    },
    activation: null,
    executionMode: null,
  };
}
