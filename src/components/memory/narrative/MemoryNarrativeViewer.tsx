/**
 * MemoryNarrativeViewer — modal that lists session summaries grouped
 * by date, surfaced from the MemorySettingsPage.
 *
 * Phase 8B.11 / T-UI-3 (S2 skeleton).  S3 will extend this to also
 * show extracted facts (compile_facts output) inside the same date
 * groups; the `FactCard` indirection keeps that future swap local.
 *
 * Per v2 §0.5 Δ-15, `MemoryBrowser.tsx` is preserved alongside this
 * new viewer — the two components address different mental models
 * (key-value browser vs. timeline) and both stay reachable from
 * settings.
 */

import { useEffect, useMemo, useState } from 'react'
import { useRuntimeProjectionSelector } from '@/runtime-projection'
import { toast } from 'sonner'
import { Loader2, RefreshCw, Search, X } from 'lucide-react'
import { DateGroupHeader } from './DateGroupHeader'
import { FactCard } from './FactCard'
import { memorySummariesList, type SessionSummaryDto } from '@/lib/tauri'

export interface MemoryNarrativeViewerProps {
  /** Whether the modal is currently visible. */
  open: boolean
  /** Caller-supplied close handler (backdrop / X / ESC). */
  onClose: () => void
  /**
   * Scope label forwarded to `memory_summaries_list`; defaults to
   * `'global'` to match the system-prompt assembler scope.
   */
  scope?: 'current' | 'all' | 'project' | 'global'
}

const SINCE_OPTIONS: ReadonlyArray<{ value: number; label: string }> = [
  { value: 7, label: '过去 7 天' },
  { value: 30, label: '过去 30 天' },
  { value: 90, label: '过去 90 天' },
  { value: 365, label: '过去 1 年' },
]

/**
 * Modal viewer that renders the per-session rolling-summary timeline.
 * Wired from `MemorySettingsPage`.
 */
