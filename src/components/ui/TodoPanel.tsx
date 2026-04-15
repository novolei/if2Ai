import * as React from 'react'
import { ArrowRight, Check, CircleDashed, ListTodo, Maximize2 } from 'lucide-react'
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
  // Only show pending and in-progress tasks
  const visibleTodos = todos.filter((todo) => todo.status !== 'completed')
  if (visibleTodos.length === 0) return null

  const expandedHeight = 44 + visibleTodos.length * 26

  return (
    <div
      ref={ref}
      className={cn('shrink-0', className)}
      style={collapsed ? undefined : { height: `${expandedHeight}px` }}
    >
      <div
        className={cn(
          'h-full rounded-t-[20px] rounded-b-none border border-border/50 bg-surface/98 shadow-xs backdrop-blur-xl',
          collapsed ? 'px-4 pt-1 pb-2' : 'px-4 pt-1 pb-2'
        )}
      >
        <div className="flex items-center justify-between text-muted-foreground">
          <div className="flex items-center gap-1.5 text-[11px] font-medium tracking-[-0.01em]">
            <ListTodo className="h-3.5 w-3.5 stroke-[1.9]" />
            <span>{visibleTodos.length} 个任务进行中</span>
          </div>
          <button
            type="button"
            className="flex h-6 w-6 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-black/[0.03] hover:text-black/60"
            aria-label={collapsed ? '展开任务面板' : '折叠任务面板'}
            onClick={onToggleCollapsed}
          >
            <Maximize2 className={cn('h-2.5 w-2.5 stroke-[1.8] transition-transform', collapsed && 'rotate-180')} />
          </button>
        </div>

        {!collapsed ? (
          <div className="mt-1.5 space-y-0.5 overflow-hidden">
            {visibleTodos.map((todo, index) => (
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
  return (
    <div className="flex h-[26px] items-center gap-2 text-[11px] leading-[1.3] tracking-[-0.01em] text-foreground/80">
      <TodoStatusIcon status={todo.status} />
      <div className="flex min-w-0 flex-1 items-center gap-1.5 overflow-hidden">
        <span className="shrink-0 tabular-nums text-muted-foreground/70">{index + 1}.</span>
        <span className="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
          {todo.activeForm}
        </span>
      </div>
    </div>
  )
}

function TodoStatusIcon({ status }: { status: TodoItem['status'] }) {
  if (status === 'completed') {
    return (
      <div className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full border-[1.5px] border-muted-foreground/40 text-muted-foreground/50">
        <Check className="h-2.5 w-2.5 stroke-[2.4]" />
      </div>
    )
  }

  if (status === 'in_progress') {
    return (
      <div className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full border-[1.5px] border-muted-foreground/50 text-muted-foreground/60">
        <ArrowRight className="h-2.5 w-2.5 stroke-[2.3]" />
      </div>
    )
  }

  return (
    <div className="flex h-4 w-4 shrink-0 items-center justify-center text-muted-foreground/40">
      <CircleDashed className="h-3.5 w-3.5 stroke-[1.8]" />
    </div>
  )
}

export { TodoPanel }
export type { TodoItem }
