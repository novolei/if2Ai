import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { InMemoryResolvedUrlCache } from './audio-cache.ts'
import type { JiaochangMusicTrack } from './audio-state.ts'
import { resolveTrackUrl } from './resolve-track-url.ts'
import { createBundledSourceAdapter, createDisabledPluginSourceAdapter, createLocalSourceAdapter } from './source-adapters.ts'

describe('resolveTrackUrl', () => {
  it('resolves bundled metadata through a source adapter before playback', async () => {
    const track: JiaochangMusicTrack = {
      id: 'bundled:training',
      title: 'Training Bell',
      source: 'bundled',
      sourceAdapterId: 'bundled',
      src: '/assets/training.mp3',
      mood: 'focus',
    }

    const resolved = await resolveTrackUrl({
      track,
      adapters: [createBundledSourceAdapter([track])],
    })

    assert.equal(resolved.url, '/assets/training.mp3')
    assert.equal(resolved.sourceAdapterId, 'bundled')
    assert.match(resolved.evidence, /packaged asset/)
  })

  it('uses cache before consulting the source adapter', async () => {
    const track: JiaochangMusicTrack = {
      id: 'bundled:cached',
      title: 'Cached Bell',
      source: 'bundled',
      sourceAdapterId: 'bundled',
      src: '/assets/cached.mp3',
      mood: 'focus',
    }
    const cache = new InMemoryResolvedUrlCache()
    cache.put({
      trackId: track.id,
      url: '/cached/cached.mp3',
      source: 'bundled',
      sourceAdapterId: 'bundled',
      quality: 'standard',
      resolvedAt: new Date().toISOString(),
      evidence: 'seeded test cache',
    })

    const resolved = await resolveTrackUrl({
      track,
      adapters: [createBundledSourceAdapter([{ ...track, src: '/wrong.mp3' }])],
      cache,
    })

    assert.equal(resolved.url, '/cached/cached.mp3')
    assert.equal(resolved.source, 'cache')
  })

  it('rejects local tracks without persistent grant evidence', async () => {
    const track: JiaochangMusicTrack = {
      id: 'local:missing',
      title: 'Missing Local Track',
      source: 'local',
      sourceAdapterId: 'local',
      mood: 'focus',
    }

    await assert.rejects(
      resolveTrackUrl({ track, adapters: [createLocalSourceAdapter()] }),
      /Local file grant is missing or revoked/,
    )
  })

  it('keeps plugin sources disabled until the Rust side isolate pack', async () => {
    const track: JiaochangMusicTrack = {
      id: 'plugin:future',
      title: 'Future Plugin Track',
      source: 'plugin',
      sourceAdapterId: 'plugin',
      mood: 'focus',
    }

    await assert.rejects(
      resolveTrackUrl({ track, adapters: [createDisabledPluginSourceAdapter()] }),
      /adapter is disabled/,
    )
  })
})
