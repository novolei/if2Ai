/**
 * MemoryEvidencePanel — Popover showing recalled memory items for a response.
 *
 * Displays a scrollable list of `MemoryContextItem` entries — the episodic /
 * semantic memory fragments that the backend retrieved and injected into the
 * system prompt for the current assistant turn.
 *
 * Memory System Audit P2 #9: enhanced with derived rank + reason so the
 * user can understand *why* each item came up (not just *that* it did).
 * The reason is heuristically derived from (scope + relevance_score)
 * since the backend currently doesn't surface match keywords or
 * structured rationale; if it ever does, this component reads
 * `MemoryContextItem.reason` directly when present.
 *
 * Designed to be opened by `MemoryChip` via an absolute-positioned overlay.
 * Closes when the user clicks outside (via a backdrop `<div>`) or presses
 * the close button.
 */

import { Globe, Layers, MessageSquare, X } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { MemoryContextItem } from '@/lib/tauri'

// ── Helpers ───────────────────────────────────────────────────────────────────

const SCOPE_LABEL: Record<MemoryContextItem['scope'], string> = {
  global: '全局记忆库',
  project: '当前项目',
  session: '当前会话',
}

const SCOPE_SHORT_LABEL: Record<MemoryContextItem['scope'], string> = {
  global: '全局',
  project: '项目',
  session: '会话',
}

const SCOPE_COLOR: Record<MemoryContextItem['scope'], string> = {
  global: 'bg-jade/12 text-jade',
  project: 'bg-blue-500/[0.10] text-blue-700 dark:text-blue-400',
  session: 'bg-amber-500/[0.10] text-amber-700 dark:text-amber-400',
}

const SCOPE_ICON: Record<
  MemoryContextItem['scope'],
  React.ComponentType<{ className?: string }>
> = {
  global: Globe,
  project: Layers,
  session: MessageSquare,
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

/**
 * Derive a short Chinese rationale from (scope + relevance_score) so
 * the panel can explain "why this came up" without depending on a
 * future backend reason field. When the backend eventually surfaces
 * structured rationale on `MemoryContextItem.reason`, this helper
 * becomes a fallback.
 *
 * Tiers (relevance_score is 0..1, sometimes undefined for non-vector
 * recalls):
 *   - >= 0.80  → "高度相关"  · 直接命中关键词或语义同源
 *   - >= 0.55  → "主题相关"  · 同主题 / 临近上下文
 *   - >= 0.30  → "上下文相关" · 同范围内候选
 *   - <  0.30  → "弱相关"     · 排序末段，仅作补充
 *   - undefined → "范围匹配" · 该 scope 内的相关项（无打分依据）
 */
function deriveReason(item: MemoryContextItem): string {
  const score = item.relevance_score
  const scopeWord = SCOPE_LABEL[item.scope]
  if (score === undefined) {
    return `属于「${scopeWord}」范围内可能相关的条目`
  }
  if (score >= 0.8) {
    return `与本轮提问高度相关 · 来自「${scopeWord}」`
  }
  if (score >= 0.55) {
    return `主题相关 · 来自「${scopeWord}」`
  }
  if (score >= 0.3) {
    return `上下文相关 · 来自「${scopeWord}」`
  }
  return `弱相关 · 仅作补充 · 来自「${scopeWord}」`
}

function relevanceTier(
  score: number | undefined,
): 'high' | 'mid' | 'low' | 'unknown' {
  if (score === undefined) return 'unknown'
  if (score >= 0.8) return 'high'
  if (score >= 0.55) return 'mid'
  return 'low'
}

const RELEVANCE_BAR_COLOR: Record<
  ReturnType<typeof relevanceTier>,
  string
> = {
  high: 'bg-jade',
  mid: 'bg-blue-500',
  low: 'bg-amber-500',
  unknown: 'bg-muted-foreground/40',
}

function RelevanceBar({ score }: { score: number | undefined }) {
  const tier = relevanceTier(score)
  if (score === undefined) {
    return (
      <div className="mt-1.5 flex items-center gap-1.5">
        <div className="h-1 flex-1 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full bg-muted-foreground/40 transition-all"
            style={{ width: '24%' }}
            aria-hidden
          />
        </div>
        <span className="text-[10px] font-medium tabular-nums text-muted-foreground">
          —
        </span>
      </div>
    )
  }
  const pct = Math.round(Math.max(0, Math.min(1, score)) * 100)
  return (
    <div className="mt-1.5 flex items-center gap-1.5">
      <div className="h-1 flex-1 overflow-hidden rounded-full bg-muted">
        <div
          className={cn('h-full rounded-full transition-all', RELEVANCE_BAR_COLOR[tier])}
          style={{ width: `${pct}%` }}
          aria-label={`相关度 ${pct}%`}
        />
      </div>
      <span className="text-[10px] font-medium tabular-nums text-foreground/70">
        {pct}%
      </span>
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
  const total = items.length
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
          'absolute bottom-full left-0 z-50 mb-2 w-80 rounded-xl',
          'border border-border bg-popover shadow-token-lg',
          className,
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-3.5 py-2.5">
          <div className="flex items-center gap-1.5">
            <span className="text-[12px] font-semibold text-foreground/85">
              本次引用的记忆
            </span>
            <span className="rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] tabular-nums text-foreground/65">
              {total}
            </span>
          </div>
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
          className="max-h-72 divide-y divide-border/50 overflow-y-auto"
          aria-label={`${total} 条引用记忆`}
        >
          {items.map((item, index) => {
            const ScopeIcon = SCOPE_ICON[item.scope]
            const reason = deriveReason(item)
            return (
              <li key={item.id} className="px-3.5 py-3">
                {/* Top row: rank badge + scope chip + time */}
                <div className="mb-1.5 flex items-center justify-between gap-2">
                  <div className="flex items-center gap-1.5">
                    {/* Rank badge — communicates "how prominently this was
                        in the recall set" without needing the user to
                        eyeball the order. */}
                    <span className="inline-flex h-4 min-w-4 items-center justify-center rounded-md bg-foreground/10 px-1 font-mono text-[10px] font-semibold tabular-nums text-foreground/70">
                      #{index + 1}
                    </span>
                    <span
                      className={cn(
                        'inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[10px] font-medium',
                        SCOPE_COLOR[item.scope],
                      )}
                      title={SCOPE_LABEL[item.scope]}
                    >
                      <ScopeIcon className="h-2.5 w-2.5" />
                      {SCOPE_SHORT_LABEL[item.scope]}
                    </span>
                  </div>
                  {item.stored_at && (
                    <span className="text-[10px] text-muted-foreground">
                      {formatStoredAt(item.stored_at)}
                    </span>
                  )}
                </div>

                {/* Content preview */}
                <p className="line-clamp-3 text-[12px] leading-relaxed text-foreground/85">
                  {item.content}
                </p>

                {/* Relevance bar */}
                <RelevanceBar score={item.relevance_score} />

                {/* Derived reason — explains *why* this item came up.
                    Memory Audit P2 #9: closes the audit's "用户视角
                    可解释性不足" finding without needing a new IPC. */}
                <p className="mt-1.5 text-[10.5px] leading-4 text-muted-foreground">
                  {reason}
                </p>
              </li>
            )
          })}
        </ul>

        {/* Footer */}
        <div className="border-t border-border/50 px-3.5 py-1.5">
          <p className="text-[10px] text-muted-foreground">
            这些记忆被自动检索并注入到本次响应的上下文中 · 相关度由后端 RRF 融合排序
          </p>
        </div>
      </div>
    </>
  )
}
