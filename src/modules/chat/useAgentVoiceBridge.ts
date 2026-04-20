/**
 * Phase TTS-D / P1（重写 v2）：Agent Voice Bridge
 *
 * 把 Agent 流式文本桥接到 TTS：
 * - text_delta → 句末检测 → FIFO 合成队列 → Web Audio gapless 播放
 *
 * ## v2 修复点
 * 原版用 useCallback + useState(enabled/voiceId) 导致 stale closure；drainQueue
 * 中取到的 voiceId 可能是初始化时的 null。
 *
 * 现在改成：
 * - `enabledRef` / `voiceIdRef` —— 始终持有最新值，在 useEffect 里跟随 state 同步
 * - `tts_stream_start` 在 Web Worker-like 异步队列里跑
 * - `player.start()` 在第一次 feed 时预热（不等到第一句话才建 AudioContext）
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  applyTtsPostprocess,
  ttsStreamStart,
  TTS_DEFAULT_PARAMS,
  TTS_DEFAULT_SETTINGS,
  type TtsGenerationParams,
  type TtsProfile,
} from '@/lib/tauri'
import { useWebAudioStreamPlayer } from '@/modules/settings/pages/useWebAudioStreamPlayer'
import { getAgentVoiceId, getAgentVoiceEnabled } from '@/modules/settings/pages/AgentVoicePicker'
import { resolveActiveProfile } from './activeTtsProfile'
import { sanitizeForTts, splitOnUnclosedFence } from './ttsSanitize'
import { broadcastChange, useCrossWindowChange } from '@/lib/crossWindowSync'

const SENTENCE_END_REGEX = /[。！？.!?]+["'"']?\s?/

function findSentenceBoundary(buf: string): number {
  const m = SENTENCE_END_REGEX.exec(buf)
  return m ? m.index + m[0].length : -1
}

export interface AgentVoiceBridge {
  enabled: boolean
  setEnabled: (v: boolean) => void
  voiceId: string | null
  feed: (delta: string) => void
  flushAndStop: () => void
  abort: () => Promise<void>
  isPlaying: boolean
  pending: number
}

export function useAgentVoiceBridge(generationParams?: Partial<TtsGenerationParams>): AgentVoiceBridge {
  const player = useWebAudioStreamPlayer()
  const [enabled, setEnabledState] = useState<boolean>(getAgentVoiceEnabled())
  const [voiceId, setVoiceIdState] = useState<string | null>(null)
  const [isPlaying, setIsPlaying] = useState(false)
  const [pending, setPending] = useState(0)

  // Refs 始终最新：彻底避免 stale closure
  const enabledRef = useRef(enabled)
  const voiceIdRef = useRef<string | null>(null)
  const playerRef = useRef(player)
  // The active TTS profile resolved from `~/.if2ai/tts_profiles.json`.
  // Drives both `voiceId` (sent to ttsStreamStart) and the per-sentence
  // text postprocess.  Refreshed on mount and on cross-window events.
  const activeProfileRef = useRef<TtsProfile | null>(null)
  // generation params: start with TTS_DEFAULT_PARAMS + caller's overrides;
  // overwritten by `applyActiveProfile` on mount / cross-window refresh.
  const generationRef = useRef<TtsGenerationParams>({
    ...TTS_DEFAULT_PARAMS,
    ...(generationParams ?? {}),
    max_new_frames: generationParams?.max_new_frames ?? 250,
  })

  useEffect(() => { enabledRef.current = enabled }, [enabled])
  useEffect(() => { playerRef.current = player }, [player])

  // Centralised loader: pulls the active profile from backend, threads
  // its `voice_id` + derived params + playback_rate through all the
  // refs that downstream synth uses.
  const applyActiveProfile = useCallback(async () => {
    const resolved = await resolveActiveProfile()
    if (resolved) {
      activeProfileRef.current = resolved.profile
      const vid = resolved.profile.voice_id || getAgentVoiceId() // legacy fallback
      voiceIdRef.current = vid
      setVoiceIdState(vid)
      generationRef.current = { ...resolved.params, ...(generationParams ?? {}) }
      playerRef.current.setPlaybackRate(resolved.profile.playback_rate)
    } else {
      // No profiles → fall back to legacy voice picker so existing
      // installs that haven't visited the new page yet still speak.
      const legacy = getAgentVoiceId()
      voiceIdRef.current = legacy
      setVoiceIdState(legacy)
      activeProfileRef.current = null
      playerRef.current.setPlaybackRate(TTS_DEFAULT_SETTINGS.playback_rate)
    }
  }, [generationParams])

  useEffect(() => {
    let alive = true
    void (async () => {
      try {
        await applyActiveProfile()
      } catch (err) {
        if (alive) console.warn('[agent-voice] applyActiveProfile failed', err)
      }
    })()
    return () => {
      alive = false
    }
  }, [applyActiveProfile])

  // Cross-window: any change in profile book / active selection refreshes us.
  useCrossWindowChange<{ id?: string | null }>('cross:tts-profiles-changed', () => {
    void applyActiveProfile()
  })
  useCrossWindowChange<{ id?: string | null }>('cross:tts-active-profile-changed', () => {
    void applyActiveProfile()
  })

  // Legacy AgentVoicePicker still around (Settings → TTS 测试) — keep
  // listening so changes there are honoured when no profile is set.
  useCrossWindowChange<{ id: string | null }>('cross:agent-voice-changed', (payload) => {
    if (activeProfileRef.current?.voice_id) return // profile takes precedence
    const id = payload?.id ?? getAgentVoiceId()
    setVoiceIdState(id)
    voiceIdRef.current = id
  })
  useCrossWindowChange<{ enabled: boolean }>('cross:agent-voice-enabled', (payload) => {
    const v = payload?.enabled ?? getAgentVoiceEnabled()
    setEnabledState(v)
    enabledRef.current = v
    console.log('[agent-voice] cross-window: enabled →', v)
    if (!v) {
      // 关闭时清空队列 + 停止播放
      bufferRef.current = ''
      queueRef.current = []
      busyRef.current = false
      setPending(0)
      setIsPlaying(false)
      void playerRef.current.stop()
    }
  })

  const setEnabled = useCallback((v: boolean) => {
    try {
      localStorage.setItem('if2ai.tts.agentVoiceEnabled', v ? '1' : '0')
    } catch { /* ignore */ }
    // 广播到所有窗口（含 settings），自身的 useCrossWindowChange 会再回调一次也无副作用
    void broadcastChange('cross:agent-voice-enabled', { enabled: v })
    setEnabledState(v)
    enabledRef.current = v
    if (!v) {
      bufferRef.current = ''
      queueRef.current = []
      busyRef.current = false
      setPending(0)
      setIsPlaying(false)
      void playerRef.current.stop()
    }
  }, [])

  // 内部合成队列（完全走 ref，不依赖 React state）
  const bufferRef = useRef<string>('')
  const queueRef = useRef<string[]>([])
  const busyRef = useRef(false)
  const playerStartedRef = useRef(false)

  // ── tts:stream-end 监听 ───────────────────────────────────────────────
  // 后端在每个合成 stream 完成后发一次 `tts:stream-end`，bridge 必须等到该事件
  // 才能启动下一句的 streamStart，避免 setExpectedStreamId 切换把上一句仍在
  // 传输的 chunks 全部丢弃（导致朗读不完整）。
  //
  // pendingResolversRef: stream_id → 等待该 stream 完成的 Promise resolvers
  // endedStreamIdsRef: 已收到 end 的 stream ids（用于 waitForStreamEnd 调用
  //   时事件已先到的竞态保护）
  const pendingResolversRef = useRef<Map<string, Array<() => void>>>(new Map())
  const endedStreamIdsRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    let mounted = true
    void listen<{ stream_id: string; success: boolean; total_audio_seconds: number }>(
      'tts:stream-end',
      (e) => {
        const sid = e.payload.stream_id
        endedStreamIdsRef.current.add(sid)
        const resolvers = pendingResolversRef.current.get(sid)
        if (resolvers) {
          pendingResolversRef.current.delete(sid)
          for (const r of resolvers) r()
        }
        // 清理：避免 endedStreamIds 无限增长（一个 stream 不会被等多次）
        if (endedStreamIdsRef.current.size > 64) {
          // 简单 LRU：转 array 后保留最近 32 个
          const arr = Array.from(endedStreamIdsRef.current)
          endedStreamIdsRef.current = new Set(arr.slice(-32))
        }
      },
    ).then((un) => {
      if (mounted) unlisten = un
      else un()
    })
    return () => {
      mounted = false
      unlisten?.()
    }
  }, [])

  /** 等到指定 stream_id 的 stream-end 事件；带超时兜底（30s）防止后端漏发卡死。 */
  const waitForStreamEnd = useCallback((streamId: string, timeoutMs = 30_000): Promise<void> => {
    if (endedStreamIdsRef.current.has(streamId)) return Promise.resolve()
    return new Promise<void>((resolve) => {
      let done = false
      const finish = () => {
        if (done) return
        done = true
        resolve()
      }
      const list = pendingResolversRef.current.get(streamId) ?? []
      list.push(finish)
      pendingResolversRef.current.set(streamId, list)
      setTimeout(() => {
        if (!done) console.warn('[agent-voice] waitForStreamEnd timeout:', streamId)
        finish()
      }, timeoutMs)
    })
  }, [])

  const synthOne = useCallback(async (sentence: string): Promise<void> => {
    // 句子级清洗：去除 emoji / Markdown / 链接等会让 TTS 念错的元素
    const baseCleaned = sanitizeForTts(sentence)
    if (!baseCleaned) {
      // 清洗后为空（比如整句都是 emoji 或代码片段）→ 静默跳过
      return
    }
    // Apply the active profile's text postprocess (soften punctuation /
    // trailing dots) — pure string transform so it's safe to call here
    // every sentence rather than on the whole stream.
    const profile = activeProfileRef.current
    const cleaned = profile ? applyTtsPostprocess(profile, baseCleaned) : baseCleaned
    const vid = voiceIdRef.current
    if (!vid) {
      console.warn('[agent-voice] no voiceId, dropping:', cleaned.slice(0, 30))
      return
    }
    try {
      const p = playerRef.current
      if (!playerStartedRef.current || !p.isActive) {
        await p.start()
        playerStartedRef.current = true
      }
      console.log('[agent-voice] synth start:', cleaned.slice(0, 50))
      const result = await ttsStreamStart(
        cleaned,
        null,
        null,
        generationRef.current,
        vid,
      )
      // 设置期望 stream_id：让 player 接收这个 stream 的 chunks
      p.setExpectedStreamId(result.stream_id)
      // 关键修复：等到后端发出 `tts:stream-end` 才返回，**严格串行**。
      // 这样下一句的 streamStart 才会调用 setExpectedStreamId(s2)，绝不会发生
      // 「上一句还在传 chunk，下一句改了 expected → 上一句尾部被丢」的问题。
      await waitForStreamEnd(result.stream_id)
      console.log('[agent-voice] synth ended:', result.stream_id)
    } catch (e) {
      console.error('[agent-voice] synth error:', e)
    }
  }, [waitForStreamEnd])

  const drainQueue = useCallback(async () => {
    if (busyRef.current) return
    busyRef.current = true
    setIsPlaying(true)
    while (queueRef.current.length > 0) {
      const next = queueRef.current.shift()!
      setPending(queueRef.current.length)
      await synthOne(next)
    }
    busyRef.current = false
    setIsPlaying(false)
  }, [synthOne])

  const enqueue = useCallback((sentence: string) => {
    queueRef.current.push(sentence)
    setPending(queueRef.current.length)
    void drainQueue()
  }, [drainQueue])

  const feed = useCallback((delta: string) => {
    if (!enabledRef.current || !voiceIdRef.current) return
    bufferRef.current += delta

    // 关键：检测尚未闭合的 ``` 围栏 —— 在它闭合之前，不要把代码内容当成句子提交合成。
    // splitOnUnclosedFence 会先剥掉所有已闭合代码块，再把 buffer 切成 (safe, remainder)。
    // - safe：没有任何代码围栏的部分，可以扫描句子边界
    // - remainder：未闭合代码块本身 + 之后的 delta（继续在 buffer 里累积）
    const { safe, remainder } = splitOnUnclosedFence(bufferRef.current)

    // 用 safe 文本扫句末标点；扫到的部分提交合成，剩余 + remainder 重新拼回 buffer
    let workSafe = safe
    let cut: number
    while ((cut = findSentenceBoundary(workSafe)) > 0) {
      const sentence = workSafe.slice(0, cut)
      workSafe = workSafe.slice(cut)
      enqueue(sentence)
    }
    // workSafe 里剩下的是「safe 部分但没到句末」→ 和 remainder 拼回 buffer 等下一轮
    bufferRef.current = workSafe + remainder
  }, [enqueue])

  const flushAndStop = useCallback(() => {
    if (!enabledRef.current) return
    const tail = bufferRef.current.trim()
    bufferRef.current = ''
    if (tail) enqueue(tail)

    // 关键修复：必须等到「所有合成完成 + 已调度的音频也真正播完」才 stop player；
    // 否则 close AudioContext 会把最后一句的尾部直接静音，听起来"朗读不完整"。
    //
    // 阶段：
    // 1) 等队列 drain 完（busy=false && queue.length=0）
    // 2) 再等已调度音频在 Web Audio timeline 上播完（getRemainingPlayoutSeconds → 0）
    // 3) 加 350ms 安全 lead，让最后几个 buffer source 完整 release
    const timer = setInterval(() => {
      if (busyRef.current || queueRef.current.length > 0) return
      const remaining = playerRef.current.getRemainingPlayoutSeconds()
      if (remaining > 0.05) return
      clearInterval(timer)
      setTimeout(() => {
        playerStartedRef.current = false
        void playerRef.current.stop()
      }, 350)
    }, 150)
  }, [enqueue])

  const abort = useCallback(async () => {
    queueRef.current = []
    bufferRef.current = ''
    busyRef.current = false
    playerStartedRef.current = false
    setPending(0)
    setIsPlaying(false)
    await playerRef.current.stop()
  }, [])

  return { enabled, setEnabled, voiceId, feed, flushAndStop, abort, isPlaying, pending }
}
