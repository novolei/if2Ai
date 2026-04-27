import { createContext, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'

import { InMemoryResolvedUrlCache } from './audio-cache.ts'
import { JiaochangAudioEventBus, type JiaochangAudioEventName, type JiaochangAudioEventPayload } from './audio-events.ts'
import {
  createInitialJiaochangAudioState,
  getActiveTrack,
  getNextTrackId,
  getPreviousTrackId,
  type JiaochangAudioError,
  type JiaochangAudioState,
  type JiaochangMusicTrack,
  type JiaochangRepeatMode,
} from './audio-state.ts'
import { resolveTrackUrl } from './resolve-track-url.ts'
import { createDefaultSourceAdapters } from './source-adapters.ts'
import { subscribeJiaochangAudioPluginEvents } from './plugin-source-adapter.ts'
import {
  createBundledTrackLibrary,
  createLocalTrackFromGrant,
  getPathDisplay,
  isSupportedAudioPath,
  readStoredAudioLibrary,
  writeStoredAudioLibrary,
} from './track-library.ts'

interface JiaochangAudioContextValue {
  state: JiaochangAudioState
  activeTrack: JiaochangMusicTrack | null
  controls: {
    play(trackId?: string): Promise<void>
    pause(): void
    togglePlay(): Promise<void>
    next(): Promise<void>
    previous(): Promise<void>
    seek(time: number): void
    setVolume(volume: number): void
    toggleMute(): void
    setRepeatMode(mode: JiaochangRepeatMode): void
    importLocalTracksFromDialog(): Promise<void>
    addTracksToQueue(tracks: JiaochangMusicTrack[]): void
    removeTrackFromQueue(trackId: string): void
  }
  subscribe(name: JiaochangAudioEventName, listener: (payload: JiaochangAudioEventPayload) => void): () => void
}

export const JiaochangAudioContext = createContext<JiaochangAudioContextValue | null>(null)

export function JiaochangAudioProvider({ children }: { children: ReactNode }) {
  const initialQueue = useMemo(() => {
    const stored = readStoredAudioLibrary()
    return [...createBundledTrackLibrary(), ...stored.localTracks, ...stored.pluginTracks]
  }, [])
  const [state, setState] = useState(() => createInitialJiaochangAudioState(initialQueue))
  const audioARef = useRef<HTMLAudioElement | null>(null)
  const audioBRef = useRef<HTMLAudioElement | null>(null)
  const eventBusRef = useRef(new JiaochangAudioEventBus())
  const adaptersRef = useRef(createDefaultSourceAdapters())
  const cacheRef = useRef(new InMemoryResolvedUrlCache())

  const activeAudio = state.primarySlot === 'A' ? audioARef.current : audioBRef.current
  const activeTrack = useMemo(() => getActiveTrack(state), [state])

  useEffect(() => {
    let disposed = false
    let cleanup: (() => void) | null = null
    subscribeJiaochangAudioPluginEvents((pluginEvent) => {
      eventBusRef.current.emit('plugin:event', { pluginEvent, trackId: pluginEvent.track_id })
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten()
          return
        }
        cleanup = unlisten
      })
      .catch(() => {
        cleanup = null
      })
    return () => {
      disposed = true
      cleanup?.()
    }
  }, [])

  useEffect(() => {
    const audio = activeAudio
    if (!audio) return
    audio.volume = state.volume
    audio.muted = state.muted
  }, [activeAudio, state.muted, state.volume])

  useEffect(() => {
    let cancelled = false
    async function restoreAssetUrls() {
      if (state.queue.every((track) => track.source !== 'local' || track.src)) return
      const convert = await loadConvertFileSrc()
      if (!convert || cancelled) return
      setState((current) => ({
        ...current,
        queue: current.queue.map((track) => {
          if (track.source !== 'local' || !track.localGrant || track.src) return track
          return {
            ...track,
            src: convert(track.localGrant.permissionRef),
            localGrant: {
              ...track.localGrant,
              restoredAt: new Date().toISOString(),
            },
          }
        }),
      }))
    }
    void restoreAssetUrls()
    return () => {
      cancelled = true
    }
  }, [state.queue])

  useEffect(() => {
    const audio = activeAudio
    if (!audio) return

    const onTimeUpdate = () => {
      const currentTime = audio.currentTime || 0
      const duration = Number.isFinite(audio.duration) ? audio.duration : 0
      setState((current) => ({ ...current, currentTime, duration }))
      eventBusRef.current.emit('track:timeupdate', { trackId: state.activeTrackId ?? undefined, currentTime, duration })
    }
    const onEnded = () => {
      eventBusRef.current.emit('track:ended', { trackId: state.activeTrackId ?? undefined })
      void playNextFromCurrent()
    }
    audio.addEventListener('timeupdate', onTimeUpdate)
    audio.addEventListener('loadedmetadata', onTimeUpdate)
    audio.addEventListener('ended', onEnded)
    return () => {
      audio.removeEventListener('timeupdate', onTimeUpdate)
      audio.removeEventListener('loadedmetadata', onTimeUpdate)
      audio.removeEventListener('ended', onEnded)
    }
  })

  const pause = useCallback(() => {
    audioARef.current?.pause()
    audioBRef.current?.pause()
    setState((current) => ({ ...current, isPlaying: false }))
    eventBusRef.current.emit('track:paused', { trackId: state.activeTrackId ?? undefined })
  }, [state.activeTrackId])

  const play = useCallback(async (trackId?: string) => {
    const snapshot = state
    const targetTrack = findTargetTrack(snapshot, trackId)
    if (!targetTrack) {
      setAudioError(setState, { code: 'empty_queue', message: 'No music tracks are available.' })
      return
    }

    const requestId = snapshot.requestId + 1
    setState((current) => ({ ...current, isResolving: true, error: null, activeTrackId: targetTrack.id, requestId }))

    try {
      const resolved = await resolveTrackUrl({
        track: targetTrack,
        quality: targetTrack.availableQualities?.[0] ?? 'standard',
        adapters: adaptersRef.current,
        cache: cacheRef.current,
      })
      const audio = snapshot.primarySlot === 'A' ? audioARef.current : audioBRef.current
      if (!audio) {
        throw createAudioError('play_failed', 'Audio element is not mounted.', targetTrack.id)
      }
      const playableUrl = await resolvePlayableUrlFromProbe(resolved.url, resolved.evidence, targetTrack.id)
      const finalResolved = playableUrl === resolved.url ? resolved : {
        ...resolved,
        url: playableUrl,
        evidence: `${resolved.evidence}; unwrapped resolver JSON to playable audio URL`,
      }
      audio.src = playableUrl
      audio.volume = snapshot.volume
      audio.muted = snapshot.muted
      await waitForPlayable(audio)
      await audio.play()
      setState((current) => ({
        ...current,
        [`src${current.primarySlot}`]: playableUrl,
        activeTrackId: targetTrack.id,
        isPlaying: true,
        isResolving: false,
        resolvedTrackUrl: finalResolved,
        error: null,
      }))
      eventBusRef.current.emit('track:resolved', { trackId: targetTrack.id, slot: snapshot.primarySlot })
      eventBusRef.current.emit('track:started', { trackId: targetTrack.id, slot: snapshot.primarySlot })
    } catch (error) {
      setAudioError(setState, normalizeAudioError(error, targetTrack.id))
      eventBusRef.current.emit('track:error', { trackId: targetTrack.id, error: normalizeAudioError(error, targetTrack.id) })
    }
  }, [state])

  const togglePlay = useCallback(async () => {
    if (state.isPlaying) {
      pause()
      return
    }
    await play()
  }, [pause, play, state.isPlaying])

  const next = useCallback(async () => {
    const nextTrackId = getNextTrackId(state)
    if (!nextTrackId) {
      pause()
      return
    }
    await play(nextTrackId)
  }, [pause, play, state])

  const previous = useCallback(async () => {
    const previousTrackId = getPreviousTrackId(state)
    if (!previousTrackId) return
    await play(previousTrackId)
  }, [play, state])

  async function playNextFromCurrent() {
    const nextTrackId = getNextTrackId(state)
    if (!nextTrackId) {
      setState((current) => ({ ...current, isPlaying: false, currentTime: 0 }))
      return
    }
    await play(nextTrackId)
  }

  const seek = useCallback((time: number) => {
    const audio = activeAudio
    if (!audio) return
    audio.currentTime = Math.max(0, Math.min(time, Number.isFinite(audio.duration) ? audio.duration : time))
    setState((current) => ({ ...current, currentTime: audio.currentTime }))
    eventBusRef.current.emit('track:seeked', { trackId: state.activeTrackId ?? undefined, currentTime: audio.currentTime })
  }, [activeAudio, state.activeTrackId])

  const setVolume = useCallback((volume: number) => {
    const normalized = Math.max(0, Math.min(1, volume))
    setState((current) => ({ ...current, volume: normalized, muted: normalized === 0 ? true : current.muted }))
  }, [])

  const toggleMute = useCallback(() => {
    setState((current) => ({ ...current, muted: !current.muted }))
  }, [])

  const setRepeatMode = useCallback((mode: JiaochangRepeatMode) => {
    setState((current) => ({ ...current, repeatMode: mode }))
  }, [])

  const importLocalTracksFromDialog = useCallback(async () => {
    const selectedPaths = await pickLocalAudioPaths()
    if (selectedPaths.length === 0) return
    const convert = await loadConvertFileSrc()
    if (!convert) {
      setAudioError(setState, { code: 'tauri_unavailable', message: 'Tauri file asset resolver is unavailable.' })
      return
    }
    const now = new Date().toISOString()
    const tracks = selectedPaths.filter(isSupportedAudioPath).map((path) => {
      const grant = {
        permissionRef: path,
        pathDisplay: getPathDisplay(path),
        status: 'active' as const,
        grantedAt: now,
      }
      return {
        ...createLocalTrackFromGrant(grant),
        src: convert(path),
      }
    })
    if (tracks.length === 0) return
    setState((current) => {
      const merged = mergeTracks(current.queue, tracks)
      persistUserTracks(merged)
      eventBusRef.current.emit('library:changed', { tracks: merged })
      return {
        ...current,
        queue: merged,
        activeTrackId: current.activeTrackId ?? tracks[0]?.id ?? null,
        error: null,
      }
    })
  }, [])

  const addTracksToQueue = useCallback((tracks: JiaochangMusicTrack[]) => {
    if (tracks.length === 0) return
    setState((current) => {
      const merged = mergeTracks(current.queue, tracks)
      persistUserTracks(merged)
      eventBusRef.current.emit('library:changed', { tracks: merged })
      return {
        ...current,
        queue: merged,
        activeTrackId: current.activeTrackId ?? tracks[0]?.id ?? null,
        error: null,
      }
    })
  }, [])

  const removeTrackFromQueue = useCallback((trackId: string) => {
    setState((current) => {
      const removedActive = current.activeTrackId === trackId
      const merged = current.queue.filter((track) => track.id !== trackId)
      persistUserTracks(merged)
      if (removedActive) {
        audioARef.current?.pause()
        audioBRef.current?.pause()
      }
      eventBusRef.current.emit('library:changed', { tracks: merged })
      return {
        ...current,
        queue: merged,
        activeTrackId: removedActive ? (merged[0]?.id ?? null) : current.activeTrackId,
        isPlaying: removedActive ? false : current.isPlaying,
        currentTime: removedActive ? 0 : current.currentTime,
        duration: removedActive ? 0 : current.duration,
        resolvedTrackUrl: removedActive ? null : current.resolvedTrackUrl,
      }
    })
  }, [])

  const subscribe = useCallback(
    (name: JiaochangAudioEventName, listener: (payload: JiaochangAudioEventPayload) => void) => {
      return eventBusRef.current.subscribe(name, listener)
    },
    [],
  )

  const value = useMemo<JiaochangAudioContextValue>(() => ({
    state,
    activeTrack,
    controls: {
      play,
      pause,
      togglePlay,
      next,
      previous,
      seek,
      setVolume,
      toggleMute,
      setRepeatMode,
      importLocalTracksFromDialog,
      addTracksToQueue,
      removeTrackFromQueue,
    },
    subscribe,
  }), [activeTrack, addTracksToQueue, importLocalTracksFromDialog, next, pause, play, previous, removeTrackFromQueue, seek, setRepeatMode, setVolume, state, subscribe, toggleMute, togglePlay])

  return (
    <JiaochangAudioContext.Provider value={value}>
      <audio ref={audioARef} preload="metadata" />
      <audio ref={audioBRef} preload="metadata" />
      {children}
    </JiaochangAudioContext.Provider>
  )
}

