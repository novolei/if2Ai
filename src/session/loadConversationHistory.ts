// GF-03 PR-2 — `handleSelectSession` history → `Message[]` reducer.
//
// Extracted from `src/App.tsx` (L1456-1649).  The original code was a
// 200-line block buried inside a React event handler that mixed three
// setter calls with a complex try/catch reducer over `getSession` +
// `getSessionHistoryPage`.  Splitting it into a pure async function
// keeps `handleSelectSession` to a thin "call → write three slices →
// fail safe" shape and lets us cover the conversion logic with focused
// node:test cases (see `__tests__/loadConversationHistory.test.ts`).

import { getSession, getSessionHistoryPage } from "@/api";
import type { SessionMeta } from "@/api";
import { replayRunLogEntriesToMessages } from "@/runtime-projection";
import type {
  Conversation,
  Message,
  SessionTitleState,
} from "@/modules/chat/types";
import type { TodoItem } from "@/components/ui/TodoPanel";
import { extractTodosFromToolResult } from "./messageExtraction";
import {
  PLACEHOLDER_SESSION_TITLE,
  getInitialSessionTitleState,
} from "./titleStage";

/**
 * Result of {@link loadConversationHistory}.  The three fields map 1:1
 * onto the three setter calls the caller must perform
 * (`setConversations`, `setSessionTitleStates`, `setSessionTodos`).
 */
export type LoadedSessionHistory = {
  conversation: Conversation;
  recoveredTodos: TodoItem[];
  titleState: SessionTitleState;
};

// GF-03 PR-4 — the title-state derivation now lives canonically in
// `./titleStage.getInitialSessionTitleState`; the local replica that
// PR-2 introduced has been removed and the call sites below import
// directly from there.

/**
 * Load + convert a session's persisted history into the shape the chat
 * UI consumes.  Strategy:
 *
 *   1. Fetch `getSession` (legacy `messages[]`) and `getSessionHistoryPage`
 *      (canonical run-log entries) in parallel.  History page failure is
 *      non-fatal — we degrade to the legacy reducer.
 *   2. Run two reducers in parallel:
 *      - **fullSession reducer**: walks `messages[].blocks[]` and folds
 *        `tool_use` + `tool_result` blocks into a single tool `Message`
 *        keyed by tool-call id.  Also harvests the latest `TodoWrite`
 *        result into `recoveredTodos`.
 *      - **replay reducer**: `replayRunLogEntriesToMessages` projects
 *        canonical run-log entries through the same reducer the live
 *        UI uses, with `disableAnimation: true` so reload doesn't
 *        re-trigger streaming animations.
 *   3. Pick the replay messages **only when** they include any
 *      assistant/tool entry (i.e. the run-log actually has substance).
 *      Otherwise fall back to the fullSession messages so legacy
 *      sessions and the moments before the run-log catches up still
 *      render correctly.
 *   4. On any thrown error (network, malformed payload, …) return an
 *      empty conversation seeded from `sessionMeta` so the UI can still
 *      show the title row instead of blanking out.
 *
 * The function never throws — it always resolves to a valid
 * `LoadedSessionHistory`.  The error path is logged via `console.error`
 * to preserve the original observability behaviour from App.tsx.
 */
