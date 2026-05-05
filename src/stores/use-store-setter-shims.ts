// GF-03 PR-3 — Store-backed setter shims for App.tsx.
//
// `App.tsx` historically defined ten `useCallback` wrappers that
// preserved React's `Dispatch<SetStateAction<T>>` shape while
// dispatching writes into the canonical zustand-style stores
// (`bootstrapStore`, `sessionStore`, chat-store / conversation
// slice). This hook collects the same ten wrappers in a single
// composition root so App.tsx stays under its 500-LOC budget
// without touching any call site contract.
//
// Behavior is byte-for-byte identical to the inline definitions
// previously living in App.tsx around L248-577 — the function
// bodies were lifted verbatim, with the only delta being the
// removal of the surrounding component closure (none of these
// wrappers ever closed over a non-`[]` dep).

import { useCallback } from 'react'

import {
  bootstrapStore,
  useBootstrapSelector,
} from '@/state'
import type { Project, ProjectMeta, SessionMeta } from '@/api'
import type { Conversation, SessionTitleState } from '@/modules/chat/types'
import type { TodoItem } from '@/components/ui/TodoPanel'
import { closeSession } from '@/api'

import {
  clearStreamAbortHandle as clearStoreStreamAbortHandle,
  getConversationSnapshot,
  initTitleState as initStoreTitleState,
  removeSession,
  sessionStore,
  setConversation,
  setSessionLoading as setStoreSessionLoading,
  setSessionTodos as setStoreSessionTodos,
  setStreamAbortHandle as setStoreStreamAbortHandle,
  setTitleState as setStoreTitleState,
  useChatStore,
  useSessionSelector,
} from '@/stores'

type SetStateLike<T> = T | ((prev: T) => T)

export interface AppStateSetters {
  // Bootstrap-store backed slices.
  projects: ProjectMeta[]
  setProjects: (next: SetStateLike<ProjectMeta[]>) => void
  projectSessions: Record<string, SessionMeta[]>
  setProjectSessions: (
    next: SetStateLike<Record<string, SessionMeta[]>>,
  ) => void
  currentProject: Project | null
  setCurrentProject: (next: SetStateLike<Project | null>) => void
  activeProjectId: string | null
  setActiveProjectId: (next: SetStateLike<string | null>) => void
  // Session-store backed slice.
  activeSessionId: string | null
  setActiveSessionId: (next: SetStateLike<string | null>) => void
  // Chat-store backed slices.
  conversations: Record<string, Conversation>
  setConversations: (
    next: SetStateLike<Record<string, Conversation>>,
  ) => void
  sessionLoading: Record<string, boolean>
  setSessionLoading: (
    next: SetStateLike<Record<string, boolean>>,
  ) => void
  sessionTodos: Record<string, TodoItem[]>
  setSessionTodos: (
    next: SetStateLike<Record<string, TodoItem[]>>,
  ) => void
  sessionTitleStates: Record<string, SessionTitleState>
  setSessionTitleStates: (
    next: SetStateLike<Record<string, SessionTitleState>>,
  ) => void
  streamAbortHandles: Record<string, string>
  setStreamAbortHandles: (
    next: SetStateLike<Record<string, string>>,
  ) => void
}

/**
 * Returns the canonical setter shims that App.tsx uses to mutate
 * the bootstrap / session / chat stores while preserving React's
 * `Dispatch<SetStateAction<T>>` shape at every call site.
 *
 * All wrappers read the *latest* store snapshot at call time
 * (not the React-rendered value) so multiple invocations within
 * a single async closure (e.g. `sendMessage`) see each other's
 * effects, matching the behavior of the inline shims previously
 * defined in App.tsx.
 */
