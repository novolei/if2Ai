// MIG-012 — agent streaming domain facade.
//
// Owns every call the chat workspace makes to drive a streaming
// agent turn: start, listen, respond to permission prompts. The
// underlying Tauri commands (`start_agent_stream`,
// `respond_permission`, `agent-token` event) stay hidden behind
// this module — callers never import `invoke` / `listen`
// directly.

import type { UnlistenFn } from '@tauri-apps/api/event'

import { AGENT_TOKEN_EVENT } from '@/transport/contracts'
import type {
  PermissionMode,
  PermissionRequestPayload,
  StreamTokenPayload,
} from '@/transport/contracts'

import { getApiClient } from './client.ts'

/**
 * Start a streaming agent turn and receive the stream id.
 *
 * The returned id is the correlation key for every subsequent
 * `agent-token` event — pair it with [`listenToStream`] to
 * project tokens into the UI.
 */
export async function startAgentStream(
  sessionId: string,
  userMessage: string,
  permissionMode?: PermissionMode,
  selectedModel?: string,
): Promise<string> {
  const [providerId, modelId] = selectedModel?.split('/') ?? []
  const args: Record<string, unknown> = {
    sessionId,
    userMessage,
    permissionMode,
  }
  if (providerId && modelId) {
    args.providerId = providerId
    args.modelId = modelId
  }
  return getApiClient().call<string>('start_agent_stream', args)
}

/** Cancel an in-flight streaming turn. Safe to call on an
 * already-completed stream id — the backend resolves silently. */
export async function stopAgentStream(streamId: string): Promise<void> {
  return getApiClient().call<void>('stop_agent_stream', { streamId })
}

/**
 * Subscribe to token events for one specific stream id. The
 * callback receives every [`StreamTokenPayload`] whose
 * `stream_id` matches; events from other concurrent streams are
 * filtered out client-side.
 *
 * Returns an `UnlistenFn` — callers MUST invoke it on cleanup
 * or the subscription leaks.
 */
export async function listenToStream(
  streamId: string,
  handler: (payload: StreamTokenPayload) => void,
): Promise<UnlistenFn> {
  return getApiClient().subscribe<StreamTokenPayload>(AGENT_TOKEN_EVENT, (event) => {
    if (event.payload.stream_id === streamId) {
      handler(event.payload)
    }
  })
}

/**
 * Respond to a backend permission prompt with allow / deny and
 * an optional scope. Used by the permission dialog glued to the
 * `permission-request` event.
 */
export async function respondPermission(
  sessionId: string,
  decision: 'allow' | 'deny',
  options?: { toolName?: string; scope?: 'once' | 'session' },
): Promise<void> {
  return getApiClient().call<void>('respond_permission', {
    sessionId,
    decision,
    toolName: options?.toolName,
    scope: options?.scope,
  })
}

/** Recoverable pending permission returned by the backend fetch seam. */
export type PendingPermissionPayload = PermissionRequestPayload & {
  request_id: string
  requested_at: string
}

/** Fetch the pending permission currently blocking a session, if any. */
export async function getPendingPermission(
  sessionId: string,
): Promise<PendingPermissionPayload | null> {
  return getApiClient().call<PendingPermissionPayload | null>('get_pending_permission', {
    sessionId,
  })
}

/**
 * Resume a failed or recoverable agent turn using a resume cursor.
 *
 * Calls the backend `resume_run` command which replays the context
 * from the resume checkpoint and continues streaming from where the
 * previous turn left off. Returns a new stream id for the resumed run.
 *
 * Relates to T-012 / MIG-021 recoverability — the `resumeCursor` is
 * obtained from the `stream_complete` / `stream_error` event's
 * `resume_cursor` field.
 */
export async function resumeRun(
  sessionId: string,
  resumeCursor: string,
  permissionMode?: PermissionMode,
  selectedModel?: string,
): Promise<string> {
  const [providerId, modelId] = selectedModel?.split('/') ?? []
  const args: Record<string, unknown> = {
    sessionId,
    resumeCursor,
    permissionMode,
  }
  if (providerId && modelId) {
    args.providerId = providerId
    args.modelId = modelId
  }
  return getApiClient().call<string>('resume_run', args)
}
