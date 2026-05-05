// GF-03 PR-6 — `sendChatTurn` extracted from App.tsx (≈ L1117-1682).
// Pure async; deps injected. Slash branch delegated to
// `./handleSlashCommand.ts`. Assistant message id sequencing must
// match App.tsx pre-extraction (two-phase createAssistantMessage).
// ER-03 rapid session-switch race window unchanged — see TODO in
// `SessionEffects.tsx`.

import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { startChatTurn } from "@/api";
import { runtimeProjectionStore } from "@/runtime-projection";
import {
  publishLatestPromptDiagnosticsSnapshot,
  type PromptDiagnosticsSnapshot,
} from "@/modules/prompt-diagnostics/storage";
import type { PermissionMode, StreamTokenPayload } from "@/transport/contracts";
import type { TodoItem } from "@/components/ui/TodoPanel";
import type { Conversation, Message } from "@/modules/chat/types";
import type { AgentVoiceBridge } from "@/modules/chat/useAgentVoiceBridge";
import { handleSlashCommand } from "./handleSlashCommand";

export interface SendChatTurnOptions {
  sessionIdOverride?: string;
  isInternalResume?: boolean;
  resumeCursor?: string;
}

export interface SendChatTurnDeps {
  activeProjectId: string | null;
  activeSessionId: string | null;
  projects: { id: string }[];
  conversations: Record<string, Conversation>;
  input: string;
  permissionMode: PermissionMode;
  selectedModel: string;
  currentProjectWorkdir: string | undefined;
  sessionLoadingRef: MutableRefObject<Record<string, boolean>>;
  autoResumeAttemptsRef: MutableRefObject<Record<string, number>>;
  attemptedAutoResumeCursorsRef: MutableRefObject<Set<string>>;
  setInput: Dispatch<SetStateAction<string>>;
  setSessionLoading: Dispatch<SetStateAction<Record<string, boolean>>>;
  setSessionTodos: Dispatch<SetStateAction<Record<string, TodoItem[]>>>;
  setStreamAbortHandles: Dispatch<SetStateAction<Record<string, string>>>;
  setConversations: Dispatch<SetStateAction<Record<string, Conversation>>>;
  handleNewChat: (projectId: string) => Promise<string | null>;
  maybeAutoRenameSession: (
    projectId: string,
    sessionId: string,
    conversation: Conversation,
  ) => void;
  refreshProjectSessions: (projectId: string) => Promise<void>;
  agentVoice: AgentVoiceBridge;
  extractTodosFromToolResult: (raw: string | null | undefined) => TodoItem[] | null;
  /// Self-reference for the auto-resume internal recursion.
  resume: (text: string, options: SendChatTurnOptions) => Promise<void>;
}

const PLACEHOLDER_TITLE = "新对话";

const buildResumePrompt = (resumeCursor: string) =>
  `[resume_cursor] ${resumeCursor}\n` +
  "请从该游标继续完成上一次任务，仅补全未完成步骤，禁止重复已确认的副作用操作。";

const setRecoveryStateForCursor = (
  setConversations: SendChatTurnDeps["setConversations"],
  sessionId: string,
  resumeCursor: string,
  isRecovering: boolean,
) => {
  setConversations((prev) => {
    const currentConv = prev[sessionId];
    if (!currentConv) return prev;
    return {
      ...prev,
      [sessionId]: {
        ...currentConv,
        messages: currentConv.messages.map((msg) => {
          if (msg.taskOutcome !== "partial_success") return msg;
          if (msg.resumeCursor !== resumeCursor) return msg;
          return { ...msg, isRecovering };
        }),
      },
    };
  });
};