function findTargetTrack(state: JiaochangAudioState, trackId?: string): JiaochangMusicTrack | null {
  if (trackId) return state.queue.find((track) => track.id === trackId) ?? null
  return getActiveTrack(state) ?? state.queue[0] ?? null
}

function mergeTracks(current: JiaochangMusicTrack[], incoming: JiaochangMusicTrack[]): JiaochangMusicTrack[] {
  const byId = new Map(current.map((track) => [track.id, track]))
  for (const track of incoming) byId.set(track.id, track)
  return Array.from(byId.values())
}

function persistUserTracks(queue: JiaochangMusicTrack[]): void {
  writeStoredAudioLibrary({
    localTracks: queue.filter((track) => track.source === 'local'),
    pluginTracks: queue.filter((track) => track.source === 'plugin'),
  })
}

async function pickLocalAudioPaths(): Promise<string[]> {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      multiple: true,
      directory: false,
      filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'flac', 'm4a', 'aac', 'ogg', 'opus'] }],
    })
    if (!selected) return []
    return Array.isArray(selected) ? selected : [selected]
  } catch {
    return []
  }
}

async function loadConvertFileSrc(): Promise<((path: string) => string) | null> {
  try {
    const { convertFileSrc } = await import('@tauri-apps/api/core')
    return convertFileSrc
  } catch {
    return null
  }
}

