/**
 * MessageVoiceButton — 悬停在 Assistant 消息上显示的语音播放按钮。
 *
 * 自包含：直接读 localStorage agentVoiceId，用 ttsStreamStart 合成。
 * 不需要 prop-drilling，只接受 message content 字符串。
 *
 * 状态：
 * - idle（未播放）→ 悬停显示 ▶
 * - loading（合成中）→ 旋转圈
 * - playing（播放中）→ ■ 停止
 * - error → 感叹号（500ms 后自动回 idle）
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { Volume2, Loader2, Square, AlertCircle } from 'lucide-react'
import { applyTtsPostprocess, ttsStreamStart, TTS_DEFAULT_PARAMS } from '@/lib/tauri'
import { getAgentVoiceId } from '@/modules/settings/pages/AgentVoicePicker'
import { useWebAudioStreamPlayer } from '@/modules/settings/pages/useWebAudioStreamPlayer'
import { resolveActiveProfile, type ResolvedActiveProfile } from './activeTtsProfile'
import { sanitizeForTts } from './ttsSanitize'
import { useCrossWindowChange } from '@/lib/crossWindowSync'

type State = 'idle' | 'loading' | 'playing' | 'error'

interface Props {
  /** The full message content (markdown ok — TTS will receive raw text). */
  text: string
  /** Extra class for button container. */
  className?: string
}

export function MessageVoiceButton({ text, className = '' }: Props) {
  const [btnState, setBtnState] = useState<State>('idle')
  const player = useWebAudioStreamPlayer()
  const abortRef = useRef(false)
  // The active TTS profile resolves asynchronously from
  // `~/.if2ai/tts_profiles.json`.  When null we still render (using
  // the legacy agent voice id) so users that haven't visited the new
  // settings page aren't broken.
  const [resolved, setResolved] = useState<ResolvedActiveProfile | null>(null)
  const [legacyVoiceId, setLegacyVoiceId] = useState<string | null>(getAgentVoiceId())

  const refreshProfile = useCallback(() => {
    void resolveActiveProfile().then((r) => setResolved(r ?? null))
  }, [])
  useEffect(refreshProfile, [refreshProfile])

  useCrossWindowChange<{ id?: string | null }>('cross:tts-profiles-changed', refreshProfile)
  useCrossWindowChange<{ id?: string | null }>('cross:tts-active-profile-changed', refreshProfile)
  useCrossWindowChange<{ id: string | null }>('cross:agent-voice-changed', (payload) => {
    setLegacyVoiceId(payload?.id ?? getAgentVoiceId())
  })

  // Effective voice + params + postprocess: profile wins; legacy
  // voice picker provides a fallback.
  const effectiveVoiceId = resolved?.profile.voice_id || legacyVoiceId
  const effectiveParams = resolved
    ? resolved.params
    : { ...TTS_DEFAULT_PARAMS, max_new_frames: 375, seed: null }

  const handlePlay = useCallback(async () => {
    if (btnState === 'playing') {
      // 停止
      abortRef.current = true
      await player.stop()
      setBtnState('idle')
      return
    }
    if (btnState === 'loading') return

    const vid = effectiveVoiceId
    if (!vid) {
      alert('请先在「设置 → TTS Profiles」中选择一个 Profile（或在 TTS 测试中选 Agent 声音）。')
      return
    }

    const baseClean = sanitizeForTts(text)
    if (!baseClean) return
    const clean = resolved ? applyTtsPostprocess(resolved.profile, baseClean) : baseClean

    abortRef.current = false
    setBtnState('loading')
    try {
      // Apply the profile's playback rate to this player too.
      if (resolved) player.setPlaybackRate(resolved.profile.playback_rate)
      await player.start()
      if (abortRef.current) return
      const result = await ttsStreamStart(
        clean,
        null,
        null,
        effectiveParams,
        vid,
      )
      if (abortRef.current) {
        await player.stop()
        return
      }
      player.setExpectedStreamId(result.stream_id)
      setBtnState('playing')
      // 监听 player 状态自动回 idle
      const check = setInterval(() => {
        if (!player.isActive || player.metrics.scheduledChunks === 0) {
          // 等一小会儿让最后一块播完
          clearInterval(check)
          setTimeout(() => {
            if (!abortRef.current) setBtnState('idle')
          }, 1500)
        }
      }, 300)
    } catch (e) {
      console.error('[MessageVoiceButton] synth failed:', e)
      setBtnState('error')
      setTimeout(() => setBtnState('idle'), 1500)
    }
  }, [btnState, effectiveVoiceId, effectiveParams, resolved, text, player])

  useEffect(() => {
    return () => {
      abortRef.current = true
    }
  }, [])

  if (!effectiveVoiceId) return null

  const icon = {
    idle: <Volume2 className="size-3.5" />,
    loading: <Loader2 className="size-3.5 animate-spin" />,
    playing: <Square className="size-3 fill-current" />,
    error: <AlertCircle className="size-3.5" />,
  }[btnState]

  const title = {
    idle: '播放语音',
    loading: '合成中…',
    playing: '停止播放',
    error: '合成失败',
  }[btnState]

  return (
    <button
      type="button"
      onClick={() => void handlePlay()}
      disabled={btnState === 'loading'}
      title={title}
      className={`flex items-center justify-center rounded-md p-0.5 text-black/35 transition-colors hover:bg-black/[0.05] hover:text-black/65 disabled:opacity-50 ${
        btnState === 'playing' ? 'text-jade hover:text-jade/80' : ''
      } ${btnState === 'error' ? 'text-rose-500' : ''} ${className}`}
    >
      {icon}
    </button>
  )
}
