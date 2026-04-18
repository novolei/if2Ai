/**
 * DateGroupHeader — sticky day-divider used by MemoryNarrativeViewer to
 * mark each `YYYY-MM-DD` group of session summaries.
 *
 * Phase 8B.11 / T-UI-3 (S2 skeleton).
 */

import { Calendar } from 'lucide-react'

export interface DateGroupHeaderProps {
  /** Display label for the group (typically `YYYY-MM-DD`). */
  date: string
  /** Number of items rendered under this header. */
  count: number
}

export function DateGroupHeader({ date, count }: DateGroupHeaderProps) {
  return (
    <div className="sticky top-0 z-10 mb-2 flex items-center gap-2 border-b border-black/[0.06] bg-background/95 py-1.5 backdrop-blur dark:border-white/[0.08]">
      <Calendar className="h-3 w-3 text-muted-foreground" />
      <span className="text-[12px] font-semibold">{date}</span>
      <span className="text-[10.5px] text-muted-foreground">({count} 条)</span>
    </div>
  )
}
