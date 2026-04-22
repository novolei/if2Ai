/**
 * MemoryAggregateOverview — single "AI 记得我什么" hero panel.
 *
 * Memory System Audit P1 #8: surfaces all four memory sources
 * (Pinned / Compiled / Facts / Rolling Summaries) in one place so a
 * user can answer "what does the AI remember about me?" without
 * diving into 4 separate sub-pages.
 *
 * Each card is a quick-glance summary with a deep link to the full
 * detail viewer that already exists. The component owns no
 * destructive operations — it's purely an overview surface.
 *
 * Auto-refreshes via the audit's `memory_invalidated` event channel
 * (P1 #5) so a write anywhere updates this panel within ~50ms.
 */

import { useEffect, useState } from 'react'
import {
  ArrowRight,
  BookOpen,
  History,
  Pin,
  Sparkles,
  Zap,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useMemoryEntries, useMemoryInvalidationKey } from '@/api/memory'
import {
  memoryCompiledRead,
  memorySummariesList,
  pinnedGet,
  type PinnedItemDto,
  type SessionSummaryDto,
} from '@/lib/tauri'

interface CompiledQuickStats {
  factsChars: number
  longtermChars: number
  todayChars: number
  weekChars: number
  lastCompiledAt: Date | null
}

interface MemoryAggregateOverviewProps {
  /** Open the Pinned editor section. */
  onShowPinned?: () => void
  /** Open the CompiledMemoryViewer modal. */
  onShowCompiled?: () => void
  /** Open the MemoryNarrativeViewer modal. */
  onShowSummaries?: () => void
  /** Optional CTA: "browse all entries" — wires to MemoryBrowser route. */
  onShowBrowser?: () => void
}

