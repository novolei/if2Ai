/**
 * GF-01 PR-08 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Owns the slash + @-mention overlay state machines that the composer
 * dock uses to surface autocompletion popovers. Each overlay carries
 * a debounce timer ref so the chat shell's cleanup effect can cancel
 * pending fetches on unmount (preserved verbatim from the original).
 */

import * as React from 'react'

import type { DirectoryEntryPreview } from '@/lib/tauri'

/** Slash command overlay state. `null` = hidden. */
export interface SlashOverlayState {
  visible: boolean
  selectedIndex: number
  suggestions: string[]
  rawInput: string
}

/** @-mention overlay state. `null` = hidden. */
export interface AtOverlayState {
  visible: boolean
  selectedIndex: number
  entries: DirectoryEntryPreview[]
  query: string
  /** Position of the @ character in the textarea (−1 = already cleaned) */
  atPos: number
  /** Currently browsed directory path; null = workdir root */
  browsePath: string | null
  /** Navigation history stack for back navigation */
  breadcrumbs: Array<{ name: string; path: string | null }>
  /** When true the overlay is detached from the textarea (@token already removed) */
  pinned: boolean
}

/** Bag returned by `useComposerOverlays`. Refs are exposed so the
 * chat-ui shell can keep its existing unmount-cleanup effect verbatim. */
export interface ComposerOverlays {
  slashOverlay: SlashOverlayState | null
  setSlashOverlay: React.Dispatch<React.SetStateAction<SlashOverlayState | null>>
  atOverlay: AtOverlayState | null
  setAtOverlay: React.Dispatch<React.SetStateAction<AtOverlayState | null>>
  slashTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
  atTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
}

/** Centralise both composer-overlay state machines + their debounce
 * timers. The hook itself does not touch the DOM; it just owns the
 * state slot + timer ref slot the chat shell previously inlined. */
export function useComposerOverlays(): ComposerOverlays {
  const [slashOverlay, setSlashOverlay] = React.useState<SlashOverlayState | null>(null)
  const [atOverlay, setAtOverlay] = React.useState<AtOverlayState | null>(null)
  const slashTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)
  const atTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)

  // Cleanup: cancel any pending debounce timers on unmount. Mirrors
  // the original chat-ui shell-level cleanup block.
  React.useEffect(() => {
    return () => {
      if (slashTimerRef.current) {
        clearTimeout(slashTimerRef.current)
        slashTimerRef.current = null
      }
      if (atTimerRef.current) {
        clearTimeout(atTimerRef.current)
        atTimerRef.current = null
      }
    }
  }, [])

  return {
    slashOverlay,
    setSlashOverlay,
    atOverlay,
    setAtOverlay,
    slashTimerRef,
    atTimerRef,
  }
}
