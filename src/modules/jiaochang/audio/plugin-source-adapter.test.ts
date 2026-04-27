import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import type { JiaochangMusicTrack } from './audio-state.ts'
import { createPluginSourceAdapter, importJiaochangAudioLxCeruJsFile, installAuthorizedChineseMusicSourceTemplate, inspectJiaochangAudioPluginJsFile, removeJiaochangAudioPluginSource, searchJiaochangAudioPluginTracks, type JiaochangPluginResolvedTrack } from './plugin-source-adapter.ts'

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
        musicInfo: { id: 'source-1', name: 'Plugin Track' },
      },
    }

    const resolved = await adapter.resolveTrackUrl(track, 'standard')

    assert.equal(calls[0]?.command, 'jiaochang_audio_plugin_resolve_track_url')
    assert.deepEqual((calls[0]?.args?.input as Record<string, unknown>).plugin_music_info, { id: 'source-1', name: 'Plugin Track' })
    assert.equal(resolved.url, 'https://music.example.test/demo/plugin-track-1.mp3')
    assert.equal(resolved.sourceAdapterId, 'demo.plugin')
  })

  it('dispatches authorized template install and JS inspection through command bridge', async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = []
    const invoke = async <T,>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args })
      return {} as T
    }

    await installAuthorizedChineseMusicSourceTemplate(invoke)
    await inspectJiaochangAudioPluginJsFile('/tmp/plugin.js', invoke)
    await importJiaochangAudioLxCeruJsFile('/tmp/plugin.js', invoke)
    await searchJiaochangAudioPluginTracks({ plugin_id: 'lx-ceru.plugin', source: 'qsvip', query: '青花瓷' }, invoke)
    await removeJiaochangAudioPluginSource('lx-ceru.plugin', invoke)

    assert.deepEqual(calls, [
      { command: 'jiaochang_audio_plugin_install_authorized_cn_template', args: undefined },
      { command: 'jiaochang_audio_plugin_inspect_js_file', args: { path: '/tmp/plugin.js' } },
      { command: 'jiaochang_audio_plugin_import_lx_ceru_js_file', args: { path: '/tmp/plugin.js' } },
      { command: 'jiaochang_audio_plugin_search_tracks', args: { input: { plugin_id: 'lx-ceru.plugin', source: 'qsvip', query: '青花瓷' } } },
      { command: 'jiaochang_audio_plugin_remove', args: { pluginId: 'lx-ceru.plugin' } },
    ])
  })
})
