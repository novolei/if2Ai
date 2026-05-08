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
  ResumeRecoverability,
  AttemptStats,
  FinalRunReport,
  SkillResolutionPlan,
  ToolAttemptStatus,
  WorkLoopDecision,
} from "@/transport/contracts";

/** Tool-call lifecycle status — accepts attempt-ledger states plus
 *  the legacy stream `error` value still emitted by the backend. */
export type ToolCallStatus = ToolAttemptStatus | "error";
export type { ToolAttemptStatus };

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
  | StreamRunBoundEvent
  | StreamTextDeltaEvent
  | StreamThinkingStartEvent
  | StreamThinkingDeltaEvent
  | StreamToolCallUpdateEvent
  | StreamFinalTextOverrideEvent
  | SkillResolutionSnapshotEvent
  | FinalRunReportEvent
  | StreamCompleteEvent
  | StreamErrorEvent
  | PermissionRequestEvent
  | PermissionResolvedEvent
  | MemoryLifecycleEvent
  | MemoryWriteDecisionEvent
  | MemoryAfterTurnEvent
  | ActivationSnapshotEvent
  | ExecutionModeDecisionEvent
  | ExecutionModeManualOverrideEvent
  | ProjectionDiscardSessionRunsEvent
  | SupervisorSnapshotEvent
  | SmartBrowserProjectionEvent
  | ToolAttemptTimelineEvent;

export interface StreamRunBoundEvent {
  kind: "stream_run_bound";
  runId: string;
  sessionId: string;
  receivedAt: number;
}

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
  /** Tool-attempt identifier from GAP-002 correlation contract.
   *  Carried through for T-010 (MIG-007 Tool Execution Contract). */
  attemptId?: string;
  receivedAt: number;
}

export interface StreamFinalTextOverrideEvent {
  kind: "stream_final_text_override";
  runId: string;
  text: string;
  requestId?: string;
  receivedAt: number;
}

export interface SkillResolutionSnapshotEvent {
  kind: "skill_resolution_snapshot";
  runId: string;
  plan: SkillResolutionPlan;
  receivedAt: number;
}

export interface FinalRunReportEvent {
  kind: "final_run_report";
  runId: string;
  report: FinalRunReport;
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
  /** P1-7 / P2-11 — provider-billable token usage + USD cost for this turn. */
  turnCost?: TurnCost;
  /** P1-8 — smart-routing decision summary for this turn. */
  routing?: RoutingInfo;
  /** P2-11 — running per-session totals after this turn. */
  sessionTotals?: SessionUsageTotals;
  /** T-012 — structured recoverability payload from MIG-021. */
  recoverability?: ResumeRecoverability;
  receivedAt: number;
}

/** Per-turn token + cost (frontend mirror of `TurnCostPayload`). */
export interface TurnCost {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
  costUsd: number;
  model: string;
}

/** Smart-routing decision summary (frontend mirror of `RoutingInfoPayload`). */
export interface RoutingInfo {
  complexityScore: number;
  complexityLevel: string;
  executionMode: string;
  usedCheapModel: boolean;
  effectiveModel: string;
}

