/**
 * Conversation store slice — module-level singleton backed by `useSyncExternalStore`.
 *
 * Extracts the heavy per-session conversation state that previously lived
 * directly in `App.tsx`, reducing that component's God-state footprint.
 *
 * All mutation functions are exported as plain functions so they can be
 * called outside React components (e.g., from Tauri stream event handlers).
 *
 * Follows the same pattern as `browser-slice.ts`.
 */

import { useSyncExternalStore } from 'react'
import type { Conversation, Message, SessionTitleState, SessionTitleStage } from '@/modules/chat/types'
import type { TodoItem } from '@/components/ui/TodoPanel'

// ── Types ─────────────────────────────────────────────────────────────────────

/** All per-session data managed by this slice. */
export interface ConversationSlice {
  /** Full conversation objects keyed by session ID. */
  readonly conversations: Readonly<Record<string, Conversation>>
  /** Per-session "is streaming / loading" flag. */
  readonly sessionLoading: Readonly<Record<string, boolean>>
  /** Per-session Todo lists produced by the agent. */
  readonly sessionTodos: Readonly<Record<string, TodoItem[]>>
  /** Per-session title stage tracking (placeholder → provisional → locked). */
  readonly sessionTitleStates: Readonly<Record<string, SessionTitleState>>
  /** Per-session abort-handle IDs returned by the stream subscription. */
  readonly streamAbortHandles: Readonly<Record<string, string>>
}

// ── Internal state ────────────────────────────────────────────────────────────

interface _State {
  conversations: Record<string, Conversation>
  sessionLoading: Record<string, boolean>
  sessionTodos: Record<string, TodoItem[]>
  sessionTitleStates: Record<string, SessionTitleState>
  streamAbortHandles: Record<string, string>
}

let _state: _State = {
  conversations: {},
  sessionLoading: {},
  sessionTodos: {},
  sessionTitleStates: {},
  streamAbortHandles: {},
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

// ── Conversation mutations ────────────────────────────────────────────────────

/**
 * Upsert a conversation object. If one with the same `id` already exists,
 * it is replaced entirely.
 */
export function setConversation(conversation: Conversation): void {
  _set({
    conversations: { ..._state.conversations, [conversation.id]: conversation },
  })
}

/**
 * Append a message to an existing conversation, or no-op if the session is
 * not yet tracked.
 */
export function appendMessage(sessionId: string, message: Message): void {
  const conv = _state.conversations[sessionId]
  if (!conv) return
  _set({
    conversations: {
      ..._state.conversations,
      [sessionId]: {
        ...conv,
        messages: [...conv.messages, message],
        updatedAt: new Date(),
      },
    },
  })
}

/**
 * Update a single message inside a conversation by `message.id`.
 * If the conversation or message is not found, this is a no-op.
 */
export function updateMessage(sessionId: string, message: Message): void {
  const conv = _state.conversations[sessionId]
  if (!conv) return
  _set({
    conversations: {
      ..._state.conversations,
      [sessionId]: {
        ...conv,
        messages: conv.messages.map((m) => (m.id === message.id ? message : m)),
        updatedAt: new Date(),
      },
    },
  })
}

/**
 * Remove all messages from a conversation, keeping the session metadata intact.
 */
export function clearMessages(sessionId: string): void {
  const conv = _state.conversations[sessionId]
  if (!conv) return
  _set({
    conversations: {
      ..._state.conversations,
      [sessionId]: { ...conv, messages: [], updatedAt: new Date() },
    },
  })
}

/**
 * Remove a conversation and all its associated per-session state.
 */
export function removeSession(sessionId: string): void {
  const { conversations, sessionLoading, sessionTodos, sessionTitleStates, streamAbortHandles } =
    _state

  const nextConversations = { ...conversations }
  delete nextConversations[sessionId]

  const nextSessionLoading = { ...sessionLoading }
  delete nextSessionLoading[sessionId]

  const nextSessionTodos = { ...sessionTodos }
  delete nextSessionTodos[sessionId]

  const nextSessionTitleStates = { ...sessionTitleStates }
  delete nextSessionTitleStates[sessionId]

  const nextStreamAbortHandles = { ...streamAbortHandles }
  delete nextStreamAbortHandles[sessionId]

  _set({
    conversations: nextConversations,
    sessionLoading: nextSessionLoading,
    sessionTodos: nextSessionTodos,
    sessionTitleStates: nextSessionTitleStates,
    streamAbortHandles: nextStreamAbortHandles,
  })
}

// ── Session loading ───────────────────────────────────────────────────────────

/** Mark a session as currently loading (streaming). */
export function setSessionLoading(sessionId: string, loading: boolean): void {
  _set({ sessionLoading: { ..._state.sessionLoading, [sessionId]: loading } })
}

// ── Session Todos ─────────────────────────────────────────────────────────────

/** Replace the todo list for a session. */
export function setSessionTodos(sessionId: string, todos: TodoItem[]): void {
  _set({ sessionTodos: { ..._state.sessionTodos, [sessionId]: todos } })
}

// ── Title states ──────────────────────────────────────────────────────────────

/** Initialise the title state for a new session. */
export function initTitleState(sessionId: string): void {
  if (_state.sessionTitleStates[sessionId]) return
  _set({
    sessionTitleStates: {
      ..._state.sessionTitleStates,
      [sessionId]: { stage: 'placeholder', autoRenameCount: 0 },
    },
  })
}

/** Advance the title stage for a session. */
export function setTitleStage(sessionId: string, stage: SessionTitleStage): void {
  const current = _state.sessionTitleStates[sessionId] ?? {
    stage: 'placeholder',
    autoRenameCount: 0,
  }
  _set({
    sessionTitleStates: {
      ..._state.sessionTitleStates,
      [sessionId]: { ...current, stage },
    },
  })
}

/** Increment the auto-rename counter for a session. */
export function incrementAutoRenameCount(sessionId: string): void {
  const current = _state.sessionTitleStates[sessionId] ?? {
    stage: 'placeholder',
    autoRenameCount: 0,
  }
  _set({
    sessionTitleStates: {
      ..._state.sessionTitleStates,
      [sessionId]: {
        ...current,
        autoRenameCount: current.autoRenameCount + 1,
      },
    },
  })
}

// ── Stream abort handles ──────────────────────────────────────────────────────

/** Register a stream abort-handle ID for a session. */
export function setStreamAbortHandle(sessionId: string, handleId: string): void {
  _set({ streamAbortHandles: { ..._state.streamAbortHandles, [sessionId]: handleId } })
}

/** Remove the abort handle once a stream is done or aborted. */
export function clearStreamAbortHandle(sessionId: string): void {
  const next = { ..._state.streamAbortHandles }
  delete next[sessionId]
  _set({ streamAbortHandles: next })
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
 * React hook that returns the full `ConversationSlice`, re-rendering the
 * calling component whenever any conversation state changes.
 */
export function useConversationStore(): ConversationSlice {
  return useSyncExternalStore(_subscribe, _getSnapshot, _getSnapshot)
}
