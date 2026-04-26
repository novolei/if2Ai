/**
 * ModelPicker — composer footer 的"当前模型"按钮 + 下拉切换器。
 *
 * 视觉与交互对齐 `BranchPicker`（Popover + 搜索框 + 列表 + 单行截断 +
 * 当前项打勾）。Items 限定单行显示（`whitespace-nowrap` + `truncate`），
 * provider/model 名再长也不会折行；超过宽度时悬停可看完整 `title`。
 *
 * 数据来源：`model_list_available` Tauri command。父组件传入：
 *   - `availableItems`  已展平的 `{ value: 'provider/model', label: '...' }[]`
 *   - `selected`        当前选中的 `value`（同一字符串格式）
 *   - `onChange(value)` 选中后回调；本组件不直接调用 `model_set_active`，
 *                       由 caller 统一处理（chat-ui 已有逻辑）
 */

import * as React from 'react'
import { Check, ChevronDown, Search, Sparkles } from 'lucide-react'

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { cn } from '@/lib/utils'

export interface ModelPickerItem {
  value: string
  label: string
}

interface Props {
  availableItems: ModelPickerItem[]
  selected: string
  onChange: (value: string) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}

export function ModelPicker({
  availableItems,
  selected,
  onChange,
  disabled = false,
  placeholder = '选择模型',
  className,
}: Props) {
  const [open, setOpen] = React.useState(false)
  const [query, setQuery] = React.useState('')

  React.useEffect(() => {
    if (!open) setQuery('')
  }, [open])

  const currentLabel = React.useMemo(() => {
    const hit = availableItems.find((i) => i.value === selected)
    if (hit) return hit.label
    if (availableItems.length > 0) return availableItems[0].label
    return placeholder
  }, [availableItems, selected, placeholder])

  const filtered = React.useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return availableItems
    return availableItems.filter((i) => i.label.toLowerCase().includes(q))
  }, [availableItems, query])

  return (
    <Popover open={open} onOpenChange={(next) => !disabled && setOpen(next)}>
      <PopoverTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          className={cn(
            'flex items-center gap-1 rounded-lg px-2 py-1 text-[12px] text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground',
            'disabled:cursor-not-allowed disabled:opacity-60',
            className,
          )}
          title={currentLabel}
          aria-label="切换模型"
        >
          <span className="max-w-[200px] truncate">{currentLabel}</span>
          <ChevronDown className="h-3 w-3 shrink-0" />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        sideOffset={8}
        collisionPadding={16}
        className={cn(
          'w-[300px] overflow-hidden rounded-2xl border border-border/70 bg-popover/96 p-0 text-[13px] text-popover-foreground backdrop-blur-2xl backdrop-saturate-150',
          'shadow-[0_2px_4px_rgba(0,0,0,0.04),0_8px_20px_rgba(0,0,0,0.08),0_24px_56px_rgba(0,0,0,0.16),0_0_0_0.5px_rgba(0,0,0,0.04)]',
          'origin-[var(--radix-popover-content-transform-origin)] transition-all duration-200 ease-out',
          'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=open]:slide-in-from-bottom-1',
          'data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95',
        )}
      >
        {/* Search */}
        <div className="flex items-center gap-2 px-3.5 pt-3 pb-2.5">
          <Search className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
          <input
            type="text"
            autoFocus
            placeholder="搜索模型 / 服务商"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="flex-1 bg-transparent text-[11.5px] leading-6 text-popover-foreground outline-none placeholder:text-muted-foreground"
          />
        </div>

        {/* List */}
        <div className="max-h-[320px] overflow-y-auto pb-1.5">
          {availableItems.length === 0 ? (
            <div className="px-3.5 py-6 text-center text-[12px] text-muted-foreground">
              <Sparkles className="mx-auto mb-1.5 h-4 w-4 opacity-50" />
              <div>尚未配置任何模型</div>
              <div className="mt-1 text-[10.5px] text-muted-foreground/75">
                先到「设置 / 服务商」添加 API Key 并选择模型
              </div>
            </div>
          ) : (
            <>
              <div className="px-3.5 pb-1 pt-1 text-[11.5px] text-muted-foreground">
                可用模型
              </div>
              {filtered.length === 0 && (
                <div className="px-3.5 py-5 text-center text-[12px] text-muted-foreground">
                  无匹配模型
                </div>
              )}
              {filtered.map((item) => {
                const isCurrent = item.value === selected
                return (
                  <button
                    key={item.value}
                    type="button"
                    aria-selected={isCurrent}
                    onClick={() => {
                      onChange(item.value)
                      setOpen(false)
                    }}
                    className={cn(
                      'flex w-full items-center gap-2.5 px-3.5 py-1.5 text-left outline-none transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:bg-accent focus-visible:text-accent-foreground',
                      isCurrent && 'the-finals-selected-menu-item',
                    )}
                    title={item.label}
                  >
                    <span
                      className={cn(
                        'min-w-0 flex-1 truncate whitespace-nowrap text-[13px] leading-6',
                        isCurrent ? 'text-popover-foreground font-medium' : 'text-popover-foreground/78',
                      )}
                    >
                      {item.label}
                    </span>
                    {isCurrent && (
                      <Check
                        className="h-[13px] w-[13px] shrink-0 text-primary"
                        strokeWidth={2}
                      />
                    )}
                  </button>
                )
              })}
            </>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}
