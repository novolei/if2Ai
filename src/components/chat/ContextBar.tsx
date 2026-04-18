/**
 * ContextBar — Token budget visualiser above the chat input.
 *
 * Displays a segmented progress bar showing how the model's context window
 * is currently allocated across:
 *   - System prompt
 *   - Message history (sliding window)
 *   - Memory context
 *   - Output reserve
 *   - Available / remaining tokens
 *
 * The bar is driven by `ContextBudgetUsage` from `StreamTokenPayload`
 * (added in FE-E Phase 1). When no budget data is available the component
 * renders nothing, ensuring zero visual impact for users on older backends
 * that do not yet emit the field.
 *
 * Also shows the current sliding-window message count when `windowSize` is
 * provided — wired from the WorkingMemory C1 implementation.
 */

import { cn } from '@/lib/utils'
import type { ContextBudgetUsage } from '@/lib/tauri'

// ── Types ─────────────────────────────────────────────────────────────────────

export interface ContextBarProps {
  /**
   * Token budget breakdown from the latest `stream_complete` event.
   * When `undefined` the component renders nothing.
   */
  usage?: ContextBudgetUsage
  /**
   * Number of messages currently in the sliding window (WorkingMemory C1).
   * When provided, shown as a compact label beside the bar.
   */
  windowSize?: number
  /** Additional CSS class names. */
  className?: string
}

// ── Segment config ────────────────────────────────────────────────────────────

interface Segment {
  key: keyof Omit<ContextBudgetUsage, 'total_budget' | 'remaining'>
  label: string
  colorClass: string
}

const SEGMENTS: Segment[] = [
  {
    key: 'system_tokens',
    label: '系统',
    colorClass: 'bg-jade-dim/70',
  },
  {
    key: 'memory_tokens',
    label: '记忆',
    colorClass: 'bg-teal/70',
  },
  {
    key: 'history_tokens',
    label: '历史',
    colorClass: 'bg-primary/50',
  },
  {
    key: 'output_reserve',
    label: '输出',
    colorClass: 'bg-celadon/70',
  },
]

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatTokenCount(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return String(n)
}

function pct(value: number, total: number): number {
  if (total <= 0) return 0
  return Math.min(100, Math.max(0, (value / total) * 100))
}

// ── ContextBar ────────────────────────────────────────────────────────────────

/**
 * Compact token-budget bar rendered above the chat input when
 * `ContextBudgetUsage` data is present in the latest stream payload.
 */
export function ContextBar({ usage, windowSize, className }: ContextBarProps) {
  if (!usage) return null

  const { total_budget, remaining } = usage
  const usedPct = pct(total_budget - remaining, total_budget)
  const remainingPct = pct(remaining, total_budget)

  return (
    <div
      className={cn('flex flex-col gap-0.5 px-3 py-1', className)}
      role="status"
      aria-label={`上下文使用量：${formatTokenCount(total_budget - remaining)} / ${formatTokenCount(total_budget)} tokens`}
    >
      {/* Segmented progress bar */}
      <div
        className="flex h-1.5 w-full overflow-hidden rounded-full bg-muted"
        aria-hidden
      >
        {SEGMENTS.map((seg) => {
          const segPct = pct(usage[seg.key], total_budget)
          if (segPct <= 0) return null
          return (
            <div
              key={seg.key}
              className={cn('h-full transition-all duration-500', seg.colorClass)}
              style={{ width: `${segPct}%` }}
            />
          )
        })}
        {/* Remaining / available — shown as transparent (bar background shows through) */}
        <div
          className="h-full flex-1 bg-transparent"
          style={{ width: `${remainingPct}%` }}
        />
      </div>

      {/* Legend row */}
      <div className="flex items-center gap-3 text-[10px] text-muted-foreground">
        {SEGMENTS.map((seg) => {
          const val = usage[seg.key]
          if (val <= 0) return null
          return (
            <span key={seg.key} className="flex items-center gap-1">
              <span
                className={cn('inline-block h-1.5 w-1.5 rounded-full', seg.colorClass)}
                aria-hidden
              />
              {seg.label} {formatTokenCount(val)}
            </span>
          )
        })}

        {/* Remaining */}
        <span className="ml-auto tabular-nums">
          剩余 {formatTokenCount(remaining)} / {formatTokenCount(total_budget)}
          {windowSize !== undefined && (
            <span className="ml-2 text-muted-foreground/60">
              · 窗口 {windowSize} 条
            </span>
          )}
        </span>

        {/* Usage percentage */}
        <span
          className={cn(
            'tabular-nums font-medium',
            usedPct >= 90
              ? 'text-status-error'
              : usedPct >= 75
                ? 'text-status-warning'
                : 'text-muted-foreground',
          )}
        >
          {Math.round(usedPct)}%
        </span>
      </div>
    </div>
  )
}
