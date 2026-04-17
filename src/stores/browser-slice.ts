/**
 * Browser status store — module-level singleton backed by `useSyncExternalStore`.
 *
 * Provides the `BrowserSlice` interface without requiring a third-party state
 * library. State is held in a module-level object; all mutations notify React
 * via the standard external-store subscription contract.
 *
 * All public mutation functions are exported as plain functions so they can be
 * called outside React components (e.g., from Tauri event handlers).
 */

import { useSyncExternalStore } from 'react'

// ── Types ─────────────────────────────────────────────────────────────────────

/** Live state snapshot for a single AI-controlled browser session. */
export interface BrowserEntry {
  /** Whether the browser process is actively running. */
  running: boolean
  /** Current page URL, if known. */
  url: string | null
  /** Base-64 JPEG thumbnail of the current viewport, if available. */
  thumbnail: string | null
}

/** Shape of the browser store exposed to components. */
export interface BrowserSlice {
  /** Map from `session_id` to current browser state. */
  browserBySession: Readonly<Record<string, BrowserEntry>>
  /** Merge a partial update into the entry for `sessionId`. */
  setBrowserStatus: (sessionId: string, partial: Partial<BrowserEntry>) => void
  /** Remove the entry for `sessionId` (called when the browser stops). */
  clearBrowserSession: (sessionId: string) => void
}

// ── Module-level store ────────────────────────────────────────────────────────

let _bySession: Record<string, BrowserEntry> = {}

/** Registered React subscriptions — notified on every mutation. */
const _listeners = new Set<() => void>()

function _notify(): void {
  for (const fn of _listeners) fn()
}

// ── Exported mutation functions ───────────────────────────────────────────────

/**
 * Merge `partial` into the stored `BrowserEntry` for `sessionId`.
 * Creates a new entry if none exists yet.
 */
export function setBrowserStatus(
  sessionId: string,
  partial: Partial<BrowserEntry>
): void {
  const existing: BrowserEntry = _bySession[sessionId] ?? {
    running: false,
    url: null,
    thumbnail: null,
  }
  _bySession = {
    ..._bySession,
    [sessionId]: { ...existing, ...partial },
  }
  _notify()
}

/**
 * Remove the entry for `sessionId` from the store.
 * Called when the backend reports `running: false` or the user stops the browser.
 */
export function clearBrowserSession(sessionId: string): void {
  const next = { ..._bySession }
  delete next[sessionId]
  _bySession = next
  _notify()
}

// ── useSyncExternalStore wiring ───────────────────────────────────────────────

function _subscribe(listener: () => void): () => void {
  _listeners.add(listener)
  return () => _listeners.delete(listener)
}

function _getSnapshot(): Record<string, BrowserEntry> {
  return _bySession
}

/**
 * React hook that returns the full `BrowserSlice`, re-rendering the calling
 * component whenever any browser session status changes.
 */
export function useBrowserStore(): BrowserSlice {
  const snapshot = useSyncExternalStore(_subscribe, _getSnapshot, _getSnapshot)
  return {
    browserBySession: snapshot,
    setBrowserStatus,
    clearBrowserSession,
  }
}
