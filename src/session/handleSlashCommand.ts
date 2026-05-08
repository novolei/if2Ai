// GF-03 PR-6 — slash-command branch extracted from `App.tsx`'s
// `sendMessage` (≈ L1426-1580).  Three sub-routes:
//
//   (1) Skill slash ────── `resolveSkillSlash` returns a non-null
//       SKILL.md invocation; route through the agent so the LLM
//       activates the skill and streams a normal turn.
//   (2) `/compact` ─────── manual context compaction via
//       `chatCompactSession`; renders a static assistant card.
//   (3) Builtin slash ─── `executeSlashCommand` → static result card
//       (`/help`, `/clear`, `/skills`, `/agents`, …).
//
// Behaviour mirrors App.tsx 1:1; the skill-slash branch reuses the
// streaming side-effect handler supplied by `sendChatTurn`.

import type { Dispatch, SetStateAction } from "react";

import {
  executeSlashCommand,
  resolveSkillSlash,
  startChatTurn,
  suggestSlashCommands,
} from "@/api";
import { runtimeProjectionStore } from "@/runtime-projection";
import type {
  PermissionMode,
  StreamTokenPayload,
} from "@/transport/contracts";

import type { Conversation, Message } from "@/modules/chat/types";

export interface SlashCommandDeps {
  sessionId: string;
  messageText: string;
  permissionMode: PermissionMode;
  selectedModel: string;
  cwd: string | undefined;
  setSessionLoading: Dispatch<
    SetStateAction<Record<string, boolean>>
  >;
  setStreamAbortHandles: Dispatch<
    SetStateAction<Record<string, string>>
  >;
  setConversations: Dispatch<
    SetStateAction<Record<string, Conversation>>
  >;
  // Streaming hooks supplied by sendChatTurn.  When the branch is the
  // skill-slash path we reuse the parent's createAssistantMessage so
  // assistant-message id sequencing matches the normal-turn path.
  createAssistantMessage: (streamId?: string) => void;
  handleProjectedStreamSideEffect: (
    payload: StreamTokenPayload,
    unlisten: () => void,
  ) => void;
}

export type SlashCommandOutcome =
  | { kind: "handled" }
  | { kind: "not_a_slash" };

/// Try to dispatch `messageText` as a slash command.  Returns
/// `{ kind: "handled" }` when the input was consumed (skill / compact /
/// builtin); returns `{ kind: "not_a_slash" }` so the caller can fall
/// through to the regular agent turn.
export async function handleSlashCommand(
  deps: SlashCommandDeps,
): Promise<SlashCommandOutcome> {
  const { messageText, sessionId } = deps;
  if (!messageText.startsWith("/")) return { kind: "not_a_slash" };

  const cmdPrefix = messageText.split(/\s+/)[0];
  const suggestions = await suggestSlashCommands(cmdPrefix, 1);
  if (suggestions.length === 0) return { kind: "not_a_slash" };

  // (1) Skill slash — full SKILL.md invocation through the agent.
  const skillInvocation = await resolveSkillSlash(messageText, deps.cwd);
  if (skillInvocation) {
    try {
      deps.createAssistantMessage();
      const handle = await startChatTurn({
        sessionId,
        userMessage: skillInvocation,
        permissionMode: deps.permissionMode,
        selectedModel: deps.selectedModel,
      });
      runtimeProjectionStore.dispatch({
        kind: "stream_run_bound",
        runId: handle.streamId,
        sessionId,
        receivedAt: Date.now(),
      });
      deps.createAssistantMessage(handle.streamId);
      deps.setStreamAbortHandles((prev) => ({
        ...prev,
        [sessionId]: handle.streamId,
      }));
      const unlisten = await handle.subscribe(
        (payload: StreamTokenPayload) => {
          deps.handleProjectedStreamSideEffect(payload, unlisten);
        },
      );
    } catch {
      deps.setSessionLoading((prev) => ({ ...prev, [sessionId]: false }));
    }
    return { kind: "handled" };
  }

  // (2) /compact — manual compaction.
  const trimmedSlash = messageText.trim();
  if (
    trimmedSlash === "/compact" ||
    trimmedSlash.startsWith("/compact ")
  ) {
    deps.setSessionLoading((prev) => ({ ...prev, [sessionId]: false }));
    try {
      // Dynamic import preserved — matches App.tsx behaviour and keeps
      // the @/lib/tauri shim out of the hot path until /compact runs.
      const { chatCompactSession } = await import("@/lib/tauri");
      const report = await chatCompactSession(sessionId);
      const assistantMsgId = crypto.randomUUID();
      const content = report.didCompact
        ? `已压缩 ${report.summarizedMessages} 条消息，释放约 ${
            report.freedTokens > 0
              ? `${(report.freedTokens / 1000).toFixed(1)}k`
              : "0"
          } tokens。\n\n摘要预览：${report.summaryExcerpt}…`
        : "上下文已是最新，无需压缩。";
      const assistantMsg: Message = {
        id: assistantMsgId,
        role: "assistant",
        content,
        timestamp: new Date(),
        isStreaming: false,
        slashCommand: "/compact",
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
    } catch (err) {
      const errorMsg: Message = {
        id: crypto.randomUUID(),
        role: "assistant",
        content: `/compact 失败: ${String(err)}`,
        timestamp: new Date(),
        isStreaming: false,
      };
      deps.setConversations((prev) => {
        const currentConv = prev[sessionId];
        if (!currentConv) return prev;
        return {
          ...prev,
          [sessionId]: {
            ...currentConv,
            messages: [...currentConv.messages, errorMsg],
          },
        };
      });
    }
    return { kind: "handled" };
  }

  // (3) Builtin slash — static result card.
  deps.setSessionLoading((prev) => ({ ...prev, [sessionId]: false }));
  try {
    const result = await executeSlashCommand(messageText, sessionId);
    const assistantMsgId = crypto.randomUUID();
    const slashToken = messageText.trim().split(/\s+/, 1)[0];
    const assistantMsg: Message = {
      id: assistantMsgId,
      role: "assistant",
      content: result,
      timestamp: new Date(),
      isStreaming: false,
      slashCommand:
        slashToken && slashToken.startsWith("/") ? slashToken : undefined,
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
  } catch (err) {
    const errorMsg: Message = {
      id: crypto.randomUUID(),
      role: "assistant",
      content: String(err),
      timestamp: new Date(),
      isStreaming: false,
    };
    deps.setConversations((prev) => {
      const currentConv = prev[sessionId];
      if (!currentConv) return prev;
      return {
        ...prev,
        [sessionId]: {
          ...currentConv,
          messages: [...currentConv.messages, errorMsg],
        },
      };
    });
  }
  return { kind: "handled" };
}
