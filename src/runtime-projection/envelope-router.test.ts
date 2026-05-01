import { strict as assert } from 'node:assert'
import { describe, it, mock } from 'node:test'

import type { RuntimeEventEnvelope } from '@/transport/contracts'
import { makeEnvelopeRouter } from './envelope-router.ts'

function envelope(eventType: string, family: string): RuntimeEventEnvelope {
  return {
    schemaVersion: { major: 1, minor: 0, patch: 0 } as never,
    eventType: eventType as never,
    payloadFamily: family,
    emittedAt: '2026-05-01T00:00:00Z',
    correlation: {},
    payload: {},
  } as RuntimeEventEnvelope
}

describe('makeEnvelopeRouter', () => {
  it('dispatches to the matching family handler', () => {
    const a = mock.fn()
    const b = mock.fn()
    const router = makeEnvelopeRouter([
      { eventType: 'conversation', family: 'text_delta', handle: a },
      { eventType: 'permission', handle: b },
    ])
    router(envelope('conversation', 'text_delta'))
    router(envelope('permission', 'prompt_opened'))
    assert.equal(a.mock.callCount(), 1)
    assert.equal(b.mock.callCount(), 1)
  })

  it('skips unmatched envelopes silently', () => {
    const a = mock.fn()
    const router = makeEnvelopeRouter([
      { eventType: 'conversation', family: 'text_delta', handle: a },
    ])
    router(envelope('conversation', 'thinking_delta'))
    router(envelope('memory', 'lifecycle'))
    assert.equal(a.mock.callCount(), 0)
  })

  it('matches on family list', () => {
    const a = mock.fn()
    const router = makeEnvelopeRouter([
      {
        eventType: 'conversation',
        family: ['text_delta', 'thinking_delta'],
        handle: a,
      },
    ])
    router(envelope('conversation', 'text_delta'))
    router(envelope('conversation', 'thinking_delta'))
    assert.equal(a.mock.callCount(), 2)
  })
})
