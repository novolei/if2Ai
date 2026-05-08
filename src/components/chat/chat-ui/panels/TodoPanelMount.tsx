/**
 * GF-01 PR-07 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Thin presentational mount around the existing
 * `@/components/ui/TodoPanel`. Owns only the responsive padding /
 * max-width chrome around the panel; the collapsed-state hook + ref +
 * todos source remain in `ChatUI` (state-machine extraction is
 * deferred to PR-08 `useTodoPanelState` hook).
 *
 * Render-equivalent move from the original inline JSX.
 */

import * as React from "react"
import { TodoPanel, type TodoItem } from "@/components/ui/TodoPanel"

/**
 * Mount the project-todos panel inside the chat surface. Chrome
 * (responsive max-width + right inset transition) is bundled here so
 * the parent `ChatUI` only needs to forward layout numbers.
 */
export function TodoPanelMount({
  todoPanelRef,
  todos,
  isTodoCollapsed,
  onToggleCollapsed,
  todoMaxWidth,
  chatVisibleRightInset,
}: {
  todoPanelRef: React.Ref<HTMLDivElement>
  todos: TodoItem[]
  isTodoCollapsed: boolean
  onToggleCollapsed: () => void
  todoMaxWidth: number
  chatVisibleRightInset: number
}) {
  return (
    <div
      className="relative z-0 shrink-0 px-10 transition-[padding-right] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
      style={{ paddingRight: `${40 + chatVisibleRightInset}px` }}
    >
      <div className="mx-auto w-full transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]" style={{ maxWidth: `${todoMaxWidth}px` }}>
        <TodoPanel
          ref={todoPanelRef}
          todos={todos}
          collapsed={isTodoCollapsed}
          onToggleCollapsed={onToggleCollapsed}
          className="w-full translate-y-[8px]"
        />
      </div>
    </div>
  )
}