/// Send a chat turn through the canonical `startChatTurn` IPC and wire
/// up the projection-driven streaming side-effects.  Mirrors the
/// pre-extraction App.tsx `sendMessage` function 1:1.
export async function sendChatTurn(args: {
  text?: string;
  options?: SendChatTurnOptions;
  deps: SendChatTurnDeps;
}): Promise<void> {
  const { text: overrideText, options, deps } = args;
  const messageText = (overrideText ?? deps.input).trim();
  let targetSessionId = options?.sessionIdOverride ?? deps.activeSessionId;
  let freshProjectId: string | undefined;
  if (!targetSessionId && !options?.sessionIdOverride) {
    const fallbackProjectId =
      deps.activeProjectId ?? deps.projects[0]?.id ?? null;
    if (fallbackProjectId) {
      freshProjectId = fallbackProjectId;
      targetSessionId = await deps.handleNewChat(fallbackProjectId);
    }
  }
  if (!messageText || !targetSessionId) return;
  const sessionId = targetSessionId;
  if (deps.sessionLoadingRef.current[sessionId]) return;
  if (!options?.isInternalResume) {
    deps.setSessionTodos((prev) => ({ ...prev, [sessionId]: [] }));
  }

  // When handleNewChat just created this session the React state hasn't
  // re-rendered yet, so conversations[sessionId] is still undefined.
  // Construct a synthetic conv for new sessions rather than bailing.
  const conv =
    deps.conversations[sessionId] ??
    (freshProjectId
      ? {
          id: sessionId,
          projectId: freshProjectId,
          title: PLACEHOLDER_TITLE,
          messages: [],
          updatedAt: new Date(),
        }
      : null);
  if (!conv) return;

  if (!options?.isInternalResume) {
    deps.autoResumeAttemptsRef.current[sessionId] = 0;
  } else if (options.resumeCursor) {
    setRecoveryStateForCursor(
      deps.setConversations,
      sessionId,
      options.resumeCursor,
      true,
    );
  }

  const userMsg: Message = {
    id: crypto.randomUUID(),
    role: "user",
    content: messageText,
    timestamp: new Date(),
  };

  const updatedConv: Conversation = {
    ...conv,
    messages: [...conv.messages, userMsg],
    updatedAt: new Date(),
  };

  deps.setConversations((prev) => ({ ...prev, [sessionId]: updatedConv }));
  deps.maybeAutoRenameSession(conv.projectId, sessionId, updatedConv);
  if (!overrideText) {
    deps.setInput("");
  }
  deps.setSessionLoading((prev) => ({ ...prev, [sessionId]: true }));

  // Note: text_delta / thinking_delta accumulation lived as dead code
  // in App.tsx (defined but never invoked because text_delta only
  // feeds the agent voice).  Dropped in extraction; behaviour
  // unchanged.
  let assistantMsgId: string | null = null;
  let streamRafId: number | null = null;

  const createAssistantMessage = (streamId?: string) => {
    if (assistantMsgId) {
      if (streamId) {
        const currentAssistantId = assistantMsgId;
        deps.setConversations((prev) => {
          const currentConv = prev[sessionId];
          if (!currentConv) return prev;
          return {
            ...prev,
            [sessionId]: {
              ...currentConv,
              messages: currentConv.messages.map((msg) =>
                msg.id === currentAssistantId ? { ...msg, streamId } : msg,
              ),
            },
          };
        });
      }
      return;
    }
    assistantMsgId = crypto.randomUUID();
    const assistantMsg: Message = {
      id: assistantMsgId,
      role: "assistant",
      content: "",
      timestamp: new Date(),
      streamId,
      isStreaming: true,
      statusLabel: options?.isInternalResume
        ? "正在恢复未完成任务…"
        : undefined,
      statusKind: options?.isInternalResume ? "info" : undefined,
    };

    deps.setConversations((prev) => {
      const currentConv = prev[sessionId];
      if (!currentConv) return prev;
      return {
        ...prev,
        [sessionId]: {
          ...currentConv,
          messages: [...currentConv.messages, assistantMsg],
        },
      };
    });
  };

  const cancelScheduledAssistantFlush = () => {
    if (streamRafId !== null) {
      window.cancelAnimationFrame(streamRafId);
      streamRafId = null;
    }
  };

  const finishProjectedStream = (
    payload: StreamTokenPayload,
    unlisten: () => void,
  ) => {
    cancelScheduledAssistantFlush();
    void deps.agentVoice.flushAndStop();
    if (payload.prompt_diagnostics) {
      const snapshot: PromptDiagnosticsSnapshot = {
        sessionId,
        projectId: conv.projectId,
        assistantMessageId: assistantMsgId ?? null,
        updatedAt: Date.now(),
        summary: payload.prompt_diagnostics,
      };
      void publishLatestPromptDiagnosticsSnapshot(snapshot);
    }
    deps.setSessionLoading((prev) => ({ ...prev, [sessionId]: false }));
    deps.setStreamAbortHandles((prev) => {
      const { [sessionId]: _removed, ...rest } = prev;
      return rest;
    });
    void deps.refreshProjectSessions(conv.projectId).catch((error) => {
      console.error("Failed to refresh session counts:", error);
    });
    deps.setConversations((prev) => {
      const currentConv = prev[sessionId];
      if (!currentConv) return prev;
      return {
        ...prev,
        [sessionId]: {
          ...currentConv,
          messages: currentConv.messages.map((msg) =>
            msg.isRecovering ? { ...msg, isRecovering: false } : msg,
          ),
        },
      };
    });
    unlisten();
  };

  const handleProjectedStreamSideEffect = (
    payload: StreamTokenPayload,
    unlisten: () => void,
  ) => {
    if (payload.event_type === "text_delta" && payload.text) {
      deps.agentVoice.feed(payload.text);
      return;
    }

    if (payload.event_type === "tool_call_update") {
      if (payload.tool_name === "TodoWrite" && payload.tool_result) {
        const nextTodos = deps.extractTodosFromToolResult(payload.tool_result);
        if (nextTodos) {
          deps.setSessionTodos((prev) => ({ ...prev, [sessionId]: nextTodos }));
        }
      }
      return;
    }

    if (payload.event_type === "stream_complete") {
      finishProjectedStream(payload, unlisten);
      deps.autoResumeAttemptsRef.current[sessionId] = 0;
      deps.attemptedAutoResumeCursorsRef.current.clear();
      return;
    }

    if (payload.event_type === "stream_error") {
      finishProjectedStream(payload, unlisten);
      const taskOutcome = payload.task_outcome ?? "failed";
      const resumeAvailable = payload.resume_available ?? false;
      const resumeCursor = payload.resume_cursor;
      if (
        taskOutcome === "partial_success" &&
        resumeAvailable &&
        resumeCursor
      ) {
        const cursorKey = `${sessionId}:${resumeCursor}`;
        const attemptCount =
          deps.autoResumeAttemptsRef.current[sessionId] ?? 0;
        if (
          !deps.attemptedAutoResumeCursorsRef.current.has(cursorKey) &&
          attemptCount < 2
        ) {
          deps.attemptedAutoResumeCursorsRef.current.add(cursorKey);
          deps.autoResumeAttemptsRef.current[sessionId] = attemptCount + 1;
          setRecoveryStateForCursor(
            deps.setConversations,
            sessionId,
            resumeCursor,
            true,
          );
          window.setTimeout(() => {
            void deps.resume(buildResumePrompt(resumeCursor), {
              sessionIdOverride: sessionId,
              isInternalResume: true,
              resumeCursor,
            });
          }, 80);
        }
      }
    }
  };

  // Slash branch (skill / /compact / builtin).
  const slashOutcome = await handleSlashCommand({
    sessionId,
    messageText,
    permissionMode: deps.permissionMode,
    selectedModel: deps.selectedModel,
    cwd: deps.currentProjectWorkdir,
    setSessionLoading: deps.setSessionLoading,
    setStreamAbortHandles: deps.setStreamAbortHandles,
    setConversations: deps.setConversations,
    createAssistantMessage,
    handleProjectedStreamSideEffect,
  });
  if (slashOutcome.kind === "handled") return;

  // Normal turn: stream through the agent.
  try {
    createAssistantMessage();
    const handle = await startChatTurn({
      sessionId,
      userMessage: userMsg.content,
      permissionMode: deps.permissionMode,
      selectedModel: deps.selectedModel,
    });
    runtimeProjectionStore.dispatch({
      kind: "stream_run_bound",
      runId: handle.streamId,
      sessionId,
      receivedAt: Date.now(),
    });
    createAssistantMessage(handle.streamId);
    deps.setStreamAbortHandles((prev) => ({
      ...prev,
      [sessionId]: handle.streamId,
    }));

    const unlisten = await handle.subscribe(
      (payload: StreamTokenPayload) => {
        handleProjectedStreamSideEffect(payload, unlisten);
      },
    );
  } catch (err) {
    console.error("startAgentStream error:", err);

    const errorMessage =
      err && typeof err === "object" && "message" in err
        ? String(
            (err as { message?: string }).message ||
              "Agent 执行失败，请稍后重试。",
          )
        : err instanceof Error
          ? err.message
          : "Agent 执行失败，请稍后重试。";

    if (assistantMsgId) {
      const currentAssistantId = assistantMsgId;
      deps.setConversations((prev) => {
        const currentConv = prev[sessionId];
        if (!currentConv) return prev;
        return {
          ...prev,
          [sessionId]: {
            ...currentConv,
            messages: currentConv.messages.map((msg) =>
              msg.id === currentAssistantId
                ? {
                    ...msg,
                    content: "",
                    isStreaming: false,
                    isError: true,
                    statusLabel: undefined,
                    statusKind: undefined,
                    toolArgs: { rawError: errorMessage },
                  }
                : msg,
            ),
          },
        };
      });
    } else {
      // No assistant message yet — create one with the error.
      const errorMsgId = crypto.randomUUID();
      const errorMsg: Message = {
        id: errorMsgId,
        role: "assistant",
        content: "",
        timestamp: new Date(),
        isStreaming: false,
        isError: true,
        toolArgs: { rawError: errorMessage },
      };
      deps.setConversations((prev) => ({
        ...prev,
        [sessionId]: {
          ...updatedConv,
          messages: [...updatedConv.messages, errorMsg],
        },
      }));
    }
    deps.setSessionLoading((prev) => ({ ...prev, [sessionId]: false }));
    void deps.refreshProjectSessions(conv.projectId).catch((error) => {
      console.error("Failed to refresh session counts:", error);
    });
    deps.setConversations((prev) => {
      const currentConv = prev[sessionId];
      if (!currentConv) return prev;
      return {
        ...prev,
        [sessionId]: {
          ...currentConv,
          messages: currentConv.messages.map((msg) =>
            msg.isRecovering ? { ...msg, isRecovering: false } : msg,
          ),
        },
      };
    });
  }
}

export { buildResumePrompt };
