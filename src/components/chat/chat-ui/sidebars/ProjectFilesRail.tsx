/**
 * GF-01 PR-07 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Right-edge project-files rail: drag-resizable width handle (pointer-
 * capture based), header chrome (folder open / skills chip), breadcrumb
 * row + sort toggle, and the recursive tree-node list below.
 *
 * Render-equivalent move from the original inline definition. The
 * pointer-capture resize closure, memo wrapper, breadcrumb logic and
 * width clamp (`PROJECT_RAIL_MIN_WIDTH`/`PROJECT_RAIL_MAX_WIDTH`) are
 * preserved verbatim so the drag interaction cannot regress.
 */

import * as React from "react"
import {
  ArrowUpDown,
  ChevronRight,
  Clock3,
  Folder,
  GripVertical,
  LoaderCircle,
  Sparkles,
} from "lucide-react"
import { cn } from "@/lib/utils"
import type { DirectoryEntryPreview } from "@/lib/tauri"
import { ProjectRailTreeNode } from "./ProjectRailTreeNode"
import { PROJECT_RAIL_MAX_WIDTH, PROJECT_RAIL_MIN_WIDTH } from "./utils"

/** Mirrors the `RailBreadcrumb` alias in `chat-ui.tsx`. */
type RailBreadcrumb = { label: string; path: string }
/** Mirrors the `RailTreeMap` alias in `chat-ui.tsx`. */
type RailTreeMap = Record<string, DirectoryEntryPreview[]>

/**
 * The collapsible right-side files rail. When `open` is false the
 * rail slides out (translate + opacity) and reports `width: 0` so its
 * inline grid track collapses cleanly. Drag handle on the left edge
 * adjusts `width` between `PROJECT_RAIL_MIN_WIDTH` and
 * `PROJECT_RAIL_MAX_WIDTH` via `onWidthChange`.
 */