function waitForPlayable(audio: HTMLAudioElement): Promise<void> {
  if (audio.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) return Promise.resolve()
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      audio.removeEventListener('canplay', onCanPlay)
      audio.removeEventListener('error', onError)
    }
    const onCanPlay = () => {
      cleanup()
      resolve()
    }
    const onError = () => {
      cleanup()
      reject(createAudioError('play_failed', '浏览器音频控件无法加载该音源。可能是链接已过期、跨域限制、或返回的不是可播放音频。'))
    }
    audio.addEventListener('canplay', onCanPlay, { once: true })
    audio.addEventListener('error', onError, { once: true })
  })
}

async function resolvePlayableUrlFromProbe(url: string, evidence: string, trackId?: string, depth = 0): Promise<string> {
  if (!/^https?:\/\//.test(url)) return url
  const parsed = new URL(url)
  const isLocalResolver = parsed.hostname === '127.0.0.1' || parsed.hostname === 'localhost'
  const shouldProbe = isLocalResolver || evidence.includes('provider') || evidence.includes('LX/Ceru')
  if (!shouldProbe) return url
  const controller = new AbortController()
  const timer = window.setTimeout(() => controller.abort(), 3500)
  try {
    const response = await fetch(url, {
      method: 'HEAD',
      signal: controller.signal,
    })
    if (!response.ok) {
      throw createAudioError(
        'play_failed',
        `音源 resolver 返回 HTTP ${response.status}。请确认授权服务已启动，并且该曲目存在可播放资源。`,
        trackId,
      )
    }
    const contentType = response.headers.get('content-type') ?? ''
    if (contentType && !isAudioLikeContentType(contentType)) {
      if (isJsonLikeContentType(contentType) && depth < 2) {
        const unwrappedUrl = await unwrapResolverJsonUrl(url, trackId)
        return resolvePlayableUrlFromProbe(unwrappedUrl, evidence, trackId, depth + 1)
      }
      throw createAudioError(
        'play_failed',
        `音源 resolver 返回的不是音频资源（Content-Type: ${contentType}）。如果这是 JSON resolver，请确认响应里包含可播放 URL 字段。`,
        trackId,
      )
    }
    return url
  } catch (error) {
    if (isJiaochangAudioError(error)) throw error
    const reason = error instanceof Error && error.name === 'AbortError' ? '请求超时' : '连接失败'
    if (isLocalResolver) {
      throw createAudioError(
        'play_failed',
        `授权 resolver 未运行或不可访问（${reason}: ${parsed.origin}）。请先启动本地合法音源 resolver 服务。`,
        trackId,
      )
    }
    return url
  } finally {
    window.clearTimeout(timer)
  }
}

async function unwrapResolverJsonUrl(url: string, trackId?: string): Promise<string> {
  const response = await fetch(url, {
    method: 'GET',
    headers: { Accept: 'application/json, audio/*;q=0.9, */*;q=0.5' },
  })
  if (!response.ok) {
    throw createAudioError(
      'play_failed',
      `音源 resolver JSON 解包失败：HTTP ${response.status}。请检查授权 resolver 是否返回了真实播放地址。`,
      trackId,
    )
  }
  const payload = await response.json().catch(() => null)
  const playableUrl = findPlayableUrlInJson(payload)
  if (!playableUrl) {
    throw createAudioError(
      'play_failed',
      '音源 resolver 返回了 JSON，但没有找到可播放 URL 字段（支持 url / musicUrl / playUrl / data.url 等）。',
      trackId,
    )
  }
  return playableUrl
}

function findPlayableUrlInJson(value: unknown, depth = 0): string | null {
  if (depth > 5 || value == null) return null
  if (typeof value === 'string') {
    return /^https?:\/\/.+/i.test(value) ? value : null
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      const found = findPlayableUrlInJson(item, depth + 1)
      if (found) return found
    }
    return null
  }
  if (typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  for (const key of ['url', 'musicUrl', 'music_url', 'playUrl', 'play_url', 'src', 'source', 'location']) {
    const found = findPlayableUrlInJson(record[key], depth + 1)
    if (found) return found
  }
  for (const key of ['data', 'result', 'track', 'song', 'resource', 'audio']) {
    const found = findPlayableUrlInJson(record[key], depth + 1)
    if (found) return found
  }
  return null
}

function isAudioLikeContentType(contentType: string): boolean {
  const normalized = contentType.toLowerCase()
  return normalized.startsWith('audio/')
    || normalized.includes('application/octet-stream')
    || normalized.includes('video/mp4')
    || normalized.includes('application/vnd.apple.mpegurl')
}

function isJsonLikeContentType(contentType: string): boolean {
  const normalized = contentType.toLowerCase()
  return normalized.includes('application/json') || normalized.includes('+json')
}

function normalizeAudioError(error: unknown, trackId?: string): JiaochangAudioError {
  if (isJiaochangAudioError(error)) {
    return error as JiaochangAudioError
  }
  if (error instanceof Error) {
    const code = error.name === 'adapter_disabled' || error.name === 'grant_missing' || error.name === 'adapter_missing'
      ? error.name
      : 'resolve_failed'
    return createAudioError(code as JiaochangAudioError['code'], error.message, trackId)
  }
  return createAudioError('resolve_failed', 'Unknown audio error.', trackId)
}

function isJiaochangAudioError(error: unknown): error is JiaochangAudioError {
  return Boolean(error && typeof error === 'object' && 'code' in error && 'message' in error)
}

function createAudioError(code: JiaochangAudioError['code'], message: string, trackId?: string): JiaochangAudioError {
  return { code, message, trackId }
}

function setAudioError(
  setState: (value: (current: JiaochangAudioState) => JiaochangAudioState) => void,
  error: JiaochangAudioError,
) {
  setState((current) => ({ ...current, isResolving: false, isPlaying: false, error }))
}
