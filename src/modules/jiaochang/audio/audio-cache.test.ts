import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { InMemoryResolvedUrlCache, isCacheableResolvedUrl } from './audio-cache.ts'

describe('InMemoryResolvedUrlCache', () => {
  it('stores cacheable resolved track URLs by track and quality', () => {
    const cache = new InMemoryResolvedUrlCache()
    cache.put({
      trackId: 'track-1',
      url: '/one.mp3',
      source: 'local',
      sourceAdapterId: 'local',
      quality: 'standard',
      resolvedAt: new Date().toISOString(),
      evidence: 'local grant',
    })

    assert.equal(cache.get('track-1', 'standard')?.url, '/one.mp3')
    assert.equal(cache.get('track-1', 'high'), null)
  })

  it('does not mark service/plugin URLs as cacheable by default', () => {
    assert.equal(isCacheableResolvedUrl({
      trackId: 'track-2',
      url: 'https://example.invalid/two.mp3',
      source: 'plugin',
      sourceAdapterId: 'plugin',
      quality: 'standard',
      resolvedAt: new Date().toISOString(),
      evidence: 'plugin adapter',
    }), false)
  })
})
