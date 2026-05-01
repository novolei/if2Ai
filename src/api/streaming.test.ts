// PR D-1 — agent-token retirement.
//
// `listenToStream(streamId, handler)` historically subscribed to
// the legacy `agent-token` Tauri channel and forwarded any
// payload whose `stream_id` matched. PR C-1 wrapped emissions in
// the canonical `runtime_event` envelope; PR D-1 retires the
// legacy channel entirely. After D-1, `listenToStream` MUST:
//
// 1. Subscribe to `runtime_event` (NOT `agent-token`).
// 2. Filter envelopes by `correlation.streamId === streamId`.
// 3. Hand the user the original `StreamTokenPayload` (envelope's
//    `payload` field), not the wrapping envelope.
//
// These tests pin that behavior so a regression cannot silently
// reintroduce the legacy channel or skip the per-stream filter.

import { strict as assert } from 'node:assert'
import { afterEach, describe, it } from 'node:test'

import {
  type ApiClient,
  type ApiEvent,
  resetApiClient,
  setApiClient,
} from './client.ts'
import { listenToStream } from './streaming.ts'
import type {
  RuntimeEventEnvelope,
  StreamTokenPayload,
} from '@/transport/contracts'

interface DispatchableClient {
  client: ApiClient
  subscribedEvents: string[]
  dispatch: (envelope: RuntimeEventEnvelope) => void
}

function dispatchableClient(): DispatchableClient {
  const subscribedEvents: string[] = []
  const handlers: ((event: ApiEvent<RuntimeEventEnvelope>) => void)[] = []
  const client: ApiClient = {
    async call() {
      throw new Error('not used in this test')
    },
    async subscribe<T>(event: string, handler: (e: ApiEvent<T>) => void) {
      subscribedEvents.push(event)
      handlers.push(handler as (e: ApiEvent<RuntimeEventEnvelope>) => void)
      return () => {}
    },
  }
  return {
    client,
    subscribedEvents,
    dispatch: (envelope) => {
      for (const h of handlers) h({ payload: envelope })
    },
  }
}

function envelope(
  streamId: string,
  payload: Partial<StreamTokenPayload> & { event_type: StreamTokenPayload['event_type'] },
): RuntimeEventEnvelope<StreamTokenPayload> {
  return {
    schemaVersion: '1.0.0-skeleton',
    eventType: 'conversation',
    payloadFamily: payload.event_type,
    emittedAt: new Date().toISOString(),
    correlation: { streamId },
    payload: {
      stream_id: streamId,
      ...payload,
    } as StreamTokenPayload,
  }
}

afterEach(() => {
  resetApiClient()
})

describe('api/streaming.listenToStream — PR D-1 runtime_event cut-over', () => {
  it('subscribes to the runtime_event channel (NOT agent-token)', async () => {
    const { client, subscribedEvents } = dispatchableClient()
    setApiClient(client)
    await listenToStream('stream-A', () => {})
    assert.equal(subscribedEvents.length, 1)
    assert.equal(
      subscribedEvents[0],
      'runtime_event',
      'D-1 retires agent-token; listenToStream must read runtime_event envelopes',
    )
  })

  it('forwards the inner StreamTokenPayload when correlation.streamId matches', async () => {
    const { client, dispatch } = dispatchableClient()
    setApiClient(client)
    const received: StreamTokenPayload[] = []
    await listenToStream('stream-A', (p) => received.push(p))
    dispatch(envelope('stream-A', { event_type: 'text_delta', text: 'hi' }))
    assert.equal(received.length, 1)
    assert.equal(received[0].event_type, 'text_delta')
    assert.equal(received[0].text, 'hi')
    assert.equal(received[0].stream_id, 'stream-A')
  })

  it('filters out envelopes whose correlation.streamId does not match', async () => {
    const { client, dispatch } = dispatchableClient()
    setApiClient(client)
    const received: StreamTokenPayload[] = []
    await listenToStream('stream-A', (p) => received.push(p))
    dispatch(envelope('stream-B', { event_type: 'text_delta', text: 'wrong' }))
    dispatch(envelope('stream-A', { event_type: 'stream_complete' }))
    assert.equal(received.length, 1, 'cross-stream payloads must be dropped')
    assert.equal(received[0].event_type, 'stream_complete')
  })

  it('falls back to payload.stream_id when envelope.correlation.streamId is missing', async () => {
    // Defensive: backend `to_envelope()` always promotes stream_id
    // into correlation, but if any future emit path forgets to,
    // listenToStream should still match on the payload field so we
    // never silently drop events.
    const { client, dispatch } = dispatchableClient()
    setApiClient(client)
    const received: StreamTokenPayload[] = []
    await listenToStream('stream-A', (p) => received.push(p))
    const env: RuntimeEventEnvelope<StreamTokenPayload> = {
      schemaVersion: '1.0.0-skeleton',
      eventType: 'conversation',
      payloadFamily: 'text_delta',
      emittedAt: new Date().toISOString(),
      correlation: {}, // no streamId here
      payload: {
        stream_id: 'stream-A',
        event_type: 'text_delta',
        text: 'fallback',
      } as StreamTokenPayload,
    }
    dispatch(env)
    assert.equal(received.length, 1)
    assert.equal(received[0].text, 'fallback')
  })
})
