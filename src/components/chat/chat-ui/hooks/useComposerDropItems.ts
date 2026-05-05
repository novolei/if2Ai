/**
 * GF-01 PR-08 — extracted from `src/components/ui/chat-ui.tsx`.
 *
 * Owns the composer's "drop items" tray (drag-and-drop file/folder
 * chips and `@`-mention chips). Add / remove handlers preserved
 * verbatim from the original inline definitions.
 */

import * as React from 'react'

import type { ComposerDropItem } from '@/components/ui/chat-ui'

/** Bag returned by `useComposerDropItems`. */
export interface ComposerDropItems {
  dropItems: ComposerDropItem[]
  setDropItems: React.Dispatch<React.SetStateAction<ComposerDropItem[]>>
  /** Append a chip; idempotent on the `(path, kind)` pair so
   * repeated drops don't pile up duplicates. */
  addDropItem: (item: ComposerDropItem) => void
  /** Remove a chip by id. */
  removeDropItem: (id: string) => void
  /** Convenience wrapper for the file-reference drop callback —
   * identical to `addDropItem` but named to match the prop the
   * `<ComposerDock>` exposes (`onFileReferenceDrop`). */
  handleFileReferenceDrop: (item: ComposerDropItem) => void
}

/** Centralise composer drop-item state. The chat-ui shell wires
 * `addDropItem` to `<ComposerDock onFileReferenceDrop>` and
 * `removeDropItem` to `<ComposerDock onRemoveDropItem>`. */
export function useComposerDropItems(): ComposerDropItems {
  const [dropItems, setDropItems] = React.useState<ComposerDropItem[]>([])

  const addDropItem = React.useCallback((item: ComposerDropItem) => {
    setDropItems((current) =>
      current.some((entry) => entry.path === item.path && entry.kind === item.kind)
        ? current
        : [...current, item],
    )
  }, [])

  const removeDropItem = React.useCallback((id: string) => {
    setDropItems((current) => current.filter((item) => item.id !== id))
  }, [])

  const handleFileReferenceDrop = addDropItem

  return {
    dropItems,
    setDropItems,
    addDropItem,
    removeDropItem,
    handleFileReferenceDrop,
  }
}
