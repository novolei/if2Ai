/**
 * PinItem — single sortable row in PinnedMemoryEditor (Phase 8A.12 / T-UI-1).
 *
 * Drag handle on the left, content in the middle, hover-revealed delete
 * button on the right.  Wired to `@dnd-kit/sortable` via `useSortable`.
 */

import { GripVertical, X } from 'lucide-react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import type { PinnedItemDto } from '@/api/memory'

export interface PinItemProps {
  pin: PinnedItemDto
  onDelete: (id: string) => void
}

export function PinItem({ pin, onDelete }: PinItemProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: pin.id,
  })

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  }

  return (
    <li
      ref={setNodeRef}
      style={style}
      className="group flex items-start gap-2 rounded border bg-background/60 px-2 py-1.5 text-sm hover:bg-amber-50 dark:hover:bg-amber-950/40"
    >
      <button
        type="button"
        {...attributes}
        {...listeners}
        className="cursor-grab text-muted-foreground hover:text-foreground"
        aria-label="拖拽排序"
        title="拖拽排序"
      >
        <GripVertical className="h-4 w-4" />
      </button>
      <span className="flex-1 break-words">{pin.content}</span>
      <button
        type="button"
        onClick={() => onDelete(pin.id)}
        className="opacity-0 transition-opacity group-hover:opacity-100 text-muted-foreground hover:text-red-600"
        aria-label="删除置顶记忆"
        title="删除"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </li>
  )
}
