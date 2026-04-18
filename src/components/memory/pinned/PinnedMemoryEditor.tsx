/**
 * PinnedMemoryEditor — Settings page section for curating pinned memory
 * (Phase 8A.12 / T-UI-1).
 *
 * Lets users curate the pinned-memory list that the agent injects into
 * every system prompt (8A.11 `build_memory_injection`).  Two scope tabs
 * (project / global), drag-to-reorder via `@dnd-kit`, hover-reveal
 * delete, char counter, and optimistic updates.
 *
 * Backend contract:
 *   - `pinnedGet(scope, projectId)` lists pins for the active scope.
 *   - `pinnedAdd(content, scope, projectId)` returns the persisted DTO
 *     after server-side `ThreatScanner.scan_and_redact` PII pass.  When
 *     the returned `content` differs from input, we surface a yellow
 *     "auto-redacted" banner.
 *   - `pinnedDelete(id)` is idempotent.
 *   - `pinnedReorder(ids)` rewrites `created_at` so list order matches
 *     the array order — drag-end calls this with the post-drag id list.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { AlertCircle, Folder, Pin, Shield } from 'lucide-react'
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core'
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { PinItem } from './PinItem'
import {
  listProjects,
  pinnedAdd,
  pinnedDelete,
  pinnedGet,
  pinnedReorder,
  type ProjectMeta,
  type PinnedItemDto,
} from '@/lib/tauri'

type Scope = 'project' | 'global'

const MAX_CHARS = 500
const MAX_PINS = 50

export interface PinnedMemoryEditorProps {
  /** Current project id; required to add project-scoped pins. */
  projectId?: string
}

