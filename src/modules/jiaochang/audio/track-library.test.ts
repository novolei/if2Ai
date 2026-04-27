import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { createPluginTrackCandidate, createPluginTrackCandidates, readStoredAudioLibrary } from './track-library.ts'

describe('jiaochang track library', () => {
  it('creates plugin track metadata without resolving a playable URL', () => {
    const track = createPluginTrackCandidate({
      plugin_id: 'demo.plugin',
      name: 'Demo Source',
      enabled: true,
      allowed_hosts: ['music.example.test'],
      resolver_template: 'https://music.example.test/{source_track_id}?q={quality}',
    }, 'song-123')

    assert.equal(track?.source, 'plugin')
    assert.equal(track?.sourceAdapterId, 'demo.plugin')
    assert.equal(track?.sourceTrackId, 'song-123')
    assert.equal(track?.src, undefined)
  })

  it('expands authorized multi-source manifests into provider candidates', () => {
    const tracks = createPluginTrackCandidates([{
      plugin_id: 'if2ai.authorized-cn-music',
      name: 'Authorized CN Music',
      enabled: true,
      allowed_hosts: ['127.0.0.1'],
      resolver_template: '',
      compatibility: 'authorized_multi_source',
      providers: [
        {
          provider_id: 'kuwo',
          name: '酷我音乐',
          enabled: true,
          allowed_hosts: ['127.0.0.1'],
          resolver_template: 'http://127.0.0.1:43179/{source_track_id}',
        },
        {
          provider_id: 'netease',
          name: '网易云音乐',
          enabled: true,
          allowed_hosts: ['127.0.0.1'],
          resolver_template: 'http://127.0.0.1:43179/{source_track_id}',
        },
      ],
    }], '青花瓷')

    assert.equal(tracks.length, 2)
    assert.equal(tracks[0]?.pluginSource?.source, 'kuwo')
    assert.equal(tracks[1]?.pluginSource?.source, 'netease')
  })

  it('restores persisted local and plugin tracks separately', () => {
    const storage = {
      getItem() {
        return JSON.stringify({
          localTracks: [{ id: 'local:1', title: 'Local', source: 'local' }],
          pluginTracks: [{ id: 'plugin:1', title: 'Plugin', source: 'plugin' }],
        })
      },
    }

    const library = readStoredAudioLibrary(storage)

    assert.equal(library.localTracks.length, 1)
    assert.equal(library.pluginTracks.length, 1)
  })
})
