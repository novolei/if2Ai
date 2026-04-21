// MIG-014 — chat store facade.
//
// Single canonical import root for the per-session chat
// runtime state — conversations, message stream, per-session
// loading flag, todo lists, title-stage tracking, stream
// abort handles. Wraps the existing
// `src/stores/conversation-slice.ts` (which already implements
// the underlying useSyncExternalStore pattern) and exposes a
// stable surface so future consumers (post-MIG-015 gateway
// streaming, post-MIG-006 chat shell rebuild) import from
// `@/stores/chat-store` rather than reaching into the slice
// internals.
//
// What this file deliberately does NOT do:
//
// - It does NOT replace `conversation-slice.ts`. The slice
//   stays the authoritative implementation; chat-store is the
//   public re-export + a small selector layer on top.
// - It does NOT own the active session cursor. That lives in
//   `session-store.ts` so a future "compare two sessions side
//   by side" UI can hold multiple cursors without forking
//   chat-store.
// - It does NOT own the pending permission prompt. That is
//   already projected from the runtime-projection store
//   (`approvals` slice) — see App.tsx::permissionPrompt.

import type { Conversation, Message } from '@/modules/chat/types'

import {
  appendMessage,
  clearMessages,
  clearStreamAbortHandle,
  incrementAutoRenameCount,
  initTitleState,
  removeSession,
  setConversation,
  setSessionLoading,
  setSessionTodos,
  setStreamAbortHandle,
  setTitleStage,
  setTitleState,
  updateMessage,
  useConversationStore,
  type ConversationSlice,
} from './conversation-slice.ts'

// ── Public surface re-export ──────────────────────────────────

export type { ConversationSlice, Conversation, Message }

// Mutations — verbatim from `conversation-slice` so call sites
// can `import { setConversation } from '@/stores/chat-store'`
// rather than the internal slice path.
export {
  appendMessage,
  clearMessages,
  clearStreamAbortHandle,
  incrementAutoRenameCount,
  initTitleState,
  removeSession,
  setConversation,
  setSessionLoading,
  setSessionTodos,
  setStreamAbortHandle,
  setTitleStage,
  setTitleState,
  updateMessage,
  useConversationStore,
}

// ── Selectors ─────────────────────────────────────────────────

/** Subscribe to the full chat slice. Re-renders on any
 * conversation / loading / todo / title / abort-handle
 * change. Re-export of `useConversationStore` under the
 * canonical chat-store name so call sites read clearly. */
export function useChatStore(): ConversationSlice {
  return useConversationStore()
}

/** Read a derived slice from the chat store.
 *
 * Honest scope: this hook subscribes the calling component to
 * the entire chat slice (via [`useConversationStore`]) and
 * applies `selector` to the snapshot every render. It does NOT
 * skip re-renders when the selected value is referentially
 * stable across slice mutations — the underlying
 * `conversation-slice` notifies on every mutation, and the
 * single shared store subscription does not thread per-call
 * memoisation. Callers who need fine-grained re-render control
 * should read the slice directly and memoise downstream. */
export function useChatSelector<T>(selector: (slice: ConversationSlice) => T): T {
  const slice = useConversationStore()
  return selector(slice)
}

/** Read the conversation for a specific session. Returns
 * `undefined` when the session has not been hydrated yet. */
export function useConversation(sessionId: string | null): Conversation | undefined {
  return useChatSelector((slice) =>
    sessionId ? slice.conversations[sessionId] : undefined,
  )
}

/** Read the loading flag for a specific session. Returns
 * `false` for unknown sessions. */
export function useSessionIsLoading(sessionId: string | null): boolean {
  return useChatSelector((slice) =>
    sessionId ? slice.sessionLoading[sessionId] === true : false,
  )
}
