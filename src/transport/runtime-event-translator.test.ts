// FEAT-INT-001 — node:test unit suite for `translateEnvelope`.
//
// Runs under Node's built-in test runner via the MIG-012
// `scripts/test-loader.mjs` path-alias loader. No vitest/jest
// dependency is added to package.json.

import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { translateEnvelope } from './runtime-event-translator.ts'

const baseEnvelope = {
  schemaVersion: '1.0',
  payloadFamily: 'transition',
  emittedAt: '2026-04-29T12:00:00Z',
  correlation: { sessionId: 'sess-1' },
}

describe('runtime-event-translator', () => {
  it('translates_daemon_health envelope into typed DaemonHealthPayload', () => {
    const envelope = {
      ...baseEnvelope,
      eventType: 'daemon_health',
      payload: {
        checkName: 'memory_ticker_daily_stuck',
        state: 'degraded',
        recoveryAction: 'clear_stuck_state',
        recoveryOutcome: 'repaired',
      },
    }
    const out = translateEnvelope(envelope)
    assert.notEqual(out, null)
    if (out === null) return
    assert.equal(out.eventType, 'daemon_health')
    if (out.eventType === 'daemon_health') {
      assert.equal(out.payload.checkName, 'memory_ticker_daily_stuck')
      assert.equal(out.payload.state, 'degraded')
    }
  })

  it('unknown_event_type_returns_null', () => {
    const envelope = {
      ...baseEnvelope,
      eventType: 'totally_made_up_event',
      payload: {},
    }
    assert.equal(translateEnvelope(envelope), null)
  })

  it('invalid_envelope_returns_null when schema fields are missing', () => {
    assert.equal(translateEnvelope(null), null)
    assert.equal(translateEnvelope({}), null)
    assert.equal(
      translateEnvelope({
        // schemaVersion missing
        eventType: 'daemon_health',
        payloadFamily: 'transition',
        emittedAt: '',
        correlation: {},
        payload: { checkName: 'x', state: 'healthy' },
      }),
      null,
    )
  })

  it('missing_required_payload_field_returns_null', () => {
    const envelope = {
      ...baseEnvelope,
      eventType: 'self_edit_proposal',
      // missing `id`, `kind`, `target`, `justification`
      payload: { stage: 'shadow' },
    }
    assert.equal(translateEnvelope(envelope), null)
  })

  // WU-001 — verify the `App.tsx` listener wiring contract: feeding an
  // envelope through the same `applyEnvelope` chain the production
  // listener uses must populate the evolution store. We import the
  // store fresh so this test stays isolated from earlier tests.
  it('app_listener_calls_apply_envelope', async () => {
    const { createEvolutionEventStore } = await import('../state/evolution-event-store.ts')
    const store = createEvolutionEventStore()
    const before = store.getSnapshot().daemon_health.length

    const envelope = {
      ...baseEnvelope,
      eventType: 'daemon_health',
      payload: {
        checkName: 'memory_ticker_daily_stuck',
        state: 'degraded',
      },
    }
    store.applyEnvelope(envelope)
    const after = store.getSnapshot().daemon_health.length
    assert.equal(after, before + 1, 'applyEnvelope must populate the daemon_health slice')
  })
})
