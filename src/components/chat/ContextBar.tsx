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

import { useState } from 'react'
import { Brain, Coins, Layers } from 'lucide-react'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import {
  chatCompactSession,
  type ContextBudgetUsage,
  type SessionTotals,
} from '@/lib/tauri'
import { CompiledMemoryViewer } from '@/components/memory/compiled/CompiledMemoryViewer'
import { useContextBarMode } from './useContextBarMode'

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
  /**
   * P2-11 — running per-session totals (provider-billable). When present
   * a second compact line is rendered below the budget bar showing the
   * accumulated input/output tokens, USD cost and turn count.
   */
  sessionTotals?: SessionTotals
  /** Additional CSS class names. */
  className?: string
  /**
   * Active session id, required for the manual compact button.
   * When absent the compact button is hidden.
   */
  sessionId?: string
}

function formatCostUsd(cost: number): string {
  if (cost <= 0) return '$0.00'
  if (cost < 0.01) return `$${cost.toFixed(4)}`
  if (cost < 1) return `$${cost.toFixed(3)}`
  return `$${cost.toFixed(2)}`
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
export function ContextBar({
  usage,
  windowSize,
  sessionTotals,
  className,
  sessionId,
}: ContextBarProps) {
  // Phase 8B.10 / T-UI-2 — wire the memory badge to the CompiledMemoryViewer modal.
  const [memoryViewerOpen, setMemoryViewerOpen] = useState(false)
  const [compactBusy, setCompactBusy] = useState(false)
  const [mode] = useContextBarMode()
  const sessionOnly = mode === 'session-only'

  const onCompact = async () => {
    if (!sessionId || compactBusy) return
    setCompactBusy(true)
    try {
      const report = await chatCompactSession(sessionId)
      if (report.didCompact) {
        const freed =
          report.freedTokens > 0
            ? `${(report.freedTokens / 1000).toFixed(1)}k`
            : '0'
        toast.success(`已压缩 ${report.summarizedMessages} 条消息，释放约 ${freed} tokens`)
      } else {
        toast('上下文已是最新，无需压缩')
      }
    } catch (err) {
      toast.error(`压缩失败：${(err as Error).message ?? err}`)
    } finally {
      setCompactBusy(false)
    }
  }

  // Render the budget block only when usage data is available, but still
  // render the per-session totals row below when only that one is present
  // (e.g. fresh session with prior persisted turns but no new stream yet).
  if (!usage && !sessionTotals) return null

  const total_budget = usage?.total_budget ?? 0
  const remaining = usage?.remaining ?? 0
  const usedPct = pct(total_budget - remaining, total_budget)
  const remainingPct = pct(remaining, total_budget)
  // P2-11 / UX — in `session-only` mode collapse the cross-session baseline
  // (system + memory + output_reserve) into a single muted segment so the
  // session-local `history_tokens` segment dominates and per-session
  // differences are visible at a glance.
  const baselineTokens = usage
    ? usage.system_tokens + usage.memory_tokens + usage.output_reserve
    : 0
  const visualSegments = !usage
    ? []
    : sessionOnly
      ? [
          {
            key: 'baseline' as const,
            label: '基线',
            colorClass: 'bg-muted-foreground/20',
            value: baselineTokens,
            badgeColor: 'bg-muted-foreground/30',
          },
          {
            key: 'history_tokens' as const,
            label: '会话历史',
            colorClass: 'bg-primary/70',
            value: usage.history_tokens,
            badgeColor: 'bg-primary/70',
          },
        ]
      : SEGMENTS.map((seg) => ({
          key: seg.key,
          label: seg.label,
          colorClass: seg.colorClass,
          value: usage[seg.key],
          badgeColor: seg.colorClass,
        }))

  return (
    <>
    <div
      className={cn('flex flex-col gap-0.5 px-3 py-1', className)}
      role="status"
      aria-label={`上下文使用量：${formatTokenCount(total_budget - remaining)} / ${formatTokenCount(total_budget)} tokens`}
    >
      {usage ? (
      <>
      {/* Segmented progress bar */}
      <div
        className="flex h-1.5 w-full overflow-hidden rounded-full bg-muted"
        aria-hidden
      >
        {visualSegments.map((seg) => {
          const segPct = pct(seg.value, total_budget)
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
        {visualSegments.map((seg) => {
          if (seg.value <= 0) return null
          return (
            <span key={seg.key} className="flex items-center gap-1">
              <span
                className={cn('inline-block h-1.5 w-1.5 rounded-full', seg.badgeColor)}
                aria-hidden
              />
              {seg.label} {formatTokenCount(seg.value)}
            </span>
          )
        })}

        {/* Memory badge — Phase 8A.12 (skeleton) wired in 8B.10 / T-UI-2.
            Click opens the CompiledMemoryViewer modal so users can inspect
            the exact memory.md being injected into the system prompt. */}
        {usage!.memory_tokens > 0 && (
          <button
            type="button"
            onClick={() => setMemoryViewerOpen(true)}
            className="flex items-center gap-1 rounded bg-teal/15 px-1.5 py-0.5 text-teal-700/80 transition-colors hover:bg-teal/25"
            title="点击查看 system prompt 注入的全部长期记忆"
          >
            <Brain className="h-3 w-3" aria-hidden />
            记忆已加载
          </button>
        )}

        {/* P2-11 — running per-session totals, sandwiched between the
            segment legend and the remaining/window stats so it doesn't
            occupy a separate row that gets clipped by the composer. */}
        {sessionTotals && sessionTotals.turns > 0 ? (
          <span
            className="mx-auto flex items-center gap-1.5 tabular-nums text-muted-foreground/80"
            title="本会话累计：基于 provider usage 与当前模型单价"
          >
            <Coins className="h-3 w-3" aria-hidden />
            <span>本会话累计</span>
            <span>{formatTokenCount(sessionTotals.input_tokens)} 输入</span>
            <span className="text-muted-foreground/50">·</span>
            <span>{formatTokenCount(sessionTotals.output_tokens)} 输出</span>
            <span className="text-muted-foreground/50">·</span>
            <span>{formatCostUsd(sessionTotals.cost_usd)}</span>
            <span className="text-muted-foreground/50">·</span>
            <span>{sessionTotals.turns} 回合</span>
          </span>
        ) : null}

        {/* Remaining */}
        <span className={cn('tabular-nums', sessionTotals && sessionTotals.turns > 0 ? '' : 'ml-auto')}>
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

        {/* Compact button — surfaces from 85% so users can fold the
            history before the auto-compact background tick fires.
            Hidden without a session id. */}
        {sessionId && usedPct >= 85 && (
          <button
            type="button"
            onClick={() => void onCompact()}
            disabled={compactBusy}
            className={cn(
              'inline-flex items-center gap-1 rounded bg-amber-500/20 px-1.5 py-0.5 text-amber-700 transition-colors hover:bg-amber-500/30',
              'disabled:cursor-not-allowed disabled:opacity-60',
            )}
            title="手动压缩历史：把已发送消息折叠为摘要，释放上下文空间"
          >
            <Layers className="h-3 w-3" aria-hidden />
            {compactBusy ? '压缩中…' : '压缩'}
          </button>
        )}
      </div>
      </>
      ) : null}
    </div>
    <CompiledMemoryViewer
      open={memoryViewerOpen}
      onClose={() => setMemoryViewerOpen(false)}
      scope="global"
    />
    </>
  )
}
