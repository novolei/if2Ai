import type { MouseEvent as ReactMouseEvent } from 'react'
import { Sparkles } from 'lucide-react'

export function SidebarTop({
  onStartWindowDrag,
  onNewThread,
}: {
  onStartWindowDrag: (event: ReactMouseEvent<HTMLElement>) => void
  onNewThread: () => void
}) {
  return (
    <div
      className="window-drag grid h-[60px] grid-cols-[minmax(0,1fr)_auto] items-center border-b border-black/[0.06] select-none"
      onMouseDown={onStartWindowDrag}
    >
      {/* ── Logo mark ──────────────────────────────── */}
      <div className="flex min-w-0 items-center gap-2.5 px-4">
        {/* Icon badge */}
        <div className="flex size-7 shrink-0 items-center justify-center rounded-[8px] bg-gradient-to-br from-jade/80 to-jade shadow-sm shadow-jade/25">
          <Sparkles className="size-3.5 text-white" strokeWidth={1.5} />
        </div>

        {/* App name */}
        <div className="flex min-w-0 flex-col justify-center gap-0">
          <span className="truncate text-[14px] font-bold tracking-tight text-foreground/85">
            If2Ai
          </span>
          <span className="text-[9.5px] font-medium tracking-widest text-muted-foreground/35 uppercase">
            智能助理
          </span>
        </div>
      </div>

      {/* ── New chat button ─────────────────────────── */}
      <div className="pr-3">
        <button
          type="button"
          data-window-no-drag="true"
          onClick={onNewThread}
          className="window-no-drag group flex h-7 cursor-pointer items-center gap-1.5 rounded-lg bg-jade/10 pl-2.5 pr-3 text-[12px] font-semibold text-jade transition-all hover:bg-jade hover:text-white hover:shadow-sm hover:shadow-jade/25 active:scale-[0.97]"
        >
          <svg
            className="size-3 transition-transform group-hover:rotate-45 group-hover:scale-110"
            fill="none"
            stroke="currentColor"
            strokeWidth={2.5}
            viewBox="0 0 16 16"
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M8 3v10M3 8h10" />
          </svg>
          新聊天
        </button>
      </div>
    </div>
  )
}
