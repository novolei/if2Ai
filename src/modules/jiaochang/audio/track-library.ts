import type { JiaochangLocalMusicGrant, JiaochangMusicTrack, JiaochangTrackMood } from './audio-state.ts'
import type { JiaochangAudioPluginManifest } from './plugin-source-adapter.ts'

const AUDIO_EXTENSIONS = new Set(['mp3', 'wav', 'flac', 'm4a', 'aac', 'ogg', 'opus'])

export interface StoredJiaochangAudioLibrary {
  localTracks: JiaochangMusicTrack[]
  pluginTracks: JiaochangMusicTrack[]
}

export function createBundledTrackLibrary(): JiaochangMusicTrack[] {
  return []
}

export function createLocalTrackFromGrant(grant: JiaochangLocalMusicGrant): JiaochangMusicTrack {
  const title = getDisplayTitle(grant.pathDisplay)
  return {
    id: `local:${stableHash(grant.permissionRef)}`,
    title,
    source: 'local',
    sourceAdapterId: 'local',
    sourceTrackId: grant.permissionRef,
    mood: inferTrackMood(title),
    localGrant: grant,
    availableQualities: ['standard'],
    license: 'user-local-file',
  }
}

export function createPluginTrackCandidate(
  manifest: JiaochangAudioPluginManifest,
  query: string,
): JiaochangMusicTrack | null {
  const normalized = query.trim()
  if (!manifest.enabled || !normalized) return null
  const provider = manifest.providers?.find((item) => item.enabled) ?? null
  if (provider) return createProviderTrackCandidate(manifest, provider, normalized)
  const id = `plugin:${manifest.plugin_id}:${stableHash(normalized)}`
  return {
    id,
    title: normalized,
    artist: manifest.name,
    source: 'plugin',
    sourceAdapterId: manifest.plugin_id,
    sourceTrackId: normalized,
    mood: inferTrackMood(normalized),
    availableQualities: ['standard', 'high'],
    license: `plugin-source:${manifest.plugin_id}`,
    pluginSource: {
      pluginId: manifest.plugin_id,
      source: manifest.plugin_id,
    },
  }
}

export function createPluginTrackCandidates(
  manifests: JiaochangAudioPluginManifest[],
  query: string,
): JiaochangMusicTrack[] {
  const normalized = query.trim()
  if (!normalized) return []
  return manifests.flatMap((manifest) => {
    if (!manifest.enabled) return []
    const providers = manifest.providers?.filter((provider) => provider.enabled) ?? []
    if (providers.length > 0) {
      return providers.map((provider) => createProviderTrackCandidate(manifest, provider, normalized))
    }
    const track = createPluginTrackCandidate(manifest, normalized)
    return track ? [track] : []
  })
}

function createProviderTrackCandidate(
  manifest: JiaochangAudioPluginManifest,
  provider: NonNullable<JiaochangAudioPluginManifest['providers']>[number],
  query: string,
): JiaochangMusicTrack {
  const id = `plugin:${manifest.plugin_id}:${provider.provider_id}:${stableHash(query)}`
  return {
    id,
    title: query,
    artist: provider.name,
    source: 'plugin',
    sourceAdapterId: manifest.plugin_id,
    sourceTrackId: query,
    mood: inferTrackMood(query),
    availableQualities: ['standard', 'high', 'lossless'],
    license: `authorized-plugin-source:${manifest.plugin_id}:${provider.provider_id}`,
    pluginSource: {
      pluginId: manifest.plugin_id,
      source: provider.provider_id,
    },
  }
}

export function readStoredAudioLibrary(storage: Pick<Storage, 'getItem'> | null = getBrowserStorage()): StoredJiaochangAudioLibrary {
  if (!storage) return { localTracks: [], pluginTracks: [] }
  try {
    const raw = storage.getItem('if2ai:jiaochang:audio-library:v1')
    if (!raw) return { localTracks: [], pluginTracks: [] }
    const parsed = JSON.parse(raw) as Partial<StoredJiaochangAudioLibrary>
    return {
      localTracks: Array.isArray(parsed.localTracks) ? parsed.localTracks.filter(isPlayableTrackMetadata) : [],
      pluginTracks: Array.isArray(parsed.pluginTracks) ? parsed.pluginTracks.filter(isPlayableTrackMetadata) : [],
    }
  } catch {
    return { localTracks: [], pluginTracks: [] }
  }
}

export function writeStoredAudioLibrary(
  library: StoredJiaochangAudioLibrary,
  storage: Pick<Storage, 'setItem'> | null = getBrowserStorage(),
): void {
  if (!storage) return
  storage.setItem('if2ai:jiaochang:audio-library:v1', JSON.stringify(library))
}

export function isSupportedAudioPath(path: string): boolean {
  const ext = path.split('.').pop()?.toLowerCase()
  return Boolean(ext && AUDIO_EXTENSIONS.has(ext))
}

export function getDisplayTitle(path: string): string {
  const basename = path.split(/[\\/]/).pop() ?? path
  return basename.replace(/\.[^.]+$/, '') || basename
}

export function getPathDisplay(path: string): string {
  return path.split(/[\\/]/).pop() ?? path
}

function isPlayableTrackMetadata(value: unknown): value is JiaochangMusicTrack {
  if (!value || typeof value !== 'object') return false
  const track = value as Partial<JiaochangMusicTrack>
  return typeof track.id === 'string'
    && typeof track.title === 'string'
    && (track.source === 'local' || track.source === 'plugin')
}

function inferTrackMood(title: string): JiaochangTrackMood {
  const lower = title.toLowerCase()
  if (lower.includes('rest') || lower.includes('sleep') || lower.includes('chill')) return 'rest'
  if (lower.includes('done') || lower.includes('complete')) return 'done'
  if (lower.includes('battle') || lower.includes('fight')) return 'battle'
  if (lower.includes('error') || lower.includes('sad')) return 'error'
  return 'focus'
}

function stableHash(value: string): string {
  let hash = 2166136261
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return (hash >>> 0).toString(36)
}

function getBrowserStorage(): Storage | null {
  if (typeof window === 'undefined') return null
  return window.localStorage
}
