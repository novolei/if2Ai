/**
 * RecordingIndicator —— 浮动录音指示器（受 open-flow `assets/recording-overlay.png` 启发）。
 *
 * 当 SttButton 处于 recording / processing 状态时，由 SttButton 通过 portal 渲染一个药丸形浮层
 * 到屏幕中央偏下位置，包含：
 * - 红色脉冲圆点 + "Recording…" 文案 + 实时秒表（recording 状态）
 * - 旋转 spinner + "转写中…" 文案（processing 状态）
 * - 振幅动画（基于 RAF 模拟，无需对接 Web Audio AnalyserNode 也能给到节拍感）
 *
 * 设计原则：
 * - 不阻挡鼠标（pointer-events: none）
 * - 浅色磨砂背景，与 if2ai 整体玉色 / 米白调一致
 * - portal 到 body，避免被父容器 overflow:hidden 裁掉
 */

import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { Loader2, Mic } from 'lucide-react'

interface Props {
  state: 'recording' | 'processing'
  /** 当前 provider，用于显示「使用 SenseVoice / Whisper / Groq 转写中…」 */
  providerLabel?: string
}

export function RecordingIndicator({ state, providerLabel }: Props) {
  const [seconds, setSeconds] = useState(0)
  const [pulse, setPulse] = useState(0)

  useEffect(() => {
    if (state !== 'recording') return
    setSeconds(0)
    const t0 = performance.now()
    let raf = 0
    const tick = () => {
      const elapsed = (performance.now() - t0) / 1000
      setSeconds(elapsed)
      // 模拟音频振幅波动 0..1
      setPulse(0.5 + 0.5 * Math.sin(elapsed * 9))
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [state])

  const content = (
    <div
      className="pointer-events-none fixed inset-x-0 bottom-32 z-[9999] flex justify-center"
      role="status"
      aria-live="polite"
    >
      <div className="flex items-center gap-2.5 rounded-full border border-black/[0.08] bg-white/92 px-4 py-2 shadow-lg shadow-black/[0.08] backdrop-blur-md">
        {state === 'recording' ? (
          <>
            <span className="relative flex h-2.5 w-2.5">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-red-500 opacity-60" />
              <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-red-500" />
            </span>
            <Mic className="size-3.5 text-black/60" />
            <span className="text-[12px] font-semibold text-black/80">Recording…</span>
            {/* 振幅条 */}
            <div className="flex items-end gap-[2px]">
              {[0, 1, 2, 3, 4].map((i) => {
                const h = 4 + Math.abs(Math.sin((pulse + i * 0.7) * Math.PI)) * 12
                return (
                  <span
                    key={i}
                    className="w-[2px] rounded-full bg-jade transition-all"
                    style={{ height: `${h}px` }}
                  />
                )
              })}
            </div>
            <span className="font-mono text-[11px] tabular-nums text-black/55">
              {formatSeconds(seconds)}
            </span>
          </>
        ) : (
          <>
            <Loader2 className="size-3.5 animate-spin text-jade" />
            <span className="text-[12px] font-semibold text-black/80">
              {providerLabel ? `${providerLabel} 转写中…` : '转写中…'}
            </span>
          </>
        )}
      </div>
    </div>
  )

  return createPortal(content, document.body)
}

function formatSeconds(s: number): string {
  const total = Math.floor(s)
  const m = Math.floor(total / 60)
  const sec = total % 60
  return `${m}:${sec.toString().padStart(2, '0')}`
}
