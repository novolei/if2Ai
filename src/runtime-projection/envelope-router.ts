// Family-based router for the canonical `runtime_event` Tauri
// channel. Used by `runtime-projection-bridge` to dispatch one
// envelope to the right translator. See:
// docs/superpowers/specs/2026-05-01-runtime-event-channel-cutover-design.md

import type { RuntimeEventEnvelope } from '@/transport/contracts'

export type EnvelopeRoute = (envelope: RuntimeEventEnvelope) => void

export interface FamilyHandler {
  /** Match against `envelope.eventType` (snake_case from the Rust enum). */
  eventType: string
  /**
   * Optional `payload_family` narrowing. When omitted, the handler
   * matches any family for the given `eventType`.
   */
  family?: string | string[]
  handle: EnvelopeRoute
}

export function makeEnvelopeRouter(handlers: FamilyHandler[]): EnvelopeRoute {
  return (envelope) => {
    for (const h of handlers) {
      if (h.eventType !== envelope.eventType) continue
      if (h.family) {
        const list = Array.isArray(h.family) ? h.family : [h.family]
        if (!list.includes(envelope.payloadFamily as string)) continue
      }
      h.handle(envelope)
      return
    }
  }
}