export function useAppStateSetters(): AppStateSetters {
  // ---- bootstrap-store reads ----
  const projects = useBootstrapSelector((s) => s.projects)
  const projectSessions = useBootstrapSelector((s) => s.projectSessions)
  const currentProject = useBootstrapSelector((s) => s.currentProject)
  const activeProjectId = useBootstrapSelector((s) => s.activeProjectId)

  // ---- session-store read ----
  const activeSessionId = useSessionSelector((s) => s.activeSessionId)

  // ---- chat-store reads ----
  const chatSlice = useChatStore()
  const conversations = chatSlice.conversations
  const sessionLoading = chatSlice.sessionLoading
  const sessionTodos = chatSlice.sessionTodos
  const sessionTitleStates = chatSlice.sessionTitleStates
  const streamAbortHandles = chatSlice.streamAbortHandles

  // ---- bootstrap-store writers ----
  const setProjects = useCallback(
    (next: SetStateLike<ProjectMeta[]>) => {
      const value =
        typeof next === 'function'
          ? (next as (prev: ProjectMeta[]) => ProjectMeta[])(
              bootstrapStore.getSnapshot().projects,
            )
          : next
      bootstrapStore.setProjectList(value)
    },
    [],
  )

  const setProjectSessions = useCallback(
    (next: SetStateLike<Record<string, SessionMeta[]>>) => {
      const value =
        typeof next === 'function'
          ? (
              next as (
                prev: Record<string, SessionMeta[]>,
              ) => Record<string, SessionMeta[]>
            )(bootstrapStore.getSnapshot().projectSessions)
          : next
      bootstrapStore.setProjectSessions(value)
    },
    [],
  )

  const setCurrentProject = useCallback(
    (next: SetStateLike<Project | null>) => {
      const value =
        typeof next === 'function'
          ? (next as (prev: Project | null) => Project | null)(
              bootstrapStore.getSnapshot().currentProject,
            )
          : next
      bootstrapStore.selectProject({
        projectId: bootstrapStore.getSnapshot().activeProjectId,
        currentProject: value,
      })
    },
    [],
  )

  const setActiveProjectId = useCallback(
    (next: SetStateLike<string | null>) => {
      const value =
        typeof next === 'function'
          ? (next as (prev: string | null) => string | null)(
              bootstrapStore.getSnapshot().activeProjectId,
            )
          : next
      bootstrapStore.selectProject({
        projectId: value,
        currentProject: bootstrapStore.getSnapshot().currentProject,
      })
    },
    [],
  )

  // ---- session-store writer ----
  const setActiveSessionId = useCallback(
    (next: SetStateLike<string | null>) => {
      const previous = sessionStore.getSnapshot().activeSessionId
      const value =
        typeof next === 'function'
          ? (next as (prev: string | null) => string | null)(previous)
          : next
      // MEM-MOD-WIRE-FIX-2 — fire `on_session_end` for the session
      // we're about to leave so the rolling-summary / compile_today
      // / reflection / learned_traits pipelines actually run.  Best-
      // effort: a failed close never blocks the switch.
      if (previous && previous !== value) {
        void closeSession(previous).catch((err) => {
          console.warn('[session_close] failed', previous, err)
        })
      }
      sessionStore.setActiveSessionId(value)
    },
    [],
  )

  // ---- chat-store writers ----
  const setConversations = useCallback(
    (next: SetStateLike<Record<string, Conversation>>) => {
      // Always read the latest snapshot from the store, not the
      // stale React-rendered chatSlice.conversations. This ensures
      // that multiple setConversations calls within the same
      // sendMessage invocation see each other's updates.
      const prev = getConversationSnapshot().conversations
      const value =
        typeof next === 'function'
          ? (
              next as (
                p: Record<string, Conversation>,
              ) => Record<string, Conversation>
            )(prev)
          : next
      // Compute add/update/delete diff against the slice and
      // dispatch through the canonical actions so subscribers
      // (chat workspace, telemetry drawer, etc.) see the same
      // events whether the call came through the wrapper or
      // through a direct `setConversation(...)` import.
      const prevIds = new Set(Object.keys(prev))
      const nextIds = new Set(Object.keys(value))
      for (const id of nextIds) {
        if (value[id] !== prev[id]) {
          setConversation(value[id])
        }
      }
      for (const id of prevIds) {
        if (!nextIds.has(id)) {
          removeSession(id)
        }
      }
    },
    [],
  )

  const setSessionLoading = useCallback(
    (next: SetStateLike<Record<string, boolean>>) => {
      const prev = getConversationSnapshot().sessionLoading
      const value =
        typeof next === 'function'
          ? (next as (p: Record<string, boolean>) => Record<string, boolean>)(
              prev,
            )
          : next
      const ids = new Set([...Object.keys(prev), ...Object.keys(value)])
      for (const id of ids) {
        const wanted = value[id] === true
        const current = prev[id] === true
        if (wanted !== current) {
          setStoreSessionLoading(id, wanted)
        }
      }
    },
    [],
  )

  const setSessionTodos = useCallback(
    (next: SetStateLike<Record<string, TodoItem[]>>) => {
      const prev = getConversationSnapshot().sessionTodos
      const value =
        typeof next === 'function'
          ? (
              next as (
                p: Record<string, TodoItem[]>,
              ) => Record<string, TodoItem[]>
            )(prev)
          : next
      const ids = new Set([...Object.keys(prev), ...Object.keys(value)])
      for (const id of ids) {
        const wanted = value[id] ?? []
        const current = prev[id]
        if (wanted !== current) {
          setStoreSessionTodos(id, wanted)
        }
      }
    },
    [],
  )

  const setSessionTitleStates = useCallback(
    (next: SetStateLike<Record<string, SessionTitleState>>) => {
      const prev = getConversationSnapshot().sessionTitleStates
      const value =
        typeof next === 'function'
          ? (
              next as (
                p: Record<string, SessionTitleState>,
              ) => Record<string, SessionTitleState>
            )(prev)
          : next
      const ids = new Set([...Object.keys(prev), ...Object.keys(value)])
      for (const id of ids) {
        const wanted = value[id]
        const current = prev[id]
        if (!wanted) continue
        if (current === wanted) continue
        // Replace wholesale via the canonical `setTitleState`
        // action so both `stage` and `autoRenameCount` end up
        // synced (the previous "stage-only" wrapper silently
        // dropped autoRenameCount mutations and broke the
        // `MAX_AUTO_RENAME_COUNT` guard in
        // `maybeAutoRenameSession`).
        if (!current) initStoreTitleState(id)
        setStoreTitleState(id, wanted)
      }
    },
    [],
  )

  const setStreamAbortHandles = useCallback(
    (next: SetStateLike<Record<string, string>>) => {
      const prev = getConversationSnapshot().streamAbortHandles
      const value =
        typeof next === 'function'
          ? (next as (p: Record<string, string>) => Record<string, string>)(
              prev,
            )
          : next
      const ids = new Set([...Object.keys(prev), ...Object.keys(value)])
      for (const id of ids) {
        const wanted = value[id]
        const current = prev[id]
        if (wanted === current) continue
        if (wanted) {
          setStoreStreamAbortHandle(id, wanted)
        } else {
          clearStoreStreamAbortHandle(id)
        }
      }
    },
    [],
  )

  return {
    projects,
    setProjects,
    projectSessions,
    setProjectSessions,
    currentProject,
    setCurrentProject,
    activeProjectId,
    setActiveProjectId,
    activeSessionId,
    setActiveSessionId,
    conversations,
    setConversations,
    sessionLoading,
    setSessionLoading,
    sessionTodos,
    setSessionTodos,
    sessionTitleStates,
    setSessionTitleStates,
    streamAbortHandles,
    setStreamAbortHandles,
  }
}
