/**
 * MemoryBrowser — 记忆浏览器主组件
 *
 * 提供记忆列表（分页）、搜索框、删除按钮和分类标签导航。
 * 通过 Tauri invoke 调用 memory_recall、memory_delete、memory_export。
 *
 * Features:
 * - 搜索框：调用 memory_recall 检索记忆
 * - 分类筛选：Core / Daily / Conversation / 全部
 * - 删除按钮：调用 memory_delete 删除单条记忆
 * - 显示 importance 进度条、trust_score 颜色渐变、access_count
 */

import { useState, useEffect } from 'react'
import type { MouseEvent as ReactMouseEvent } from 'react'
import { Search, TrendingUp } from 'lucide-react'
import { useRuntimeProjectionSelector } from '@/runtime-projection'
import { MemoryCard, type MemoryEntryDto } from './MemoryCard'
import { MemoryCategoryNav } from './MemoryCategoryNav'
import { Button } from '@/components/ui/button'
import {
  memoryDelete,
  memoryDemote,
  memoryExport,
  memoryPromote,
  memoryPromotionCandidates,
  type MemoryPromotionCandidateDto,
  type MemoryScopeArgs,
  type MemoryScopeKind,
} from '@/api/memory'

const PAGE_SIZE = 20

/**
 * Visible scope choices in the Memory Browser segmented control.
 *
 * `'all'` keeps the legacy unscoped behaviour (every entry in the shared
 * library, regardless of session/project tagging) so power users / admins
 * still have a full-library view.  The other three options drive the
 * three-tier `MemoryScopeKind` exposed by the backend.
 */
type ScopeFilter = 'all' | MemoryScopeKind

interface MemoryBrowserProps {
  /** Forward the App-level startWindowDrag handler so the header is draggable. */
  onStartWindowDrag?: (e: ReactMouseEvent<HTMLElement>) => void
  /** Active project id (for `project` and `session` scopes); null when no project is selected. */
  activeProjectId?: string | null
  /** Active session id (for `session` scope); null when no chat is open. */
  activeSessionId?: string | null
}

const SCOPE_OPTIONS: ReadonlyArray<{ id: ScopeFilter; label: string; help: string }> = [
  { id: 'all', label: '全部', help: '不应用任何 scope 过滤（管理视图）' },
  { id: 'global', label: '全局', help: '所有项目共享的全局记忆' },
  { id: 'project', label: '项目', help: '当前项目的记忆 + 全局记忆' },
  { id: 'session', label: '会话', help: '当前会话 + 当前项目 + 全局记忆' },
]

function normalizeMemoryKeyForDuplicateHint(key: string): string {
  return key
    .toLowerCase()
    .replace(/^episode:session:/, '')
    .replace(/[^a-z0-9\u4e00-\u9fa5]+/g, '')
}

