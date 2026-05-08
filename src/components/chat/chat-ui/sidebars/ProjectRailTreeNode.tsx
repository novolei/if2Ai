/**
 * GF-01 PR-07 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Recursive single tree-node row used inside the project-files rail.
 * Folder rows toggle expansion on click; file rows open on
 * double-click / Enter / Space. Drag-out publishes a JSON payload on the
 * `application/x-if2ai-rail-entry` mime so composer drop-zones can pick
 * it up. Render-equivalent move from the original inline definition —
 * memo wrapper, key handling and recursion preserved verbatim.
 */

import * as React from "react"
import { ChevronRight, File, Folder, FolderOpen, LoaderCircle } from "lucide-react"
import { cn } from "@/lib/utils"
import type { DirectoryEntryPreview } from "@/lib/tauri"

/** Mirrors the `RailTreeMap` alias in `chat-ui.tsx`. */
type RailTreeMap = Record<string, DirectoryEntryPreview[]>

/**
 * Render one tree-node row in the project-files rail. Recursively
 * renders its own children when expanded; an empty expanded folder
 * shows a localized "这个文件夹目前是空的" stub instead of vanishing.
 */
export const ProjectRailTreeNode = React.memo(function ProjectRailTreeNode({
  entry,
  level,
  selectedPath,
  childEntries,
  expandedPaths,
  loadingPaths,
  onOpen,
  onToggleFolder,
}: {
  entry: DirectoryEntryPreview
  level: number
  selectedPath: string | null
  childEntries: RailTreeMap
  expandedPaths: string[]
  loadingPaths: string[]
  onOpen: (entry: DirectoryEntryPreview) => void | Promise<void>
  onToggleFolder: (entry: DirectoryEntryPreview) => void
}) {
  const isFolder = entry.kind === 'folder'
  const isExpanded = expandedPaths.includes(entry.path)
  const isLoading = loadingPaths.includes(entry.path)
  const children = childEntries[entry.path] ?? []
  const Icon = isFolder ? (isExpanded ? FolderOpen : Folder) : File
  const isSelected = selectedPath === entry.path

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        draggable
        onClick={() => {
          if (entry.kind === 'folder') {
            void onOpen(entry)
          }
        }}
        onDoubleClick={() => {
          if (entry.kind === 'file') {
            void onOpen(entry)
          }
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            void onOpen(entry)
          }
        }}
        onDragStart={(event) => {
          const payload = JSON.stringify({
            path: entry.path,
            name: entry.name,
            kind: entry.kind,
          })
          event.dataTransfer.setData('application/x-if2ai-rail-entry', payload)
          event.dataTransfer.setData('text/plain', entry.path)
          event.dataTransfer.effectAllowed = 'copy'
        }}
        className={cn(
          'group flex w-full cursor-pointer items-center gap-2 rounded-[6px] px-0.5 py-1 text-left text-foreground/80 transition-all duration-150 hover:bg-muted',
          isSelected && 'bg-accent',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
        )}
        style={{ paddingLeft: `${2 + level * 12}px` }}
      >
        <div className="flex h-4 w-4 shrink-0 items-center justify-center text-muted-foreground/60">
          {isFolder ? (
            <span className="inline-flex h-4 w-4 items-center justify-center text-muted-foreground/60">
              <ChevronRight className={cn('h-3.5 w-3.5 transition-transform duration-150', isExpanded && 'rotate-90')} />
            </span>
          ) : (
            <span className="h-3.5 w-3.5" />
          )}
        </div>
        <div className="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground transition-transform duration-150 group-hover:-translate-y-0.5">
          <Icon className="h-[15px] w-[15px] stroke-[1.65]" />
        </div>
        <div className="min-w-0 flex-1 truncate text-[12px] font-[380] tracking-[-0.01em] text-foreground/75 [font-feature-settings:'ss01'_1,'cv01'_1]">
          {entry.name}
        </div>
        {isLoading ? (
          <LoaderCircle className="h-3.5 w-3.5 shrink-0 animate-spin text-muted-foreground/60" />
        ) : isFolder ? (
          <div className="text-[9.5px] uppercase tracking-[0.16em] text-muted-foreground/40">{children.length > 0 ? '' : ''}</div>
        ) : (
          <ChevronRight className="h-3.5 w-3.5 shrink-0 text-muted-foreground/40 opacity-0 transition-all duration-150 group-hover:translate-x-0.5 group-hover:opacity-100" />
        )}
      </div>

      {isFolder && isExpanded ? (
        <div>
          <div className="space-y-0 pt-0.5">
            {children.length > 0 ? (
              children.map((child) => (
                <ProjectRailTreeNode
                  key={child.path}
                  entry={child}
                  level={level + 1}
                  selectedPath={selectedPath}
                  childEntries={childEntries}
                  expandedPaths={expandedPaths}
                  loadingPaths={loadingPaths}
                  onOpen={onOpen}
                  onToggleFolder={onToggleFolder}
                />
              ))
            ) : !isLoading ? (
              <div className="px-2 py-1 text-[10px] italic text-muted-foreground/50" style={{ paddingLeft: `${16 + level * 12}px` }}>
                这个文件夹目前是空的
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  )
})
