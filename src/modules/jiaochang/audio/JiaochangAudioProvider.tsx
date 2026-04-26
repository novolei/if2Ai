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
  }
  subscribe(name: JiaochangAudioEventName, listener: (payload: JiaochangAudioEventPayload) => void): () => void
}

export const JiaochangAudioContext = createContext<JiaochangAudioContextValue | null>(null)

export function JiaochangAudioProvider({ children }: { children: ReactNode }) {
  const initialQueue = useMemo(() => {
    const stored = readStoredAudioLibrary()
    return [...createBundledTrackLibrary(), ...stored.localTracks]
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
      audio.src = resolved.url
      audio.volume = snapshot.volume
      audio.muted = snapshot.muted
      await waitForPlayable(audio)
      await audio.play()
      setState((current) => ({
        ...current,
        [`src${current.primarySlot}`]: resolved.url,
        activeTrackId: targetTrack.id,
        isPlaying: true,
        isResolving: false,
        resolvedTrackUrl: resolved,
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
      writeStoredAudioLibrary({ localTracks: merged.filter((track) => track.source === 'local') })
      eventBusRef.current.emit('library:changed', { tracks: merged })
      return {
        ...current,
        queue: merged,
        activeTrackId: current.activeTrackId ?? tracks[0]?.id ?? null,
        error: null,
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
    },
    subscribe,
  }), [activeTrack, importLocalTracksFromDialog, next, pause, play, previous, seek, setRepeatMode, setVolume, state, subscribe, toggleMute, togglePlay])

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
      reject(createAudioError('play_failed', 'Audio element failed to load source.'))
    }
    audio.addEventListener('canplay', onCanPlay, { once: true })
    audio.addEventListener('error', onError, { once: true })
  })
}

function normalizeAudioError(error: unknown, trackId?: string): JiaochangAudioError {
  if (error && typeof error === 'object' && 'code' in error && 'message' in error) {
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

function createAudioError(code: JiaochangAudioError['code'], message: string, trackId?: string): JiaochangAudioError {
  return { code, message, trackId }
}

function setAudioError(
  setState: (value: (current: JiaochangAudioState) => JiaochangAudioState) => void,
  error: JiaochangAudioError,
) {
  setState((current) => ({ ...current, isResolving: false, isPlaying: false, error }))
}