/** Per-session running totals (frontend mirror of `SessionUsageTotalsPayload`). */
export interface SessionUsageTotals {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
  costUsd: number;
  turns: number;
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
  /** T-012 — structured recoverability payload from MIG-021. */
  recoverability?: ResumeRecoverability;
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
  workLoop?: WorkLoopDecision;
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

/** UI-driven: drop all stream run projections tied to one session (e.g. transcript undo). */
export interface ProjectionDiscardSessionRunsEvent {
  kind: "projection_discard_session_runs";
  sessionId: string;
  receivedAt: number;
}

/**
 * T-007 — supervisor snapshot event dispatched when the backend
 * `SessionSupervisor` emits a state change or the frontend fetches
 * the initial snapshot via `get_supervisor_snapshot`.
 *
 * Carries the full supervisor payload so the reducer can do a
 * complete snapshot replacement (latest-wins semantics).
 */
export interface SupervisorSnapshotEvent {
  kind: "supervisor_snapshot";
  sessionId: string;
  status: SupervisorStatus;
  activeRunId: string | null;
  activeRunStatus: RunStatus | null;
  pendingPermissionCount: number;
  lastErrorKind: string | null;
  recoverable: boolean;
  retryBudgetRemaining: number;
  disconnectGraceUntil: string | null;
  lastUpdatedAt: string;
  receivedAt: number;
}

export type SmartBrowserBackend =
  | "local_rust_cdp"
  | "browser_use_mcp"
  | "browser_use_cloud";

export type SmartBrowserEscalationState =
  | "none"
  | "suggested"
  | "approval_required"
  | "approved"
  | "active"
  | "blocked";

export interface SmartBrowserDiagnosticsProjection {
  downloads: number;
  console: number;
  networkErrors: number;
}

export interface SmartBrowserProjectionEvent {
  kind: "smart_browser_projection";
  sessionId: string;
  smartBrowserSessionId: string;
  backend: SmartBrowserBackend;
  running: boolean;
  url: string | null;
  title: string | null;
  thumbnail: string | null;
  takenOver: boolean;
  lastAction: string | null;
  diagnostics: SmartBrowserDiagnosticsProjection;
  escalationState: SmartBrowserEscalationState;
  receivedAt: number;
}

/** Top-level supervisor lifecycle states (mirrors Rust `SupervisorStatus`). */
export type SupervisorStatus =
  | "idle"
  | "running"
  | "blocked"
  | "recoverable_failed"
  | "completed"
  | "closed";

/** Per-run status (mirrors Rust `RunStatus`). */
export type RunStatus =
  | "streaming"
  | "completed"
  | "failed"
  | "cancelled";

// ───────────────────────── Projection state ────────────────────────

/** Per-run projection state assembled from the stream event family. */
export interface RunProjection {
  runId: string;
  /** Session that owns this run, bound by the chat command facade. */
  sessionId?: string;
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
  /** T-012 — structured recoverability (reason + safe-to-retry + budget). */
  recoverability?: ResumeRecoverability;
  /** All tool-call updates indexed by `toolCallId`, last-write wins. */
  toolCalls: Record<string, ToolCallProjection>;
  /** Optional context-budget snapshot from `stream_complete`. */
  contextBudgetUsage?: ContextBudgetUsage;
  /** P1-7 / P2-11 — provider-billable usage + USD cost for this run. */
  turnCost?: TurnCost;
  /** P1-8 — smart-routing decision for this run. */
  routing?: RoutingInfo;
  /** Selected internal work loop for this run. */
  workLoop?: WorkLoopDecision;
  /** Skill resolver snapshot for this run. */
  skillResolution?: SkillResolutionPlan;
  /** Final report emitted when the run terminates. */
  finalRunReport?: FinalRunReport;
  /** P2-11 — per-session running totals snapshot at end of this run. */
  sessionTotals?: SessionUsageTotals;
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
  /** Current attempt identifier from GAP-002 correlation (T-014). */
  attemptId?: string;
  /** Current attempt number in retry sequence (T-014). */
  attemptNo?: number;
  /** Failure classification for the current attempt (T-014). */
  failureKind?: string | null;
  /** Full attempt history in chronological order (T-014). */
  attemptHistory: ToolAttemptTimestamp[];
  /** First-seen wall-clock time. */
  firstSeenAt: number;
  /** Last-write wall-clock time. */
  lastUpdatedAt: number;
}

/** T-014 — per-attempt projection snapshot. */
export interface ToolAttemptTimestamp {
  attemptId: string;
  attemptNo: number;
  toolCallId: string;
  runId: string;
  toolName: string;
  status: ToolAttemptStatus;
  /** Wall-clock timestamp (ms since epoch). */
  startedAt: number;
  /** Optional terminal timestamp. */
  endedAt?: number;
  /** Execution duration in ms. */
  durationMs?: number;
  /** Policy decision for this attempt. */
  policyDecision?: string;
  /** Failure classification. */
  failureKind?: string | null;
}

/** T-014 — fetch-seam event carrying the full attempt ledger snapshot. */
export interface ToolAttemptTimelineEvent {
  kind: "tool_attempt_timeline_snapshot";
  sessionId: string;
  attempts: ToolAttemptTimestamp[];
  stats: Record<string, AttemptStats>;
  receivedAt: number;
}

/** T-014 — top-level attempt timeline projection. */
export interface AttemptTimelineProjection {
  sessionId: string;
  /** Attempts indexed by tool_call_id. */
  byToolCallId: Record<string, ToolAttemptTimestamp[]>;
  /** Attempts indexed by attempt_id for dedup. */
  byAttemptId: Record<string, ToolAttemptTimestamp>;
  /** Per-tool_call_id statistics. */
  stats: Record<string, AttemptStats>;
  /** Wall-clock of last fetch. */
  lastFetchedAt: number;
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
  /** Agent-loop router decision paired with the classifier judgment. */
  workLoop?: WorkLoopDecision;
  capturedAt: number;
  /** Phase M2 audit fix — alias of `capturedAt` retained for the
   * YAML m2.5 spec field name. Both refer to the wall-clock time
   * the projection was last refreshed. */
  lastUpdatedAt: number;
}

/**
 * T-007 — supervisor projection per session (latest-wins).
 * Populated by `SupervisorSnapshotEvent`; `null` until the
 * first backend snapshot arrives.
 */
export interface SupervisorProjection {
  sessionId: string;
  status: SupervisorStatus;
  activeRunId: string | null;
  activeRunStatus: RunStatus | null;
  pendingPermissionCount: number;
  lastErrorKind: string | null;
  recoverable: boolean;
  retryBudgetRemaining: number;
  disconnectGraceUntil: string | null;
  lastUpdatedAt: string;
  capturedAt: number;
}

export interface SmartBrowserProjection {
  sessionId: string;
  smartBrowserSessionId: string;
  backend: SmartBrowserBackend;
  running: boolean;
  url: string | null;
  title: string | null;
  thumbnail: string | null;
  takenOver: boolean;
  lastAction: string | null;
  diagnostics: SmartBrowserDiagnosticsProjection;
  escalationState: SmartBrowserEscalationState;
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
  /** T-007 — `null` until backend emits a supervisor snapshot event. */
  supervisor: SupervisorProjection | null;
  /** Smart Browser sessions indexed by `smartBrowserSessionId`. */
  browsers: Record<string, SmartBrowserProjection>;
  /** T-014 — attempt timeline snapshot, `null` until fetched via bridge seam. */
  attemptTimeline: AttemptTimelineProjection | null;
  /** ER-03 — id of the session this snapshot is currently scoped to.
   *  Set via [`RuntimeProjectionStore.swapSession`].  When non-null,
   *  the reducer drops events whose session id is set and differs.
   *  `null` means "no session scope guard" — every event is accepted
   *  (legacy / pre-swap behavior). */
  currentSessionId: string | null;
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
    supervisor: null,
    browsers: {},
    attemptTimeline: null,
    currentSessionId: null,
  };
}

/** ER-03 — build a fresh empty snapshot scoped to a session id.
 *  Used by [`RuntimeProjectionStore.swapSession`] so the new
 *  snapshot's `currentSessionId` guard is established atomically
 *  alongside the cleared per-run state. */
export function emptyProjectionSnapshotForSession(
  sessionId: string | null,
): RuntimeProjectionSnapshot {
  return { ...emptyProjectionSnapshot(), currentSessionId: sessionId };
}
