import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import {
  draftToPluginManifest,
  parseAllowedHosts,
  validatePluginManifestDraft,
  type JiaochangPluginManifestDraft,
} from './plugin-source-settings.ts'

describe('jiaochang plugin source settings helpers', () => {
  it('normalizes comma and newline separated allowed hosts', () => {
    assert.deepEqual(parseAllowedHosts('music.example.test, cdn.example.test\nmusic.example.test'), [
      'music.example.test',
      'cdn.example.test',
    ])
  })

  it('validates required manifest fields before register command', () => {
    const draft: JiaochangPluginManifestDraft = {
      pluginId: 'demo.plugin',
      name: 'Demo Plugin',
      version: '',
      enabled: true,
      allowedHosts: 'music.example.test',
      resolverTemplate: 'https://music.example.test/{source_track_id}?q={quality}',
    }

    assert.equal(validatePluginManifestDraft(draft), null)
    assert.deepEqual(draftToPluginManifest(draft), {
      plugin_id: 'demo.plugin',
      name: 'Demo Plugin',
      version: undefined,
      enabled: true,
      allowed_hosts: ['music.example.test'],
      resolver_template: 'https://music.example.test/{source_track_id}?q={quality}',
    })
  })
})
