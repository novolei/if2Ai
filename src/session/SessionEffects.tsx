// GF-03 PR-6 — `useSessionRuntime` consolidates the chat-stream
// lifecycle previously inlined in App.tsx: sendChatTurn, stop,
// resume-from-cursor, undo / redo, undo-status polling, plus refs.
//
// ER-03 (resolved): the rapid session-switch race window is closed by
// `runtimeProjectionStore.swapSession(newSessionId)`, called from
// `handleSelectSession` in App.tsx before `loadConversationHistory`.
// The store atomically clears its snapshot and binds a session guard
// so the reducer drops any in-flight `runtime_event` rows that still
// reference the previous session.

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import { toast } from "sonner";

import {
  sessionRedo,
  sessionUndo,
  sessionUndoStatus,
  type ConversationUndoStatus,
} from "@/api";
import { stopAgentStream as stopAgentStreamCommand } from "@/api/streaming";
import { runtimeProjectionStore } from "@/runtime-projection";
import type { PermissionMode } from "@/transport/contracts";
import type { TodoItem } from "@/components/ui/TodoPanel";

import type { Conversation } from "@/modules/chat/types";
import type { AgentVoiceBridge } from "@/modules/chat/useAgentVoiceBridge";

import {
  buildResumePrompt,
  sendChatTurn,
  type SendChatTurnOptions,
} from "./sendChatTurn";

type Set_<T> = Dispatch<SetStateAction<T>>;

export interface UseSessionRuntimeDeps {
  activeProjectId: string | null;
  activeSessionId: string | null;
  projects: { id: string }[];
  conversations: Record<string, Conversation>;
  sessionLoading: Record<string, boolean>;
  streamAbortHandles: Record<string, string>;
  input: string;
  permissionMode: PermissionMode;
  selectedModel: string;
  currentProjectWorkdir: string | undefined;
  agentVoice: AgentVoiceBridge;
  setInput: Set_<string>;
  setSessionLoading: Set_<Record<string, boolean>>;
  setSessionTodos: Set_<Record<string, TodoItem[]>>;
  setStreamAbortHandles: Set_<Record<string, string>>;
  setConversations: Set_<Record<string, Conversation>>;
  handleNewChat: (projectId: string) => Promise<string | null>;
  maybeAutoRenameSession: (p: string, s: string, c: Conversation) => void;
  refreshProjectSessions: (projectId: string) => Promise<void>;
  reloadSession: (projectId: string, sessionId: string) => Promise<void>;
  extractTodosFromToolResult: (raw: string | null | undefined) => TodoItem[] | null;
  /// Active conversation message count — drives undo-status refetch.
  activeMessagesLength: number;
}

export interface SessionRuntimeApi {
  sendChatTurn: (
    overrideText?: string,
    options?: SendChatTurnOptions,
  ) => Promise<void>;
  stopAgentStream: (sessionIdOverride?: string) => Promise<void>;
  handleResumeFromCursor: (resumeCursor: string) => Promise<void>;
  handleConversationUndo: () => Promise<void>;
  handleConversationRedo: () => Promise<void>;
  conversationUndoStatus: ConversationUndoStatus | null;
}