export function MemoryNarrativeViewer({
  open,
  onClose,
  scope = 'global',
}: MemoryNarrativeViewerProps) {
  const [summaries, setSummaries] = useState<SessionSummaryDto[]>([])
  const [loading, setLoading] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [sinceDays, setSinceDays] = useState(30)

  // Memory Audit P1 #5 — refresh when the backend signals summaries
  // were modified (current writers: rolling summarizer ticker, manual
  // wipes via memory_clear_all).
  const invalidationVersion = useRuntimeProjectionSelector(
    (s) => s.memory.invalidationVersion,
  )
  const lastInvalidationScope = useRuntimeProjectionSelector(
    (s) => s.memory.lastInvalidationScope,
  )
  const summaryInvalidationKey =
    lastInvalidationScope === 'summaries' || lastInvalidationScope === 'all'
      ? invalidationVersion
      : 0

  useEffect(() => {
    if (!open) return
    let cancelled = false
    const load = async () => {
      setLoading(true)
      try {
        const data = await memorySummariesList(scope, 200, sinceDays)
        if (!cancelled) setSummaries(data)
      } catch (e) {
        if (!cancelled) toast.error('加载摘要失败', { description: String(e) })
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [open, scope, sinceDays, summaryInvalidationKey])

  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onClose])

  const handleRefresh = async () => {
    setLoading(true)
    try {
      const data = await memorySummariesList(scope, 200, sinceDays)
      setSummaries(data)
    } catch (e) {
      toast.error('加载摘要失败', { description: String(e) })
    } finally {
      setLoading(false)
    }
  }

  // Filter (in-memory) + group by `YYYY-MM-DD`, ordered newest-first.
  const grouped = useMemo<ReadonlyArray<readonly [string, SessionSummaryDto[]]>>(() => {
    const lc = searchQuery.trim().toLowerCase()
    const filtered = lc
      ? summaries.filter(
          (s) =>
            s.summary.toLowerCase().includes(lc) ||
            s.session_id.toLowerCase().includes(lc),
        )
      : summaries
    const groups: Record<string, SessionSummaryDto[]> = {}
    for (const s of filtered) {
      const date = s.updated_at.slice(0, 10)
      if (!groups[date]) groups[date] = []
      groups[date].push(s)
    }
    return Object.entries(groups).sort(([a], [b]) => b.localeCompare(a))
  }, [summaries, searchQuery])

  if (!open) return null

  const filteredCount = grouped.reduce((sum, [, items]) => sum + items.length, 0)

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label="Memory Narrative Viewer"
    >
      <div
        className="flex h-[80vh] w-full max-w-4xl flex-col rounded-2xl bg-background shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex items-center justify-between border-b border-black/[0.06] px-5 py-3 dark:border-white/[0.08]">
          <div>
            <h2 className="text-base font-semibold">📖 Memory Narrative</h2>
            <p className="text-[11px] text-muted-foreground">
              所有 session 滚动摘要的时间线 (scope: {scope})
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void handleRefresh()}
              disabled={loading}
              className="flex items-center gap-1.5 rounded-lg border border-black/[0.09] bg-black/[0.025] px-3 py-1.5 text-[12px] transition-colors hover:bg-black/[0.05] disabled:opacity-50 dark:border-white/[0.12] dark:bg-white/[0.04] dark:hover:bg-white/[0.08]"
            >
              {loading ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <RefreshCw className="h-3.5 w-3.5" />
              )}
              刷新
            </button>
            <button
              type="button"
              onClick={onClose}
              aria-label="关闭"
              className="rounded-lg p-1.5 text-muted-foreground transition-colors hover:bg-black/[0.04] dark:hover:bg-white/[0.06]"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        </header>

        <div className="flex items-center gap-3 border-b border-black/[0.04] px-5 py-2 dark:border-white/[0.06]">
          <div className="relative flex-1">
            <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
            <input
              type="text"
              placeholder="搜索摘要 / session_id…"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full rounded-md border border-black/[0.06] bg-background py-1.5 pl-8 pr-3 text-[12px] outline-none focus:border-blue-400 focus:ring-2 focus:ring-blue-100 dark:border-white/[0.1] dark:focus:ring-blue-950/40"
            />
          </div>
          <select
            value={sinceDays}
            onChange={(e) => setSinceDays(Number(e.target.value))}
            aria-label="时间范围"
            className="rounded-md border border-black/[0.06] bg-background px-2 py-1.5 text-[12px] dark:border-white/[0.1]"
          >
            {SINCE_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </div>

        <main className="flex-1 overflow-y-auto px-5 py-3">
          {loading && summaries.length === 0 ? (
            <div className="flex h-full items-center justify-center text-[12px] text-muted-foreground">
              <Loader2 className="mr-2 h-4 w-4 animate-spin" /> 加载中…
            </div>
          ) : grouped.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-center text-[12px] text-muted-foreground">
              <p>{searchQuery ? '没有匹配的摘要' : '暂无 session 摘要'}</p>
              {!searchQuery && (
                <p className="text-[10.5px] opacity-70">
                  先与 agent 聊几轮触发 RollingSummarizer，然后回来查看
                </p>
              )}
            </div>
          ) : (
            <div className="space-y-4">
              {grouped.map(([date, items]) => (
                <section key={date}>
                  <DateGroupHeader date={date} count={items.length} />
                  <div className="space-y-2">
                    {items.map((s) => (
                      <FactCard key={`${s.session_id}-${s.updated_at}`} summary={s} />
                    ))}
                  </div>
                </section>
              ))}
            </div>
          )}
        </main>

        <footer className="border-t border-black/[0.04] px-5 py-2 text-[10.5px] text-muted-foreground dark:border-white/[0.06]">
          共 {summaries.length} 条 session 摘要
          {searchQuery && ` (筛选后: ${filteredCount})`}
        </footer>
      </div>
    </div>
  )
}
