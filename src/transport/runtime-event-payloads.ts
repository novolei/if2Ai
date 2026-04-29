// FEAT-INT-001 — Typed payload contracts for the 10 Agent Evolution
// runtime event families. Each interface mirrors the matching Rust
// payload struct field-for-field (camelCase ↔ snake_case via serde).
//
// The translator (`runtime-event-translator.ts`) returns one of these
// shapes; the reducer (`runtime-event-reducer.ts`) routes on
// `eventType` to the right slice in `useEvolutionEventStore`.
//
// Important: **DO NOT add fields here without also adding them to the
// matching Rust struct.** The Pack contract requires 1:1 field
// alignment so a future MessagePack / wire-narrowing slice can flip
// to a typed enum without re-deriving the contract.

/** SH-001~003 — daemon health transitions / liveness probe results. */
export interface DaemonHealthPayload {
  readonly checkName: string
  /** `'healthy' | 'degraded' | 'failed'` (DaemonState) */
  readonly state: 'healthy' | 'degraded' | 'recovering' | 'failed'
  readonly recoveryAction?: string
  readonly recoveryOutcome?: 'repaired' | 'no_op_skipped' | 'failed'
}

/** SE-001 — a new SkillDraft was sedimented from a session. */
export interface SkillSedimentedPayload {
  readonly name: string
  readonly description: string
  readonly toolSequence: readonly string[]
  readonly sourceTurns: readonly number[]
}

/** TE-001..004 — a context-compression / digest pass ran. */
export interface CompressionEventPayload {
  readonly keptTokens: number
  readonly dropped: number
  readonly passthrough: boolean
  readonly tier?: 'system' | 'compressed_history' | 'working_checkpoint' | 'recent_messages' | 'tool_buffer'
}

/** SE-003 — a constitution rule fired against a draft / proposal. */
export interface ConstitutionViolationPayload {
  readonly ruleId: string
  readonly severity: 'critical' | 'high' | 'medium'
  readonly matchedSnippet: string
  readonly explanation: string
}

/** AE-001~003 — a self-edit proposal lifecycle event. */
export interface SelfEditProposalPayload {
  readonly id: string
  readonly kind: 'prompt_tweak' | 'tool_guard_rule' | 'skill_draft' | 'retry_policy'
  readonly target: string
  readonly justification: string
  readonly stage?: 'shadow' | 'canary_1pct' | 'canary_10pct' | 'production'
}

/** SH-003 / BR-002 — browser session health / coordinate strategy decision. */
export interface BrowserHealthPayload {
  readonly sessionId: string
  readonly status: 'connected' | 'stale' | 'disconnected' | 'recovering' | 'crashed'
  readonly recoveryPlan?: 'reconnect' | 'restart_process' | 'escalate_to_cloud' | 'no_action'
}

/** DK-001 / DK-003 — domain-knowledge entry lookup or auto-contribution. */
export interface DomainKnowledgePayload {
  readonly entryId: string
  readonly kind: 'website_domain' | 'interaction_primitive' | 'task_sop'
  readonly source: 'lookup' | 'contribution'
  readonly accessCount?: number
}

/** DK-002 — a working checkpoint was extracted or injected. */
export interface CheckpointUpdatedPayload {
  readonly sessionId: string
  readonly action: 'extracted' | 'injected' | 'cleared'
  readonly keyInfoTokens?: number
  readonly relatedSop?: string
}

/** AE-002 — verification verdict (pass / fail with gates). */
export interface VerificationDecisionPayload {
  readonly proposalId: string
  readonly verdict: 'pass' | 'fail'
  readonly failedGates: readonly string[]
}

/** BR-001 — a SimplifiedContent payload was produced for a web_scan. */
export interface ContentSimplifiedPayload {
  readonly originalChars: number
  readonly simplifiedChars: number
  readonly tokenEstimate: number
  readonly compressionRatio: number
  readonly keyElementCount: number
}

/** Discriminated-union map keyed by RuntimeEventType. */
export interface EvolutionEventPayloadMap {
  readonly daemon_health: DaemonHealthPayload
  readonly skill_sedimented: SkillSedimentedPayload
  readonly compression_event: CompressionEventPayload
  readonly constitution_violation: ConstitutionViolationPayload
  readonly self_edit_proposal: SelfEditProposalPayload
  readonly browser_health: BrowserHealthPayload
  readonly domain_knowledge: DomainKnowledgePayload
  readonly checkpoint_updated: CheckpointUpdatedPayload
  readonly verification_decision: VerificationDecisionPayload
  readonly content_simplified: ContentSimplifiedPayload
}

/** All evolution event-type literal values, for `Object.keys` style enumeration. */
export const EVOLUTION_EVENT_TYPES = [
  'daemon_health',
  'skill_sedimented',
  'compression_event',
  'constitution_violation',
  'self_edit_proposal',
  'browser_health',
  'domain_knowledge',
  'checkpoint_updated',
  'verification_decision',
  'content_simplified',
] as const satisfies readonly (keyof EvolutionEventPayloadMap)[]

export type EvolutionEventType = (typeof EVOLUTION_EVENT_TYPES)[number]
