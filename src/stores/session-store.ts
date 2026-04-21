// MIG-014 — session store.
//
// Owns the canonical "which session is the user currently
// looking at?" cursor + lightweight per-session metadata that
// is too granular for the bootstrap store (which holds the
// project list / project-keyed sessions map populated at boot)
// but too cross-cutting for per-component `useState`.
//
// Lives next to the existing
// `src/stores/conversation-slice.ts` (chat-side per-session
// runtime state) and the MIG-013
// `src/state/bootstrap-store.ts` (boot phase + project list).
// The three stores have non-overlapping responsibilities by
// design — pack §8 guardrail 1: "store 不能只是把 useState 平
// 移成另一个全局 god store".
//
// Mirrors the `useSyncExternalStore`-compatible shape already
// used by `conversation-slice.ts` so wiring into React stays
// cheap. `createSessionStore()` is exposed for tests; the
// production singleton is `sessionStore`.

import { useCallback, useSyncExternalStore } from 'react'

/** Immutable session-cursor snapshot. */
export interface SessionState {
  /** Currently focused session id. `null` when the user is on
   * the home / project picker / no session selected. */
  activeSessionId: string | null
  /** Last time the user explicitly selected a session
   * (millis). Used by future MIG-006 surfaces to highlight
   * recency without re-querying the backend. */
  lastSelectedAt: number | null
}

export const INITIAL_SESSION_STATE: SessionState = Object.freeze({
  activeSessionId: null,
  lastSelectedAt: null,
})

export type SessionListener = () => void

export interface SessionStore {
  getSnapshot(): SessionState
  subscribe(listener: SessionListener): () => void
  /** Set the active session id (or `null` to clear). Stamps
   * `lastSelectedAt` whenever `id` is non-null. */
  setActiveSessionId(id: string | null): void
  /** Hard reset for tests / the post-onboarding boot reset. */
  reset(): void
}

/** Build a fresh, isolated session store. Tests call this
 * directly; production code uses [`sessionStore`]. */
export function createSessionStore(): SessionStore {
  let state: SessionState = INITIAL_SESSION_STATE
  const listeners = new Set<SessionListener>()

  const notify = () => {
    for (const l of listeners) l()
  }

  const replace = (next: SessionState) => {
    if (next === state) return
    state = next
    notify()
  }

  return {
    getSnapshot: () => state,
    subscribe: (listener) => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    setActiveSessionId: (id) => {
      replace({
        activeSessionId: id,
        lastSelectedAt: id ? Date.now() : null,
      })
    },
    reset: () => {
      replace(INITIAL_SESSION_STATE)
    },
  }
}

/** Process-wide session store singleton. */
export const sessionStore: SessionStore = createSessionStore()

// ── React adapters ────────────────────────────────────────────

/** Subscribe the calling component to the entire session
 * snapshot. */
export function useSessionState(store: SessionStore = sessionStore): SessionState {
  const subscribe = useCallback((cb: () => void) => store.subscribe(cb), [store])
  const get = useCallback(() => store.getSnapshot(), [store])
  return useSyncExternalStore(subscribe, get, get)
}

/** Subscribe to a slice of the session snapshot. */
export function useSessionSelector<T>(
  selector: (state: SessionState) => T,
  store: SessionStore = sessionStore,
): T {
  const subscribe = useCallback((cb: () => void) => store.subscribe(cb), [store])
  const get = useCallback(() => selector(store.getSnapshot()), [store, selector])
  return useSyncExternalStore(subscribe, get, get)
}
