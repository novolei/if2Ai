import type {
  JiaochangAudioQuality,
  JiaochangMusicTrack,
  JiaochangResolvedTrackUrl,
  JiaochangTrackSource,
} from './audio-state.ts'
import { createBundledTrackLibrary } from './track-library.ts'

export interface JiaochangSourceAdapter {
  id: string
  source: JiaochangTrackSource
  enabled: boolean
  listTracks(): Promise<JiaochangMusicTrack[]>
  resolveTrackUrl(track: JiaochangMusicTrack, quality: JiaochangAudioQuality): Promise<JiaochangResolvedTrackUrl>
}

export function createBundledSourceAdapter(tracks = createBundledTrackLibrary()): JiaochangSourceAdapter {
  return {
    id: 'bundled',
    source: 'bundled',
    enabled: true,
    async listTracks() {
      return tracks
    },
    async resolveTrackUrl(track, quality) {
      if (track.source !== 'bundled' || !track.src) {
        throw createAdapterError('track_missing', track.id, 'Bundled track has no packaged URL.')
      }
      return createResolvedTrackUrl(track, quality, track.src, 'bundled adapter packaged asset')
    },
  }
}

export function createLocalSourceAdapter(): JiaochangSourceAdapter {
  return {
    id: 'local',
    source: 'local',
    enabled: true,
    async listTracks() {
      return []
    },
    async resolveTrackUrl(track, quality) {
      if (track.source !== 'local') {
        throw createAdapterError('track_missing', track.id, 'Track is not a local file.')
      }
      if (!track.localGrant || track.localGrant.status === 'revoked' || track.localGrant.status === 'missing') {
        throw createAdapterError('grant_missing', track.id, 'Local file grant is missing or revoked.')
      }
      if (!track.src) {
        throw createAdapterError('grant_missing', track.id, 'Local file grant has no resolved asset URL.')
      }
      return createResolvedTrackUrl(track, quality, track.src, `local grant ${track.localGrant.status}`)
    },
  }
}

export function createDisabledServiceSourceAdapter(): JiaochangSourceAdapter {
  return createDisabledAdapter('service', 'CeruMusic-style service source is reserved for FEAT-JC-006.')
}

export function createDisabledPluginSourceAdapter(): JiaochangSourceAdapter {
  return createDisabledAdapter('plugin', 'Plugin source must run in Rust side isolate / worker in FEAT-JC-006.')
}

export function createDefaultSourceAdapters(): JiaochangSourceAdapter[] {
  return [
    createBundledSourceAdapter(),
    createLocalSourceAdapter(),
    createDisabledServiceSourceAdapter(),
    createDisabledPluginSourceAdapter(),
  ]
}

function createDisabledAdapter(source: JiaochangTrackSource, reason: string): JiaochangSourceAdapter {
  return {
    id: source,
    source,
    enabled: false,
    async listTracks() {
      return []
    },
    async resolveTrackUrl(track) {
      throw createAdapterError('adapter_disabled', track.id, reason)
    },
  }
}

function createResolvedTrackUrl(
  track: JiaochangMusicTrack,
  quality: JiaochangAudioQuality,
  url: string,
  evidence: string,
): JiaochangResolvedTrackUrl {
  return {
    trackId: track.id,
    url,
    source: track.source,
    sourceAdapterId: track.sourceAdapterId,
    quality,
    cacheKey: `${track.id}::${quality}`,
    resolvedAt: new Date().toISOString(),
    evidence,
  }
}

function createAdapterError(code: string, trackId: string, message: string): Error {
  const error = new Error(message)
  error.name = code
  ;(error as Error & { trackId?: string }).trackId = trackId
  return error
}
