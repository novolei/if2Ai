/**
 * AgentVoiceIndicator — 聊天窗口左下角的语音状态浮层。
 *
 * 显示条件：agentVoiceId 不为空时。
 * 状态：
 * - 静止：小图标 + 声音名字，点击 → 跳转 Settings/TTS 页；
 *   右边有"启用/禁用 Agent 语音"快捷 toggle。
 * - 播放中（isPlaying=true）：绿色动画圆点 + "正在播放"。
 * - pending > 0：显示 pending 数（队列中）。
 *
 * 用法：直接放在 ChatUI 外层（App.tsx），读 agentVoice 来自全局 hook。
 */

import { useCallback, useState } from 'react'
import { Volume2, VolumeX, Loader2 } from 'lucide-react'
import { getAgentVoiceEnabled, setAgentVoiceEnabled } from '@/modules/settings/pages/AgentVoicePicker'
import { useCrossWindowChange } from '@/lib/crossWindowSync'
import { TtsProfilePicker } from './TtsProfilePicker'

interface Props {
  isPlaying: boolean
  pending: number
}

export function AgentVoiceIndicator({ isPlaying, pending }: Props) {
  const [enabled, setEnabledState] = useState(getAgentVoiceEnabled())

  useCrossWindowChange<{ enabled: boolean }>('cross:agent-voice-enabled', (payload) => {
    setEnabledState(payload?.enabled ?? getAgentVoiceEnabled())
  })

  const toggleEnabled = useCallback(() => {
    const next = !enabled
    setAgentVoiceEnabled(next)
    setEnabledState(next)
  }, [enabled])

  return (
    <div className="pointer-events-auto absolute bottom-[72px] left-4 z-30 flex items-center gap-1.5 rounded-xl border border-black/[0.08] bg-white/90 px-2 py-1.5 text-[10.5px] shadow-sm backdrop-blur-md transition-opacity">
      {/* 播放状态指示 */}
      {isPlaying ? (
        <span className="relative flex size-2">
          <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-jade opacity-75" />
          <span className="relative inline-flex size-2 rounded-full bg-jade" />
        </span>
      ) : (
        <Volume2 className={`size-3 ${enabled ? 'text-jade' : 'text-black/30'}`} />
      )}

      {/* 状态文字 / Profile picker */}
      {isPlaying ? (
        <span className="text-foreground/75">语音回复中</span>
      ) : pending > 0 ? (
        <span className="flex items-center gap-1 text-foreground/75">
          <Loader2 className="size-2.5 animate-spin" />
          合成中 {pending > 1 ? `(${pending})` : ''}
        </span>
      ) : (
        <TtsProfilePicker />
      )}

      {/* 启用 / 禁用快捷 toggle */}
      <button
        type="button"
        onClick={toggleEnabled}
        title={enabled ? '关闭 Agent 语音' : '开启 Agent 语音'}
        className="ml-0.5 rounded p-0.5 text-black/35 hover:bg-black/5 hover:text-black/65"
      >
        {enabled ? <Volume2 className="size-3" /> : <VolumeX className="size-3" />}
      </button>
    </div>
  )
}
