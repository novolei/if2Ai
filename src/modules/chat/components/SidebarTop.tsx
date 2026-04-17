import type { MouseEvent as ReactMouseEvent } from 'react'
import { Button } from '@/components/ui/button'

export function SidebarTop({
  onStartWindowDrag,
  onNewThread,
}: {
  onStartWindowDrag: (event: ReactMouseEvent<HTMLElement>) => void
  onNewThread: () => void
}) {
  return (
    <div
      className="window-drag grid h-[72px] grid-cols-[minmax(0,1fr)_auto] items-center border-b border-black/5 select-none"
      onMouseDown={onStartWindowDrag}
    >
      <div className="flex min-w-0 flex-col justify-center px-4">
        <div className="truncate text-[22px] font-semibold tracking-tight text-black/90">Chat</div>
      </div>

      <div className="pr-4">
        <Button
          className="window-no-drag h-10 rounded-full bg-emerald-500 px-5 text-[13px] font-semibold text-white shadow-none hover:bg-emerald-500/90"
          data-window-no-drag="true"
          onClick={onNewThread}
          type="button"
        >
          新线程
        </Button>
      </div>
    </div>
  )
}
