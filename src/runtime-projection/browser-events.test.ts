import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { translateBrowserStatusPayload } from './browser-events.ts'
import { reduceRuntimeEvent } from './runtime-event-reducer.ts'
import { emptyProjectionSnapshot } from './types.ts'

describe('runtime projection — smart browser', () => {
  it('projects_smart_browser_events', () => {
    const event = translateBrowserStatusPayload(
      {
        session_id: 'session-1',
        running: true,
        url: 'https://example.com',
        thumbnail: 'abc',
        backend: 'browser_use_mcp',
        title: 'Example',
        taken_over: true,
        last_action: 'navigate',
        downloads_count: 1,
        console_count: 2,
        network_error_count: 3,
        escalation_state: 'suggested',
      },
      123,
    )

    const snapshot = reduceRuntimeEvent(emptyProjectionSnapshot(), event)
    const browser = snapshot.browsers['session-1']

    assert.equal(browser.sessionId, 'session-1')
    assert.equal(browser.backend, 'browser_use_mcp')
    assert.equal(browser.running, true)
    assert.equal(browser.url, 'https://example.com')
    assert.equal(browser.takenOver, true)
    assert.deepEqual(browser.diagnostics, {
      downloads: 1,
      console: 2,
      networkErrors: 3,
    })
  })

  it('preserves_browser_projection_fields', () => {
    const event = translateBrowserStatusPayload(
      {
        session_id: 'session-2',
        running: false,
        url: null,
        thumbnail: null,
      },
      456,
    )

    assert.equal(event.backend, 'local_rust_cdp')
    assert.equal(event.title, null)
    assert.equal(event.takenOver, false)
    assert.equal(event.escalationState, 'none')
    assert.deepEqual(event.diagnostics, {
      downloads: 0,
      console: 0,
      networkErrors: 0,
    })
  })
})