export function MemoryBrowser({
  onStartWindowDrag,
  activeProjectId,
  activeSessionId,
}: MemoryBrowserProps) {
  const [entries, setEntries] = useState<MemoryEntryDto[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [activeCategory, setActiveCategory] = useState('all')
  const [totalCount, setTotalCount] = useState<number | null>(null)
  const [searchTotalCount, setSearchTotalCount] = useState<number | null>(null)
  const [scopeFilter, setScopeFilter] = useState<ScopeFilter>('all')
  const [promotionOpen, setPromotionOpen] = useState(false)
  const [candidates, setCandidates] = useState<MemoryPromotionCandidateDto[]>([])
  const [promotionLoading, setPromotionLoading] = useState(false)
  // Memory Audit P2 #11 — pagination. Today this is client-side
  // (slice the full export); when the library grows past a few
  // thousand entries we should swap the IPC for a paged
  // memory_export_paged command with SQL LIMIT/OFFSET. The UX
  // surface stays the same.
  const [pageIndex, setPageIndex] = useState(0)

  // Some scopes need extra ids; if the caller hasn't supplied them we fall
  // back to "all" rather than silently returning an empty list.
  const effectiveScope: MemoryScopeArgs | undefined = (() => {
    switch (scopeFilter) {
      case 'all':
        return undefined
      case 'global':
        return { scopeKind: 'global' }
      case 'project':
        return activeProjectId
          ? { scopeKind: 'project', projectId: activeProjectId }
          : undefined
      case 'session':
        return activeSessionId
          ? {
              scopeKind: 'session',
              sessionId: activeSessionId,
              projectId: activeProjectId ?? undefined,
            }
          : undefined
    }
  })()

  // Some scopes are meaningful only when the caller has the right context;
  // surface that to the user instead of silently falling back to "all".
  const scopeMissingContext =
    (scopeFilter === 'project' && !activeProjectId) ||
    (scopeFilter === 'session' && !activeSessionId)

  // Memory Audit P1 #5 — listen for any backend `memory_invalidated`
  // event so deletes / promotes / clears performed in *another* surface
  // (e.g. the agent's memory_purge tool) propagate into this list
  // without a manual refresh.
  const invalidationVersion = useRuntimeProjectionSelector(
    (s) => s.memory.invalidationVersion,
  )

  // Reload whenever the filter dimensions that affect server response
  // change, or when the backend signals invalidation.
  useEffect(() => {
    loadEntries()
    setPageIndex(0)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    activeCategory,
    scopeFilter,
    activeProjectId,
    activeSessionId,
    invalidationVersion,
  ])

  const loadEntries = async () => {
    setLoading(true)
    setError(null)
    try {
      const category = activeCategory === 'all' ? null : activeCategory
      const results = await memoryExport({
        category,
        scope: effectiveScope,
      })
      setEntries(results)
      setTotalCount(results.length)
      setSearchTotalCount(null)
    } catch (e) {
      setError(`加载记忆失败: ${e}`)
    } finally {
      setLoading(false)
    }
  }

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      loadEntries()
      return
    }

    setLoading(true)
    setError(null)
    try {
      const category = activeCategory === 'all' ? null : activeCategory
      const allEntries = await memoryExport({
        category,
        scope: effectiveScope,
      })
      const needle = searchQuery.trim().toLowerCase()
      const results = allEntries.filter((entry) =>
        [entry.key, entry.content, entry.category]
          .some((value) => value.toLowerCase().includes(needle)),
      )
      setEntries(results)
      setTotalCount(allEntries.length)
      setSearchTotalCount(results.length)
      setPageIndex(0)
    } catch (e) {
      setError(`搜索失败: ${e}`)
    } finally {
      setLoading(false)
    }
  }

  const handleDelete = async (key: string) => {
    try {
      await memoryDelete(key)
      setEntries((prev) => prev.filter((e) => e.key !== key))
    } catch (e) {
      setError(`删除失败: ${e}`)
    }
  }

  const handleCategoryChange = (category: string) => {
    setActiveCategory(category)
    setSearchQuery('')
  }

  const loadCandidates = async () => {
    setPromotionLoading(true)
    setError(null)
    try {
      const list = await memoryPromotionCandidates()
      setCandidates(list)
    } catch (e) {
      setError(`加载候选失败: ${e}`)
    } finally {
      setPromotionLoading(false)
    }
  }

  const togglePromotionPanel = () => {
    const next = !promotionOpen
    setPromotionOpen(next)
    if (next) loadCandidates()
  }

  /**
   * Demote one entry one tier down (`global → project` or `project → session`),
   * landing in whatever active project/session the browser currently has.
   *
   * The button is hidden / disabled at the card level when context is
   * missing, so reaching this handler without a usable target is a logic
   * bug — we still surface an error toast instead of throwing so the user
   * never gets stuck.
   */
  const handleDemote = async (entry: MemoryEntryDto) => {
    try {
      if (entry.session_id) {
        // Already at the bottom — should not happen because the card hides
        // the button, but guard anyway.
        return
      }
      if (entry.project_id) {
        // Currently project-scoped → demote to session.  Need both ids.
        if (!activeSessionId || !activeProjectId) {
          setError('需要打开一个项目和会话才能将记忆降级到 session 范围')
          return
        }
        await memoryDemote({
          key: entry.key,
          targetScopeKind: 'session',
          sessionId: activeSessionId,
          projectId: activeProjectId,
        })
      } else {
        // Currently global → demote to project.  Need a project id.
        if (!activeProjectId) {
          setError('需要打开一个项目才能将全局记忆降级到 project 范围')
          return
        }
        await memoryDemote({
          key: entry.key,
          targetScopeKind: 'project',
          projectId: activeProjectId,
        })
      }
      // Reload so the chip color reflects the new tier and refresh the
      // promotion candidate list — a freshly demoted entry may now qualify
      // for re-promotion or drop off the list.
      loadEntries()
      if (promotionOpen) loadCandidates()
    } catch (e) {
      setError(`降级失败: ${e}`)
    }
  }

  const applyPromotion = async (cand: MemoryPromotionCandidateDto) => {
    if (cand.target_tier === 'session') return // backend rejects this anyway
    const confirmed = window.confirm(
      [
        `确认晋升记忆「${cand.key}」？`,
        '',
        `范围: ${cand.current_tier} -> ${cand.target_tier}`,
        `原因: ${cand.reason}`,
        '',
        '这会影响后续对话可见的记忆范围；如结果不符合预期，可在记忆卡片中降级。',
      ].join('\n'),
    )
    if (!confirmed) return
    try {
      await memoryPromote({
        key: cand.key,
        targetScopeKind: cand.target_tier as 'project' | 'global',
        projectId:
          cand.target_tier === 'project' ? activeProjectId ?? undefined : undefined,
      })
      setCandidates((prev) => prev.filter((c) => c.key !== cand.key))
      // Refresh the main entry list so the chip color updates immediately.
      loadEntries()
    } catch (e) {
      setError(`晋升失败: ${e}`)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-[var(--app-main-bg,#f6f7f8)] text-foreground">
      {/* Header — draggable via onStartWindowDrag when running in main window */}
      <div
        className="shrink-0 cursor-default select-none border-b border-border/60 bg-card/60 px-4 py-3 backdrop-blur"
        onMouseDown={onStartWindowDrag}
      >
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-[15px] font-semibold">记忆浏览器</h2>
          <div className="flex items-center gap-3">
            {totalCount !== null && (
              <span className="text-[12px] text-muted-foreground">
                {searchTotalCount !== null
                  ? `${searchTotalCount} / ${totalCount} 条匹配`
                  : `共 ${totalCount} 条记忆`}
              </span>
            )}
            <Button
              type="button"
              size="sm"
              variant={promotionOpen ? 'default' : 'outline'}
              onClick={togglePromotionPanel}
              className="h-7 gap-1.5 px-2 text-[12px]"
              title="基于 importance / access_count 推荐 session→project / project→global 升级"
            >
              <TrendingUp className="h-3.5 w-3.5" />
              候选晋升
              {candidates.length > 0 && (
                <span className="ml-1 rounded-full bg-amber-500/20 px-1.5 text-[10px] font-medium text-amber-700">
                  {candidates.length}
                </span>
              )}
            </Button>
          </div>
        </div>
      </div>

      {/* Search */}
      <div className="shrink-0 border-b border-border/60 bg-card/40 px-4 py-2.5 backdrop-blur">
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground/70" />
            <input
              type="text"
              placeholder="搜索记忆..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleSearch()
              }}
              className="w-full rounded-md border border-border/70 bg-card/80 py-2 pl-9 pr-3 text-[13px] outline-none placeholder:text-muted-foreground/55 focus:border-primary/40 focus:ring-1 focus:ring-primary/20"
            />
          </div>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={handleSearch}
            disabled={loading}
          >
            搜索
          </Button>
          {searchQuery && (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => {
                setSearchQuery('')
                setSearchTotalCount(null)
                loadEntries()
              }}
            >
              清除
            </Button>
          )}
        </div>
      </div>

      {/* Scope Segmented Control — three-tier visibility filter */}
      <div className="shrink-0 border-b border-border/60 bg-card/40 px-4 py-2 backdrop-blur">
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
            范围
          </span>
          <div
            className="inline-flex overflow-hidden rounded-md border border-border/70 bg-card/70 text-[12px]"
            role="tablist"
            aria-label="Memory scope filter"
          >
            {SCOPE_OPTIONS.map((opt) => {
              const disabled =
                (opt.id === 'project' && !activeProjectId) ||
                (opt.id === 'session' && !activeSessionId)
              const isActive = scopeFilter === opt.id
              return (
                <button
                  key={opt.id}
                  type="button"
                  role="tab"
                  aria-selected={isActive}
                  disabled={disabled}
                  title={
                    disabled
                      ? opt.id === 'project'
                        ? '需要先选中一个项目'
                        : '需要先打开一个会话'
                      : opt.help
                  }
                  onClick={() => setScopeFilter(opt.id)}
                  className={
                    'px-3 py-1 transition-colors ' +
                    (isActive
                      ? 'bg-primary/15 text-primary font-medium'
                      : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground disabled:cursor-not-allowed disabled:opacity-40')
                  }
                >
                  {opt.label}
                </button>
              )
            })}
          </div>
          {scopeMissingContext && (
            <span className="text-[11px] text-amber-600">
              {scopeFilter === 'project'
                ? '未选中项目，已退化为「全部」'
                : '未打开会话，已退化为「全部」'}
            </span>
          )}
        </div>
      </div>

      {/* Category Navigation */}
      <MemoryCategoryNav
        activeCategory={activeCategory}
        onCategoryChange={handleCategoryChange}
      />

      {/* Promotion Candidates Panel */}
      {promotionOpen && (
        <div className="shrink-0 border-b border-amber-200/60 bg-amber-50/60 px-4 py-3">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[12px] font-medium text-amber-800">
              候选晋升
              <span className="ml-2 text-[11px] font-normal text-amber-700/70">
                根据 importance & 访问频次推荐
              </span>
            </span>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={loadCandidates}
              disabled={promotionLoading}
              className="h-6 px-2 text-[11px] text-amber-800 hover:bg-amber-100"
            >
              {promotionLoading ? '扫描中...' : '刷新'}
            </Button>
          </div>
          {candidates.length === 0 ? (
            <p className="text-[12px] text-amber-700/70">
              {promotionLoading ? '正在扫描...' : '暂无符合阈值的候选条目'}
            </p>
          ) : (
            <ul className="flex flex-col gap-1.5">
              {candidates.map((c) => {
                const needsProjectId = c.target_tier === 'project' && !activeProjectId
                const duplicateCount = candidates.filter(
                  (item) =>
                    item.key !== c.key &&
                    normalizeMemoryKeyForDuplicateHint(item.key) ===
                      normalizeMemoryKeyForDuplicateHint(c.key),
                ).length
                return (
                  <li
                    key={c.key}
                    className="flex items-center gap-3 rounded-md border border-amber-500/30 bg-card/60 px-3 py-2"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[12px] font-medium text-foreground/90">
                        {c.key}
                      </div>
                      <div className="text-[11px] text-amber-800/80">
                        {c.current_tier} → <span className="font-medium">{c.target_tier}</span>
                        {' · '}
                        {c.reason}
                      </div>
                      {duplicateCount > 0 ? (
                        <div className="mt-0.5 text-[10.5px] text-rose-600/80">
                          检测到 {duplicateCount} 条相似候选，建议先复核/合并再晋升
                        </div>
                      ) : null}
                    </div>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      disabled={needsProjectId}
                      title={needsProjectId ? '先选中一个项目' : '应用此晋升'}
                      className="h-7 px-2 text-[11px]"
                      onClick={() => applyPromotion(c)}
                    >
                      应用
                    </Button>
                  </li>
                )
              })}
            </ul>
          )}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-4 py-3">
        {error && (
          <div className="mb-3 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-[13px] text-red-600">
            {error}
          </div>
        )}

        {loading ? (
          <div className="flex items-center justify-center py-12 text-[13px] text-muted-foreground">
            加载中...
          </div>
        ) : entries.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <p className="text-[14px] text-muted-foreground">
              {searchQuery.trim()
                ? `未找到包含「${searchQuery.trim()}」的记忆`
                : '暂无记忆条目'}
            </p>
            <p className="mt-1 text-[12px] text-muted-foreground/70">
              {searchQuery.trim() && totalCount !== null
                ? `当前范围共 ${totalCount} 条，匹配 0 条`
                : '记忆会在对话过程中自动存储'}
            </p>
          </div>
        ) : (
          (() => {
            // Memory Audit P2 #11 — paginate the rendered slice.
            // Keep the count helpers local so the JSX above doesn't
            // change shape and the conditional structure stays
            // readable.
            const totalPages = Math.max(1, Math.ceil(entries.length / PAGE_SIZE))
            const safePageIndex = Math.min(pageIndex, totalPages - 1)
            const start = safePageIndex * PAGE_SIZE
            const pagedEntries = entries.slice(start, start + PAGE_SIZE)
            return (
              <>
                <div className="flex flex-col gap-2">
                  {pagedEntries.map((entry) => {
                    const isProjectTier =
                      entry.session_id === null && entry.project_id !== null
                    const isGlobalTier =
                      entry.session_id === null && entry.project_id === null
                    const canDemote =
                      (isGlobalTier && Boolean(activeProjectId)) ||
                      (isProjectTier &&
                        Boolean(activeProjectId) &&
                        Boolean(activeSessionId))
                    return (
                      <MemoryCard
                        key={entry.key}
                        entry={entry}
                        onDelete={handleDelete}
                        onDemote={handleDemote}
                        canDemote={canDemote}
                      />
                    )
                  })}
                </div>
                {totalPages > 1 ? (
                  <div className="mt-3 flex items-center justify-between rounded-xl border border-border/60 bg-muted/30 px-3 py-2 text-[11.5px]">
                    <span className="text-muted-foreground">
                      第 <strong className="font-semibold tabular-nums text-foreground/85">{safePageIndex + 1}</strong>
                      <span className="mx-0.5 text-muted-foreground/60">/</span>
                      <strong className="font-semibold tabular-nums text-foreground/85">{totalPages}</strong> 页
                      <span className="ml-2 text-muted-foreground">
                        共 {entries.length} 条 · 每页 {PAGE_SIZE}
                      </span>
                    </span>
                    <div className="flex items-center gap-1.5">
                      <button
                        type="button"
                        onClick={() => setPageIndex((i) => Math.max(0, i - 1))}
                        disabled={safePageIndex === 0}
                        className="rounded-md border border-border/70 bg-card px-2 py-1 text-[11px] font-medium text-foreground/75 transition-colors hover:bg-accent hover:text-accent-foreground disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        上一页
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          setPageIndex((i) => Math.min(totalPages - 1, i + 1))
                        }
                        disabled={safePageIndex >= totalPages - 1}
                        className="rounded-md border border-border/70 bg-card px-2 py-1 text-[11px] font-medium text-foreground/75 transition-colors hover:bg-accent hover:text-accent-foreground disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        下一页
                      </button>
                    </div>
                  </div>
                ) : null}
              </>
            )
          })()
        )}
      </div>
    </div>
  )
}
