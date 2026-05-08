/**
 * GF-01 PR-03 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * The small clipboard glyph that appears next to a chat message, used to
 * copy the message body. Visibility is hover-driven by default but the
 * parent can pin it open via the `visible` prop (e.g. immediately after a
 * successful copy so the green confirmation glyph remains readable).
 */

import * as React from "react"
import { Check, Copy } from "lucide-react"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"

/** Render a hover-revealed copy-message button anchored to one side of a message row. */
export function MessageCopyButton({
  side,
  copied,
  visible,
  onClick,
}: {
  side: 'left' | 'right'
  copied: boolean
  visible: boolean
  onClick: () => void
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      onClick={onClick}
      aria-label={copied ? '已复制' : '复制消息'}
      className={cn(
        'h-4 w-4 shrink-0 rounded-full border-0 bg-transparent p-0 text-muted-foreground/55 shadow-none transition-[opacity,color,background-color,box-shadow] duration-150 hover:bg-accent hover:text-accent-foreground',
        visible ? 'opacity-100' : 'opacity-0 group-hover:opacity-100',
        copied && 'opacity-100 bg-emerald-500/8 text-emerald-600 shadow-[0_0_0_1px_rgba(16,185,129,0.1)]',
        side === 'left' ? 'order-first' : 'order-last'
      )}
    >
      {copied ? <Check className="size-2.5" /> : <Copy className="size-2.5" />}
    </Button>
  )
}
