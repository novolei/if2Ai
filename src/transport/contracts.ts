// If2Ai canonical runtime contracts — TypeScript twin of
// `src-tauri/src/modules/runtime/contracts/`.
//
// Phase M0.3 + M0.4 + M0.5 skeleton.
//
// Hard rules (mirrored in
// `docs/staff-remediation/if2ai-canonical-domain-model.md` and
// `docs/staff-remediation/if2ai-workflow-truth.md`):
//
// 1. The frontend MUST consume canonical contracts from this file
//    only. New runtime types do NOT belong in `src/lib/tauri.ts`
//    (which is going back to a thin transport bridge in M2).
// 2. `ExecutionMode` is runtime truth produced by the backend
//    classifier. The frontend MUST NOT recompute it; it only
//    projects an `ExecutionModeDecision`.
// 3. `Chat / Coding / Research / Planning / Review` are scenario
//    profiles, layered above `ExecutionMode`. Use
//    `ScenarioProfileHint`, never collapse them into `ExecutionMode`.
// 4. `ActivationStatusKind = 'activated'` does NOT imply the main
//    shell is permitted; consumers MUST gate on
//    `ActivationSnapshot.allowsMainShell`.
//
// Field naming on the wire is `camelCase` (matches the Rust serde
// `rename_all = "camelCase"` derive on every payload struct).

/** Current canonical schema version — keep in sync with
 * `CONTRACTS_SCHEMA_VERSION` in `common.rs`. */
export const CONTRACTS_SCHEMA_VERSION = '1.0.0-skeleton'

/** Schema version marker carried on every envelope. */
export type SchemaVersion = string

// ───────────────────────── Common envelope ─────────────────────────

/** Canonical runtime event type families. New families MUST be
 * appended here before being emitted. */
export type RuntimeEventType =
  | 'conversation'
  | 'tool'
  | 'permission'
  | 'memory'
  | 'activation'
  | 'execution_mode'
  | 'harness'
  | 'system'

/** Cross-cutting correlation ids carried by every envelope. All
 * fields are optional; absent means "not applicable", not "unknown". */
export interface CorrelationIds {
  sessionId?: string
  projectId?: string
  /** Canonical `run` id. Emitted from M1 onwards. */
  runId?: string
  /** Legacy stream id — held during the M1 cut-over. New code should
   * prefer `runId`. */
  streamId?: string
  /** Optional turn / step counter inside a run. */
  turnIndex?: number
}

/** Canonical envelope wrapping every runtime-emitted event. */
export interface RuntimeEventEnvelope<TPayload = unknown> {
  schemaVersion: SchemaVersion
  eventType: RuntimeEventType
  /** Free-form family tag narrowing `eventType` (e.g. `eventType =
   * 'conversation'`, `payloadFamily = 'delta'`). M1 will narrow to
   * typed enums per family. */
  payloadFamily: string
  /** RFC3339 timestamp at emit time. */
  emittedAt: string
  correlation: CorrelationIds
  payload: TPayload
}

// ───────────────────────── Activation contract ─────────────────────

/** Canonical activation status discriminator (10 states).
 * Mirrors `ActivationStatusKind` in Rust. */
export type ActivationStatusKind =
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

/** Coarse failure / terminal-state reason taxonomy. Open enum: the
 * frontend MUST handle the `'other'` fallback. */
export type ActivationFailureReason =
  | 'no_local_license_offline'
  | 'server_expired'
  | 'server_revoked'
  | 'user_deactivated'
  | 'refresh_transient'
  | 'other'

export interface ActivationStatus {
  kind: ActivationStatusKind
  message?: string
  /** RFC3339; present when `kind === 'expired' | 'offline_grace'`. */
  graceUntil?: string
  /** Present when `kind === 'pending_approval'`. */
  pendingRequestId?: string
  failureReason?: ActivationFailureReason
}

/** Compact license summary safe to expose to the frontend. */
export interface ActivationLicense {
  licenseId: string
  plan?: string
  /** RFC3339. */
  issuedAt: string
  /** RFC3339. `undefined` for non-expiring licenses. */
  expiresAt?: string
  /** RFC3339. */
  lastRefreshedAt?: string
}

