import * as React from 'react'
import { Circle, ListTodo, Maximize2 } from 'lucide-react'
import { cn } from '@/lib/utils'

interface TodoItem {
  content: string
  activeForm: string
  status: 'pending' | 'in_progress' | 'completed'
}

const TodoPanel = React.forwardRef<
  HTMLDivElement,
  {
    todos: TodoItem[]
    className?: string
    collapsed?: boolean
    onToggleCollapsed?: () => void
  }
>(function TodoPanel({
  todos,
  className,
  collapsed = false,
  onToggleCollapsed,
}, ref) {
  if (todos.length === 0) return null

  const completedCount = todos.filter((todo) => todo.status === 'completed').length
  const expandedHeight = 52 + todos.length * 34

  return (
    <div
      ref={ref}
      className={cn('shrink-0', className)}
      style={collapsed ? undefined : { height: `${expandedHeight}px` }}
    >
      <div
        className={cn(
          'h-full rounded-t-[20px] rounded-b-none border border-[#e1e3e6] bg-[#f6f7f8]/98 shadow-[0_10px_28px_rgba(15,23,42,0.04)] backdrop-blur-xl',
          collapsed ? 'px-5 pt-1 pb-2.5' : 'px-5 pt-1.5 pb-3'
        )}
      >
        <div className="flex items-center justify-between text-[#8c8c8c]">
          <div className="flex items-center gap-2 text-[13px] font-medium tracking-[-0.015em]">
            <ListTodo className="h-4 w-4 stroke-[1.9]" />
            <span>共 {todos.length} 个任务，已经完成 {completedCount} 个</span>
          </div>
          <button
            type="button"
            className="flex h-7 w-7 items-center justify-center rounded-full text-[#8c8c8c] transition-colors hover:bg-black/[0.03] hover:text-black/60"
            aria-label={collapsed ? '展开任务面板' : '折叠任务面板'}
            onClick={onToggleCollapsed}
          >
            <Maximize2 className={cn('h-3 w-3 stroke-[1.8] transition-transform', collapsed && 'rotate-180')} />
          </button>
        </div>

        {!collapsed ? (
          <div className="mt-2.5 space-y-1 overflow-hidden">
            {todos.map((todo, index) => (
              <TodoItemRow key={`${todo.content}-${index}`} index={index} todo={todo} />
            ))}
          </div>
        ) : null}
      </div>
    </div>
  )
})

TodoPanel.displayName = 'TodoPanel'

function TodoItemRow({ index, todo }: { index: number; todo: TodoItem }) {
  const isCompleted = todo.status === 'completed'
  const isActive = todo.status === 'in_progress'

  return (
    <div className="flex h-[30px] items-center gap-2.5 text-[13px] leading-[1.3] tracking-[-0.01em] text-[#1f1f1f]">
      <div className="flex h-5 w-5 shrink-0 items-center justify-center">
        <Circle
          className={cn(
            'h-3 w-3 stroke-[1.85]',
            isCompleted && 'fill-[#1f1f1f] text-[#1f1f1f]',
            isActive && 'text-[#7b7b7b]',
            !isCompleted && !isActive && 'text-[#3a3a3a]'
          )}
        />
      </div>
      <div className="flex min-w-0 flex-1 items-center gap-2 overflow-hidden">
        <span className="shrink-0 tabular-nums text-[#1f1f1f]">{index + 1}.</span>
        <span
          className={cn(
            'min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap',
            isCompleted && 'text-[#7f7f7f] line-through'
          )}
        >
          {todo.activeForm}
        </span>
      </div>
    </div>
  )
}

export { TodoPanel }
export type { TodoItem }
