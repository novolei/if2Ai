export type JiaochangAudioSlot = 'A' | 'B'
export type JiaochangRepeatMode = 'off' | 'one' | 'all' | 'shuffle'
export type JiaochangAudioQuality = 'ambient' | 'standard' | 'high' | 'lossless'
export type JiaochangTrackSource = 'bundled' | 'local' | 'cache' | 'service' | 'plugin'
export type JiaochangTrackMood = 'focus' | 'battle' | 'rest' | 'error' | 'done'

export interface JiaochangLocalMusicGrant {
  permissionRef: string
  pathDisplay: string
  status: 'active' | 'missing' | 'revoked' | 'ephemeral'
  grantedAt: string
  restoredAt?: string
}

export interface JiaochangMusicTrack {
  id: string
  title: string
  artist?: string
  album?: string
  duration?: number
  source: JiaochangTrackSource
  sourceAdapterId: string
  sourceTrackId?: string
  src?: string
  hash?: string
  mood: JiaochangTrackMood
  coverSrc?: string
  availableQualities?: JiaochangAudioQuality[]
  license?: string
  localGrant?: JiaochangLocalMusicGrant
  pluginSource?: {
    pluginId: string
    source: string
  }
}

export interface JiaochangResolvedTrackUrl {
  trackId: string
  url: string
  source: JiaochangTrackSource
  sourceAdapterId: string
  quality: JiaochangAudioQuality
  cacheKey?: string
  expiresAt?: string
  resolvedAt: string
  evidence: string
}

export interface JiaochangAudioError {
  code:
    | 'empty_queue'
    | 'track_missing'
    | 'adapter_missing'
    | 'adapter_disabled'
    | 'grant_missing'
    | 'resolve_failed'
    | 'play_failed'
    | 'tauri_unavailable'
  message: string
  trackId?: string
}

export interface JiaochangAudioState {
  primarySlot: JiaochangAudioSlot
  srcA: string | null
  srcB: string | null
  activeTrackId: string | null
  isPlaying: boolean
  isResolving: boolean
  currentTime: number
  duration: number
  volume: number
  muted: boolean
  repeatMode: JiaochangRepeatMode
  queue: JiaochangMusicTrack[]
  resolvedTrackUrl: JiaochangResolvedTrackUrl | null
  error: JiaochangAudioError | null
  requestId: number
}

export const JIACHANG_AUDIO_STORAGE_KEY = 'if2ai:jiaochang:audio:v1'

export function createInitialJiaochangAudioState(queue: JiaochangMusicTrack[] = []): JiaochangAudioState {
  return {
    primarySlot: 'A',
    srcA: null,
    srcB: null,
    activeTrackId: queue[0]?.id ?? null,
    isPlaying: false,
    isResolving: false,
    currentTime: 0,
    duration: 0,
    volume: 0.72,
    muted: false,
    repeatMode: 'off',
    queue,
    resolvedTrackUrl: null,
    error: null,
    requestId: 0,
  }
}

export function getActiveTrack(state: JiaochangAudioState): JiaochangMusicTrack | null {
  if (!state.activeTrackId) return null
  return state.queue.find((track) => track.id === state.activeTrackId) ?? null
}

export function getNextTrackId(state: JiaochangAudioState): string | null {
  if (state.queue.length === 0) return null
  if (!state.activeTrackId) return state.queue[0]?.id ?? null
  if (state.repeatMode === 'one') return state.activeTrackId
  if (state.repeatMode === 'shuffle') {
    const candidates = state.queue.filter((track) => track.id !== state.activeTrackId)
    return (candidates[Math.floor(Math.random() * candidates.length)] ?? state.queue[0])?.id ?? null
  }
  const index = state.queue.findIndex((track) => track.id === state.activeTrackId)
  if (index < 0) return state.queue[0]?.id ?? null
  const next = state.queue[index + 1]
  if (next) return next.id
  return state.repeatMode === 'all' ? state.queue[0]?.id ?? null : null
}

export function getPreviousTrackId(state: JiaochangAudioState): string | null {
  if (state.queue.length === 0) return null
  if (!state.activeTrackId) return state.queue[0]?.id ?? null
  const index = state.queue.findIndex((track) => track.id === state.activeTrackId)
  if (index <= 0) return state.repeatMode === 'all' ? state.queue[state.queue.length - 1]?.id ?? null : state.activeTrackId
  return state.queue[index - 1]?.id ?? null
}
