// GF-03 PR-4 — Session title stage React surface.
//
// Wraps the pure helpers in `./titleStage` with the four React-bound
// operations the chat UI needs.  Source: App.tsx ≈ L283-288 (refs),
// L632-856 (4 callbacks), L1356-1368 (`handleRenameSession`).

import { useEffect } from "react";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";

import { generateSessionTitle, renameSession } from "@/api";
import type { SessionMeta } from "@/api";
import { toast } from "sonner";

import type { Conversation, SessionTitleState } from "@/modules/chat/types";

import {
  MAX_AUTO_RENAME_COUNT,
  PLACEHOLDER_SESSION_TITLE,
  currentTitleIsResolved,
  getInitialSessionTitleCandidate,
  getInitialSessionTitleState,
  getMeaningfulUserMessages,
} from "./titleStage";

export interface UseSessionTitleStageDeps {
  conversations: Record<string, Conversation>;
  projectSessions: Record<string, SessionMeta[]>;
  sessionTitleStates: Record<string, SessionTitleState>;
  setConversations: Dispatch<SetStateAction<Record<string, Conversation>>>;
  setProjectSessions: Dispatch<SetStateAction<Record<string, SessionMeta[]>>>;
  setSessionTitleStates: Dispatch<
    SetStateAction<Record<string, SessionTitleState>>
  >;
  sessionTitleStatesRef: MutableRefObject<Record<string, SessionTitleState>>;
  pendingAutoTitleSessionIdsRef: MutableRefObject<Set<string>>;
}

export interface SessionTitleStageApi {
  maybeAutoRenameSession: (
    projectId: string,
    sessionId: string,
    conversation: Conversation,
  ) => void;
  syncSessionTitle: (projectId: string, sessionId: string, title: string) => void;
  syncGeneratedSessionTitle: (
    projectId: string,
    sessionId: string,
    titleHint: string,
  ) => void;
  handleRenameSession: (sessionId: string, newTitle: string) => void;
}

/**
 * React surface over the pure title-stage helpers.  Mirrors the
 * legacy App.tsx behaviour, including non-stable callback identity
 * (callers must not depend on referential stability).
 */
