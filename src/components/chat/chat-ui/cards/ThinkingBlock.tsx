/**
 * ThinkingBlock — collapsible "已完成思考" panel rendered above
 * assistant messages that carried hidden reasoning.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) without
 * behaviour change. Open/close state, transitions, and typography
 * match the legacy declaration.
 */

import * as React from 'react'
import { ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'
import { formatDuration } from '../utils/text'

/**
 * Toggleable thinking panel. `defaultOpen` controls the initial state
 * and re-collapses whenever the `thinking` text identity changes (so
 * each new message starts collapsed by default).
 */
export function ThinkingBlock({
  thinking,
  thinkingTime,
  defaultOpen = false,
}: {
  thinking: string
  thinkingTime?: number
  defaultOpen?: boolean
}): React.JSX.Element {
  const [open, setOpen] = React.useState(defaultOpen)
  React.useEffect(() => {
    setOpen(defaultOpen)
  }, [defaultOpen, thinking])
  const durationLabel = thinkingTime ? formatDuration(thinkingTime) : '—'

  return (
    <div className="relative mb-3 pl-4">
      <div className="absolute bottom-0 left-[5px] top-0 w-px bg-border/70" />
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="flex items-center gap-1.5 rounded-none px-0 py-[2px] text-[12px] font-light italic tracking-tight text-muted-foreground transition-colors hover:text-foreground"
      >
        <span className="h-1.5 w-1.5 rounded-full bg-muted-foreground/70" />
        <span>已完成思考</span>
        <span className="text-muted-foreground/70 not-italic"> {durationLabel}</span>
        <ChevronDown className={cn('ml-0.5 h-3 w-3 transition-transform', open && 'rotate-180')} />
      </button>

      <div
        className={cn(
          'overflow-hidden transition-[max-height,opacity] duration-200',
          open ? 'max-h-[360px] opacity-100' : 'max-h-0 opacity-0'
        )}
      >
        <div className="ml-[10px] border-l border-border/55 pl-3 pt-1">
          <div className="whitespace-pre-wrap px-1 text-[11.5px] font-light italic leading-6 text-muted-foreground">
            {thinking}
          </div>
        </div>
      </div>
    </div>
  )
}
