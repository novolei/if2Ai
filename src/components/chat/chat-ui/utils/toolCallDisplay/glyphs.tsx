/**
 * Tool-card glyphs + status helpers + inline summary renderer. Extracted
 * from `src/components/ui/chat-ui.tsx` (GF-01 PR-02) without behavioural
 * change.
 */

import * as React from 'react'
import {
  Bot,
  Check,
  FileText,
  FolderOpen,
  Globe,
  LoaderCircle,
  Search,
  Sparkles,
  TerminalSquare,
  Wrench,
  X,
} from 'lucide-react'

import type { Message } from '@/components/ui/chat-ui'
import {
  isToolPendingStatus,
  pickToolString,
  type ChatToolStatus,
} from '@/components/chat/chat-ui/utils/toolCallDisplay/builders'

/**
 * Map an assistant message into chip-container colour class metadata.
 * Pure; consumed by `<ChatMessage>` via spreading into the badge `<span>`.
 */
export function getAssistantStatusMeta(message: Message) {
  const kind = message.statusKind
    ?? (message.taskOutcome === 'partial_success'
      ? 'partial'
      : message.taskOutcome === 'failed'
        ? 'failed'
        : message.taskOutcome === 'completed'
          ? 'success'
          : 'info')
  if (kind === 'success') {
    return {
      containerClass: 'border-emerald-500/35 bg-emerald-500/12 text-emerald-500',
      dotClass: 'bg-emerald-500',
    }
  }
  if (kind === 'partial') {
    return {
      containerClass: 'border-amber-500/35 bg-amber-500/12 text-amber-500',
      dotClass: 'bg-amber-500',
    }
  }
  if (kind === 'failed') {
    return {
      containerClass: 'border-rose-500/35 bg-rose-500/12 text-rose-500',
      dotClass: 'bg-rose-500',
    }
  }
  return {
    containerClass: 'border-sky-500/35 bg-sky-500/12 text-sky-500',
    dotClass: 'bg-sky-500',
  }
}

/**
 * Visual glyph rendered next to a tool name, communicating
 * pending / completed / failed states.
 */
export function ToolStatusGlyph({
  status,
}: {
  status: ChatToolStatus
}) {
  if (isToolPendingStatus(status)) {
    return (
      <span className="relative flex h-3 w-3 shrink-0 items-center justify-center opacity-80">
        <span className="absolute inset-0 rounded-full bg-sky-400/10 animate-ping" />
        <LoaderCircle className="relative h-3 w-3 animate-spin text-sky-500/80" strokeWidth={2} />
      </span>
    )
  }

  if (status === 'completed') {
    return (
      <span className="inline-flex h-3 w-3 shrink-0 items-center justify-center rounded-full bg-emerald-500/88 text-white">
        <Check className="h-2.1 w-2.1" strokeWidth={3} />
      </span>
    )
  }

  return (
    <span className="inline-flex h-3 w-3 shrink-0 items-center justify-center rounded-full bg-rose-500/88 text-white">
      <X className="h-2.1 w-2.1" strokeWidth={3} />
    </span>
  )
}

/**
 * Pick the icon component to show for a tool call, based on tool name
 * and (when relevant) command-shaped args.
 */
export function getToolCallGlyph(
  toolName?: string,
  args?: Record<string, unknown>
): React.ComponentType<{ className?: string }> {
  const normalized = (toolName ?? '').toLowerCase()
  const maybeCommand = pickToolString(args ?? {}, ['command', 'cmd', 'shell'])

  if (normalized === 'skill' || normalized.includes('skill/') || normalized.includes('.skill')) {
    return Sparkles
  }
  if (normalized.includes('web_search')) {
    return Globe
  }
  if (normalized.includes('glob_search') || normalized.includes('grep_search') || normalized.includes('content_search') || normalized.includes('tool_search') || normalized.includes('skill_search')) {
    return Search
  }
  if (normalized.includes('read_file')) {
    return FileText
  }
  if (normalized.includes('file_write')) {
    return FolderOpen
  }
  if (maybeCommand || normalized.includes('command') || normalized.includes('exec') || normalized.includes('shell')) {
    return Wrench
  }
  if (normalized.includes('weather')) {
    return Globe
  }
  if (normalized.includes('agent') || normalized.includes('assistant')) {
    return Bot
  }
  return TerminalSquare
}

/**
 * Render a tool summary line (`label：value · label：value`) as a
 * compact inline pill row. Pure render helper; no side effects.
 */
export function renderInlineToolSummary(line: string) {
  const segments = line.split(' · ').filter(Boolean)

  return (
    <span className="inline-flex min-w-0 max-w-full items-center gap-1.5 truncate">
      {segments.map((segment, index) => {
        const separatorNeeded = index > 0
        const colonIndex = segment.indexOf('：')
        const label = colonIndex >= 0 ? segment.slice(0, colonIndex + 1) : ''
        const value = colonIndex >= 0 ? segment.slice(colonIndex + 1).trim() : segment

        return (
          <React.Fragment key={`${segment}-${index}`}>
            {separatorNeeded ? <span className="shrink-0 text-muted-foreground/45">·</span> : null}
            <span className="inline-flex min-w-0 items-center gap-1">
              {label ? <span className="shrink-0 text-muted-foreground/70">{label}</span> : null}
              <span className="min-w-0 truncate rounded-[3px] bg-muted px-1.5 py-[1px] font-mono italic text-foreground/70">
                {value}
              </span>
            </span>
          </React.Fragment>
        )
      })}
    </span>
  )
}