export function useSessionTitleStage(
  deps: UseSessionTitleStageDeps,
): SessionTitleStageApi {
  const {
    conversations,
    projectSessions,
    sessionTitleStates,
    setConversations,
    setProjectSessions,
    setSessionTitleStates,
    sessionTitleStatesRef,
    pendingAutoTitleSessionIdsRef,
  } = deps;

  // Mirror App.tsx L286-288 — keep ref in sync with latest state.
  useEffect(() => {
    sessionTitleStatesRef.current = sessionTitleStates;
  }, [sessionTitleStates, sessionTitleStatesRef]);

  const syncSessionTitle = (
    projectId: string,
    sessionId: string,
    title: string,
  ) => {
    const nextTitle = title.trim();
    if (!nextTitle) return;
    const previousTitle =
      conversations[sessionId]?.title ?? PLACEHOLDER_SESSION_TITLE;

    setConversations((prev) => {
      const c = prev[sessionId];
      if (!c || c.title === nextTitle) return prev;
      return { ...prev, [sessionId]: { ...c, title: nextTitle } };
    });
    setProjectSessions((prev) => {
      const sessions = prev[projectId];
      if (!sessions) return prev;
      let changed = false;
      const next = sessions.map((s) => {
        if (s.id !== sessionId || s.title === nextTitle) return s;
        changed = true;
        return { ...s, title: nextTitle };
      });
      return changed ? { ...prev, [projectId]: next } : prev;
    });

    void renameSession(sessionId, nextTitle).catch((err) => {
      console.error("Failed to rename session:", err);
      toast.error("重命名失败，已恢复原名称", { duration: 3000 });
      setConversations((prev) => {
        const c = prev[sessionId];
        if (!c || c.title !== nextTitle) return prev;
        return { ...prev, [sessionId]: { ...c, title: previousTitle } };
      });
      setProjectSessions((prev) => {
        const sessions = prev[projectId];
        if (!sessions) return prev;
        return {
          ...prev,
          [projectId]: sessions.map((s) =>
            s.id === sessionId && s.title === nextTitle
              ? { ...s, title: previousTitle }
              : s,
          ),
        };
      });
    });
  };

  /** Roll a session-meta entry back to its previous snapshot (or
   * reset its title fields if no prior snapshot is recorded). */
  const rollbackProjectSession = (
    projectId: string,
    sessionId: string,
    previousSession: SessionMeta | null,
    previousTitle: string,
  ) =>
    setProjectSessions((prev) => {
      const sessions = prev[projectId];
      if (!sessions) return prev;
      return {
        ...prev,
        [projectId]: sessions.map((s) =>
          s.id === sessionId
            ? previousSession ?? {
                ...s,
                title: previousTitle,
                title_pending: false,
              }
            : s,
        ),
      };
    });

  const syncGeneratedSessionTitle = (
    projectId: string,
    sessionId: string,
    titleHint: string,
  ) => {
    const nextTitle = titleHint.trim();
    if (!nextTitle) return;
    const previousTitle =
      conversations[sessionId]?.title ?? PLACEHOLDER_SESSION_TITLE;
    const previousSession =
      projectSessions[projectId]?.find((s) => s.id === sessionId) ?? null;
    pendingAutoTitleSessionIdsRef.current.add(sessionId);

    setConversations((prev) => {
      const c = prev[sessionId];
      if (!c || c.title === nextTitle) return prev;
      return {
        ...prev,
        [sessionId]: { ...c, title: nextTitle, titlePending: true },
      };
    });
    setProjectSessions((prev) => {
      const sessions = prev[projectId];
      if (!sessions) return prev;
      return {
        ...prev,
        [projectId]: sessions.map((s) =>
          s.id === sessionId
            ? { ...s, title: nextTitle, title_pending: true }
            : s,
        ),
      };
    });

    void generateSessionTitle(sessionId, nextTitle)
      .then((updated) => {
        pendingAutoTitleSessionIdsRef.current.delete(sessionId);
        setConversations((prev) => {
          const c = prev[sessionId];
          if (!c) return prev;
          return {
            ...prev,
            [sessionId]: {
              ...c,
              title: updated.title,
              titleIcon: updated.title_icon ?? null,
              titlePending: Boolean(updated.title_pending),
            },
          };
        });
        setProjectSessions((prev) => {
          const sessions = prev[projectId];
          if (!sessions) return prev;
          return {
            ...prev,
            [projectId]: sessions.map((s) =>
              s.id === sessionId ? { ...s, ...updated } : s,
            ),
          };
        });
      })
      .catch((err) => {
        pendingAutoTitleSessionIdsRef.current.delete(sessionId);
        console.error("Failed to generate session title:", err);
        setConversations((prev) => {
          const c = prev[sessionId];
          if (!c || c.title !== nextTitle) return prev;
          return {
            ...prev,
            [sessionId]: { ...c, title: previousTitle, titlePending: false },
          };
        });
        rollbackProjectSession(
          projectId,
          sessionId,
          previousSession,
          previousTitle,
        );
      });
  };

  const maybeAutoRenameSession = (
    projectId: string,
    sessionId: string,
    conversation: Conversation,
  ) => {
    const titleState =
      sessionTitleStates[sessionId] ??
      getInitialSessionTitleState(conversation.title, conversation.messages);

    if (titleState.stage === "manual" || titleState.stage === "locked") return;
    if (getMeaningfulUserMessages(conversation.messages).length === 0) return;
    if (!conversation.messages.some((m) => m.role === "assistant")) return;
    if (
      conversation.titlePending ||
      pendingAutoTitleSessionIdsRef.current.has(sessionId)
    ) {
      return;
    }

    if (currentTitleIsResolved(conversation.title)) {
      setSessionTitleStates((prev) => ({
        ...prev,
        [sessionId]: {
          ...titleState,
          stage: "locked",
          autoRenameCount: Math.max(titleState.autoRenameCount, 1),
        },
      }));
      return;
    }

    const initialCandidate = getInitialSessionTitleCandidate(
      conversation.messages,
      conversation,
    );
    if (titleState.stage === "placeholder" && initialCandidate) {
      syncGeneratedSessionTitle(projectId, sessionId, initialCandidate);
      setSessionTitleStates((prev) => ({
        ...prev,
        [sessionId]: {
          stage: "locked",
          autoRenameCount: MAX_AUTO_RENAME_COUNT,
        },
      }));
    }
  };

  // P0: user-initiated rename — locks stage to "manual".
  const handleRenameSession = (sessionId: string, newTitle: string) => {
    const conv = conversations[sessionId];
    if (!conv) return;
    setSessionTitleStates((prev) => ({
      ...prev,
      [sessionId]: {
        stage: "manual",
        autoRenameCount: MAX_AUTO_RENAME_COUNT,
      },
    }));
    syncSessionTitle(conv.projectId, sessionId, newTitle);
  };

  return {
    maybeAutoRenameSession,
    syncSessionTitle,
    syncGeneratedSessionTitle,
    handleRenameSession,
  };
}