/** Full activation snapshot — the canonical projection consumed by
 * the boot shell and the (future) settings panel.
 *
 * IMPORTANT: `allowsMainShell` is computed by the backend; the
 * frontend MUST NOT recompute it. */
export interface ActivationSnapshot {
  status: ActivationStatus
  license?: ActivationLicense
  allowsMainShell: boolean
  correlation: CorrelationIds
  /** RFC3339 capture time. */
  capturedAt: string
}

/** Canonical activation lifecycle action discriminator. Mirrors
 * `ActivationActionKind` in Rust. M1 `activation_service` will
 * dispatch on this. */
export type ActivationActionKind =
  | 'request'
  | 'redeem'
  | 'refresh'
  | 'revoke_check'
  | 'deactivate'
  | 'local_boot_restore'

export interface ActivationAction<TPayload = unknown> {
  kind: ActivationActionKind
  payload?: TPayload
  correlation: CorrelationIds
  /** RFC3339. */
  requestedAt: string
}

// ───────────────────────── Execution-mode contract ─────────────────

/** Canonical, closed set of runtime execution modes. Adding a value
 * is a contract break that requires a schema version bump. */
export type ExecutionMode =
  | 'direct_execute'
  | 'auto_plan_execute'
  | 'plan_then_confirm'
  | 'specialized_surface'

/** Coarse risk taxonomy. Closed: frontends rely on these three
 * buckets for explainable chips. */
export type RiskLevel = 'low' | 'medium' | 'high'

/** Coarse complexity taxonomy. Closed. */
export type ComplexityLevel = 'trivial' | 'simple' | 'moderate' | 'complex'

/** Scenario profile hint, layered above `ExecutionMode`. Advisory
 * only — the run is governed by `ExecutionMode`. */
export type ScenarioProfileHint = 'chat' | 'coding' | 'research' | 'planning' | 'review'

/** Optional route hint string for `specialized_surface` or sub-target
 * routing. Free-form in v1. */
export type RouteHint = string

/** Stable reason-code identifier (`snake_case`). Map to the frontend
 * i18n catalog. */
export type ReasonCode = string

/** Classifier evidence snapshot — explainability backbone. */
export interface ClassifierEvidence {
  policyVersion: string
  matchedRuleIds: string[]
  /** Free-form key/value summary; rendered as a transparency table. */
  slotSummary: unknown
  ambiguousEscalated: boolean
  escalationSource?: string
}

/** Canonical execution-mode decision per request.
 *
 * The frontend MUST treat this as a projection only:
 * - Do NOT recompute `executionMode` client-side.
 * - Render `complexityLevel` (bucket) for users; `complexityScore`
 *   is advisory diagnostic data.
 * - Send overrides through a backend-mediated action, never by
 *   mutating this object locally. */
export interface ExecutionModeDecision {
  executionMode: ExecutionMode
  riskLevel: RiskLevel
  complexityLevel: ComplexityLevel
  /** 0.0..=1.0 raw score. Advisory only. */
  complexityScore: number
  reasonCodes: ReasonCode[]
  routeHint?: RouteHint
  requiresPlan: boolean
  scenarioProfileHint?: ScenarioProfileHint
  classifierPolicyVersion: string
  classifierMatchedRuleIds: string[]
  classifierSlotSummary: unknown
  classifierAmbiguousEscalated: boolean
  classifierEscalationSource?: string
}

// ───────────────────────── Memory contract ─────────────────────────

/** Canonical memory kinds. Closed in v1. */
export type MemoryKind =
  | 'working'
  | 'session_summary'
  | 'episodic'
  | 'pinned'
  | 'compiled'
  | 'reflection'

/** Canonical scope tier. Mirrors `MemoryExecutionScope`. */
export type MemoryScope = 'session' | 'project' | 'global'

