/**
 * GF-01 PR-08 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Owns the transcript scroll bookkeeping: bottom anchor refs, the
 * "stick to bottom while streaming" flag, the per-frame scroll handler
 * (rAF-coalesced) and a `scrollToBottom` helper for the floating jump
 * button. Render-equivalent move from the original inline definitions.
 */

import * as React from 'react'

/** Pixel slack within which the transcript is still considered "at
 * bottom" (e.g. one extra inline tool card showing). */
export const BOTTOM_EPSILON_PX = 120

/** Return shape: bundles every scroll-related ref + state the chat-ui
 * shell hands to `<ChatTranscript>` and the floating "jump to bottom"
 * button. Refs stay stable across renders. */
export interface ChatScrollState {
  bottomRef: React.RefObject<HTMLDivElement | null>
  scrollRef: React.RefObject<HTMLDivElement | null>
  scrollRafRef: React.MutableRefObject<number | null>
  isAtBottomRef: React.MutableRefObject<boolean>
  forceAutoScrollRef: React.MutableRefObject<boolean>
  stickToBottomDuringStreamRef: React.MutableRefObject<boolean>
  lastScrollTopRef: React.MutableRefObject<number>
  lastBottomOccupancyRef: React.MutableRefObject<number>
  isAtBottom: boolean
  setIsAtBottom: React.Dispatch<React.SetStateAction<boolean>>
  /** Imperative helper used by the `<ArrowDown>` floating button. */
  scrollToBottom: () => void
  /** Mutates `isAtBottomRef` + `setIsAtBottom` from a container ref. */
  updateBottomState: (container: HTMLDivElement) => void
  /** Wired to `<ChatTranscript onScroll>` — handles rAF batching, lock
   * release on user-initiated upward scroll, and bottom-state refresh. */
  handleScroll: (isLoading?: boolean) => void
}

/** Centralise transcript-scroll state previously inlined inside
 * `ChatUI`. Everything the shell handed to <ChatTranscript /> + the
 * scroll-to-bottom button comes back here verbatim, so the call site
 * is `const scroll = useChatScroll()` plus prop forwarding. */
export function useChatScroll(): ChatScrollState {
  const bottomRef = React.useRef<HTMLDivElement>(null)
  const scrollRef = React.useRef<HTMLDivElement>(null)
  const scrollRafRef = React.useRef<number | null>(null)
  const isAtBottomRef = React.useRef(true)
  const forceAutoScrollRef = React.useRef(false)
  const stickToBottomDuringStreamRef = React.useRef(true)
  const lastScrollTopRef = React.useRef(0)
  const lastBottomOccupancyRef = React.useRef(0)
  const [isAtBottom, setIsAtBottom] = React.useState(true)

  const updateBottomState = React.useCallback((container: HTMLDivElement) => {
    const maxScrollTop = container.scrollHeight - container.clientHeight
    const nextIsAtBottom = maxScrollTop - container.scrollTop < BOTTOM_EPSILON_PX
    isAtBottomRef.current = nextIsAtBottom
    if (nextIsAtBottom) {
      stickToBottomDuringStreamRef.current = true
    }
    setIsAtBottom((prev) => (prev === nextIsAtBottom ? prev : nextIsAtBottom))
  }, [])

  const handleScroll = React.useCallback((isLoading?: boolean) => {
    const container = scrollRef.current
    if (!container) return
    if (container.scrollLeft !== 0) {
      container.scrollLeft = 0
    }
    if (scrollRafRef.current !== null) return
    scrollRafRef.current = window.requestAnimationFrame(() => {
      scrollRafRef.current = null
      const nextContainer = scrollRef.current
      if (!nextContainer) return
      const nextScrollTop = nextContainer.scrollTop
      const scrollDelta = nextScrollTop - lastScrollTopRef.current
      lastScrollTopRef.current = nextScrollTop
      if (isLoading && scrollDelta < -2) {
        // User actively scrolls upward while streaming: pause sticky auto-follow.
        stickToBottomDuringStreamRef.current = false
      }
      updateBottomState(nextContainer)
    })
  }, [updateBottomState])

  const scrollToBottom = React.useCallback(() => {
    forceAutoScrollRef.current = true
    scrollRef.current?.scrollTo({
      top: scrollRef.current.scrollHeight,
      behavior: 'smooth',
    })
  }, [])

  // Cleanup: release any pending rAF on unmount.
  React.useEffect(() => {
    return () => {
      if (scrollRafRef.current !== null) {
        window.cancelAnimationFrame(scrollRafRef.current)
        scrollRafRef.current = null
      }
    }
  }, [])

  return {
    bottomRef,
    scrollRef,
    scrollRafRef,
    isAtBottomRef,
    forceAutoScrollRef,
    stickToBottomDuringStreamRef,
    lastScrollTopRef,
    lastBottomOccupancyRef,
    isAtBottom,
    setIsAtBottom,
    scrollToBottom,
    updateBottomState,
    handleScroll,
  }
}
