/**
 * GF-01 PR-06 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Slash command suggestion overlay anchored to the top edge of the
 * composer. Render-equivalent move — header, suggestion rows, ↵ hint
 * and selected-row styling are preserved verbatim from the legacy
 * inline definition.
 */

import * as React from "react"
import { cn } from "@/lib/utils"

/** Props for {@link SlashCommandSuggestions} — copied from the legacy inline declaration. */
export interface SlashCommandSuggestionsProps {
  /** List of slash command strings (each starting with `/`) to render. */
  suggestions: string[]
  /** Index of the currently highlighted suggestion. */
  selectedIndex: number
  /** Called when the user clicks a suggestion. */
  onSelect: (value: string) => void
  /** Raw input echoed in the header label (`/foo`); falls back to `/`. */
  rawInput?: string
}

/**
 * Render the slash-command suggestion popover above the composer.
 */
export function SlashCommandSuggestions({
  suggestions,
  selectedIndex,
  onSelect,
  rawInput,
}: SlashCommandSuggestionsProps) {
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
        {/* Header bar */}
        <div className="flex items-center justify-between border-b border-border/65 px-4 py-2.5">
          <div className="flex items-center gap-2">
            <div className="flex size-5 shrink-0 items-center justify-center rounded-md bg-jade/10 text-[11px] font-bold text-jade">
              /
            </div>
            <span className="font-mono text-[13px] font-semibold tracking-tight text-jade">
              {rawInput ?? '/'}
            </span>
          </div>
          <span className="text-[10.5px] font-medium tracking-wide text-muted-foreground">Tab 补全</span>
        </div>

        {/* Suggestion rows */}
        <div className="py-1">
          {suggestions.map((cmd, i) => (
            <button
              key={cmd}
              type="button"
              className={cn(
                'flex w-full cursor-pointer items-center gap-3 px-4 py-[7px] text-left transition-colors',
                i === selectedIndex
                  ? 'bg-jade/[0.07] text-jade'
                  : 'text-popover-foreground/70 hover:bg-accent hover:text-accent-foreground'
              )}
              onClick={() => onSelect(cmd)}
            >
              {/* Icon badge */}
              <div
                className={cn(
                  'flex size-[22px] shrink-0 items-center justify-center rounded-[6px] text-[11px] font-bold transition-colors',
                  i === selectedIndex ? 'bg-jade/[0.14] text-jade' : 'bg-muted text-muted-foreground'
                )}
              >
                /
              </div>

              {/* Command name */}
              <span className="flex-1 truncate font-mono text-[12.5px] font-medium">
                {cmd.slice(1)}
              </span>

              {/* Enter hint for selected */}
              {i === selectedIndex && (
                <kbd className="shrink-0 rounded-md border border-border/70 bg-muted px-1.5 py-[2px] font-sans text-[10px] font-medium text-muted-foreground">
                  ↵
                </kbd>
              )}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
