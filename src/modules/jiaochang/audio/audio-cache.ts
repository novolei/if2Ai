import type { JiaochangAudioQuality, JiaochangResolvedTrackUrl } from './audio-state.ts'

export interface JiaochangResolvedUrlCache {
  get(trackId: string, quality: JiaochangAudioQuality): JiaochangResolvedTrackUrl | null
  put(resolved: JiaochangResolvedTrackUrl): void
  clear(): void
}

export class InMemoryResolvedUrlCache implements JiaochangResolvedUrlCache {
  private readonly entries = new Map<string, JiaochangResolvedTrackUrl>()

  get(trackId: string, quality: JiaochangAudioQuality): JiaochangResolvedTrackUrl | null {
    const entry = this.entries.get(createResolvedUrlCacheKey(trackId, quality)) ?? null
    if (!entry) return null
    if (entry.expiresAt && Date.parse(entry.expiresAt) <= Date.now()) {
      this.entries.delete(createResolvedUrlCacheKey(trackId, quality))
      return null
    }
    return entry
  }

  put(resolved: JiaochangResolvedTrackUrl): void {
    this.entries.set(createResolvedUrlCacheKey(resolved.trackId, resolved.quality), resolved)
  }

  clear(): void {
    this.entries.clear()
  }
}

export function createResolvedUrlCacheKey(trackId: string, quality: JiaochangAudioQuality): string {
  return `${trackId}::${quality}`
}

export function isCacheableResolvedUrl(resolved: JiaochangResolvedTrackUrl): boolean {
  return resolved.source === 'bundled' || resolved.source === 'local' || resolved.source === 'cache'
}
