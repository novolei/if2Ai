/**
 * FactCard — S2 skeleton card that renders one `SessionSummaryDto`
 * inside MemoryNarrativeViewer's date-grouped timeline.
 *
 * Phase 8B.11 / T-UI-3 (S2 skeleton).  The S3 increment will extend
 * this to also render extracted facts (compile_facts output) — the
 * `fact.id` / `fact.tags` / `fact.importance` fields are intentionally
 * not surfaced here yet so the prop shape stays narrow until S3.
 */

import { MessageSquare, Sparkles } from 'lucide-react'
import type { SessionSummaryDto } from '@/api/memory'

export interface FactCardProps {
  /** Session-summary row to render. */
  summary: SessionSummaryDto
}

export function FactCard({ summary }: FactCardProps) {
  const time = new Date(summary.updated_at).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
  })
  const isRolling = summary.source === 'rolling'
  return (
    <div className="rounded-lg border border-black/[0.06] bg-background/40 p-3 transition-colors hover:bg-black/[0.02] dark:border-white/[0.08] dark:hover:bg-white/[0.04]">
      <div className="mb-1.5 flex items-center justify-between">
        <div className="flex items-center gap-2 text-[10.5px] text-muted-foreground">
          <span
            className={`flex items-center gap-1 rounded px-1.5 py-0.5 ${
              isRolling
                ? 'bg-blue-100 text-blue-700 dark:bg-blue-950/40 dark:text-blue-200'
                : 'bg-purple-100 text-purple-700 dark:bg-purple-950/40 dark:text-purple-200'
            }`}
          >
            {isRolling ? (
              <>
                <MessageSquare className="h-2.5 w-2.5" /> 滚动摘要
              </>
            ) : (
              <>
                <Sparkles className="h-2.5 w-2.5" /> Compact
              </>
            )}
          </span>
          <span className="font-mono">{time}</span>
          <span className="font-mono">{summary.message_count} 条消息</span>
        </div>
        <span className="font-mono text-[10px] text-muted-foreground">
          {summary.session_id.slice(0, 8)}
        </span>
      </div>
      <p className="whitespace-pre-wrap text-[12px] leading-relaxed text-foreground/85">
        {summary.summary || '(空摘要)'}
      </p>
    </div>
  )
}
