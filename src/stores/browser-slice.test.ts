import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import {
  clearBrowserSession,
  getBrowserSnapshot,
  setBrowserStatus,
} from './browser-slice.ts'

describe('browser-slice', () => {
  it('hydrates BrowserCard-compatible state during projection transition', () => {
    clearBrowserSession('session-store')
    setBrowserStatus('session-store', {
      running: true,
      url: 'https://example.com',
      thumbnail: 'thumb',
      backend: 'browser_use_mcp',
      title: 'Example',
      takenOver: true,
      lastAction: 'navigate',
      diagnostics: {
        downloads: 1,
        console: 0,
        networkErrors: 2,
      },
      escalationState: 'approval_required',
    })

    const entry = getBrowserSnapshot()['session-store']

    assert.equal(entry.running, true)
    assert.equal(entry.url, 'https://example.com')
    assert.equal(entry.backend, 'browser_use_mcp')
    assert.equal(entry.takenOver, true)
    assert.deepEqual(entry.diagnostics, {
      downloads: 1,
      console: 0,
      networkErrors: 2,
    })
    assert.equal(entry.escalationState, 'approval_required')

    clearBrowserSession('session-store')
  })
})
