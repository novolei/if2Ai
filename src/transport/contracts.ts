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
