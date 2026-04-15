/**
 * MemoryCategoryNav — 分类标签导航组件
 *
 * 提供 Core / Daily / Conversation / Custom 四个分类标签页，
 * 用户点击后触发 onCategoryChange 回调。
 *
 * Props:
 * - activeCategory: 当前选中的分类
 * - onCategoryChange: 分类切换回调
 */

import { cn } from '@/lib/utils'

const CATEGORIES = [
  { value: 'all', label: '全部' },
  { value: 'core', label: 'Core' },
  { value: 'daily', label: 'Daily' },
  { value: 'conversation', label: 'Conversation' },
] as const

export interface MemoryCategoryNavProps {
  activeCategory: string
  onCategoryChange: (category: string) => void
}

export function MemoryCategoryNav({
  activeCategory = 'all',
  onCategoryChange,
}: MemoryCategoryNavProps) {
  return (
    <nav className="flex gap-1 border-b border-black/5 px-4">
      {CATEGORIES.map((cat) => (
        <button
          key={cat.value}
          type="button"
          className={cn(
            'rounded-t-md px-3 py-2 text-[13px] font-medium transition-colors duration-150',
            activeCategory === cat.value
              ? 'border-b-2 border-primary bg-primary/5 text-primary'
              : 'text-muted-foreground hover:bg-black/[0.03] hover:text-foreground',
          )}
          onClick={() => onCategoryChange(cat.value)}
        >
          {cat.label}
        </button>
      ))}
    </nav>
  )
}
