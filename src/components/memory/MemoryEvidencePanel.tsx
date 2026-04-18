/**
 * MemoryEvidencePanel — Popover showing recalled memory items for a response.
 *
 * Displays a scrollable list of `MemoryContextItem` entries — the episodic /
 * semantic memory fragments that the backend retrieved and injected into the
 * system prompt for the current assistant turn.
 *
 * Designed to be opened by `MemoryChip` via an absolute-positioned overlay.
 * Closes when the user clicks outside (via a backdrop `<div>`) or presses
 * the close button.
 */

import { X } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { MemoryContextItem } from '@/lib/tauri'

// ── Helpers ───────────────────────────────────────────────────────────────────

const SCOPE_LABEL: Record<MemoryContextItem['scope'], string> = {
  global: '全局',
  project: '项目',
  session: '会话',
}

const SCOPE_COLOR: Record<MemoryContextItem['scope'], string> = {
  global: 'bg-jade-light/60 text-jade-dim',
  project: 'bg-teal-light/60 text-teal',
  session: 'bg-muted text-muted-foreground',
}

function formatStoredAt(iso: string | undefined): string {
  if (!iso) return ''
  try {
    const date = new Date(iso)
    const now = new Date()
    const diffMs = now.getTime() - date.getTime()
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60))
    if (diffHours < 1) return '刚刚存入'
    if (diffHours < 24) return `${diffHours} 小时前`
    const diffDays = Math.floor(diffHours / 24)
    return `${diffDays} 天前`
  } catch {
    return iso
  }
}

function RelevanceBar({ score }: { score: number | undefined }) {
  if (score === undefined) return null
  const pct = Math.round(Math.max(0, Math.min(1, score)) * 100)
  return (
    <div className="mt-1.5 flex items-center gap-1.5">
      <div className="h-1 flex-1 overflow-hidden rounded-full bg-muted">
        <div
          className="h-full rounded-full bg-primary/50 transition-all"
          style={{ width: `${pct}%` }}
          aria-label={`相关度 ${pct}%`}
        />
      </div>
      <span className="text-[10px] tabular-nums text-muted-foreground">{pct}%</span>
    </div>
  )
}

// ── MemoryEvidencePanel ───────────────────────────────────────────────────────

export interface MemoryEvidencePanelProps {
  /** Memory items to display. */
  items: MemoryContextItem[]
  /** Called when the panel should be closed. */
  onClose: () => void
  /** Additional CSS class names for the panel container. */
  className?: string
}

/**
 * A floating panel listing the memory items recalled for the current response.
 * Renders as an absolute-positioned card; use inside a `relative` container.
 */
export function MemoryEvidencePanel({ items, onClose, className }: MemoryEvidencePanelProps) {
  return (
    <>
      {/* Invisible backdrop to capture outside clicks */}
      <div
        className="fixed inset-0 z-40"
        aria-hidden
        onClick={onClose}
      />

      <div
        role="dialog"
        aria-label="引用的记忆"
        className={cn(
          'absolute bottom-full left-0 z-50 mb-2 w-72 rounded-xl',
          'border border-border bg-popover shadow-token-lg',
          className,
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-3 py-2">
          <span className="text-[12px] font-semibold text-foreground/80">
            本次引用的记忆
          </span>
          <button
            type="button"
            onClick={onClose}
            className="rounded p-0.5 text-muted-foreground hover:bg-accent hover:text-foreground"
            aria-label="关闭"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        {/* Items list */}
        <ul
          className="max-h-64 divide-y divide-border/50 overflow-y-auto"
          aria-label={`${items.length} 条引用记忆`}
        >
          {items.map((item) => (
            <li key={item.id} className="px-3 py-2.5">
              {/* Scope badge + time */}
              <div className="mb-1 flex items-center justify-between gap-2">
                <span
                  className={cn(
                    'rounded-full px-1.5 py-0.5 text-[10px] font-medium',
                    SCOPE_COLOR[item.scope],
                  )}
                >
                  {SCOPE_LABEL[item.scope]}
                </span>
                {item.stored_at && (
                  <span className="text-[10px] text-muted-foreground">
                    {formatStoredAt(item.stored_at)}
                  </span>
                )}
              </div>

              {/* Content preview */}
              <p className="line-clamp-3 text-[12px] leading-relaxed text-foreground/80">
                {item.content}
              </p>

              {/* Relevance bar */}
              <RelevanceBar score={item.relevance_score} />
            </li>
          ))}
        </ul>

        {/* Footer */}
        <div className="border-t border-border/50 px-3 py-1.5">
          <p className="text-[10px] text-muted-foreground">
            这些记忆被自动检索并注入到本次响应的上下文中
          </p>
        </div>
      </div>
    </>
  )
}
