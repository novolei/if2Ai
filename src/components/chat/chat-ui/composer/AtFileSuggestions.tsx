/**
 * GF-01 PR-06 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * File / folder picker overlay triggered by typing `@query` in the
 * composer. Supports nested folder navigation: Enter / → enters a
 * folder, ← / back button goes up. Render-equivalent move from the
 * legacy inline definition — header, breadcrumb logic, entry rows
 * and footer hint are preserved verbatim.
 */

import * as React from "react"
import { ChevronRight, FileText, Folder } from "lucide-react"
import { cn } from "@/lib/utils"
import type { DirectoryEntryPreview } from "@/lib/tauri"

/** Props for {@link AtFileSuggestions} — copied from the legacy inline declaration. */
export interface AtFileSuggestionsProps {
  /** File / folder entries to render in the popover. */
  entries: DirectoryEntryPreview[]
  /** Index of the currently highlighted row. */
  selectedIndex: number
  /** Called when the user picks a (file) entry — folders use {@link onNavigateInto}. */
  onSelect: (entry: DirectoryEntryPreview) => void
  /** Called when the user navigates into a folder (Enter / → / click). */
  onNavigateInto?: (entry: DirectoryEntryPreview) => void
  /** Called when the user presses back (← / back button) to leave a subfolder. */
  onNavigateUp?: () => void
  /** Current `@`-prefixed query string. */
  query?: string
  /** Currently browsed sub-folder path (null at workspace root). */
  browsePath?: string | null
  /** Breadcrumb chain rendered in the header (last 2 ancestors shown). */
  breadcrumbs?: Array<{ name: string; path: string | null }>
}

/**
 * Render the `@`-mention file / folder picker popover above the composer.
 */
export function AtFileSuggestions({
  entries,
  selectedIndex,
  onSelect,
  onNavigateInto,
  onNavigateUp,
  query,
  browsePath,
  breadcrumbs,
}: AtFileSuggestionsProps) {
  const isInSubfolder = Boolean(browsePath)
  const currentDirName = browsePath ? browsePath.split('/').filter(Boolean).pop() ?? browsePath : null

  return (
    <div className="absolute inset-x-0 bottom-full z-50 mb-2.5 pointer-events-none">
      <div
        className="pointer-events-auto w-full overflow-hidden rounded-2xl"
        style={{
          background: 'color-mix(in srgb, hsl(var(--popover)) 96%, transparent)',
          backdropFilter: 'blur(24px) saturate(180%)',
          WebkitBackdropFilter: 'blur(24px) saturate(180%)',
          border: '1px solid color-mix(in srgb, hsl(var(--border)) 75%, transparent)',
          boxShadow:
            '0 4px 6px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.09), 0 20px 48px rgba(0,0,0,0.06), 0 0 0 0.5px rgba(0,0,0,0.05)',
        }}
      >
        {/* ── Header ─────────────────────────────────────────────────────── */}
        <div className="flex items-center gap-2 border-b border-border/65 px-3 py-2">
          {/* Back button (shown when inside a subfolder) */}
          {isInSubfolder && onNavigateUp && (
            <button
              type="button"
              onClick={() => onNavigateUp()}
              className="flex size-[22px] shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
              title="返回上层 (←)"
            >
              <ChevronRight className="size-3.5 rotate-180" />
            </button>
          )}

          {/* @ badge */}
          <div className="flex size-5 shrink-0 items-center justify-center rounded-md bg-jade/10 text-[11px] font-bold text-jade">
            @
          </div>

          {/* Breadcrumb path */}
          <div className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden">
            {isInSubfolder && breadcrumbs && breadcrumbs.length > 0 ? (
              <>
                {/* Ancestors (truncated to last 2) */}
                {breadcrumbs.slice(-2).map((crumb, idx, arr) => (
                  <React.Fragment key={crumb.name + idx}>
                    <span className="shrink-0 text-[11px] font-medium text-muted-foreground">{crumb.name}</span>
                    {idx < arr.length - 1 || currentDirName ? (
                      <ChevronRight className="size-2.5 shrink-0 text-muted-foreground/55" />
                    ) : null}
                  </React.Fragment>
                ))}
                {/* Current directory — highlighted */}
                <span className="truncate text-[12px] font-semibold tracking-tight text-jade">
                  {currentDirName}
                </span>
              </>
            ) : (
              /* Root level: show @query */
              <span className="truncate font-mono text-[13px] font-semibold tracking-tight text-jade">
                {query ? `@${query}` : '@'}
              </span>
            )}
          </div>

          {/* Hint */}
          <span className="ml-auto shrink-0 text-[10px] font-medium text-muted-foreground">
            {isInSubfolder ? '← 返回  ↵ 选中  → 进入' : '↑↓ 选择  ↵/→ 进入  Tab 选中'}
          </span>
        </div>

        {/* ── Entry rows ─────────────────────────────────────────────────── */}
        <div className="py-1">
          {entries.map((entry, i) => {
            const isFolder = entry.kind === 'folder'
            const isSelected = i === selectedIndex
            return (
              <button
                key={entry.path}
                type="button"
                className={cn(
                  'flex w-full cursor-pointer items-center gap-3 px-3 py-[7px] text-left transition-colors',
                  isSelected
                    ? 'bg-jade/[0.07] text-jade'
                    : 'text-popover-foreground/70 hover:bg-accent hover:text-accent-foreground'
                )}
                onClick={() => {
                  if (isFolder && onNavigateInto) {
                    onNavigateInto(entry)
                  } else {
                    onSelect(entry)
                  }
                }}
              >
                {/* Kind badge */}
                <div
                  className={cn(
                    'flex size-[22px] shrink-0 items-center justify-center rounded-[6px] transition-colors',
                    isSelected
                      ? isFolder
                        ? 'bg-jade/[0.14] text-jade'
                        : 'bg-jade/[0.10] text-jade'
                      : isFolder
                        ? 'bg-muted text-muted-foreground'
                        : 'bg-muted text-muted-foreground/80'
                  )}
                >
                  {isFolder ? <Folder className="size-3" /> : <FileText className="size-3" />}
                </div>

                {/* Name */}
                <span className="min-w-0 flex-1 truncate text-[12.5px] font-medium leading-tight">
                  {entry.name}
                </span>

                {/* Right side: chevron for folders (navigable), or ↵ for files */}
                {isFolder ? (
                  <ChevronRight
                    className={cn(
                      'size-3 shrink-0 transition-colors',
                      isSelected ? 'text-jade/60' : 'text-muted-foreground/55'
                    )}
                  />
                ) : isSelected ? (
                  <kbd className="shrink-0 rounded-md border border-border/70 bg-muted px-1.5 py-[2px] font-sans text-[10px] font-medium text-muted-foreground">
                    ↵
                  </kbd>
                ) : null}
              </button>
            )
          })}
        </div>

        {/* ── Footer hint (browse mode only) ─────────────────────────────── */}
        {isInSubfolder && (
          <div className="border-t border-border/55 px-4 py-1.5">
            <span className="text-[10px] text-muted-foreground">
              Tab 选中当前目录作为引用
            </span>
          </div>
        )}
      </div>
    </div>
  )
}
