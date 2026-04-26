import type { JiaochangResolvedUrlCache } from './audio-cache.ts'
import { isCacheableResolvedUrl } from './audio-cache.ts'
import type { JiaochangAudioQuality, JiaochangMusicTrack, JiaochangResolvedTrackUrl } from './audio-state.ts'
import type { JiaochangSourceAdapter } from './source-adapters.ts'

export interface ResolveTrackUrlInput {
  track: JiaochangMusicTrack
  quality?: JiaochangAudioQuality
  adapters: JiaochangSourceAdapter[]
  cache?: JiaochangResolvedUrlCache
}

export async function resolveTrackUrl(input: ResolveTrackUrlInput): Promise<JiaochangResolvedTrackUrl> {
  const quality = input.quality ?? input.track.availableQualities?.[0] ?? 'standard'
  const cached = input.cache?.get(input.track.id, quality)
  if (cached) {
    return {
      ...cached,
      source: 'cache',
      evidence: `cache hit for ${cached.sourceAdapterId}`,
    }
  }

  const adapter = input.adapters.find((item) => item.id === input.track.sourceAdapterId)
    ?? input.adapters.find((item) => item.source === input.track.source)

  if (!adapter) {
    throw createResolveError('adapter_missing', input.track.id, `No source adapter for ${input.track.source}.`)
  }
  if (!adapter.enabled) {
    throw createResolveError('adapter_disabled', input.track.id, `${adapter.id} adapter is disabled.`)
  }

  try {
    const resolved = await adapter.resolveTrackUrl(input.track, quality)
    if (input.cache && isCacheableResolvedUrl(resolved)) {
      input.cache.put(resolved)
    }
    return resolved
  } catch (error) {
    if (error instanceof Error) {
      throw createResolveError(error.name === 'Error' ? 'resolve_failed' : error.name, input.track.id, error.message)
    }
    throw createResolveError('resolve_failed', input.track.id, 'Unknown track URL resolution failure.')
  }
}

function createResolveError(code: string, trackId: string, message: string): Error {
  const error = new Error(message)
  error.name = code
  ;(error as Error & { trackId?: string }).trackId = trackId
  return error
}