export function MemoryAggregateOverview({
  onShowPinned,
  onShowCompiled,
  onShowSummaries,
  onShowBrowser,
}: MemoryAggregateOverviewProps) {
  // Entries (covers the "raw memory library" — what would show up in
  // the Browser). The hook auto-refreshes on invalidation.
  const { entries, loading: entriesLoading } = useMemoryEntries()
  const totalEntries = entries.length
  const coreEntries = entries.filter((e) => e.category === 'core').length
  const dailyEntries = entries.filter((e) => e.category === 'daily').length
  // MEM-MOD-P2 — surface the three new built-in facets so the overview
  // reflects the real category mix, not just the original two.
  const reflectionEntries = entries.filter(
    (e) => e.category === 'reflection',
  ).length
  const workingEntries = entries.filter((e) => e.category === 'working').length
  const proceduralEntries = entries.filter(
    (e) => e.category === 'procedural',
  ).length

  // Pinned, Compiled, and Recent Summaries — load on mount + on
  // invalidation. We share one invalidation key for these three since
  // any "all" or relevant scope event should refresh all of them.
  const pinnedInvKey = useMemoryInvalidationKey('pinned')
  const compiledInvKey = useMemoryInvalidationKey('compiled')
  const summariesInvKey = useMemoryInvalidationKey('summaries')

  const [pinned, setPinned] = useState<PinnedItemDto[]>([])
  const [compiledStats, setCompiledStats] = useState<CompiledQuickStats | null>(
    null,
  )
  const [recentSummaries, setRecentSummaries] = useState<SessionSummaryDto[]>(
    [],
  )

  useEffect(() => {
    let cancelled = false
    pinnedGet('both')
      .then((items) => {
        if (!cancelled) setPinned(items)
      })
      .catch(() => {
        // Best-effort overview; an empty pinned column on error is
        // acceptable since the user can still go to the editor.
        if (!cancelled) setPinned([])
      })
    return () => {
      cancelled = true
    }
  }, [pinnedInvKey])

  useEffect(() => {
    let cancelled = false
    memoryCompiledRead('global')
      .then((dto) => {
        if (cancelled) return
        const stats: CompiledQuickStats = {
          factsChars: dto.facts.chars,
          longtermChars: dto.longterm.chars,
          todayChars: dto.today.chars,
          weekChars: dto.week.chars,
          lastCompiledAt: pickMostRecent([
            dto.facts.last_compiled_at,
            dto.longterm.last_compiled_at,
            dto.today.last_compiled_at,
            dto.week.last_compiled_at,
          ]),
        }
        setCompiledStats(stats)
      })
      .catch(() => {
        if (!cancelled) setCompiledStats(null)
      })
    return () => {
      cancelled = true
    }
  }, [compiledInvKey])

  useEffect(() => {
    let cancelled = false
    memorySummariesList('global', 5, 30)
      .then((data) => {
        if (!cancelled) setRecentSummaries(data)
      })
      .catch(() => {
        if (!cancelled) setRecentSummaries([])
      })
    return () => {
      cancelled = true
    }
  }, [summariesInvKey])

  const compiledTotalChars = compiledStats
    ? compiledStats.factsChars +
      compiledStats.longtermChars +
      compiledStats.todayChars +
      compiledStats.weekChars
    : 0

  return (
    <div className="rounded-2xl border border-jade/15 bg-gradient-to-br from-jade/[0.05] via-white to-white px-5 py-5 dark:via-background dark:to-background">
      <div className="mb-4 flex items-start justify-between">
        <div>
          <div className="flex items-center gap-2">
            <Sparkles className="h-3.5 w-3.5 text-jade" />
            <span className="text-[10.5px] font-semibold uppercase tracking-widest text-black/35">
              AI 记得我什么
            </span>
          </div>
          <h2 className="mt-2 text-[16px] font-semibold tracking-tight text-foreground/90">
            一眼看完 AI 关于你的全部记忆
          </h2>
          <p className="mt-1 max-w-2xl text-[12px] leading-5 text-muted-foreground">
            四类源（置顶 / 长期编译 / 关键事实 / 摘要时间线）的快速快照。
            点击任意一张卡片打开对应详情视图。
          </p>
        </div>
        {onShowBrowser ? (
          <button
            type="button"
            onClick={onShowBrowser}
            className="hidden shrink-0 items-center gap-1 rounded-lg border border-jade/30 bg-jade/10 px-3 py-1.5 text-[11.5px] font-semibold text-jade transition-colors hover:bg-jade/20 sm:inline-flex"
          >
            浏览全部记忆
            <ArrowRight className="h-3 w-3" />
          </button>
        ) : null}
      </div>

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {/* Card 1: Pinned */}
        <OverviewCard
          icon={<Pin className="h-3.5 w-3.5" />}
          accent="amber"
          label="置顶"
          subtitle="Pinned"
          value={String(pinned.length)}
          unit="条"
          hint={
            pinned.length === 0
              ? '尚未置顶任何内容'
              : pinned.length === 1
                ? '1 条永远在 prompt 顶部'
                : `${pinned.length} 条永远在 prompt 顶部`
          }
          onClick={onShowPinned}
        />

        {/* Card 2: Compiled long-term */}
        <OverviewCard
          icon={<BookOpen className="h-3.5 w-3.5" />}
          accent="blue"
          label="长期编译"
          subtitle="Compiled memory.md"
          value={
            compiledStats
              ? formatChars(compiledTotalChars)
              : '—'
          }
          unit="字"
          hint={
            compiledStats?.lastCompiledAt
              ? `最近编译：${formatRelative(compiledStats.lastCompiledAt)}`
              : '尚未编译过'
          }
          onClick={onShowCompiled}
        />

        {/* Card 3: Facts */}
        <OverviewCard
          icon={<Zap className="h-3.5 w-3.5" />}
          accent="violet"
          label="关键事实"
          subtitle="Facts"
          value={
            compiledStats
              ? formatChars(compiledStats.factsChars)
              : '—'
          }
          unit="字"
          hint={
            compiledStats && compiledStats.factsChars > 0
              ? '从对话中提取的稳定事实'
              : '点击进入详情查看 / 触发提取'
          }
          onClick={onShowCompiled}
        />

        {/* Card 4: Recent Summaries */}
        <OverviewCard
          icon={<History className="h-3.5 w-3.5" />}
          accent="emerald"
          label="摘要时间线"
          subtitle="Rolling summaries"
          value={String(recentSummaries.length)}
          unit="近期"
          hint={
            recentSummaries.length === 0
              ? '近 30 天暂无摘要'
              : `近 30 天 ${recentSummaries.length} 条 session 摘要`
          }
          onClick={onShowSummaries}
        />
      </div>

      {/* Footer hint — entries library counts */}
      <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-black/[0.06] pt-3 text-[11px] text-muted-foreground">
        <span>
          记忆库共 <strong className="font-semibold text-foreground/75">{entriesLoading ? '…' : totalEntries}</strong> 条
        </span>
        <span aria-hidden>·</span>
        <span>
          Core <strong className="font-semibold text-foreground/75">{coreEntries}</strong>
        </span>
        <span aria-hidden>·</span>
        <span>
          Daily <strong className="font-semibold text-foreground/75">{dailyEntries}</strong>
        </span>
        <span aria-hidden>·</span>
        <span>
          Working <strong className="font-semibold text-foreground/75">{workingEntries}</strong>
        </span>
        <span aria-hidden>·</span>
        <span>
          Procedural <strong className="font-semibold text-foreground/75">{proceduralEntries}</strong>
        </span>
        <span aria-hidden>·</span>
        <span>
          Reflection <strong className="font-semibold text-foreground/75">{reflectionEntries}</strong>
        </span>
        {onShowBrowser ? (
          <button
            type="button"
            onClick={onShowBrowser}
            className="ml-auto inline-flex items-center gap-1 text-[11px] font-medium text-jade hover:text-jade/80 sm:hidden"
          >
            浏览全部
            <ArrowRight className="h-3 w-3" />
          </button>
        ) : null}
      </div>
    </div>
  )
}

