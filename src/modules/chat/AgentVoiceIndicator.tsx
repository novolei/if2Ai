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

import { useCallback, useEffect, useRef, useState } from 'react'
import { GripVertical, Volume2, VolumeX, Loader2 } from 'lucide-react'
import { getAgentVoiceEnabled, setAgentVoiceEnabled } from '@/modules/settings/pages/AgentVoicePicker'
import { useCrossWindowChange } from '@/lib/crossWindowSync'
import { TtsProfilePicker } from './TtsProfilePicker'

interface Props {
  isPlaying: boolean
  pending: number
}

/**
 * 浮层位置在 localStorage 里持久化，键名加版本以便后续重置默认值。
 * 坐标是"距 viewport 左/上"的绝对像素，原始默认锚点 (bottom 72, left 16)
 * 在第一次渲染时换算成等价的 (left, top)。
 */
const POSITION_STORAGE_KEY = 'agentVoiceIndicator.position.v1'
const DEFAULT_BOTTOM_OFFSET = 72
const DEFAULT_LEFT_OFFSET = 16

interface Position {
  left: number
  top: number
}

function loadInitialPosition(): Position | null {
  if (typeof window === 'undefined') return null
  try {
    const raw = window.localStorage.getItem(POSITION_STORAGE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as Partial<Position>
    if (typeof parsed.left === 'number' && typeof parsed.top === 'number') {
      return { left: parsed.left, top: parsed.top }
    }
  } catch {
    /* 静默回退到默认锚点 */
  }
  return null
}

/**
 * Clamp `p` (in `el.offsetParent` 坐标系) 到容器可视区域内，留出 8px
 * 边距。`offsetParent` 缺失时退化为 viewport，避免脱离 DOM 时崩溃。
 */
function clampToContainer(p: Position, el: HTMLElement | null): Position {
  if (!el) return p
  const parent = (el.offsetParent as HTMLElement | null) ?? null
  const elRect = el.getBoundingClientRect()
  const containerWidth = parent
    ? parent.clientWidth
    : typeof window !== 'undefined'
      ? window.innerWidth
      : Number.POSITIVE_INFINITY
  const containerHeight = parent
    ? parent.clientHeight
    : typeof window !== 'undefined'
      ? window.innerHeight
      : Number.POSITIVE_INFINITY
  const margin = 8
  const maxLeft = Math.max(margin, containerWidth - elRect.width - margin)
  const maxTop = Math.max(margin, containerHeight - elRect.height - margin)
  return {
    left: Math.min(Math.max(margin, p.left), maxLeft),
    top: Math.min(Math.max(margin, p.top), maxTop),
  }
}

export function AgentVoiceIndicator({ isPlaying, pending }: Props) {
  const [enabled, setEnabledState] = useState(getAgentVoiceEnabled())
  const containerRef = useRef<HTMLDivElement>(null)
  const dragStateRef = useRef<{
    pointerId: number
    startX: number
    startY: number
    originLeft: number
    originTop: number
  } | null>(null)
  const [position, setPosition] = useState<Position | null>(loadInitialPosition)
  const [dragging, setDragging] = useState(false)

  useCrossWindowChange<{ enabled: boolean }>('cross:agent-voice-enabled', (payload) => {
    setEnabledState(payload?.enabled ?? getAgentVoiceEnabled())
  })

  // 第一次挂载、且 localStorage 没有记录时，把默认 (bottom-72, left-16)
  // 换算成 (left, top) 的容器内坐标。容器是 ChatUI 内部最近的
  // `position: relative`/`absolute` 祖先 (offsetParent)；这样左侧不会
  // 被 sidebar 遮住，行为与原本 `bottom-72 left-4` 一致。
  useEffect(() => {
    if (position !== null) return
    const el = containerRef.current
    if (!el) return
    const elRect = el.getBoundingClientRect()
    const parent = (el.offsetParent as HTMLElement | null) ?? null
    const parentHeight = parent?.clientHeight ?? window.innerHeight
    setPosition({
      left: DEFAULT_LEFT_OFFSET,
      top: parentHeight - elRect.height - DEFAULT_BOTTOM_OFFSET,
    })
  }, [position])

  // 容器尺寸变化时把面板拉回可视区域。监听 ResizeObserver 在
  // offsetParent 上，比 window resize 更精确（侧栏抽屉开合也能触发）。
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const parent = (el.offsetParent as HTMLElement | null) ?? null
    if (!parent || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(() => {
      setPosition((prev) =>
        prev ? clampToContainer(prev, containerRef.current) : prev,
      )
    })
    observer.observe(parent)
    return () => observer.disconnect()
  }, [])

  // 持久化最新位置（debounced via micro-task is overkill；setItem 已经够快）
  useEffect(() => {
    if (!position || typeof window === 'undefined') return
    try {
      window.localStorage.setItem(POSITION_STORAGE_KEY, JSON.stringify(position))
    } catch {
      /* 忽略 storage 配额或隐私模式异常 */
    }
  }, [position])

  const toggleEnabled = useCallback(() => {
    const next = !enabled
    setAgentVoiceEnabled(next)
    setEnabledState(next)
  }, [enabled])

  // 拖动期间把"抓取"光标贴在 <html> 上，这样光标移出把手后仍然
  // 显示 grabbing；同时禁用 user-select，避免拖快了选中下面的文本。
  // 拖动结束清理两侧 inline 样式即可，不污染全局 CSS。
  useEffect(() => {
    if (!dragging || typeof document === 'undefined') return
    const root = document.documentElement
    const prevCursor = root.style.cursor
    const prevSelect = root.style.userSelect
    root.style.cursor = 'grabbing'
    root.style.userSelect = 'none'
    return () => {
      root.style.cursor = prevCursor
      root.style.userSelect = prevSelect
    }
  }, [dragging])

  const onHandlePointerDown = useCallback((e: React.PointerEvent<HTMLButtonElement>) => {
    if (!position || !containerRef.current) return
    e.preventDefault()
    e.stopPropagation()
    const target = e.currentTarget
    target.setPointerCapture(e.pointerId)
    dragStateRef.current = {
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
      originLeft: position.left,
      originTop: position.top,
    }
    setDragging(true)
  }, [position])

  const onHandlePointerMove = useCallback((e: React.PointerEvent<HTMLButtonElement>) => {
    const drag = dragStateRef.current
    if (!drag || drag.pointerId !== e.pointerId) return
    const next = clampToContainer(
      {
        left: drag.originLeft + (e.clientX - drag.startX),
        top: drag.originTop + (e.clientY - drag.startY),
      },
      containerRef.current,
    )
    setPosition(next)
  }, [])

  const onHandlePointerUp = useCallback((e: React.PointerEvent<HTMLButtonElement>) => {
    const drag = dragStateRef.current
    if (!drag || drag.pointerId !== e.pointerId) return
    e.currentTarget.releasePointerCapture(e.pointerId)
    dragStateRef.current = null
    setDragging(false)
  }, [])

  // 第一次还没算出位置时用 invisible 占位，让 useEffect 拿到 height
  // 后再正式渲染；这样初次挂载不会闪到屏幕中心。
  const positioningStyle: React.CSSProperties = position
    ? { left: position.left, top: position.top, bottom: 'auto', right: 'auto' }
    : {
        left: DEFAULT_LEFT_OFFSET,
        bottom: DEFAULT_BOTTOM_OFFSET,
        visibility: 'hidden',
      }

  return (
    <div
      ref={containerRef}
      style={positioningStyle}
      className={`pointer-events-auto absolute z-30 flex items-center gap-1 rounded-xl border border-black/[0.08] bg-white/90 px-1.5 py-1.5 text-[10.5px] shadow-sm backdrop-blur-md transition-shadow ${
        dragging ? 'shadow-md' : ''
      }`}
    >
      {/* 拖拽把手 */}
      <button
        type="button"
        onPointerDown={onHandlePointerDown}
        onPointerMove={onHandlePointerMove}
        onPointerUp={onHandlePointerUp}
        onPointerCancel={onHandlePointerUp}
        title="拖动浮层"
        aria-label="拖动浮层"
        className={`group/grip flex items-center justify-center rounded p-0.5 text-black/25 hover:text-black/55 ${
          dragging ? 'cursor-grabbing text-black/55' : 'cursor-grab'
        }`}
      >
        <GripVertical className="size-3" />
      </button>

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
