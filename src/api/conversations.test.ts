// MIG-015 — canonical conversations gateway surface tests.
//
// Runs under Node's built-in `node --test` with the MIG-012
// path-alias loader. Verifies that `startChatTurn(...)` is the
// single entry point chat workspaces consume — the returned
// handle bundles streaming + cancellation + permission
// response so callers never reach into individual Tauri
// command names.
//
// Pack §10 acceptance: "至少一条 conversations / streaming /
// permission surface 测试通过".

import { strict as assert } from 'node:assert'
import { afterEach, describe, it } from 'node:test'

import {
  type ApiClient,
  type ApiEvent,
  resetApiClient,
  setApiClient,
} from './client.ts'
import { loadConversationHistory, startChatTurn } from './conversations.ts'

interface RecordedCall {
  command: string
  args?: Record<string, unknown>
}

function recordingClient(returns: Record<string, unknown> = {}): {
  client: ApiClient
  calls: RecordedCall[]
  subscriptions: { event: string; handlerCount: number }[]
} {
  const calls: RecordedCall[] = []
  const subscriptions: { event: string; handlerCount: number }[] = []
  const client: ApiClient = {
    async call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
      calls.push({ command, args })
      return (returns[command] ?? undefined) as T
    },
    async subscribe<T>(event: string, _handler: (e: ApiEvent<T>) => void) {
      const existing = subscriptions.find((s) => s.event === event)
      if (existing) existing.handlerCount += 1
      else subscriptions.push({ event, handlerCount: 1 })
      return () => {}
    },
  }
  return { client, calls, subscriptions }
}

afterEach(() => {
  resetApiClient()
})

describe('api/conversations — startChatTurn handle bundles every chat-turn primitive', () => {
  it('startChatTurn opens the stream via `start_agent_stream` and returns the backend stream id', async () => {
    const { client, calls } = recordingClient({ start_agent_stream: 'stream-canonical-1' })
    setApiClient(client)
    const handle = await startChatTurn({
      sessionId: 'session-7',
      userMessage: 'hello world',
      permissionMode: 'workspaceWrite',
    })
    assert.equal(handle.streamId, 'stream-canonical-1')
    assert.deepEqual(calls, [
      {
        command: 'start_agent_stream',
        args: {
          sessionId: 'session-7',
          userMessage: 'hello world',
          permissionMode: 'workspaceWrite',
        },
      },
    ])
  })

  it('handle.subscribe forwards to the runtime_event envelope channel (PR D-1)', async () => {
    const { client, subscriptions } = recordingClient({ start_agent_stream: 'sx' })
    setApiClient(client)
    const handle = await startChatTurn({ sessionId: 's', userMessage: 'hi' })
    await handle.subscribe(() => {})
    assert.equal(subscriptions.length, 1)
    assert.equal(
      subscriptions[0].event,
      'runtime_event',
      'PR D-1 retired the legacy agent-token channel; subscribers must read from runtime_event',
    )
    assert.equal(subscriptions[0].handlerCount, 1)
  })

  it('handle.stop forwards to `stop_agent_stream` with the bound stream id', async () => {
    const { client, calls } = recordingClient({ start_agent_stream: 'sx-stop' })
    setApiClient(client)
    const handle = await startChatTurn({ sessionId: 's', userMessage: 'hi' })
    await handle.stop()
    const stopCall = calls.find((c) => c.command === 'stop_agent_stream')
    assert.ok(stopCall, 'stop_agent_stream must have been called')
    assert.deepEqual(stopCall!.args, { streamId: 'sx-stop' })
  })

  it('handle.respondPermission binds the original sessionId and forwards toolName + scope', async () => {
    const { client, calls } = recordingClient({ start_agent_stream: 's-id' })
    setApiClient(client)
    const handle = await startChatTurn({
      sessionId: 'permission-session',
      userMessage: 'do thing',
    })
    await handle.respondPermission({
      decision: 'allow',
      toolName: 'bash',
      scope: 'session',
    })
    const respCall = calls.find((c) => c.command === 'respond_permission')
    assert.ok(respCall, 'respond_permission must have been called')
    assert.deepEqual(respCall!.args, {
      sessionId: 'permission-session',
      decision: 'allow',
      toolName: 'bash',
      scope: 'session',
    })
  })

  it('handle.respondPermission cannot be misrouted to a different session even if the chat workspace forgets', async () => {
    // Guards pack §8 guardrail 2: the canonical surface MUST
    // bind the permission decision to the original sessionId
    // (the chat workspace does not pass sessionId again — the
    // handle remembers it). If a future refactor accidentally
    // accepts a session override on `respondPermission`, this
    // test pins the contract.
    const { client, calls } = recordingClient({ start_agent_stream: 's-id' })
    setApiClient(client)
    const handle = await startChatTurn({
      sessionId: 'original-session',
      userMessage: 'x',
    })
    await handle.respondPermission({ decision: 'deny' })
    const respCall = calls.find((c) => c.command === 'respond_permission')
    assert.equal(respCall!.args!.sessionId, 'original-session')
  })

  it('loadConversationHistory wraps `get_session` so chat surfaces never call the raw command', async () => {
    const { client, calls } = recordingClient({
      get_session: { id: 'sess-1', title: 'T', messages: [] } as unknown,
    })
    setApiClient(client)
    const session = await loadConversationHistory('sess-1')
    assert.equal((session as { id: string }).id, 'sess-1')
    assert.deepEqual(calls, [{ command: 'get_session', args: { id: 'sess-1' } }])
  })
})
