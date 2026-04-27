import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import type { JiaochangAudioQuality, JiaochangMusicTrack, JiaochangResolvedTrackUrl } from './audio-state.ts'
import type { JiaochangSourceAdapter } from './source-adapters.ts'

export const JIACHANG_PLUGIN_AUDIO_EVENT = 'jiaochang://audio/plugin-event'

export interface JiaochangAudioPluginManifest {
  plugin_id: string
  name: string
  version?: string
  source_url?: string
  signature?: string
  enabled: boolean
  allowed_hosts: string[]
  resolver_template: string
  compatibility?: JiaochangAudioPluginCompatibility
  providers?: JiaochangAudioPluginProvider[]
  risk_flags?: string[]
  local_js_path?: string
}

export type JiaochangAudioPluginCompatibility = 'constrained_manifest' | 'lx_ceru_js' | 'authorized_multi_source'

export interface JiaochangAudioPluginProvider {
  provider_id: string
  name: string
  enabled: boolean
  allowed_hosts: string[]
  resolver_template: string
  actions?: string[]
}

export interface JiaochangAudioPluginInspection {
  path: string
  file_name: string
  size_bytes: number
  sha256: string
  detected_kind: JiaochangAudioPluginCompatibility
  name?: string
  version?: string
  homepage?: string
  risk_flags: string[]
  supported_hosts: string[]
  executable_in_renderer: boolean
  recommendation: string
}

export interface JiaochangPluginTrackResolveInput {
  track_id: string
  plugin_id: string
  source: string
  title: string
  artist?: string
  quality: string
  source_track_id?: string
  plugin_music_info?: unknown
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

export interface JiaochangPluginTrackSearchInput {
  plugin_id: string
  source: string
  query: string
  page?: number
  page_size?: number
}

export interface JiaochangPluginSearchedTrack {
  track_id: string
  plugin_id: string
  source: string
  title: string
  artist?: string
  album?: string
  duration?: number
  source_track_id?: string
  music_info: unknown
}

export interface JiaochangAudioPluginCacheInfo {
  entries: number
}

export interface JiaochangAudioPluginRuntimeEvent {
  event_type: 'registered' | 'removed' | 'resolving' | 'resolved' | 'cache_hit' | 'error'
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

export async function installAuthorizedChineseMusicSourceTemplate(
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangAudioPluginManifest> {
  return invokeCommand<JiaochangAudioPluginManifest>('jiaochang_audio_plugin_install_authorized_cn_template')
}

export async function inspectJiaochangAudioPluginJsFile(
  path: string,
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangAudioPluginInspection> {
  return invokeCommand<JiaochangAudioPluginInspection>('jiaochang_audio_plugin_inspect_js_file', { path })
}

export async function importJiaochangAudioLxCeruJsFile(
  path: string,
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangAudioPluginManifest> {
  return invokeCommand<JiaochangAudioPluginManifest>('jiaochang_audio_plugin_import_lx_ceru_js_file', { path })
}

export async function setJiaochangAudioPluginSourceEnabled(
  pluginId: string,
  enabled: boolean,
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangAudioPluginManifest> {
  return invokeCommand<JiaochangAudioPluginManifest>('jiaochang_audio_plugin_set_enabled', {
    pluginId,
    enabled,
  })
}

export async function removeJiaochangAudioPluginSource(
  pluginId: string,
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<void> {
  return invokeCommand<void>('jiaochang_audio_plugin_remove', { pluginId })
}

export async function searchJiaochangAudioPluginTracks(
  input: JiaochangPluginTrackSearchInput,
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangPluginSearchedTrack[]> {
  return invokeCommand<JiaochangPluginSearchedTrack[]>('jiaochang_audio_plugin_search_tracks', { input })
}

export async function getJiaochangAudioPluginCacheInfo(
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangAudioPluginCacheInfo> {
  return invokeCommand<JiaochangAudioPluginCacheInfo>('jiaochang_audio_plugin_cache_info')
}

export async function clearJiaochangAudioPluginCache(
  invokeCommand: JiaochangPluginInvoke = invoke,
): Promise<JiaochangAudioPluginCacheInfo> {
  return invokeCommand<JiaochangAudioPluginCacheInfo>('jiaochang_audio_plugin_cache_clear')
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
    plugin_music_info: track.pluginSource?.musicInfo,
    use_cache: true,
  }
}

function createPluginAdapterError(code: string, trackId: string, message: string): Error {
  const error = new Error(message)
  error.name = code
  ;(error as Error & { trackId?: string }).trackId = trackId
  return error
}
