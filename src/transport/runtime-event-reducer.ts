// FEAT-INT-001 — pure reducer routing one TypedEvolutionEvent into
// the per-family ring buffer slice held by `useEvolutionEventStore`.
//
// Pure function: same `(state, evt)` always yields the same next
// state. No I/O, no side effects, no random ids — keeps the store
// fully testable and rehydratable.

import type {
  CheckpointUpdatedPayload,
  CompressionEventPayload,
  ConstitutionViolationPayload,
  ContentSimplifiedPayload,
  DaemonHealthPayload,
  DomainKnowledgePayload,
  EvolutionEventType,
  SelfEditProposalPayload,
  SkillSedimentedPayload,
  VerificationDecisionPayload,
  BrowserHealthPayload,
} from './runtime-event-payloads.ts'
import type { TypedEvolutionEvent } from './runtime-event-translator.ts'

/** Per-family ring buffer cap. Chosen to cover roughly the last
 * minute of high-traffic evolution events at typical agent rates;
 * tune in a future Pack if needed. */
export const EVOLUTION_RING_BUFFER_CAP = 50

/** Shape held by `useEvolutionEventStore`. Keys are 1:1 with
 * `EvolutionEventType`. */
export interface EvolutionEventState {
  readonly daemon_health: readonly DaemonHealthPayload[]
  readonly skill_sedimented: readonly SkillSedimentedPayload[]
  readonly compression_event: readonly CompressionEventPayload[]
  readonly constitution_violation: readonly ConstitutionViolationPayload[]
  readonly self_edit_proposal: readonly SelfEditProposalPayload[]
  readonly browser_health: readonly BrowserHealthPayload[]
  readonly domain_knowledge: readonly DomainKnowledgePayload[]
  readonly checkpoint_updated: readonly CheckpointUpdatedPayload[]
  readonly verification_decision: readonly VerificationDecisionPayload[]
  readonly content_simplified: readonly ContentSimplifiedPayload[]
}

export const INITIAL_EVOLUTION_EVENT_STATE: EvolutionEventState = {
  daemon_health: [],
  skill_sedimented: [],
  compression_event: [],
  constitution_violation: [],
  self_edit_proposal: [],
  browser_health: [],
  domain_knowledge: [],
  checkpoint_updated: [],
  verification_decision: [],
  content_simplified: [],
}

function pushBounded<T>(buffer: readonly T[], item: T, cap: number): readonly T[] {
  const next = [...buffer, item]
  if (next.length <= cap) return next
  return next.slice(next.length - cap)
}

/** Pure reducer. Append `event.payload` to the matching family slice
 * (bounded by [`EVOLUTION_RING_BUFFER_CAP`]). Unknown event types are
 * impossible at this seam — the translator returns `null` upstream —
 * but we preserve totality with an exhaustive switch. */
export function evolutionEventReducer(
  state: EvolutionEventState,
  event: TypedEvolutionEvent,
): EvolutionEventState {
  const cap = EVOLUTION_RING_BUFFER_CAP
  const t: EvolutionEventType = event.eventType
  switch (t) {
    case 'daemon_health':
      return { ...state, daemon_health: pushBounded(state.daemon_health, event.payload, cap) }
    case 'skill_sedimented':
      return {
        ...state,
        skill_sedimented: pushBounded(state.skill_sedimented, event.payload, cap),
      }
    case 'compression_event':
      return {
        ...state,
        compression_event: pushBounded(state.compression_event, event.payload, cap),
      }
    case 'constitution_violation':
      return {
        ...state,
        constitution_violation: pushBounded(state.constitution_violation, event.payload, cap),
      }
    case 'self_edit_proposal':
      return {
        ...state,
        self_edit_proposal: pushBounded(state.self_edit_proposal, event.payload, cap),
      }
    case 'browser_health':
      return { ...state, browser_health: pushBounded(state.browser_health, event.payload, cap) }
    case 'domain_knowledge':
      return {
        ...state,
        domain_knowledge: pushBounded(state.domain_knowledge, event.payload, cap),
      }
    case 'checkpoint_updated':
      return {
        ...state,
        checkpoint_updated: pushBounded(state.checkpoint_updated, event.payload, cap),
      }
    case 'verification_decision':
      return {
        ...state,
        verification_decision: pushBounded(state.verification_decision, event.payload, cap),
      }
    case 'content_simplified':
      return {
        ...state,
        content_simplified: pushBounded(state.content_simplified, event.payload, cap),
      }
    default: {
      // Exhaustiveness check — if `EvolutionEventType` grows, this
      // line stops compiling.
      const _exhaustive: never = t
      void _exhaustive
      return state
    }
  }
}