interface OverviewCardProps {
  icon: React.ReactNode
  accent: 'amber' | 'blue' | 'violet' | 'emerald'
  label: string
  subtitle: string
  value: string
  unit?: string
  hint: string
  onClick?: () => void
}

const ACCENT_BG: Record<OverviewCardProps['accent'], string> = {
  amber: 'bg-amber-500/[0.08] text-amber-600',
  blue: 'bg-blue-500/[0.08] text-blue-600',
  violet: 'bg-violet-500/[0.08] text-violet-600',
  emerald: 'bg-emerald-500/[0.08] text-emerald-600',
}

function OverviewCard({
  icon,
  accent,
  label,
  subtitle,
  value,
  unit,
  hint,
  onClick,
}: OverviewCardProps) {
  const interactive = Boolean(onClick)
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!interactive}
      className={cn(
        'group rounded-xl border border-black/[0.06] bg-white px-3.5 py-3 text-left transition-all dark:bg-background',
        interactive
          ? 'cursor-pointer hover:-translate-y-0.5 hover:border-black/[0.12] hover:shadow-[0_4px_14px_rgba(15,23,42,0.06)]'
          : 'cursor-default opacity-95',
      )}
    >
      <div className="flex items-center justify-between">
        <div
          className={cn(
            'inline-flex h-7 w-7 items-center justify-center rounded-lg',
            ACCENT_BG[accent],
          )}
        >
          {icon}
        </div>
        {interactive ? (
          <ArrowRight className="h-3 w-3 text-black/30 transition-colors group-hover:text-foreground/65" />
        ) : null}
      </div>
      <div className="mt-2 flex items-baseline gap-1">
        <span className="font-mono text-[20px] font-bold tabular-nums text-foreground/90">
          {value}
        </span>
        {unit ? (
          <span className="text-[10.5px] font-medium text-black/40">{unit}</span>
        ) : null}
      </div>
      <div className="mt-0.5 text-[12px] font-semibold text-foreground/85">
        {label}
        <span className="ml-1.5 font-normal text-black/35">{subtitle}</span>
      </div>
      <div className="mt-1 line-clamp-2 text-[10.5px] leading-4 text-muted-foreground">
        {hint}
      </div>
    </button>
  )
}

// ─── helpers ─────────────────────────────────────────────────────────

function formatChars(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}

function pickMostRecent(timestamps: Array<string | null | undefined>): Date | null {
  let latest: Date | null = null
  for (const ts of timestamps) {
    if (!ts) continue
    const d = new Date(ts)
    if (Number.isNaN(d.getTime())) continue
    if (!latest || d > latest) latest = d
  }
  return latest
}

function formatRelative(d: Date): string {
  const diffMs = Date.now() - d.getTime()
  const min = Math.floor(diffMs / 60_000)
  if (min < 1) return '刚刚'
  if (min < 60) return `${min} 分钟前`
  const hr = Math.floor(min / 60)
  if (hr < 24) return `${hr} 小时前`
  const day = Math.floor(hr / 24)
  if (day < 30) return `${day} 天前`
  return d.toLocaleDateString('zh-CN')
}
