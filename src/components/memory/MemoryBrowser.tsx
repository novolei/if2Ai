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
import { Search } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { MemoryCard, type MemoryEntryDto } from './MemoryCard'
import { MemoryCategoryNav } from './MemoryCategoryNav'
import { Button } from '@/components/ui/button'

const PAGE_SIZE = 20

interface MemoryBrowserProps {
  /** Forward the App-level startWindowDrag handler so the header is draggable. */
  onStartWindowDrag?: (e: ReactMouseEvent<HTMLElement>) => void
}

export function MemoryBrowser({ onStartWindowDrag }: MemoryBrowserProps) {
  const [entries, setEntries] = useState<MemoryEntryDto[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [activeCategory, setActiveCategory] = useState('all')
  const [totalCount, setTotalCount] = useState<number | null>(null)

  // Load all entries on mount and when category changes
  useEffect(() => {
    loadEntries()
  }, [activeCategory])

  const loadEntries = async () => {
    setLoading(true)
    setError(null)
    try {
      const category = activeCategory === 'all' ? null : activeCategory
      const results = await invoke<MemoryEntryDto[]>('memory_export', {
        category,
      })
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
      const results = await invoke<MemoryEntryDto[]>('memory_recall', {
        query: searchQuery.trim(),
        category,
        limit: PAGE_SIZE,
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
      await invoke<void>('memory_delete', { key })
      setEntries((prev) => prev.filter((e) => e.key !== key))
    } catch (e) {
      setError(`删除失败: ${e}`)
    }
  }

  const handleCategoryChange = (category: string) => {
    setActiveCategory(category)
    setSearchQuery('')
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-[#f6f7f8]">
      {/* Header — draggable via onStartWindowDrag when running in main window */}
      <div
        className="shrink-0 cursor-default select-none border-b border-black/5 bg-white/60 px-4 py-3"
        onMouseDown={onStartWindowDrag}
      >
        <div className="flex items-center justify-between">
          <h2 className="text-[15px] font-semibold">记忆浏览器</h2>
          {totalCount !== null && (
            <span className="text-[12px] text-black/40">
              共 {totalCount} 条记忆
            </span>
          )}
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

      {/* Category Navigation */}
      <MemoryCategoryNav
        activeCategory={activeCategory}
        onCategoryChange={handleCategoryChange}
      />

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
