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
import { MemoryCard, type MemoryEntryDto } from './MemoryCard'
import { MemoryCategoryNav } from './MemoryCategoryNav'
import { Button } from '@/components/ui/button'
import {
  memoryDelete,
  memoryExport,
  memoryPromote,
  memoryPromotionCandidates,
  memoryRecall,
  type MemoryPromotionCandidateDto,
  type MemoryScopeArgs,
  type MemoryScopeKind,
} from '@/lib/tauri'

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
  const [scopeFilter, setScopeFilter] = useState<ScopeFilter>('all')
  const [promotionOpen, setPromotionOpen] = useState(false)
  const [candidates, setCandidates] = useState<MemoryPromotionCandidateDto[]>([])
  const [promotionLoading, setPromotionLoading] = useState(false)

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

  // Reload whenever the filter dimensions that affect server response change.
  useEffect(() => {
    loadEntries()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeCategory, scopeFilter, activeProjectId, activeSessionId])

  const loadEntries = async () => {
    setLoading(true)
    setError(null)
    try {
      const category = activeCategory === 'all' ? null : activeCategory
      const results = await memoryExport({ category, scope: effectiveScope })
      setEntries(results)
      setTotalCount(results.length)
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
      const results = await memoryRecall({
        query: searchQuery.trim(),
        category,
        limit: PAGE_SIZE,
        scope: effectiveScope,
      })
      setEntries(results)
      setTotalCount(null)
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

  const applyPromotion = async (cand: MemoryPromotionCandidateDto) => {
    if (cand.target_tier === 'session') return // backend rejects this anyway
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
    <div className="flex h-full min-h-0 flex-col bg-[#f6f7f8]">
      {/* Header — draggable via onStartWindowDrag when running in main window */}
      <div
        className="shrink-0 cursor-default select-none border-b border-black/5 bg-white/60 px-4 py-3"
        onMouseDown={onStartWindowDrag}
      >
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-[15px] font-semibold">记忆浏览器</h2>
          <div className="flex items-center gap-3">
            {totalCount !== null && (
              <span className="text-[12px] text-black/40">
                共 {totalCount} 条记忆
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
      <div className="shrink-0 border-b border-black/5 bg-white/40 px-4 py-2.5">
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-black/30" />
            <input
              type="text"
              placeholder="搜索记忆..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleSearch()
              }}
              className="w-full rounded-md border border-black/10 bg-white/80 py-2 pl-9 pr-3 text-[13px] outline-none placeholder:text-black/30 focus:border-primary/30 focus:ring-1 focus:ring-primary/20"
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
                loadEntries()
              }}
            >
              清除
            </Button>
          )}
        </div>
      </div>

      {/* Scope Segmented Control — three-tier visibility filter */}
      <div className="shrink-0 border-b border-black/5 bg-white/40 px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-medium uppercase tracking-wide text-black/40">
            范围
          </span>
          <div
            className="inline-flex overflow-hidden rounded-md border border-black/10 bg-white/70 text-[12px]"
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
                      : 'text-black/60 hover:bg-black/5 disabled:cursor-not-allowed disabled:opacity-40')
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
                return (
                  <li
                    key={c.key}
                    className="flex items-center gap-3 rounded-md border border-amber-200/60 bg-white/60 px-3 py-2"
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
          <div className="flex items-center justify-center py-12 text-[13px] text-black/40">
            加载中...
          </div>
        ) : entries.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <p className="text-[14px] text-black/50">暂无记忆条目</p>
            <p className="mt-1 text-[12px] text-black/30">
              记忆会在对话过程中自动存储
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {entries.map((entry) => (
              <MemoryCard key={entry.key} entry={entry} onDelete={handleDelete} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
