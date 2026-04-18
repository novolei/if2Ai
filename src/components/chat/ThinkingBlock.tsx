/**
 * ThinkingBlock — Collapsible display of assistant chain-of-thought.
 *
 * Extracted from `src/components/ui/chat-ui.tsx` as part of the FE-G
 * God-Component split.  This is the canonical implementation; chat-ui.tsx
 * will import from here once the migration is complete.
 *
 * Two variants:
 *   - `ThinkingBlock` — full collapsible block with the thinking text
 *   - `ThinkingSummaryNode` — compact non-expandable summary for non-primary messages
 */

import React from 'react'
import { ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Format a millisecond duration as a human-readable string. */
function formatDuration(value: number): string {
  if (value < 1000) return `${value}ms`
  return `${(value / 1000).toFixed(1)}s`
}

/** Summarise long thinking text to a single-line preview. */
function summarizeThinkingText(thinking: string): string {
  const lines = thinking.split('\n').filter((l) => l.trim().length > 0)
  const first = lines[0]?.trim()
  if (!first) return '已完成思考'
  const normalized = first.replace(/\s+/g, ' ')
  return normalized.length > 44 ? normalized.slice(0, 44) + '…' : normalized
}

// ── ThinkingBlock ─────────────────────────────────────────────────────────────

export interface ThinkingBlockProps {
  /** The raw chain-of-thought text from the model. */
  thinking: string
  /** Optional duration in milliseconds shown alongside the header. */
  thinkingTime?: number
  /** Whether the block starts expanded. Defaults to false. */
  defaultOpen?: boolean
}

/**
 * Collapsible block that renders the model's chain-of-thought.
 * Used for the primary (latest) assistant message in a turn.
 */
export function ThinkingBlock({
  thinking,
  thinkingTime,
  defaultOpen = false,
}: ThinkingBlockProps) {
  const [open, setOpen] = React.useState(defaultOpen)

  React.useEffect(() => {
    setOpen(defaultOpen)
  }, [defaultOpen, thinking])

  const durationLabel = thinkingTime ? formatDuration(thinkingTime) : '—'

  return (
    <div className="relative mb-3 pl-4">
      <div className="absolute bottom-0 left-[5px] top-0 w-px bg-black/14" />
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className="flex items-center gap-1.5 rounded-none px-0 py-[2px] text-[12px] font-light italic tracking-tight text-black/30 transition-colors hover:text-black/46"
        aria-expanded={open}
        aria-label={open ? '折叠思考过程' : '展开思考过程'}
      >
        <span className="h-1.5 w-1.5 rounded-full bg-black/30" />
        <span>已完成思考</span>
        <span className="text-black/26 not-italic"> {durationLabel}</span>
        <ChevronDown
          className={cn('ml-0.5 h-3 w-3 transition-transform', open && 'rotate-180')}
          aria-hidden
        />
      </button>

      <div
        className={cn(
          'overflow-hidden transition-[max-height,opacity] duration-200',
          open ? 'max-h-[360px] opacity-100' : 'max-h-0 opacity-0',
        )}
        aria-hidden={!open}
      >
        <div className="ml-[10px] border-l border-black/8 pl-3 pt-1">
          <div className="whitespace-pre-wrap px-1 text-[11.5px] font-light italic leading-6 text-black/44">
            {thinking}
          </div>
        </div>
      </div>
    </div>
  )
}

// ── ThinkingSummaryNode ───────────────────────────────────────────────────────

export interface ThinkingSummaryNodeProps {
  /** The raw chain-of-thought text — summarised to a single line. */
  thinking: string
  /** Optional duration in milliseconds. */
  thinkingTime?: number
}

/**
 * Non-expandable compact node showing a one-line thinking summary.
 * Used for earlier assistant messages in the conversation where full
 * thinking text is less relevant.
 */
export function ThinkingSummaryNode({ thinking, thinkingTime }: ThinkingSummaryNodeProps) {
  const durationLabel = thinkingTime ? formatDuration(thinkingTime) : ''
  const summary = summarizeThinkingText(thinking)
  const label = durationLabel ? `${summary} · ${durationLabel}` : summary

  return (
    <div className="relative mb-3 pl-4">
      <div className="absolute bottom-0 left-[5px] top-0 w-px bg-black/14" />
      <div className="flex items-center gap-1.5 rounded-none px-0 py-[2px] text-[12px] italic tracking-tight text-black/28">
        <span className="h-1.5 w-1.5 rounded-full bg-black/28" />
        <span className="truncate">{label}</span>
      </div>
    </div>
  )
}
