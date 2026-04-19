/**
 * Phase TTS-B.2：Web Audio API gapless 流式播放钩子。
 *
 * 监听后端 `tts:stream-chunk` Tauri 事件，把 PCM16LE → Float32 → AudioBuffer
 * → AudioBufferSourceNode 链式调度，保证连续 chunk 之间无 click/gap。
 *
 * 同时提供：
 * - `pause()` / `resume()` 通过 `AudioContext.suspend/resume` 实现
 * - `stop()` 重置全部状态 + close AudioContext
 * - `metrics` 实时暴露：firstAudioLatencyMs / leadMs / scheduledChunks / playedSeconds
 *
 * 用法：
 * ```ts
 * const player = useWebAudioStreamPlayer()
 * await player.start()  // 创建 AudioContext + 订阅 event
 * // 然后调用 ttsStreamStart() 开始推送
 * player.pause(); player.resume(); player.stop()
 * ```
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

interface StreamChunkEvent {
  stream_id: string
  chunk_index: number
  sample_rate: number
  channels: number
  pcm_base64: string
  emitted_audio_seconds: number
  lead_seconds: number
}

export interface WebAudioPlayerMetrics {
  firstAudioLatencyMs: number | null
  leadMs: number
  scheduledChunks: number
  playedSeconds: number
  sampleRate: number | null
  channels: number | null
}

export interface WebAudioStreamPlayer {
  start: () => Promise<void>
  stop: () => Promise<void>
  pause: () => Promise<void>
  resume: () => Promise<void>
  isPaused: boolean
  isActive: boolean
  metrics: WebAudioPlayerMetrics
  /** 当前期望接收的 stream_id；若 chunk.stream_id ≠ 此值则丢弃 */
  setExpectedStreamId: (id: string | null) => void
  /**
   * 返回还有多少秒已调度的音频未播放完毕。
   * 用于 `flushAndStop` 决定何时真正 close AudioContext，避免最后一句被静音。
   */
  getRemainingPlayoutSeconds: () => number
}

const initialMetrics: WebAudioPlayerMetrics = {
  firstAudioLatencyMs: null,
  leadMs: 0,
  scheduledChunks: 0,
  playedSeconds: 0,
  sampleRate: null,
  channels: null,
}

/** 把 base64 → Uint8Array */
function base64ToUint8(b64: string): Uint8Array {
  const binStr = atob(b64)
  const len = binStr.length
  const arr = new Uint8Array(len)
  for (let i = 0; i < len; i++) arr[i] = binStr.charCodeAt(i)
  return arr
}

/** PCM16LE interleaved bytes → planar Float32 channels */
function pcm16LEToFloat32Planar(bytes: Uint8Array, channels: number): Float32Array[] {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const totalSamples = bytes.byteLength / 2
  const framesPerChannel = totalSamples / Math.max(channels, 1)
  const out: Float32Array[] = []
  for (let c = 0; c < channels; c++) out.push(new Float32Array(framesPerChannel))
  for (let i = 0; i < framesPerChannel; i++) {
    for (let c = 0; c < channels; c++) {
      const sampleIdx = i * channels + c
      const i16 = view.getInt16(sampleIdx * 2, true)
      out[c][i] = i16 / 32768
    }
  }
  return out
}