export async function loadConversationHistory(args: {
  sessionId: string;
  projectId: string;
  sessionMeta: SessionMeta | undefined;
  projectWorkdir: string | undefined;
}): Promise<LoadedSessionHistory> {
  const { sessionId, projectId, sessionMeta, projectWorkdir } = args;

  try {
    const [fullSession, historyPage] = await Promise.all([
      getSession(sessionId),
      getSessionHistoryPage(sessionId, { limit: 1000 }).catch((error) => {
        console.warn("Failed to load session run-log history:", error);
        return null;
      }),
    ]);

    const baseTimestamp = new Date(fullSession.updated_at);
    let convertedMessages: Message[] = [];
    const replayedMessages =
      historyPage && historyPage.eventPage.entries.length > 0
        ? replayRunLogEntriesToMessages(
            historyPage.eventPage.entries,
            sessionId,
          ).map((message) => ({
            ...message,
            disableAnimation: true,
          }))
        : [];
    const toolMessageIndexById = new Map<string, number>();
    let recoveredTodos: TodoItem[] = [];

    const upsertToolMessage = (toolCallId: string, nextMessage: Message) => {
      const existingIndex = toolMessageIndexById.get(toolCallId);
      if (existingIndex !== undefined) {
        convertedMessages[existingIndex] = {
          ...convertedMessages[existingIndex],
          ...nextMessage,
          id: convertedMessages[existingIndex].id,
          toolCallId,
        };
        return;
      }

      const index = convertedMessages.push(nextMessage) - 1;
      toolMessageIndexById.set(toolCallId, index);
    };

    for (const msg of fullSession.messages) {
      if (msg.role === "system") {
        continue;
      }
      const messageOutcome = {
        requestId: msg.request_id,
        taskOutcome: msg.task_outcome,
        degradedReason: msg.degraded_reason,
        resumeAvailable: msg.resume_available,
        resumeCursor: msg.resume_cursor,
      };
      let pushedAssistantText = false;
      for (const block of msg.blocks) {
        if (block.type === "tool_use" && block.tool_use_block) {
          const toolCallId = block.tool_use_block.id;
          upsertToolMessage(toolCallId, {
            id: `tool-use-${toolCallId}`,
            role: "tool",
            content: "",
            timestamp: baseTimestamp,
            toolCallId,
            toolName: block.tool_use_block.name,
            toolArgs: block.tool_use_block.input as
              | Record<string, unknown>
              | undefined,
            toolStatus: "running",
            policyDecision: "prompt",
            evidenceId: toolCallId,
            effectiveWorkdir: projectWorkdir,
            disableAnimation: true,
            ...messageOutcome,
          });
          continue;
        }

        if (block.type === "tool_result" && block.tool_use_id) {
          const toolCallId = block.tool_use_id;
          const existingIndex = toolMessageIndexById.get(toolCallId);
          const toolArgs =
            existingIndex !== undefined
              ? convertedMessages[existingIndex]?.toolArgs
              : undefined;
          upsertToolMessage(toolCallId, {
            id: `tool-${toolCallId}-${Date.now()}`,
            role: "tool",
            content: block.output || "",
            timestamp: baseTimestamp,
            toolCallId,
            toolName: block.tool_name || "unknown",
            toolArgs,
            toolStatus: "completed",
            policyDecision: "allow",
            evidenceId: toolCallId,
            effectiveWorkdir: projectWorkdir,
            isError: false,
            disableAnimation: true,
            ...messageOutcome,
          });
          const isTodoWriteResult =
            (block.tool_name ?? "").trim() === "TodoWrite" ||
            (existingIndex !== undefined &&
              convertedMessages[existingIndex]?.toolName?.trim() ===
                "TodoWrite");
          if (isTodoWriteResult) {
            const nextTodos = extractTodosFromToolResult(block.output);
            if (nextTodos) {
              recoveredTodos = nextTodos;
            }
          }
          continue;
        }

        if (block.type === "text" && block.text) {
          pushedAssistantText = true;
          convertedMessages.push({
            id: `${msg.role}-${crypto.randomUUID()}`,
            role: msg.role as "user" | "assistant",
            content: block.text,
            timestamp: baseTimestamp,
            thinking: msg.thinking,
            disableAnimation: true,
            ...messageOutcome,
          });
        }
      }

      if (msg.role === "assistant" && msg.thinking && !pushedAssistantText) {
        convertedMessages.push({
          id: `${msg.role}-${crypto.randomUUID()}`,
          role: "assistant",
          content: "",
          timestamp: baseTimestamp,
          thinking: msg.thinking,
          disableAnimation: true,
          ...messageOutcome,
        });
      }
    }

    if (
      replayedMessages.some(
        (message) => message.role === "assistant" || message.role === "tool",
      )
    ) {
      convertedMessages = replayedMessages;
    }

    const resolvedTitle =
      fullSession.title || sessionMeta?.title || PLACEHOLDER_SESSION_TITLE;

    return {
      conversation: {
        id: sessionId,
        projectId,
        title: resolvedTitle,
        titleIcon: fullSession.title_icon ?? sessionMeta?.title_icon ?? null,
        titlePending: Boolean(
          fullSession.title_pending ?? sessionMeta?.title_pending,
        ),
        messages: convertedMessages,
        updatedAt: new Date(fullSession.updated_at),
        sessionTotals: fullSession.session_totals,
      },
      recoveredTodos,
      titleState: getInitialSessionTitleState(resolvedTitle),
    };
  } catch (err) {
    console.error("Failed to load session:", err);
    const fallbackTitle = sessionMeta?.title || PLACEHOLDER_SESSION_TITLE;
    return {
      conversation: {
        id: sessionId,
        projectId,
        title: fallbackTitle,
        titleIcon: sessionMeta?.title_icon ?? null,
        titlePending: Boolean(sessionMeta?.title_pending),
        messages: [],
        updatedAt: new Date(),
      },
      recoveredTodos: [],
      titleState: getInitialSessionTitleState(fallbackTitle),
    };
  }
}