/** Verdict produced by the (future) `WritePolicy` / `QualityGate`. */
export type MemoryDecisionVerdict =
  | 'persisted'
  | 'held_in_working'
  | 'rejected'
  | 'promoted'
  | 'demoted'
  | 'expired'

/** Wire-level memory decision record. Frontends render this as a
 * `MemoryWriteCard` / `MemoryChip` explainer. */
export interface MemoryDecision {
  memoryId: string
  verdict: MemoryDecisionVerdict
  kind: MemoryKind
  scope: MemoryScope
  reasonCodes: string[]
  /** Optional 0.0..=1.0 quality / importance score. */
  score?: number
  policyVersion: string
  correlation: CorrelationIds
  /** RFC3339. */
  decidedAt: string
}

// ───────────────────────── Legacy wire DTOs ────────────────────────
//
// Phase M2.1 — these types are migrated verbatim from the legacy
// `src/lib/tauri.ts` so the transport boundary owns the single
// source of truth for IPC payload shapes.  They keep snake_case
// because the backend still emits snake_case wire payloads (legacy
// `agent-token` / `permission-request` / `memory_event` channels).
//
// Hard rules:
// 1. Snake_case stays here. The new canonical camelCase shapes
//    above are consumed only inside the runtime-projection
//    pipeline (`src/runtime-projection/*`).  The translator is the
//    only place that crosses between them.
// 2. Adding fields here is a transport-level change. Adding fields
//    to a canonical contract above is a contract-version bump.

/** Token budget usage breakdown emitted by the backend WorkingMemory
 * + ContextBudget components at the end of each streaming turn
 * (event_type: 'stream_complete'). */
export interface ContextBudgetUsage {
  total_budget: number
  system_tokens: number
  history_tokens: number
  memory_tokens: number
  output_reserve: number
  remaining: number
}

/** Memory lifecycle event emitted by `MemoryAuditEmitter` over the
 * Tauri `memory_event` channel.  Mirrors `MemoryEventPayload` in
 * `src-tauri/src/modules/memory/audit.rs`. */
export interface MemoryEventPayload {
  event:
    | 'memory_captured'
    | 'memory_write_decision'
    | 'memory_persisted'
    | 'memory_recall_served'
    | 'memory_rejected'
    | 'memory_promoted'
    | 'memory_promotion_candidate'
    | 'memory_demoted'
    | 'memory_cleared'
    | 'memory_pii_redacted'
    | 'memory_job_failed'
    | 'memory_job_skipped'
    | 'memory_summary_rolled'
    | 'memory_pinned'
    | 'memory_unpinned'
    | 'memory_compiled'
    | 'memory_ticker_recovery'
    | 'memory_assembled'
  trace_id?: string
  session_id?: string
  project_id?: string
  effective_workdir?: string
  memory_key?: string
  memory_category?: string
  policy_decision?: 'allow' | 'deny' | 'prompt'
  reason_code?: string
  reason_message?: string
  recall_query?: string
  recall_category?: string
  result_count?: number
  from_category?: string
  to_category?: string
  extra?: Record<string, unknown>
  /** ISO 8601 timestamp captured server-side at emit time. */
  timestamp: string
}

/** Single recalled memory item surfaced by `MemoryAuditEmitter`. */
export interface MemoryContextItem {
  id: string
  content: string
  scope: 'global' | 'project' | 'session'
  relevance_score?: number
  stored_at?: string
}

/** One lane summary row inside prompt diagnostics. */
export interface PromptDiagnosticsLaneSummary {
  lane: string
  status: 'active' | 'suppressed'
  entry_count: number
}

export interface PromptDiagnosticsActivatedEntry {
  entry_id: string
  lane: string
  source: string
}

export interface PromptDiagnosticsSuppressedEntry {
  entry_id: string
  lane: string
  reason_code: string
}

export interface PromptDiagnosticsActivationReason {
  entry_id: string
  lane: string
  reason_code: string
  detail: string
}