/// React-bound surface around `sendChatTurn` + the stream lifecycle
/// it co-owns with the projection store.  Callers depend on the
/// returned object identity lasting across renders the same way the
/// original closures in App.tsx did, so we memoise via refs that
/// always read current snapshots.
export function useSessionRuntime(
  deps: UseSessionRuntimeDeps,
): SessionRuntimeApi {
  const sessionLoadingRef = useRef<Record<string, boolean>>({});
  const autoResumeAttemptsRef = useRef<Record<string, number>>({});
  const attemptedAutoResumeCursorsRef = useRef<Set<string>>(new Set());

  // Mirror sessionLoading into the ref so the (synchronous) re-entry
  // guard inside sendChatTurn sees the latest value without rebinding.
  useEffect(() => {
    sessionLoadingRef.current = deps.sessionLoading;
  }, [deps.sessionLoading]);

  // Snapshot deps in a ref so callbacks keep stable identity while
  // observing the latest values (matches the captured-closure
  // semantics of App.tsx prior to this extraction).
  const depsRef = useRef(deps);
  useEffect(() => {
    depsRef.current = deps;
  });

  const sendChatTurnRef = useRef<
    ((overrideText?: string, options?: SendChatTurnOptions) => Promise<void>) | null
  >(null);
  const doSendChatTurn = useCallback(
    async (overrideText?: string, options?: SendChatTurnOptions) => {
      const d = depsRef.current;
      await sendChatTurn({
        text: overrideText,
        options,
        deps: {
          activeProjectId: d.activeProjectId,
          activeSessionId: d.activeSessionId,
          projects: d.projects,
          conversations: d.conversations,
          input: d.input,
          permissionMode: d.permissionMode,
          selectedModel: d.selectedModel,
          currentProjectWorkdir: d.currentProjectWorkdir,
          sessionLoadingRef,
          autoResumeAttemptsRef,
          attemptedAutoResumeCursorsRef,
          setInput: d.setInput,
          setSessionLoading: d.setSessionLoading,
          setSessionTodos: d.setSessionTodos,
          setStreamAbortHandles: d.setStreamAbortHandles,
          setConversations: d.setConversations,
          handleNewChat: d.handleNewChat,
          maybeAutoRenameSession: d.maybeAutoRenameSession,
          refreshProjectSessions: d.refreshProjectSessions,
          agentVoice: d.agentVoice,
          extractTodosFromToolResult: d.extractTodosFromToolResult,
          resume: (text, opts) => sendChatTurnRef.current!(text, opts),
        },
      });
    },
    [],
  );
  sendChatTurnRef.current = doSendChatTurn;

  const stopAgentStream = useCallback(
    async (sessionIdOverride?: string) => {
      const d = depsRef.current;
      const sessionId = sessionIdOverride ?? d.activeSessionId;
      if (!sessionId) return;
      const streamAbortHandle = d.streamAbortHandles[sessionId];
      if (!streamAbortHandle) return;
      try {
        await stopAgentStreamCommand(streamAbortHandle);
      } catch (err) {
        console.error("Failed to stop stream:", err);
      }
      d.setStreamAbortHandles((prev) => {
        const { [sessionId]: _removed, ...rest } = prev;
        return rest;
      });
      d.setSessionLoading((prev) => ({ ...prev, [sessionId]: false }));
    },
    [],
  );

  const handleResumeFromCursor = useCallback(
    async (resumeCursor: string) => {
      await doSendChatTurn(buildResumePrompt(resumeCursor), {
        isInternalResume: true,
        resumeCursor,
      });
    },
    [doSendChatTurn],
  );

  const [conversationUndoStatus, setConversationUndoStatus] =
    useState<ConversationUndoStatus | null>(null);

  const refreshConversationUndoStatus = useCallback(async () => {
    const d = depsRef.current;
    if (!d.activeSessionId) {
      setConversationUndoStatus(null);
      return;
    }
    try {
      setConversationUndoStatus(await sessionUndoStatus(d.activeSessionId));
    } catch {
      setConversationUndoStatus(null);
    }
  }, []);

  useEffect(() => {
    void refreshConversationUndoStatus();
  }, [
    refreshConversationUndoStatus,
    deps.activeSessionId,
    deps.activeMessagesLength,
  ]);

  const undoRedoCommon = useCallback(
    async (op: "undo" | "redo") => {
      const d = depsRef.current;
      if (!d.activeSessionId || !d.activeProjectId) return;
      if (d.sessionLoading[d.activeSessionId]) {
        const ok = window.confirm(
          op === "undo"
            ? "当前会话仍在生成回复，撤销将先停止流式输出。是否继续？"
            : "当前会话仍在生成回复，重做将先停止流式输出。是否继续？",
        );
        if (!ok) return;
        await stopAgentStream(d.activeSessionId);
      }
      try {
        if (op === "undo") {
          await sessionUndo(d.activeSessionId);
        } else {
          await sessionRedo(d.activeSessionId);
        }
        runtimeProjectionStore.dispatch({
          kind: "projection_discard_session_runs",
          sessionId: d.activeSessionId,
          receivedAt: Date.now(),
        });
        runtimeProjectionStore.flush();
        await d.reloadSession(d.activeProjectId, d.activeSessionId);
        toast.success(op === "undo" ? "已撤销" : "已重做");
      } catch (err) {
        toast.error(op === "undo" ? "撤销失败" : "重做失败", {
          description: String(err),
        });
      }
      void refreshConversationUndoStatus();
    },
    [refreshConversationUndoStatus, stopAgentStream],
  );

  const handleConversationUndo = useCallback(
    () => undoRedoCommon("undo"),
    [undoRedoCommon],
  );
  const handleConversationRedo = useCallback(
    () => undoRedoCommon("redo"),
    [undoRedoCommon],
  );

  return useMemo<SessionRuntimeApi>(
    () => ({
      sendChatTurn: doSendChatTurn,
      stopAgentStream,
      handleResumeFromCursor,
      handleConversationUndo,
      handleConversationRedo,
      conversationUndoStatus,
    }),
    [
      doSendChatTurn,
      stopAgentStream,
      handleResumeFromCursor,
      handleConversationUndo,
      handleConversationRedo,
      conversationUndoStatus,
    ],
  );
}
