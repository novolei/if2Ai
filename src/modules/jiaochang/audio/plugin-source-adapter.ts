import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import type { JiaochangAudioQuality, JiaochangMusicTrack, JiaochangResolvedTrackUrl } from './audio-state.ts'
import type { JiaochangSourceAdapter } from './source-adapters.ts'

export const JIACHANG_PLUGIN_AUDIO_EVENT = 'jiaochang://audio/plugin-event'

export interface JiaochangAudioPluginManifest {
  plugin_id: string
  name: string
  version?: string
  enabled: boolean
  allowed_hosts: string[]
  resolver_template: string
}

export interface JiaochangPluginTrackResolveInput {
  track_id: string
  plugin_id: string
  source: string
  title: string
  artist?: string
  quality: string
  source_track_id?: string
  use_cache: boolean
}

export interface JiaochangPluginResolvedTrack {
  track_id: string
  plugin_id: string
  url: string
  quality: string
  cache_hit: boolean
  resolved_at: string
  evidence: string
}

export interface JiaochangAudioPluginRuntimeEvent {
  event_type: 'registered' | 'resolving' | 'resolved' | 'cache_hit' | 'error'
  plugin_id: string
  track_id?: string
  message: string
  cache_hit?: boolean
  url?: string
  timestamp: string
}

export type JiaochangPluginInvoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>

export function createPluginSourceAdapter(invokeCommand: JiaochangPluginInvoke = invoke): JiaochangSourceAdapter {
  return {
    id: 'plugin',
    source: 'plugin',
    enabled: true,
    async listTracks() {
      return []
    },
    async resolveTrackUrl(track, quality) {
      if (track.source !== 'plugin') {
        throw createPluginAdapterError('track_missing', track.id, 'Track is not a plugin source track.')
      }
      const pluginId = track.pluginSource?.pluginId ?? track.sourceAdapterId
      if (!pluginId || pluginId === 'plugin') {
        throw createPluginAdapterError('adapter_missing', track.id, 'Plugin source track has no plugin id.')
      }
      const resolved = await invokeCommand<JiaochangPluginResolvedTrack>('jiaochang_audio_plugin_resolve_track_url', {
        input: toPluginResolveInput(track, quality, pluginId),
      })
      return {
        trackId: track.id,
        url: resolved.url,
        source: resolved.cache_hit ? 'cache' : 'plugin',
        sourceAdapterId: pluginId,
        quality,
        cacheKey: `${pluginId}::${track.sourceTrackId ?? track.id}::${quality}`,
        resolvedAt: resolved.resolved_at,
        evidence: resolved.evidence,
      }
    },
  }
}

export async function registerJiaochangAudioPluginSource(
  manifest: JiaochangAudioPluginManifest,
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangAudioPluginManifest> {
  return invokeCommand<JiaochangAudioPluginManifest>('jiaochang_audio_plugin_register', { manifest })
}

export async function listJiaochangAudioPluginSources(
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangAudioPluginManifest[]> {
  return invokeCommand<JiaochangAudioPluginManifest[]>('jiaochang_audio_plugin_list')
}

export function subscribeJiaochangAudioPluginEvents(
  listener: (event: JiaochangAudioPluginRuntimeEvent) => void,
): Promise<UnlistenFn> {
  return listen<JiaochangAudioPluginRuntimeEvent>(JIACHANG_PLUGIN_AUDIO_EVENT, (event) => {
    listener(event.payload)
  })
}

function toPluginResolveInput(
  track: JiaochangMusicTrack,
  quality: JiaochangAudioQuality,
  pluginId: string,
): JiaochangPluginTrackResolveInput {
  return {
    track_id: track.id,
    plugin_id: pluginId,
    source: track.pluginSource?.source ?? 'plugin',
    title: track.title,
    artist: track.artist,
    quality,
    source_track_id: track.sourceTrackId,
    use_cache: true,
  }
}

function createPluginAdapterError(code: string, trackId: string, message: string): Error {
  const error = new Error(message)
  error.name = code
  ;(error as Error & { trackId?: string }).trackId = trackId
  return error
}
