/**
 * EmptyState — placeholder shown when a chat session has no messages.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` (GF-01 PR-01) without
 * behaviour change. Markup, copy, and tailwind classes match the
 * legacy declaration.
 */

import * as React from 'react'
import { Sparkles } from 'lucide-react'

/**
 * Render the empty-transcript hero card. `sessionTitle` and
 * `projectLabel` are inserted verbatim into the headline / body.
 */
export function EmptyState({
  sessionTitle,
  projectLabel,
}: {
  sessionTitle: string
  projectLabel: string
}): React.JSX.Element {
  return (
    <div className="flex min-h-[55vh] flex-col items-center justify-center gap-6 rounded-[2rem] border border-dashed border-border/70 bg-surface px-8 py-16 text-center">
      <div className="flex h-18 w-18 items-center justify-center rounded-[1.75rem] bg-muted text-muted-foreground shadow-inner">
        <Sparkles className="h-8 w-8" />
      </div>
        <div className="max-w-xl space-y-3">
        <h2 className="text-[22px] font-semibold tracking-tight">{sessionTitle}</h2>
        <p className="text-[13px] leading-6 text-muted-foreground">
          当前工作区是 <span className="text-foreground/85">{projectLabel}</span>。输入任务后，右侧会按照 Codex 的节奏显示变更摘要、思考过程和正文。
        </p>
      </div>
    </div>
  )
}