/** Frontend-safe summary of prompt assembly diagnostics. */
export interface PromptDiagnosticsSummary {
  trace_id: string
  block_count: number
  active_lane_count: number
  lane_summaries: PromptDiagnosticsLaneSummary[]
  activated_entry_ids: string[]
  activated_entries: PromptDiagnosticsActivatedEntry[]
  suppressed_entry_ids: string[]
  suppressed_entries: PromptDiagnosticsSuppressedEntry[]
  activation_reason_codes: string[]
  activation_reasons: PromptDiagnosticsActivationReason[]
}

/** Stream token payload emitted on the `agent-token` channel. */
export interface StreamTokenPayload {
  stream_id: string
  text?: string
  thinking?: string
  event_type:
    | 'text_delta'
    | 'thinking_delta'
    | 'thinking_start'
    | 'tool_call_update'
    | 'final_text_override'
    | 'stream_complete'
    | 'stream_error'
  tool_call_id?: string
  tool_name?: string
  tool_status?: 'queued' | 'running' | 'completed' | 'error'
  tool_args?: Record<string, unknown>
  tool_result?: string
  tool_duration_ms?: number
  effective_workdir?: string
  policy_decision?: 'allow' | 'deny' | 'prompt'
  evidence_id?: string
  request_id?: string
  task_outcome?: 'completed' | 'partial_success' | 'failed'
  degraded_reason?: string
  resume_available?: boolean
  resume_cursor?: string
  context_budget_usage?: ContextBudgetUsage
  memory_context?: MemoryContextItem[]
  prompt_diagnostics?: PromptDiagnosticsSummary
}

/** Permission prompt event emitted on the `permission-request` channel. */
export interface PermissionRequestPayload {
  session_id: string
  tool_name: string
  permission_mode: string
  current_mode: string
  message: string
}

/** Active agent permission mode (mirrors backend `PermissionMode`). */
export type PermissionMode = 'readOnly' | 'workspaceWrite' | 'dangerFullAccess'

/** Canonical Tauri event names used by the agent loop. Centralised
 * here so the runtime-projection translator and any future dev
 * inspector subscribe against the same constants instead of magic
 * strings. */
export const AGENT_TOKEN_EVENT = 'agent-token'
export const PERMISSION_REQUEST_EVENT = 'permission-request'
export const MEMORY_EVENT = 'memory_event'

/** Phase M3-C closeout — Tauri event name carrying the **batch
 * envelope** emitted by the backend `MemoryCoordinator::after_turn`
 * pipeline at every turn end.  Replaces the short-lived M3-B
 * `memory_write_decision` event.  Empty `decisions` array is
 * meaningful signal — the event fires even when zero candidates
 * were extracted, so "after_turn ran but produced nothing" is now
 * an explicit, observable signal instead of an absence.
 * Payload shape: `MemoryAfterTurnPayload`. */
export const MEMORY_AFTER_TURN_EVENT = 'memory_after_turn'

/** Wire-shape summary of one quality-gate accepted candidate.  Mirrors
 * the Rust `QualityGateAccepted` (`#[serde(rename_all = "camelCase")]`). */
export interface QualityGateAcceptedPayload {
  candidate: MemoryWriteCandidatePayload
  decision: MemoryWriteDecisionPayload
}

/** Wire-shape summary of one quality-gate rejected candidate. */
export interface QualityGateRejectedPayload {
  candidate: MemoryWriteCandidatePayload
  decision: MemoryWriteDecisionPayload
  gateReason: string
}

/** Wire-shape non-blocking warning emitted by the quality gate. */
export interface QualityGateWarningPayload {
  candidateIndex: number
  code: string
  message: string
}

/** Wire-shape result of running the quality gate over a batch. */
export interface QualityGateResultPayload {
  accepted: QualityGateAcceptedPayload[]
  rejected: QualityGateRejectedPayload[]
  warnings: QualityGateWarningPayload[]
  policyVersion: string
}

/** Conflict-resolver outcome enum (mirrors Rust
 * `ConflictResolutionOutcome`, snake_case via serde). */
export type ConflictResolutionOutcome =
  | 'accept_replacement'
  | 'keep_existing'
  | 'require_prompt'
  | 'reject_candidate'
  | 'no_conflict'

