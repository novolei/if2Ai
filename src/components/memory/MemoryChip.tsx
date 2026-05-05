/**
 * MemoryChip — Inline memory reference indicator for chat messages.
 *
 * Renders a compact, pill-shaped badge that shows how many memory items
 * were recalled for the current assistant response. Clicking it opens the
 * `MemoryEvidencePanel` popover.
 *
 * This component is intended to be rendered inside `ChatMessage` (or its
 * future `src/components/chat/ChatMessage.tsx` split) when the
 * `StreamTokenPayload.memory_context` field is present.
 *
 * Dependencies (will be wired once MemoryAuditEmitter + MemoryEventBridge
 * are fully live on the backend):
 *   - `MemoryContextItem` from `@/lib/tauri`
 *   - `MemoryEvidencePanel` (co-located in this file)
 */

import { useState } from 'react'
import { Brain } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { MemoryContextItem } from '@/lib/tauri'
import { MemoryEvidencePanel } from './MemoryEvidencePanel'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

// ── Hover helpers ─────────────────────────────────────────────────────────────

const SCOPE_LABEL: Record<MemoryContextItem['scope'], string> = {
  global: '全局',
  project: '项目',
  session: '会话',
}

// ── MemoryChip ────────────────────────────────────────────────────────────────

export interface MemoryChipProps {
  /** Memory items recalled for this response. Must be non-empty. */
  items: MemoryContextItem[]
  /** Additional CSS class names for the chip container. */
  className?: string
}

/**
 * A compact pill that shows the number of recalled memory items.
 * Clicking it toggles the `MemoryEvidencePanel` popover.
 */
export function MemoryChip({ items, className }: MemoryChipProps) {
  const [open, setOpen] = useState(false)

  if (items.length === 0) return null

  return (
    <div className={cn('relative inline-block', className)}>
      <TooltipProvider delayDuration={300}>
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              onClick={() => setOpen((prev) => !prev)}
              className={cn(
                'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium transition-colors duration-150',
                'border border-primary/20 bg-primary/8 text-primary/70',
                'hover:border-primary/40 hover:bg-primary/15 hover:text-primary',
                open && 'border-primary/40 bg-primary/15 text-primary',
              )}
              aria-label={`查看 ${items.length} 条引用记忆`}
              aria-expanded={open}
            >
              <Brain className="h-3 w-3 shrink-0" aria-hidden />
              <span>{items.length} 条记忆</span>
            </button>
          </TooltipTrigger>
          {!open && (
            <TooltipContent
              side="top"
              align="start"
              className="max-w-xs border border-border bg-popover p-3 text-popover-foreground shadow-md"
            >
              <div className="space-y-2">
                <div className="text-[11px] font-semibold text-muted-foreground">
                  已捕获 {items.length} 条记忆
                </div>
                {items.slice(0, 5).map((m) => (
                  <div key={m.id} className="space-y-0.5">
                    <div className="flex items-center gap-1.5">
                      <span className="rounded bg-muted px-1 py-px font-mono text-[10px] text-foreground/70">
                        {SCOPE_LABEL[m.scope]}
                      </span>
                      {m.relevance_score !== undefined && (
                        <span className="text-[10px] tabular-nums text-muted-foreground">
                          {Math.round(m.relevance_score * 100)}%
                        </span>
                      )}
                    </div>
                    <p className="line-clamp-2 text-xs leading-relaxed text-foreground/85">
                      {m.content}
                    </p>
                  </div>
                ))}
                {items.length > 5 && (
                  <p className="text-[10px] text-muted-foreground">
                    还有 {items.length - 5} 条…点击查看全部
                  </p>
                )}
              </div>
            </TooltipContent>
          )}
        </Tooltip>
      </TooltipProvider>

      {open && (
        <MemoryEvidencePanel
          items={items}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  )
}
