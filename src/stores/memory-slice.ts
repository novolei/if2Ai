/**
 * Memory store slice — module-level singleton backed by `useSyncExternalStore`.
 *
 * Tracks frontend-visible memory state:
 *   - Permission prompts raised by the `memory_store` tool (allow / deny / prompt)
 *   - Lightweight per-session memory event log for the MemoryBrowser and
 *     future MemoryChip / MemoryWriteCard components
 *
 * Follows the same pattern as `browser-slice.ts` and `conversation-slice.ts`.
 */

import { useSyncExternalStore } from 'react'
import type { PermissionRequestPayload } from '@/lib/tauri'

// ── Types ─────────────────────────────────────────────────────────────────────

/**
 * A memory write event recorded in the frontend, mirroring what the backend
 * MemoryAuditEmitter emits. Used by MemoryWriteCard and MemoryBrowser.
 */
export interface MemoryWriteEvent {
  /** Unique event ID (matches the backend audit event ID). */
  id: string
  /** Session that triggered the write. */
  sessionId: string
  /** Memory content being stored. */
  content: string
  /** Policy decision rendered by MemoryPolicyEngine. */
  policyDecision: 'allow' | 'deny' | 'prompt'
  /** Human-readable reason code from the policy engine. */
  reasonCode: string
  /** ISO timestamp from the backend. */
  timestamp: string
  /** Memory scope: 'global' | 'project' | 'session'. */
  scope: 'global' | 'project' | 'session'
}

/** Shape of the memory store exposed to components. */
export interface MemorySlice {
  /** Pending permission prompt raised by the agent, or null when idle. */
  readonly permissionPrompt: PermissionRequestPayload | null
  /** Recent memory write events, newest first, capped at `MAX_EVENT_LOG` entries. */
  readonly memoryEvents: ReadonlyArray<MemoryWriteEvent>
  /** True while a memory recall operation is in progress for the given session. */
  readonly recallInProgress: Readonly<Record<string, boolean>>
}

// ── Constants ─────────────────────────────────────────────────────────────────

/** Maximum number of memory write events retained in the frontend log. */
const MAX_EVENT_LOG = 200

// ── Internal state ────────────────────────────────────────────────────────────

interface _State {
  permissionPrompt: PermissionRequestPayload | null
  memoryEvents: MemoryWriteEvent[]
  recallInProgress: Record<string, boolean>
}

let _state: _State = {
  permissionPrompt: null,
  memoryEvents: [],
  recallInProgress: {},
}

/** Registered React subscriptions — notified on every mutation. */
const _listeners = new Set<() => void>()

function _notify(): void {
  for (const fn of _listeners) fn()
}

function _set(partial: Partial<_State>): void {
  _state = { ..._state, ...partial }
  _notify()
}

// ── Permission prompt mutations ───────────────────────────────────────────────

/**
 * Show a permission prompt to the user. Called when the backend emits a
 * `memory_store` permission request.
 */
export function showPermissionPrompt(payload: PermissionRequestPayload): void {
  _set({ permissionPrompt: payload })
}

/**
 * Dismiss the current permission prompt (e.g., after the user responds or
 * the request times out).
 */
export function dismissPermissionPrompt(): void {
  _set({ permissionPrompt: null })
}

// ── Memory event log mutations ────────────────────────────────────────────────

/**
 * Prepend a new `MemoryWriteEvent` to the log, evicting the oldest entries
 * when the log exceeds `MAX_EVENT_LOG` entries.
 */
export function recordMemoryWriteEvent(event: MemoryWriteEvent): void {
  const next = [event, ..._state.memoryEvents]
  _set({ memoryEvents: next.length > MAX_EVENT_LOG ? next.slice(0, MAX_EVENT_LOG) : next })
}

/**
 * Remove all memory state belonging to a session (events + recall progress).
 * Call this when a session is deleted to avoid per-session state leaks.
 */
export function clearSessionMemoryEvents(sessionId: string): void {
  const nextRecall = { ..._state.recallInProgress }
  delete nextRecall[sessionId]
  _set({
    memoryEvents: _state.memoryEvents.filter((e) => e.sessionId !== sessionId),
    recallInProgress: nextRecall,
  })
}

// ── Recall progress mutations ─────────────────────────────────────────────────

/** Mark a session as currently performing a memory recall. */
export function setRecallInProgress(sessionId: string, inProgress: boolean): void {
  const next = { ..._state.recallInProgress }
  if (inProgress) {
    next[sessionId] = true
  } else {
    delete next[sessionId]
  }
  _set({ recallInProgress: next })
}

// ── useSyncExternalStore wiring ───────────────────────────────────────────────

function _subscribe(listener: () => void): () => void {
  _listeners.add(listener)
  return () => _listeners.delete(listener)
}

function _getSnapshot(): _State {
  return _state
}

/**
 * React hook that returns the full `MemorySlice`, re-rendering the calling
 * component whenever any memory state changes.
 */
export function useMemoryStore(): MemorySlice {
  return useSyncExternalStore(_subscribe, _getSnapshot, _getSnapshot)
}
