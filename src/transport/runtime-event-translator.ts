// FEAT-INT-001 — RuntimeEventEnvelope → typed evolution event.
//
// Pure function. Returns `null` when:
//   - envelope shape is invalid (missing required fields),
//   - eventType is not one of the 10 evolution variants,
//   - payload is missing a field marked required by the matching
//     `runtime-event-payloads.ts` interface.
//
// Callers (the reducer + the zustand slice) MUST handle `null` as
// "skip this event" — a malformed envelope must NEVER crash the
// reducer.

import type { RuntimeEventEnvelope } from './contracts.ts'
import {
  EVOLUTION_EVENT_TYPES,
  type EvolutionEventPayloadMap,
  type EvolutionEventType,
} from './runtime-event-payloads.ts'

/** A typed evolution event, discriminated on `eventType`. */
export type TypedEvolutionEvent = {
  [K in EvolutionEventType]: {
    readonly eventType: K
    readonly payloadFamily: string
    readonly emittedAt: string
    readonly correlation: RuntimeEventEnvelope['correlation']
    readonly payload: EvolutionEventPayloadMap[K]
  }
}[EvolutionEventType]

const EVOLUTION_TYPE_SET: ReadonlySet<string> = new Set(EVOLUTION_EVENT_TYPES)

function isEvolutionEventType(s: string): s is EvolutionEventType {
  return EVOLUTION_TYPE_SET.has(s)
}

function hasRequiredEnvelopeShape(env: unknown): env is RuntimeEventEnvelope {
  if (env === null || typeof env !== 'object') return false
  const e = env as Record<string, unknown>
  if (typeof e.schemaVersion !== 'string' || e.schemaVersion === '') return false
  if (typeof e.eventType !== 'string') return false
  if (typeof e.payloadFamily !== 'string') return false
  if (typeof e.emittedAt !== 'string') return false
  if (typeof e.correlation !== 'object' || e.correlation === null) return false
  if (e.payload === undefined) return false
  return true
}

function hasField(payload: unknown, key: string): boolean {
  return payload !== null && typeof payload === 'object' && key in (payload as object)
}

/** Required-field guards per evolution event type. Mirrors the
 * `readonly` (non-`?`) properties on the matching payload interface. */
const REQUIRED_FIELDS: Record<EvolutionEventType, readonly string[]> = {
  daemon_health: ['checkName', 'state'],
  skill_sedimented: ['name', 'description', 'toolSequence', 'sourceTurns'],
  compression_event: ['keptTokens', 'dropped', 'passthrough'],
  constitution_violation: ['ruleId', 'severity', 'matchedSnippet', 'explanation'],
  self_edit_proposal: ['id', 'kind', 'target', 'justification'],
  browser_health: ['sessionId', 'status'],
  domain_knowledge: ['entryId', 'kind', 'source'],
  checkpoint_updated: ['sessionId', 'action'],
  verification_decision: ['proposalId', 'verdict', 'failedGates'],
  content_simplified: [
    'originalChars',
    'simplifiedChars',
    'tokenEstimate',
    'compressionRatio',
    'keyElementCount',
  ],
}

function payloadHasAllRequired(eventType: EvolutionEventType, payload: unknown): boolean {
  const required = REQUIRED_FIELDS[eventType]
  return required.every((field) => hasField(payload, field))
}

/** Translate one runtime envelope into a typed evolution event.
 * Returns `null` for non-evolution event types or malformed payloads. */
export function translateEnvelope(envelope: unknown): TypedEvolutionEvent | null {
  if (!hasRequiredEnvelopeShape(envelope)) return null
  const eventType = envelope.eventType
  if (!isEvolutionEventType(eventType)) return null
  if (!payloadHasAllRequired(eventType, envelope.payload)) return null

  return {
    eventType,
    payloadFamily: envelope.payloadFamily,
    emittedAt: envelope.emittedAt,
    correlation: envelope.correlation,
    // SAFETY: required-field guard above narrows payload shape.
    payload: envelope.payload as EvolutionEventPayloadMap[typeof eventType],
  } as TypedEvolutionEvent
}
