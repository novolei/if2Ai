// ER-02 — coverage for the models facade additions:
// `fetchAvailableModelGroups()` + `subscribeModelsChanged()`.
// `useAvailableModels()` itself depends on React's useState/useEffect
// runtime which can't execute without a renderer; we cover its
// declaration via the source-text assertion below.

import { strict as assert } from 'node:assert'
import { readFileSync } from 'node:fs'
import { afterEach, describe, it } from 'node:test'

import {
  type ApiClient,
  type ApiEvent,
  resetApiClient,
  setApiClient,
} from '../client.ts'
import {
  fetchAvailableModelGroups,
  subscribeModelsChanged,
} from '../models.ts'

const source = readFileSync(new URL('../models.ts', import.meta.url), 'utf8')

afterEach(() => {
  resetApiClient()
})

describe('api/models — useAvailableModels hook declaration', () => {
  it('exports useAvailableModels with the documented return shape', () => {
    assert.match(source, /export function useAvailableModels\(\)/)
    assert.match(source, /groups: AvailableModelGroup\[\]/)
    assert.match(source, /loading: boolean/)
    assert.match(source, /refetch: \(\) => void/)
  })
})

describe('api/models — fetchAvailableModelGroups returns the wire array shape', () => {
  it('forwards to model_list_available and returns the groups array', async () => {
    const sample = [
      {
        provider_id: 'anthropic',
        provider_name: 'Anthropic',
        models: [{ model_id: 'claude-4', name: 'Claude 4' }],
      },
    ]
    const calls: string[] = []
    const client: ApiClient = {
      async call<T>(command: string): Promise<T> {
        calls.push(command)
        return sample as unknown as T
      },
      async subscribe<T>(_event: string, _handler: (e: ApiEvent<T>) => void) {
        return () => {}
      },
    }
    setApiClient(client)

    const groups = await fetchAvailableModelGroups()
    assert.equal(calls[0], 'model_list_available')
    assert.equal(Array.isArray(groups), true)
    assert.equal(groups.length, 1)
    assert.equal(groups[0].provider_id, 'anthropic')
  })

  it('returns [] when the underlying call rejects', async () => {
    const client: ApiClient = {
      async call<T>(): Promise<T> {
        throw new Error('boom')
      },
      async subscribe<T>(_event: string, _handler: (e: ApiEvent<T>) => void) {
        return () => {}
      },
    }
    setApiClient(client)

    const groups = await fetchAvailableModelGroups()
    assert.deepEqual(groups, [])
  })
})

describe('api/models — subscribeModelsChanged listener cleanup', () => {
  it('returns a disposer that detaches both Tauri + window listeners', async () => {
    let unlistenInvoked = 0
    const subscribed: string[] = []
    const client: ApiClient = {
      async call<T>(): Promise<T> {
        return undefined as unknown as T
      },
      async subscribe<T>(event: string, _handler: (e: ApiEvent<T>) => void) {
        subscribed.push(event)
        return () => {
          unlistenInvoked += 1
        }
      },
    }
    setApiClient(client)

    // Provide a minimal window stub so the hook's
    // `window.addEventListener` path executes without a real DOM.
    const previousWindow = (globalThis as { window?: unknown }).window
    let windowAdded = 0
    let windowRemoved = 0
    ;(globalThis as { window?: unknown }).window = {
      addEventListener: () => {
        windowAdded += 1
      },
      removeEventListener: () => {
        windowRemoved += 1
      },
    }

    try {
      const dispose = subscribeModelsChanged(() => {})
      assert.equal(typeof dispose, 'function')
      assert.equal(windowAdded, 1)

      // Wait a tick so the deferred Tauri subscribe resolves and
      // the disposer captures the unlisten function.
      await new Promise((r) => setTimeout(r, 0))
      dispose()

      assert.equal(windowRemoved, 1)
      assert.equal(unlistenInvoked, 1)
      assert.deepEqual(subscribed, ['if2ai://models-changed'])
    } finally {
      if (previousWindow === undefined) {
        delete (globalThis as { window?: unknown }).window
      } else {
        ;(globalThis as { window?: unknown }).window = previousWindow
      }
    }
  })
})
