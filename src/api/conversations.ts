// MIG-015 — canonical conversations gateway surface.
//
// Single, narrow service entry the main chat path consumes
// instead of stitching together the primitive Tauri agent
// commands by hand. Wraps:
//
//   - `startAgentStream(...)`  → opens a streaming turn
//   - `listenToStream(id, fn)` → fans out the per-stream events
//   - `stopAgentStream(id)`    → cooperative cancellation
//   - `respondPermission(...)` → permission prompt response
//
// The chat workspace receives a [`ChatStreamHandle`] that
// bundles the four primitives so callers no longer learn the
// underlying Tauri command names. This lets MIG-011 (post-pack)
// swap the transport to a real local gateway sidecar without
// touching any chat surface code.
//
// Pack §8 guardrails enforced:
//
//   1. There is exactly one canonical surface — once the chat
//      path consumes `startChatTurn(...)`, the previous direct
//      `startAgentStream + listenToStream + stopAgentStream`
//      sequence is no longer needed there.
//   2. Permission response goes through the handle, not via a
//      separate global call site, so the runtime semantics
//      (which session is being responded to) stays bound to the
//      handle instance.

import type { UnlistenFn } from '@tauri-apps/api/event'

import type {
  PermissionMode,
  StreamTokenPayload,
} from '@/transport/contracts'

import {
  listenToStream,
  respondPermission,
  startAgentStream,
  stopAgentStream,
} from './streaming.ts'
import { getSession, type Session } from './sessions.ts'

/** Per-turn input bundle for [`startChatTurn`]. Mirrors the
 * `StreamTurnRequest` shape MIG-001-c gave the backend — same
 * field names so the wire stays bit-identical. */
export interface ChatTurnRequest {
  sessionId: string
  userMessage: string
  permissionMode?: PermissionMode
}

/** Decision shape forwarded to the backend permission service.
 * Mirrors `application::permission_service::respond_to_permission_prompt`. */
export interface PermissionDecisionRequest {
  decision: 'allow' | 'deny'
  toolName?: string
  scope?: 'once' | 'session'
}

/** Handle returned from [`startChatTurn`].
 *
 * Bundles every operation the chat workspace needs to drive
 * one streaming turn: subscribe to events, cancel, respond to
 * any permission prompt that fires mid-turn. The chat surface
 * never imports the underlying Tauri primitives directly.
 */
export interface ChatStreamHandle {
  /** Backend-allocated stream id. Stable for the life of the
   * turn; use for correlating UI state. */
  readonly streamId: string
  /** Subscribe a handler to events for this stream id. Returns
   * an `UnlistenFn`; callers MUST invoke it on cleanup. */
  subscribe(handler: (payload: StreamTokenPayload) => void): Promise<UnlistenFn>
  /** Cooperative cancellation. Safe to call after the stream
   * has already completed (the backend resolves silently). */
  stop(): Promise<void>
  /** Respond to a permission prompt that fired during this
   * turn. Bound to the same `sessionId` the turn was started
   * for so the chat workspace cannot mis-route a decision to a
   * different session. */
  respondPermission(decision: PermissionDecisionRequest): Promise<void>
}

/**
 * Start one streaming chat turn and return a handle that owns
 * every follow-up operation for that turn.
 *
 * The chat workspace flow is:
 *
 * ```ts
 * const handle = await startChatTurn({ sessionId, userMessage, permissionMode })
 * const unlisten = await handle.subscribe((payload) => projectIntoStore(payload))
 * // ...later...
 * await handle.stop()                       // cancel
 * await handle.respondPermission({ ... })   // permission prompt
 * unlisten()
 * ```
 */
export async function startChatTurn(
  request: ChatTurnRequest,
): Promise<ChatStreamHandle> {
  const { sessionId, userMessage, permissionMode } = request
  const streamId = await startAgentStream(sessionId, userMessage, permissionMode)
  return {
    streamId,
    subscribe: (handler) => listenToStream(streamId, handler),
    stop: () => stopAgentStream(streamId),
    respondPermission: ({ decision, toolName, scope }) =>
      respondPermission(sessionId, decision, { toolName, scope }),
  }
}

/**
 * Load the canonical conversation history for `sessionId`.
 *
 * Wraps the lower-level [`getSession`] under a verb that names
 * the chat-workspace intent (history projection) rather than
 * the Tauri command name (`get_session`). Use this from the
 * chat surface so the next transport swap (gateway sidecar) is
 * a one-line change here, not a sweep across the chat tree.
 */
export async function loadConversationHistory(sessionId: string): Promise<Session> {
  return getSession(sessionId)
}