export const ProjectFilesRail = React.memo(function ProjectFilesRail({
  open,
  width,
  title,
  projectLabel: _projectLabel,
  breadcrumbs,
  currentPath,
  entries,
  childEntries,
  expandedPaths,
  loadingPaths,
  isLoading,
  sortMode,
  onSortModeChange,
  previewError,
  onWidthChange,
  onNavigateUp,
  onJumpToBreadcrumb,
  onOpenEntry,
  onToggleFolder,
  onOpenFolder,
}: {
  open: boolean
  width: number
  title: string
  projectLabel: string
  breadcrumbs: RailBreadcrumb[]
  currentPath: string | null
  entries: DirectoryEntryPreview[]
  childEntries: RailTreeMap
  expandedPaths: string[]
  loadingPaths: string[]
  isLoading: boolean
  sortMode: 'recent' | 'name'
  onSortModeChange: React.Dispatch<React.SetStateAction<'recent' | 'name'>>
  previewError: string | null
  onWidthChange: React.Dispatch<React.SetStateAction<number>>
  onNavigateUp: () => void
  onJumpToBreadcrumb: (path: string) => void
  onOpenEntry: (entry: DirectoryEntryPreview) => void | Promise<void>
  onToggleFolder: (entry: DirectoryEntryPreview) => void
  onOpenFolder: () => void
}) {
  const startResize = React.useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    event.preventDefault()
    const startX = event.clientX
    const startWidth = width
    const separatorEl = event.currentTarget
    const pointerId = event.pointerId

    const handleMove = (moveEvent: PointerEvent) => {
      const delta = startX - moveEvent.clientX
      onWidthChange(Math.min(PROJECT_RAIL_MAX_WIDTH, Math.max(PROJECT_RAIL_MIN_WIDTH, startWidth + delta)))
    }

    const handleUp = () => {
      document.body.style.userSelect = ''
      document.body.style.cursor = ''
      window.removeEventListener('pointermove', handleMove)
      window.removeEventListener('pointerup', handleUp)
      try {
        separatorEl.releasePointerCapture(pointerId)
      } catch {
        // ignore pointer capture release failures
      }
    }

    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
    try {
      separatorEl.setPointerCapture(pointerId)
    } catch {
      // ignore pointer capture failures
    }
    window.addEventListener('pointermove', handleMove)
    window.addEventListener('pointerup', handleUp)
  }, [onWidthChange, width])

  return (
    <aside
      className={cn(
        'pointer-events-none absolute inset-y-2.5 right-2.5 z-20 overflow-hidden origin-right transition-[width,transform,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]',
        open ? 'translate-x-0 opacity-100' : 'translate-x-8 opacity-0'
      )}
      style={{ width: open ? `${width}px` : '0px' }}
      aria-hidden={!open}
    >
      <div
        className="pointer-events-auto absolute inset-y-5 -left-3 flex w-6 cursor-col-resize items-center justify-center"
        onPointerDown={startResize}
      >
        <div className="flex h-16 w-3 items-center justify-center rounded-full bg-surface-raised/82 shadow-[0_8px_20px_rgba(78,61,38,0.08)] backdrop-blur-sm">
          <GripVertical className="h-4 w-4 text-muted-foreground" />
        </div>
      </div>
      <div className="pointer-events-auto flex h-full flex-col justify-start">
        <div className="h-3 shrink-0" />
        <div className="min-h-0 flex-1 px-2 pb-2">
          <div className="flex h-full min-h-0 flex-col rounded-[18px] border border-border bg-surface shadow-xs">
            <div className="flex items-start justify-between gap-3 px-4 pt-4">
              <div className="min-w-0 flex-1 pr-2">
                <div
                  className="truncate text-[18px] font-medium tracking-[-0.03em] text-foreground"
                  style={{ fontFamily: '"Iowan Old Style", "Baskerville", ui-serif, Georgia, serif' }}
                >
                  {title}
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1 pt-0.5">
                <button
                  type="button"
                  onClick={onOpenFolder}
                  className="inline-flex h-7 items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.25 text-[10px] font-medium text-muted-foreground transition-all duration-200 hover:border-border hover:bg-accent hover:text-foreground"
                >
                  <Folder className="h-3.25 w-3.25 stroke-[1.85]" />
                  <span>打开文件夹</span>
                </button>
                <button
                  type="button"
                  className="inline-flex h-7 items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.25 text-[10px] font-medium text-muted-foreground transition-all duration-200 hover:border-border hover:bg-accent hover:text-foreground"
                >
                  <Sparkles className="h-3.25 w-3.25" />
                  <span>项目技能 · 2</span>
                </button>
              </div>
            </div>

            <div className="mt-2 flex items-center justify-between border-b border-border/60 px-4 pb-3">
              <div className="flex items-center gap-2">
                <div className="text-[11px] font-semibold tracking-[-0.01em] text-muted-foreground">技能</div>
                <div className="inline-flex min-w-7 items-center justify-center rounded-full bg-muted px-2 py-0.5 text-[10px] text-muted-foreground">
                  {entries.length}
                </div>
              </div>
              <button
                type="button"
                className="inline-flex h-6 w-6 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                <ChevronRight className="h-3.5 w-3.5" />
              </button>
            </div>

            <div className="flex min-h-0 flex-1 flex-col px-3 pb-3 pt-3">
              <div className="flex items-center justify-between gap-3 px-0.5">
                {breadcrumbs.length > 1 ? (
                  <div className="flex min-w-0 items-center gap-1.5 overflow-hidden rounded-[6px] bg-surface-raised px-2.5 py-1 text-[10.5px] text-muted-foreground">
                    <>
                      <button
                        type="button"
                        onClick={onNavigateUp}
                        className="inline-flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground/60 transition-colors hover:bg-muted hover:text-foreground"
                      >
                        <ChevronRight className="h-3 w-3 rotate-180" />
                      </button>
                      <div className="flex min-w-0 items-center overflow-hidden">
                        {breadcrumbs.map((crumb, index) => (
                          <React.Fragment key={crumb.path}>
                            {index > 0 ? <ChevronRight className="h-3 w-3 shrink-0 text-muted-foreground/60" /> : null}
                            <button
                              type="button"
                              onClick={() => onJumpToBreadcrumb(crumb.path)}
                              className={cn(
                                'min-w-0 shrink truncate rounded px-0.5 py-0.5 transition-colors hover:text-foreground',
                                index === breadcrumbs.length - 1 ? 'font-medium text-foreground' : 'text-muted-foreground'
                              )}
                            >
                              {crumb.label}
                            </button>
                          </React.Fragment>
                        ))}
                      </div>
                    </>
                  </div>
                ) : (
                  <div className="min-w-0 flex-1" />
                )}
                <button
                  type="button"
                  onClick={() => onSortModeChange((current) => current === 'recent' ? 'name' : 'recent')}
                  className="inline-flex shrink-0 items-center gap-1 rounded-full px-1.5 py-1 text-[11px] text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground"
                >
                  {sortMode === 'recent' ? (
                    <Clock3 className="h-3.25 w-3.25" />
                  ) : (
                    <ArrowUpDown className="h-3.25 w-3.25" />
                  )}
                  <span>{sortMode === 'recent' ? '时间' : '名称'}</span>
                </button>
              </div>

              <div className="mt-2 min-h-0 flex-1 overflow-hidden bg-transparent">
                {previewError ? (
                  <div className="mb-2 rounded-[6px] border border-border bg-muted px-3 py-2 text-[11px] text-muted-foreground">
                    {previewError}
                  </div>
                ) : null}
                {isLoading ? (
                  <div className="flex items-center gap-2 px-1 py-3 text-[11.5px] text-muted-foreground">
                    <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
                    <span>正在整理当前项目文件…</span>
                  </div>
                ) : entries.length === 0 ? (
                  <div className="px-1 py-5 text-center text-[11.5px] text-muted-foreground">
                    当前目录里还没有可显示的文件。
                  </div>
                ) : (
                  <div className="h-full min-h-0 overflow-y-auto px-0 py-0 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                    <div className="space-y-0 pb-1">
                      {entries.map((entry) => (
                        <ProjectRailTreeNode
                          key={entry.path}
                          entry={entry}
                          level={0}
                          selectedPath={currentPath}
                          childEntries={childEntries}
                          expandedPaths={expandedPaths}
                          loadingPaths={loadingPaths}
                          onOpen={onOpenEntry}
                          onToggleFolder={onToggleFolder}
                        />
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
    </aside>
  )
})
