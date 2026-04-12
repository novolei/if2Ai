import { cn } from '@/lib/utils'

interface TodoItem {
  content: string
  activeForm: string
  status: 'pending' | 'in_progress' | 'completed'
}

function TodoPanel({ todos }: { todos: TodoItem[] }) {
  if (todos.length === 0) return null
  const completedCount = todos.filter((t) => t.status === 'completed').length
  const allDone = todos.every((t) => t.status === 'completed')

  return (
    <div className="border-b bg-muted/20 px-4 py-2">
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span className="font-medium text-foreground">
          Tasks ({completedCount}/{todos.length} completed)
        </span>
        {allDone && <span>All tasks completed</span>}
      </div>
      {!allDone && (
        <div className="mt-1 space-y-0.5">
          {todos.map((todo, i) => (
            <TodoItemRow key={i} todo={todo} />
          ))}
        </div>
      )}
    </div>
  )
}

function TodoItemRow({ todo }: { todo: TodoItem }) {
  const icon = { pending: '○', in_progress: '◐', completed: '✓' }[todo.status]
  const color = {
    pending: 'text-muted-foreground',
    in_progress: 'text-blue-500 animate-pulse',
    completed: 'text-green-500',
  }[todo.status]
  return (
    <div className={cn('flex items-center gap-2 text-sm', color)}>
      <span className="w-4 text-center font-mono">{icon}</span>
      <span className="flex-1 truncate">{todo.activeForm}</span>
    </div>
  )
}

export { TodoPanel }
export type { TodoItem }
