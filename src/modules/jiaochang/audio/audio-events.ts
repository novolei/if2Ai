import type { JiaochangAudioError, JiaochangAudioSlot, JiaochangMusicTrack } from './audio-state.ts'

export type JiaochangAudioEventName =
  | 'track:resolved'
  | 'track:started'
  | 'track:paused'
  | 'track:ended'
  | 'track:seeked'
  | 'track:timeupdate'
  | 'track:error'
  | 'library:changed'
  | 'slot:swapped'

export interface JiaochangAudioEventPayload {
  trackId?: string
  slot?: JiaochangAudioSlot
  currentTime?: number
  duration?: number
  error?: JiaochangAudioError
  tracks?: JiaochangMusicTrack[]
}

export type JiaochangAudioEventListener = (payload: JiaochangAudioEventPayload) => void

export class JiaochangAudioEventBus {
  private readonly listeners = new Map<JiaochangAudioEventName, Set<JiaochangAudioEventListener>>()

  subscribe(name: JiaochangAudioEventName, listener: JiaochangAudioEventListener): () => void {
    const next = this.listeners.get(name) ?? new Set<JiaochangAudioEventListener>()
    next.add(listener)
    this.listeners.set(name, next)
    return () => {
      next.delete(listener)
      if (next.size === 0) this.listeners.delete(name)
    }
  }

  emit(name: JiaochangAudioEventName, payload: JiaochangAudioEventPayload = {}): void {
    for (const listener of this.listeners.get(name) ?? []) {
      listener(payload)
    }
  }

  clear(): void {
    this.listeners.clear()
  }
}
