/**
 * MemoryCategoryNav — 分类标签导航组件
 *
 * 提供 Core / Daily / Conversation / Working / Procedural / Reflection
 * 六个内置分类 + Custom（其他用户自定义字符串），用户点击后触发
 * onCategoryChange 回调。
 *
 * MEM-MOD-P2 — Working / Procedural / Reflection 三个新分类与后端
 * `MemoryCategory` enum 一一对应（src-tauri/src/modules/memory/mod.rs）。
 */

import { cn } from '@/lib/utils'

const CATEGORIES = [
  { value: 'all', label: '全部' },
  { value: 'core', label: 'Core' },
  { value: 'daily', label: 'Daily' },
  { value: 'conversation', label: 'Conversation' },
  { value: 'working', label: 'Working' },
  { value: 'procedural', label: 'Procedural' },
  { value: 'reflection', label: 'Reflection' },
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
    <nav className="flex gap-1 border-b border-border px-4">
      {CATEGORIES.map((cat) => (
        <button
          key={cat.value}
          type="button"
          className={cn(
            'rounded-t-md px-3 py-2 text-[13px] font-medium transition-colors duration-150',
            activeCategory === cat.value
              ? 'border-b-2 border-primary bg-primary/5 text-primary'
              : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground',
          )}
          onClick={() => onCategoryChange(cat.value)}
        >
          {cat.label}
        </button>
      ))}
    </nav>
  )
}