/** Wire-shape resolver result for one candidate. */
export interface ConflictResolutionPayload {
  outcome: ConflictResolutionOutcome
  reasonCodes: string[]
  policyVersion: string
}

/** Wire-shape candidate echoed inside `QualityGateAcceptedPayload` /
 * `QualityGateRejectedPayload` (mirrors Rust `MemoryWriteCandidate`,
 * camelCase via serde). */
export interface MemoryWriteCandidatePayload {
  objectKind: MemoryObjectKind
  scope: MemoryScope
  contentPreview: string
  evidenceId?: string
  source: string
}

/** Wire-shape payload for the `memory_after_turn` Tauri event.
 * Carries the full coordinator output for one turn:
 *   - `decisions`  — write-policy stage 1 verdicts
 *   - `quality`    — quality-gate stage 2 result
 *   - `conflicts`  — conflict-resolver stage 3 outcomes (parallel
 *                    to `decisions`, indexed by candidate position)
 * All three arrays may be empty when the turn produced no
 * candidates — that is the explicit no-op signal.
 *
 * Phase M4.1 — `traceVersion` is the **stable governance trace
 * contract version** pinned by the backend
 * (`MEMORY_AFTER_TURN_TRACE_VERSION`).  Future M4 graders /
 * replay tooling MUST honor it; reading it from the wire avoids
 * cross-version misalignment if the trace shape evolves.  Field is
 * optional on the read side because pre-M4.1 emitters did not
 * carry it. */
export interface MemoryAfterTurnPayload {
  /** Phase M4.1 — governance trace contract version (e.g.
   * `"memory-after-turn-trace@m4.1"`). */
  traceVersion?: string
  caller: string
  policyVersion: string
  decidedAt: string
  decisions: MemoryWriteDecisionPayload[]
  quality: QualityGateResultPayload
  conflicts: ConflictResolutionPayload[]
}

// ───────────────────────── M3 memory write decision ────────────────
//
// Phase M3.3 + M3.6 — typed pre-write decision rendered by the
// backend `MemoryCoordinator::after_turn` write-policy gate.
// Mirrors the Rust `MemoryWriteDecision` (camelCase via serde
// `rename_all = "camelCase"`).  No backend event source emits this
// to the frontend yet — the seam exists so M3-B+ persistence /
// audit wiring can dispatch through the same translator without a
// contract bump.  See `runtime-projection/types.ts` for the
// `MemoryWriteDecisionEvent` variant + reducer projection.

/** Canonical pre-write disposition (mirrors Rust
 * `MemoryWriteDisposition`). */
export type MemoryWriteDisposition = 'allow' | 'deny' | 'prompt'

/** Canonical memory object kind (mirrors Rust `MemoryObjectKind`). */
export type MemoryObjectKind =
  | 'fact'
  | 'preference'
  | 'strategy'
  | 'episode'
  | 'unknown'

/** Wire shape of one pre-write decision. */
export interface MemoryWriteDecisionPayload {
  disposition: MemoryWriteDisposition
  reasonCodes: string[]
  objectKind: MemoryObjectKind
  scope: MemoryScope
  /** Optional id of the originating evidence (turn id / tool trace). */
  evidenceId?: string
  /** Stable policy version (`"memory-write-policy@m3.3-skeleton"`). */
  policyVersion: string
  /** RFC3339 decision timestamp. */
  decidedAt: string
}

// ───────────────────────── Canonical projection types ──────────────

/** Canonical memory item projection consumed by the frontend.
 * `bodyPreview` is a server-truncated short preview; full content
 * is fetched on expand via the existing `memory_*` IPC commands. */
export interface MemoryProjection {
  memoryId: string
  kind: MemoryKind
  scope: MemoryScope
  bodyPreview: string
  importanceScore?: number
  origin?: string
  tags: string[]
  /** RFC3339. */
  createdAt: string
  /** RFC3339. */
  lastTouchedAt?: string
  correlation: CorrelationIds
}
