/**
 * VirtualMessageList — Virtualised chat message renderer.
 *
 * Uses `@tanstack/react-virtual` to render only the visible messages,
 * solving the long-session rendering performance problem.
 *
 * ## Why virtualise?
 * Chat sessions can accumulate hundreds of messages, each potentially
 * containing heavy Markdown / code-block rendering.  Without
 * virtualisation React renders and lays out all DOM nodes even when
 * they are far out of the viewport, causing noticeable jank.
 *
 * ## Design
 * - Pinned to the bottom by default (auto-scrolls on new messages).
 * - Dynamic item sizing: each message reports its own height via a
 *   `measureElement` ref callback so the virtual list adjusts accurately.
 * - Scroll anchor: the list stays at the bottom as long as the user
 *   has not scrolled up manually.  Once they scroll up the auto-scroll
 *   is suspended; it resumes when they scroll back to the bottom.
 *
 * ## Usage
 * Intended as a drop-in replacement for the `<div className="flex flex-col gap-2">
 * {messages.map(...)}` pattern inside `ChatTranscript`.
 *
 * @see src/components/ui/chat-ui.tsx (ChatTranscript) — target for migration
 */

import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { cn } from '@/lib/utils'
import type { Message } from '@/modules/chat/types'

// ── Types ─────────────────────────────────────────────────────────────────────

/** Props for the {@link VirtualMessageList} component. */
export interface VirtualMessageListProps {
  /**
   * The messages to render.
   * The list will auto-scroll to the bottom when this array grows.
   */
  messages: Message[]
  /**
   * Render function for a single message item.
   * Receives the message and its index.
   */
  renderMessage: (message: Message, index: number) => React.ReactNode
  /**
   * Estimated height of a single message row in pixels.
   * A good estimate reduces layout shift on initial render.
   * Defaults to 72.
   */
  estimatedItemSize?: number
  /**
   * Number of items to render above and below the visible window.
   * Higher values reduce blank flashes while scrolling fast but increase
   * DOM node count.  Defaults to 10.
   */
  overscan?: number
  /**
   * Additional CSS class names for the scroll container.
   */
  className?: string
  /**
   * When provided, the virtualiser uses this external element as the scroll
   * container instead of mounting its own.  Required to integrate
   * VirtualMessageList inside an existing scrollable transcript without
   * introducing nested scroll regions.
   */
  scrollElementRef?: React.RefObject<HTMLDivElement | null>
}

// ── Constants ─────────────────────────────────────────────────────────────────

/** Threshold in pixels from the bottom that counts as "at bottom". */
const SCROLL_BOTTOM_THRESHOLD = 80

// ── useScrollAnchor ───────────────────────────────────────────────────────────

/**
 * Tracks whether the scroll container is "at the bottom" and provides a
 * `scrollToBottom` helper.  Auto-scroll is paused when the user scrolls up.
 */
function useScrollAnchor(scrollRef: React.RefObject<HTMLDivElement | null>) {
  const [atBottom, setAtBottom] = useState(true)

  const scrollToBottom = useCallback((behavior: ScrollBehavior = 'auto') => {
    const el = scrollRef.current
    if (!el) return
    el.scrollTo({ top: el.scrollHeight, behavior })
  }, [scrollRef])

  const handleScroll = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
    setAtBottom(distanceFromBottom <= SCROLL_BOTTOM_THRESHOLD)
  }, [scrollRef])

  return { atBottom, scrollToBottom, handleScroll }
}

// ── VirtualMessageList ────────────────────────────────────────────────────────

/**
 * Virtualised message list that renders only visible rows.
 * Automatically scrolls to the bottom on new messages while the user is
 * not scrolled up.
 */
export function VirtualMessageList({
  messages,
  renderMessage,
  estimatedItemSize = 72,
  overscan = 10,
  className,
  scrollElementRef,
}: VirtualMessageListProps) {
  const internalRef = useRef<HTMLDivElement>(null)
  const parentRef = scrollElementRef ?? internalRef
  const isExternalScroll = scrollElementRef !== undefined
  const { atBottom, scrollToBottom, handleScroll } = useScrollAnchor(parentRef)

  const rowVirtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => estimatedItemSize,
    overscan,
    measureElement:
      typeof window !== 'undefined' && !navigator.userAgent.includes('Firefox')
        ? (element) => element.getBoundingClientRect().height
        : undefined,
  })

  const virtualItems = rowVirtualizer.getVirtualItems()
  const totalSize = rowVirtualizer.getTotalSize()

  // Auto-scroll to bottom when new messages arrive (if already at bottom).
  // Skipped when the parent owns scroll behaviour to avoid fighting it.
  const prevMessageCount = useRef(messages.length)
  useEffect(() => {
    const grew = messages.length > prevMessageCount.current
    prevMessageCount.current = messages.length
    if (!isExternalScroll && grew && atBottom) {
      scrollToBottom('auto')
    }
  }, [messages.length, atBottom, scrollToBottom, isExternalScroll])

  // Scroll to bottom on mount (initial load) — only when we own the scroll
  // container. With an external scroll element the parent already manages it.
  useEffect(() => {
    if (isExternalScroll) return
    scrollToBottom('auto')
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // When integrated into an external scroll container we render only the
  // virtualised inner block — the outer container, scroll listener, and
  // "jump to latest" affordance are owned by the parent transcript.
  if (isExternalScroll) {
    return (
      <div
        role="log"
        aria-label="对话消息列表"
        aria-live="polite"
        style={{ height: totalSize, width: '100%', position: 'relative' }}
      >
        <div
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            width: '100%',
            transform: `translateY(${virtualItems[0]?.start ?? 0}px)`,
          }}
        >
          {virtualItems.map((virtualRow) => {
            const message = messages[virtualRow.index]
            if (!message) return null
            return (
              <div
                key={virtualRow.key}
                data-index={virtualRow.index}
                ref={rowVirtualizer.measureElement}
              >
                {renderMessage(message, virtualRow.index)}
              </div>
            )
          })}
        </div>
      </div>
    )
  }

  return (
    <div
      ref={internalRef}
      className={cn('relative overflow-y-auto', className)}
      onScroll={handleScroll}
      role="log"
      aria-label="对话消息列表"
      aria-live="polite"
    >
      <div style={{ height: totalSize, width: '100%', position: 'relative' }}>
        <div
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            width: '100%',
            transform: `translateY(${virtualItems[0]?.start ?? 0}px)`,
          }}
        >
          {virtualItems.map((virtualRow) => {
            const message = messages[virtualRow.index]
            if (!message) return null
            return (
              <div
                key={virtualRow.key}
                data-index={virtualRow.index}
                ref={rowVirtualizer.measureElement}
              >
                {renderMessage(message, virtualRow.index)}
              </div>
            )
          })}
        </div>
      </div>

      {!atBottom && (
        <button
          type="button"
          onClick={() => scrollToBottom('smooth')}
          className={cn(
            'sticky bottom-4 mx-auto flex items-center gap-1.5 rounded-full',
            'bg-popover/90 px-3 py-1.5 text-[11px] font-medium text-foreground/70',
            'shadow-token-lg border border-border/60 backdrop-blur-sm',
            'transition-opacity duration-150 hover:text-foreground',
          )}
          aria-label="滚动到最新消息"
        >
          ↓ 最新消息
        </button>
      )}
    </div>
  )
}
