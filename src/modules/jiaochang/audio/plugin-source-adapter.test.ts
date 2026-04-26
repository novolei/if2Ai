import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import type { JiaochangMusicTrack } from './audio-state.ts'
import { createPluginSourceAdapter, type JiaochangPluginResolvedTrack } from './plugin-source-adapter.ts'

describe('createPluginSourceAdapter', () => {
  it('delegates playable URL resolution to the Rust command bridge', async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = []
    const adapter = createPluginSourceAdapter(async <T,>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args })
      return ({
        track_id: 'plugin-track-1',
        plugin_id: 'demo.plugin',
        url: 'https://music.example.test/demo/plugin-track-1.mp3',
        quality: 'standard',
        cache_hit: false,
        resolved_at: '2026-04-26T00:00:00Z',
        evidence: 'rust-side plugin worker resolved URL from constrained manifest',
      } satisfies JiaochangPluginResolvedTrack) as T
    })
    const track: JiaochangMusicTrack = {
      id: 'plugin-track-1',
      title: 'Plugin Track',
      artist: 'Demo',
      source: 'plugin',
      sourceAdapterId: 'demo.plugin',
      sourceTrackId: 'source-1',
      mood: 'focus',
      pluginSource: {
        pluginId: 'demo.plugin',
        source: 'demo',
      },
    }

    const resolved = await adapter.resolveTrackUrl(track, 'standard')

    assert.equal(calls[0]?.command, 'jiaochang_audio_plugin_resolve_track_url')
    assert.equal(resolved.url, 'https://music.example.test/demo/plugin-track-1.mp3')
    assert.equal(resolved.sourceAdapterId, 'demo.plugin')
  })
})
