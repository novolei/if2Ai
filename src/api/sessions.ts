// MIG-012 — session domain facade.
//
// Every session CRUD path AppShell / chat store needs, with the
// underlying Tauri command names (`create_session`,
// `get_session`, `rename_session`, ...) kept as implementation
// details of this module.

import { getApiClient } from "./client.ts";

// Domain types are re-exported from `@/lib/tauri` today; once
// MIG-003 lands a canonical projection layer they will move to
// `@/transport/contracts`. Re-exporting here means every
// `src/api/*` consumer imports from a single place even during
// the cut-over.
import type {
  ConversationUndoStatus,
  Session,
  SessionIdentityInput,
  SessionMeta,
} from "@/lib/tauri";

export type {
  ConversationUndoStatus,
  Session,
  SessionIdentityInput,
  SessionMeta,
};

export interface RunLogEntry {
  event_id: string;
  session_id: string;
  run_id: string;
  seq: number;
  event_type: string;
  occurred_at: string;
  payload: Record<string, unknown>;
  causation_id?: string | null;
  correlation_id?: string | null;
  tool_call_id?: string | null;
  attempt_id?: string | null;
}

export interface SessionHistoryEventPage {
  sessionId: string;
  entries: RunLogEntry[];
  nextCursor?: string | null;
  hasMore: boolean;
}

export interface HistoryReplayMessage {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  runId: string;
  occurredAt: string;
  thinking?: string | null;
  toolCallId?: string | null;
  toolName?: string | null;
  toolStatus?: string | null;
  taskOutcome?: "completed" | "partial_success" | "failed" | null;
  degradedReason?: string | null;
  resumeCursor?: string | null;
}

export interface SessionHistoryReplay {
  sessionId: string;
  messages: HistoryReplayMessage[];
  runCount: number;
  eventCount: number;
}

export interface SessionHistoryPageResponse {
  eventPage: SessionHistoryEventPage;
  replay: SessionHistoryReplay;
  fallbackSession?: Session | null;
}

/** Create a new session in the given project. Pass `''` to
 * associate the session with no project. */
export async function createSession(
  projectId: string,
  title: string,
  identity?: SessionIdentityInput | null,
): Promise<SessionMeta> {
  return getApiClient().call<SessionMeta>(
    "create_session",
    identity === undefined
      ? { projectId, title }
      : { projectId, title, identity },
  );
}

/** Fetch the full session record (metadata + message history). */
export async function getSession(id: string): Promise<Session> {
  return getApiClient().call<Session>("get_session", { id });
}

export async function sessionUndoStatus(
  id: string,
): Promise<ConversationUndoStatus> {
  return getApiClient().call<ConversationUndoStatus>("session_undo_status", {
    id,
  });
}

export async function sessionUndo(id: string): Promise<Session> {
  return getApiClient().call<Session>("session_undo", { id });
}

export async function sessionRedo(id: string): Promise<Session> {
  return getApiClient().call<Session>("session_redo", { id });
}

/** Drain buffered job-monitor diagnostic lines (P2-12). */
export async function drainJobMonitorLines(id: string): Promise<string[]> {
  return getApiClient().call<string[]>("drain_job_monitor_lines", { id });
}

/** Fetch a canonical event-log history page and replay projection. */
export async function getSessionHistoryPage(
  id: string,
  options: { limit?: number; cursor?: string | null } = {},
): Promise<SessionHistoryPageResponse> {
  return getApiClient().call<SessionHistoryPageResponse>(
    "get_session_history_page",
    {
      id,
      limit: options.limit,
      cursor: options.cursor ?? null,
    },
  );
}

/** Rename a session. Returns the updated metadata. */
export async function renameSession(
  id: string,
  title: string,
): Promise<SessionMeta> {
  return getApiClient().call<SessionMeta>("rename_session", { id, title });
}

/** Delete a session by id. */
export async function deleteSession(id: string): Promise<void> {
  return getApiClient().call<void>("delete_session", { id });
}

/**
 * MEM-MOD-WIRE-FIX-2 — fire the backend `MemoryTicker::on_session_end`
 * hook for `id`.  Call this whenever a session loses focus (creating
 * a new one, switching, closing the app, etc.) so the rolling summary
 * → compile_today → reflection extraction → learned_traits
 * distillation pipeline actually runs.
 *
 * Best-effort: failures are logged but do not surface to the user
 * (the session itself is unaffected — only the post-session memory
 * pipeline is delayed until the next end-hook).
 */
export async function closeSession(id: string): Promise<void> {
  return getApiClient().call<void>("close_session", { id });
}

/** Pin / unpin a session in the project rail. */
export async function setSessionPinned(
  id: string,
  pinned: boolean,
): Promise<void> {
  return getApiClient().call<void>("set_session_pinned", { id, pinned });
}

/** Update the session-level identity override. */
export async function setSessionIdentity(
  id: string,
  identity: SessionIdentityInput,
): Promise<SessionMeta> {
  return getApiClient().call<SessionMeta>("set_session_identity", {
    id,
    identity,
  });
}