export function PinnedMemoryEditor({ projectId: propProjectId }: PinnedMemoryEditorProps) {
  // Settings is opened in a separate window without App.tsx state; fall back
  // to localStorage `lastActiveProjectId` (set by App.tsx on every project
  // switch) so users can pin to "current project" without an explicit prop.
  // Falls back further to a fetched project list for picker UX.
  const [projects, setProjects] = useState<ProjectMeta[]>([])
  const [pickedProjectId, setPickedProjectId] = useState<string | undefined>(() => {
    try {
      return localStorage.getItem('lastActiveProjectId') ?? undefined
    } catch {
      return undefined
    }
  })
  const projectId = propProjectId ?? pickedProjectId

  // Default the scope to "global" when we have no project context — the
  // "project" tab would otherwise be functionally inert.
  const [scope, setScope] = useState<Scope>(projectId ? 'project' : 'global')
  const [pins, setPins] = useState<PinnedItemDto[]>([])
  const [input, setInput] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [redactedNotice, setRedactedNotice] = useState<string | null>(null)

  useEffect(() => {
    void (async () => {
      try {
        const list = await listProjects()
        setProjects(list)
        // If localStorage gave us an id but it no longer exists, drop it.
        if (pickedProjectId && !list.some((p) => p.id === pickedProjectId)) {
          setPickedProjectId(list[0]?.id)
        } else if (!pickedProjectId && list[0]) {
          setPickedProjectId(list[0].id)
        }
      } catch (e) {
        console.warn('PinnedMemoryEditor: listProjects failed:', e)
      }
    })()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const activeProject = useMemo(
    () => projects.find((p) => p.id === projectId),
    [projects, projectId],
  )

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const items = await pinnedGet(scope, scope === 'project' ? projectId : undefined)
      setPins(items)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }, [scope, projectId])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const remaining = MAX_CHARS - input.length
  const overLimit = remaining < 0
  const atCapacity = pins.length >= MAX_PINS

  const handleAdd = async () => {
    const content = input.trim()
    if (!content || overLimit || atCapacity) return
    if (scope === 'project' && !projectId) {
      setError('当前未选择项目，无法添加项目范围的置顶记忆。')
      return
    }
    const optimisticId = `optimistic-${Date.now()}`
    const optimistic: PinnedItemDto = {
      id: optimisticId,
      content,
      scope,
      projectId: scope === 'project' ? projectId : undefined,
      createdAt: new Date().toISOString(),
      createdByKind: 'user',
    }
    setPins((prev) => [...prev, optimistic])
    setInput('')
    setError(null)
    setRedactedNotice(null)
    try {
      const real = await pinnedAdd(content, scope, scope === 'project' ? projectId : undefined)
      setPins((prev) => prev.map((p) => (p.id === optimisticId ? real : p)))
      if (real.content !== content) {
        setRedactedNotice('系统检测到敏感信息（如 API key / 邮箱），已自动脱敏后再保存。')
        window.setTimeout(() => setRedactedNotice(null), 6000)
      }
    } catch (e) {
      setPins((prev) => prev.filter((p) => p.id !== optimisticId))
      setError(String(e))
    }
  }

  const handleDelete = async (id: string) => {
    const previous = pins
    setPins((prev) => prev.filter((p) => p.id !== id))
    try {
      await pinnedDelete(id)
    } catch (e) {
      setPins(previous)
      setError(String(e))
    }
  }

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event
    if (!over || active.id === over.id) return
    const oldIndex = pins.findIndex((p) => p.id === active.id)
    const newIndex = pins.findIndex((p) => p.id === over.id)
    if (oldIndex < 0 || newIndex < 0) return
    const reordered = arrayMove(pins, oldIndex, newIndex)
    setPins(reordered)
    try {
      await pinnedReorder(reordered.map((p) => p.id))
    } catch (e) {
      void refresh()
      setError(String(e))
    }
  }

  return (
    <section className="rounded-lg border border-amber-200/60 bg-amber-50/30 p-4 dark:border-amber-900/40 dark:bg-amber-950/20">
      <header className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Pin className="h-4 w-4 text-amber-600" />
          <h3 className="text-sm font-semibold">置顶记忆 / Pinned Memory</h3>
          <span
            className="text-xs text-muted-foreground"
            title="最多 50 条，每条 ≤ 500 字，自动注入 system prompt"
          >
            ({pins.length}/{MAX_PINS})
          </span>
        </div>
        <div className="flex gap-1 rounded-md bg-background/60 p-0.5 text-xs">
          <button
            type="button"
            disabled={!projectId}
            className={`rounded px-2 py-1 ${
              scope === 'project' ? 'bg-amber-200 dark:bg-amber-800' : ''
            } disabled:cursor-not-allowed disabled:opacity-40`}
            onClick={() => setScope('project')}
            title={projectId ? '项目范围' : '当前未选择项目；请先在下方选择'}
          >
            当前项目
          </button>
          <button
            type="button"
            className={`rounded px-2 py-1 ${
              scope === 'global' ? 'bg-amber-200 dark:bg-amber-800' : ''
            }`}
            onClick={() => setScope('global')}
          >
            全局
          </button>
        </div>
      </header>

      {/* Project picker — only shown in 'project' tab. Settings is a separate
          window without App.tsx state, so we read localStorage + listProjects
          to give the user an explicit dropdown. */}
      {scope === 'project' && (
        <div className="mb-3 flex items-center gap-2 rounded-md border border-amber-200/40 bg-background/40 p-2 text-xs">
          <Folder className="h-3.5 w-3.5 text-amber-700/70" />
          <span className="shrink-0 text-muted-foreground">项目:</span>
          {projects.length === 0 ? (
            <span className="italic text-muted-foreground">
              暂无项目 — 请在主界面创建一个项目后再来 pin 项目级记忆，或切到「全局」。
            </span>
          ) : (
            <select
              value={pickedProjectId ?? ''}
              onChange={(e) => {
                const id = e.target.value || undefined
                setPickedProjectId(id)
                try {
                  if (id) localStorage.setItem('lastActiveProjectId', id)
                } catch {
                  /* ignore */
                }
              }}
              disabled={!!propProjectId} // upstream-controlled when given as prop
              className="flex-1 rounded-md border border-black/10 bg-background px-2 py-1 text-xs disabled:opacity-60"
            >
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} ({p.id.slice(0, 8)})
                </option>
              ))}
            </select>
          )}
          {activeProject && (
            <span className="shrink-0 text-[10px] text-muted-foreground">
              ✅ {activeProject.name}
            </span>
          )}
        </div>
      )}

      {redactedNotice && (
        <div className="mb-2 flex items-center gap-2 rounded bg-yellow-50 p-2 text-xs text-yellow-900 dark:bg-yellow-950/30 dark:text-yellow-200">
          <Shield className="h-3 w-3 shrink-0" />
          <span>{redactedNotice}</span>
        </div>
      )}

      {error && (
        <div className="mb-2 flex items-center gap-2 rounded bg-red-50 p-2 text-xs text-red-900 dark:bg-red-950/30 dark:text-red-200">
          <AlertCircle className="h-3 w-3 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={pins.map((p) => p.id)} strategy={verticalListSortingStrategy}>
          <ul className="mb-3 space-y-1.5">
            {pins.length === 0 && !loading && (
              <li className="px-2 py-3 text-center text-sm italic text-muted-foreground">
                还没有置顶记忆。在下方输入框添加第一条。
              </li>
            )}
            {pins.map((pin) => (
              <PinItem key={pin.id} pin={pin} onDelete={handleDelete} />
            ))}
          </ul>
        </SortableContext>
      </DndContext>

      <div className="flex gap-2">
        <input
          type="text"
          className="flex-1 rounded border bg-background px-2 py-1.5 text-sm"
          placeholder="例如：我叫 Ryan，使用 Rust + Tauri"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault()
              void handleAdd()
            }
          }}
          disabled={atCapacity}
          aria-label="新增置顶记忆"
        />
        <span
          className={`self-center font-mono text-xs tabular-nums ${overLimit ? 'font-bold text-red-600' : 'text-muted-foreground'}`}
          aria-live="polite"
        >
          {remaining}
        </span>
        <button
          type="button"
          onClick={() => void handleAdd()}
          disabled={!input.trim() || overLimit || atCapacity}
          className="rounded bg-amber-600 px-3 py-1.5 text-sm font-medium text-white transition-opacity disabled:opacity-50"
        >
          添加
        </button>
      </div>
      {atCapacity && (
        <p className="mt-2 text-xs text-amber-700 dark:text-amber-400">
          已达 {MAX_PINS} 条上限。删除一些以添加新的。
        </p>
      )}
    </section>
  )
}