export function useWebAudioStreamPlayer(): WebAudioStreamPlayer {
  const audioCtxRef = useRef<AudioContext | null>(null)
  const nextStartTimeRef = useRef<number>(0)
  const startInstantRef = useRef<number>(0)
  const firstAudioInstantRef = useRef<number | null>(null)
  const expectedStreamIdRef = useRef<string | null>(null)
  const unlistenRef = useRef<UnlistenFn | null>(null)
  const metricsRef = useRef<WebAudioPlayerMetrics>({ ...initialMetrics })

  const [isPaused, setIsPaused] = useState(false)
  const [isActive, setIsActive] = useState(false)
  const [metrics, setMetrics] = useState<WebAudioPlayerMetrics>(initialMetrics)

  const refreshMetrics = useCallback(() => {
    setMetrics({ ...metricsRef.current })
  }, [])

  const handleChunk = useCallback((evt: { payload: StreamChunkEvent }) => {
    const chunk = evt.payload
    if (expectedStreamIdRef.current && chunk.stream_id !== expectedStreamIdRef.current) {
      return
    }
    let ctx = audioCtxRef.current
    if (!ctx || ctx.sampleRate !== chunk.sample_rate) {
      // 重新建一个匹配 sample_rate 的 AudioContext
      try {
        ctx?.close()
      } catch {
        /* ignore */
      }
      ctx = new AudioContext({ sampleRate: chunk.sample_rate, latencyHint: 'interactive' })
      audioCtxRef.current = ctx
      nextStartTimeRef.current = ctx.currentTime
    }
    metricsRef.current.sampleRate = chunk.sample_rate
    metricsRef.current.channels = chunk.channels

    const bytes = base64ToUint8(chunk.pcm_base64)
    if (bytes.byteLength === 0) return
    const planar = pcm16LEToFloat32Planar(bytes, chunk.channels)
    const framesPerChannel = planar[0]?.length ?? 0
    if (framesPerChannel === 0) return

    const buffer = ctx.createBuffer(chunk.channels, framesPerChannel, chunk.sample_rate)
    for (let c = 0; c < chunk.channels; c++) buffer.copyToChannel(planar[c], c)

    const source = ctx.createBufferSource()
    source.buffer = buffer
    source.connect(ctx.destination)

    const now = ctx.currentTime
    const startAt = Math.max(nextStartTimeRef.current, now + 0.005) // 5ms safety lead
    source.start(startAt)
    nextStartTimeRef.current = startAt + buffer.duration

    if (firstAudioInstantRef.current === null) {
      firstAudioInstantRef.current = performance.now()
      metricsRef.current.firstAudioLatencyMs =
        firstAudioInstantRef.current - startInstantRef.current
    }
    metricsRef.current.scheduledChunks += 1
    metricsRef.current.playedSeconds += buffer.duration
    metricsRef.current.leadMs = Math.round(chunk.lead_seconds * 1000)
    refreshMetrics()
  }, [refreshMetrics])

  const start = useCallback(async () => {
    metricsRef.current = { ...initialMetrics }
    setMetrics({ ...initialMetrics })
    startInstantRef.current = performance.now()
    firstAudioInstantRef.current = null
    nextStartTimeRef.current = 0
    setIsPaused(false)
    setIsActive(true)

    if (unlistenRef.current) {
      try { unlistenRef.current() } catch { /* ignore */ }
      unlistenRef.current = null
    }
    unlistenRef.current = await listen<StreamChunkEvent>('tts:stream-chunk', handleChunk)
  }, [handleChunk])

  const pause = useCallback(async () => {
    if (audioCtxRef.current && audioCtxRef.current.state === 'running') {
      await audioCtxRef.current.suspend()
      setIsPaused(true)
    }
  }, [])

  const resume = useCallback(async () => {
    if (audioCtxRef.current && audioCtxRef.current.state === 'suspended') {
      await audioCtxRef.current.resume()
      setIsPaused(false)
    }
  }, [])

  const stop = useCallback(async () => {
    setIsActive(false)
    setIsPaused(false)
    expectedStreamIdRef.current = null
    if (unlistenRef.current) {
      try { unlistenRef.current() } catch { /* ignore */ }
      unlistenRef.current = null
    }
    if (audioCtxRef.current) {
      try {
        await audioCtxRef.current.close()
      } catch {
        /* ignore */
      }
      audioCtxRef.current = null
    }
  }, [])

  const setExpectedStreamId = useCallback((id: string | null) => {
    expectedStreamIdRef.current = id
  }, [])

  /** 已调度但未播放完毕的剩余秒数。AudioContext 不存在时返回 0。 */
  const getRemainingPlayoutSeconds = useCallback((): number => {
    const ctx = audioCtxRef.current
    if (!ctx) return 0
    return Math.max(0, nextStartTimeRef.current - ctx.currentTime)
  }, [])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      void stop()
    }
  }, [stop])

  return {
    start,
    stop,
    pause,
    resume,
    isPaused,
    isActive,
    metrics,
    setExpectedStreamId,
    getRemainingPlayoutSeconds,
  }
}
