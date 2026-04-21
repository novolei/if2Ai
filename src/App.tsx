import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
// MIG-012 — canonical App.tsx transport seam.
//
// Business helpers come from `@/api/*` domain facades;
// `@/lib/tauri` is only kept as the `invoke` passthrough plus
// wire-level DTO re-exports that have not yet been moved to
// `@/transport/contracts`. Every command name in this import
// block is now hidden behind a domain facade.
import {
  createPermanentWorktree,
  createProject,
  createSession,
  deleteProject,
  deleteSession,
  ensureDefaultWorkdir,
  executeSlashCommand,
  getOnboardingState,
  getSession,
  listenToChatPrefill,
  listenToStream,
  listProjects,
  listProjectSessions,
  openProjectInFinder,
  openSettingsWindow,
  pickFolderDialog,
  renameProject,
  renameSession,
  resolveSkillSlash,
  respondPermission,
  setSessionPinned,
  startAgentStream,
  suggestSlashCommands,
  type Project,
  type ProjectMeta,
  type SessionMeta,
} from '@/api'
import { invoke } from '@/lib/tauri'
import type {
  PermissionMode,
  PermissionRequestPayload,
  StreamTokenPayload,
} from '@/transport/contracts'
import { toast } from 'sonner'
import {
  runtimeProjectionStore,
  useExecutionModePreview,
  useRuntimeProjectionSelector,
  wireRuntimeProjectionListeners,
} from '@/runtime-projection'
// MIG-013 — AppShell is the canonical top-level shell container
// (BootShell + MainShell + ContentRouter). Boot state lives in
// the bootstrap-store; App.tsx is now a data-flow host, not a
// render / boot orchestrator.
import { AppShell } from '@/modules/app-shell/AppShell'
import { runBootSequence } from '@/boot/boot-orchestrator'
import { bootstrapStore, useBootstrapSelector } from '@/state'
// MIG-014 — chat + session stores own per-session runtime
// state. App.tsx no longer holds the canonical truth; the
// `setX` wrappers below diff against the store snapshot and
// dispatch the store's explicit actions so the React-shaped
// `Dispatch<SetStateAction<T>>` API is preserved at every
// existing call site.
import {
  removeSession,
  setConversation,
  setSessionLoading as setStoreSessionLoading,
  setSessionTodos as setStoreSessionTodos,
  setStreamAbortHandle as setStoreStreamAbortHandle,
  clearStreamAbortHandle as clearStoreStreamAbortHandle,
  initTitleState as initStoreTitleState,
  setTitleState as setStoreTitleState,
  useChatStore,
} from '@/stores'
import { sessionStore, useSessionSelector } from '@/stores'
// AppVersionWatermark moved into MainShell (Phase M2.7).
import { SectionWorkspace } from '@/modules/app-shell/components/SectionWorkspace'
import type { AppSection } from '@/modules/app-shell/types'
import { ChatWorkspace } from '@/modules/chat/components/ChatWorkspace'
import type { Conversation, Message, RecentSession, SessionTitleState } from '@/modules/chat/types'
import { useAgentVoiceBridge } from '@/modules/chat/useAgentVoiceBridge'
import { AgentVoiceIndicator } from '@/modules/chat/AgentVoiceIndicator'
import { useCrossWindowChange } from '@/lib/crossWindowSync'
// OnboardingApp moved into BootShell (Phase M2.7).
import { MemoryBrowser } from '@/components/memory/MemoryBrowser'
// If2AiLoadingScreen moved into BootShell (Phase M2.7).
import { TelemetryDrawer } from '@/components/chat/TelemetryDrawer'
import { CreateProjectDialog } from '@/components/CreateProjectDialog'
import type { TodoItem } from '@/components/ui/TodoPanel'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

const appIconSrc = `${new URL('../src-tauri/icons/icon-128.png', import.meta.url).href}?v=20260414c`
const PLACEHOLDER_SESSION_TITLE = '新对话'
const MAX_AUTO_TITLE_TURNS = 3
const MAX_AUTO_RENAME_COUNT = 2
const GENERIC_USER_PROMPTS = [
  '继续',
  '继续完成',
  '帮我看看',
  '看一下',
  '改一下',
  '优化一下',
  '修一下',
  '处理一下',
  '请继续',
  '开始',
  '你好',
  'hi',
  'hello',
]

function App() {
  const appWindow = getCurrentWindow()
  // MIG-013 — boot phase / project list / active project now
  // live in the bootstrap store. Local reads go through
  // `useBootstrapSelector` so App.tsx re-renders on exactly the
  // slices it consumes; writes use the store's explicit actions.
  // MIG-013 — boot phase lives in the bootstrap store; AppShell
  // reads it internally and re-renders when the phase changes.
  // App.tsx itself subscribes to specific slices below
  // (projects / activeProjectId / currentProject / projectSessions),
  // which transition together on every phase mutation, so no
  // separate phase subscription is needed here.
  // Phase TTS-D / P1：Agent 语音桥接
  const agentVoice = useAgentVoiceBridge()

  // 跨窗口监听 Onboarding 重置：设置窗口点重置后，主窗口立即跳回 Onboarding 流程
  useCrossWindowChange('cross:onboarding-reset', () => {
    console.log('[App] cross-window: onboarding reset → re-entering onboarding flow')
    bootstrapStore.enterOnboarding()
  })

  // MIG-013 — boot orchestration moved to `src/boot/boot-orchestrator.ts`.
  // The `useEffect` below is now a thin call into the canonical
  // runner; the store transitions own every phase change.
  useEffect(() => {
    const signal = { cancelled: false }
    void (async () => {
      await runBootSequence(bootstrapStore, {
        getOnboardingState,
        ensureDefaultWorkdir,
        listProjects,
        listProjectSessions,
        signal,
      })
      if (signal.cancelled) return
      // Model bootstrap is orthogonal to project-list bootstrap;
      // kept inline here until the future settings-store pack
      // picks it up.
      try {
        const activeModel = await invoke<{ provider_id: string; model_id: string } | null>(
          'model_get_active',
        )
        if (activeModel) {
          setSelectedModel(`${activeModel.provider_id}/${activeModel.model_id}`)
        }
      } catch {
        // Fallback: leave empty so chat-ui shows first available model from list
      }
    })()
    return () => {
      signal.cancelled = true
    }
  }, [])

  const [activeSection, setActiveSection] = useState<AppSection>(() => {
    if (typeof window === 'undefined') return 'chat'
    const stored = localStorage.getItem('lastActiveSection')
    return stored === 'skills' || stored === 'automation' ? stored : 'chat'
  })
  // MIG-013 — these four slices live in the bootstrap store.
  // Reads go through `useBootstrapSelector` so React re-renders
  // only when the slice changes; writes go through store-aware
  // setters that preserve React's `Dispatch<SetStateAction<T>>`
  // shape so existing call sites (`setProjects(prev => ...)`,
  // `setCurrentProject({ ... })`) compile unchanged.
  const projects = useBootstrapSelector((s) => s.projects)
  const setProjects = useCallback(
    (next: ProjectMeta[] | ((prev: ProjectMeta[]) => ProjectMeta[])) => {
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
  const projectSessions = useBootstrapSelector((s) => s.projectSessions)
  const setProjectSessions = useCallback(
    (
      next:
        | Record<string, SessionMeta[]>
        | ((prev: Record<string, SessionMeta[]>) => Record<string, SessionMeta[]>),
    ) => {
      const value =
        typeof next === 'function'
          ? (next as (prev: Record<string, SessionMeta[]>) => Record<string, SessionMeta[]>)(
              bootstrapStore.getSnapshot().projectSessions,
            )
          : next
      bootstrapStore.setProjectSessions(value)
    },
    [],
  )
  const currentProject = useBootstrapSelector((s) => s.currentProject)
  const activeProjectId = useBootstrapSelector((s) => s.activeProjectId)
  const setCurrentProject = useCallback(
    (next: Project | null | ((prev: Project | null) => Project | null)) => {
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
    (next: string | null | ((prev: string | null) => string | null)) => {
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
  // MIG-014 — activeSessionId moved to the canonical session
  // store; reads via `useSessionSelector`, writes via the
  // store-backed wrapper that keeps React's
  // `Dispatch<SetStateAction<string | null>>` shape so existing
  // call sites compile unchanged.
  const activeSessionId = useSessionSelector((s) => s.activeSessionId)
  const setActiveSessionId = useCallback(
    (next: string | null | ((prev: string | null) => string | null)) => {
      const value =
        typeof next === 'function'
          ? (next as (prev: string | null) => string | null)(
              sessionStore.getSnapshot().activeSessionId,
            )
          : next
      sessionStore.setActiveSessionId(value)
    },
    [],
  )
  const [leftPaneWidth, setLeftPaneWidth] = useState(240)
  const [isLeftPaneCollapsed, setIsLeftPaneCollapsed] = useState(false)
  const [loading, setLoading] = useState(false)
  // MIG-014 — `conversations` lives in the chat store
  // (conversation-slice.ts). Reads go through `useChatStore`;
  // writes go through `setConversations` wrapper that diffs
  // against the previous record and dispatches the appropriate
  // chat-store mutation so call sites that use
  // `setConversations(prev => ({ ...prev, [id]: conv }))`
  // continue to compile unchanged.
  const chatSlice = useChatStore()
  const conversations = chatSlice.conversations
  const setConversations = useCallback(
    (
      next:
        | Record<string, Conversation>
        | ((prev: Record<string, Conversation>) => Record<string, Conversation>),
    ) => {
      const prev = chatSlice.conversations
      const value =
        typeof next === 'function'
          ? (next as (p: Record<string, Conversation>) => Record<string, Conversation>)(prev)
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
    [chatSlice.conversations],
  )
  const [input, setInput] = useState('')
  // Phase M2.6 — opt-in classifier preview. Watches the active
  // chat draft and dispatches the deterministic
  // `ExecutionModeDecision` into the projection store. The
  // <ExecutionModePill /> below renders the resulting judgment.
  // Honest scope: pure preview, the agent loop is NOT auto-routed.
  useExecutionModePreview(input, { sessionId: activeSessionId ?? undefined })
  // MIG-014 — sessionLoading lives in the chat store. Wrapper
  // diffs against the previous record and dispatches the
  // canonical `setSessionLoading(id, bool)` action per id
  // change.
  const sessionLoading = chatSlice.sessionLoading
  const setSessionLoading = useCallback(
    (
      next:
        | Record<string, boolean>
        | ((prev: Record<string, boolean>) => Record<string, boolean>),
    ) => {
      const prev = chatSlice.sessionLoading
      const value =
        typeof next === 'function'
          ? (next as (p: Record<string, boolean>) => Record<string, boolean>)(prev)
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
    [chatSlice.sessionLoading],
  )
  const [isCreateProjectOpen, setIsCreateProjectOpen] = useState(false)
  const [selectedModel, setSelectedModel] = useState('')
  const [isRightRailOpen, setIsRightRailOpen] = useState(false)
  const [permissionMode, setPermissionMode] = useState<PermissionMode>(() => {
    if (typeof window === 'undefined') return 'dangerFullAccess'
    const stored = localStorage.getItem('permissionMode')
    if (stored === 'readOnly' || stored === 'workspaceWrite' || stored === 'dangerFullAccess') {
      return stored
    }
    return 'dangerFullAccess'
  })
  // MIG-014 — sessionTodos + sessionTitleStates live in the
  // chat store. Wrappers preserve `Dispatch<SetStateAction<T>>`.
  const sessionTodos = chatSlice.sessionTodos
  const setSessionTodos = useCallback(
    (
      next:
        | Record<string, TodoItem[]>
        | ((prev: Record<string, TodoItem[]>) => Record<string, TodoItem[]>),
    ) => {
      const prev = chatSlice.sessionTodos
      const value =
        typeof next === 'function'
          ? (next as (p: Record<string, TodoItem[]>) => Record<string, TodoItem[]>)(prev)
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
    [chatSlice.sessionTodos],
  )
  const sessionTitleStates = chatSlice.sessionTitleStates
  const setSessionTitleStates = useCallback(
    (
      next:
        | Record<string, SessionTitleState>
        | ((prev: Record<string, SessionTitleState>) => Record<string, SessionTitleState>),
    ) => {
      const prev = chatSlice.sessionTitleStates
      const value =
        typeof next === 'function'
          ? (next as (p: Record<string, SessionTitleState>) => Record<string, SessionTitleState>)(
              prev,
            )
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
    [chatSlice.sessionTitleStates],
  )
  // Ref to allow reading sessionTitleStates inside async callbacks (e.g. refreshProjectSessions)
  const sessionTitleStatesRef = useRef<Record<string, SessionTitleState>>({})
  useEffect(() => { sessionTitleStatesRef.current = sessionTitleStates }, [sessionTitleStates])
  const setRecoveryStateForCursor = (sessionId: string, resumeCursor: string, isRecovering: boolean) => {
    setConversations((prev) => {
      const currentConv = prev[sessionId]
      if (!currentConv) return prev
      return {
        ...prev,
        [sessionId]: {
          ...currentConv,
          messages: currentConv.messages.map((msg) => {
            if (msg.taskOutcome !== 'partial_success') return msg
            if (msg.resumeCursor !== resumeCursor) return msg
            return { ...msg, isRecovering }
          }),
        },
      }
    })
  }

  // MIG-014 — streamAbortHandles live in the chat store.
  // Wrapper diffs and dispatches `setStreamAbortHandle` /
  // `clearStreamAbortHandle` per id.
  const streamAbortHandles = chatSlice.streamAbortHandles
  const setStreamAbortHandles = useCallback(
    (
      next:
        | Record<string, string>
        | ((prev: Record<string, string>) => Record<string, string>),
    ) => {
      const prev = chatSlice.streamAbortHandles
      const value =
        typeof next === 'function'
          ? (next as (p: Record<string, string>) => Record<string, string>)(prev)
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
    [chatSlice.streamAbortHandles],
  )
  // Phase M2.8 — permission prompt now reads from the canonical
  // projection store (`snapshot.approvals`).  The bridge feeds the
  // store via `translatePermissionRequestPayload`; we pick the
  // first pending approval (single-prompt UX preserved) and
  // dispatch `permission_resolved` after the user decides so the
  // reducer clears the entry.  No more `useState<PermissionRequestPayload>`.
  const approvals = useRuntimeProjectionSelector((s) => s.approvals)
  const permissionPrompt = useMemo<PermissionRequestPayload | null>(() => {
    const ids = Object.keys(approvals)
    if (ids.length === 0) return null
    const a = approvals[ids[0]]
    return {
      session_id: a.sessionId,
      tool_name: a.toolName,
      permission_mode: a.permissionMode,
      current_mode: a.currentMode,
      message: a.message,
    }
  }, [approvals])
  const sessionLoadingRef = useRef<Record<string, boolean>>({})
  const autoResumeAttemptsRef = useRef<Record<string, number>>({})
  const attemptedAutoResumeCursorsRef = useRef<Set<string>>(new Set())
  const leftPaneCollapsedBeforePreviewRef = useRef(false)
  const wasPreviewFocusModeRef = useRef(false)
  const resizeRef = useRef<{
    startX: number
    startWidth: number
  } | null>(null)

  const normalizeTodoItem = (value: unknown): TodoItem | null => {
    if (!value || typeof value !== 'object') return null
    const item = value as Record<string, unknown>
    const content = typeof item.content === 'string' ? item.content : ''
    const activeForm =
      typeof item.activeForm === 'string'
        ? item.activeForm
        : typeof item.active_form === 'string'
          ? item.active_form
          : content
    const status = item.status
    if (
      !content ||
      (status !== 'pending' && status !== 'in_progress' && status !== 'completed')
    ) {
      return null
    }
    return {
      content,
      activeForm,
      status,
    }
  }

  const extractTodosFromToolResult = (raw: string | null | undefined): TodoItem[] | null => {
    if (!raw) return null
    try {
      const parsed = JSON.parse(raw) as {
        new_todos?: unknown[]
        newTodos?: unknown[]
      }
      const candidates = Array.isArray(parsed.new_todos)
        ? parsed.new_todos
        : Array.isArray(parsed.newTodos)
          ? parsed.newTodos
          : null
      if (!candidates) return null
      return candidates
        .map((item) => normalizeTodoItem(item))
        .filter((item): item is TodoItem => item !== null)
    } catch {
      return null
    }
  }

  /**
   * Parse the structured `memory_store` tool result emitted by the backend
   * (`src-tauri/src/modules/tools/builtin/memory_store.rs`).  The handler
   * always returns a JSON object with `policy_decision`, `scope`,
   * `reason_code`, and `status`; we lift those onto `Message` so the
   * `MemoryStoreToolCard` highlight (deny / prompt) fires deterministically
   * instead of relying on the control-plane permission `policy_decision`,
   * which is unrelated to memory-write policy.
   *
   * Returns `null` when the result is missing, not JSON, or not produced by
   * the new structured `memory_store` handler — callers should fall back to
   * existing behaviour in that case so legacy tool flows are unaffected.
   */
  const extractMemoryStoreFields = (
    raw: string | null | undefined,
  ): {
    policyDecision?: 'allow' | 'deny' | 'prompt'
    memoryScope?: 'global' | 'project' | 'session'
    memoryReasonCode?: string
  } | null => {
    if (!raw) return null
    let parsed: Record<string, unknown>
    try {
      const candidate = JSON.parse(raw)
      if (!candidate || typeof candidate !== 'object' || Array.isArray(candidate)) return null
      parsed = candidate as Record<string, unknown>
    } catch {
      return null
    }
    const decisionRaw = parsed.policy_decision
    const scopeRaw = parsed.scope
    const reasonCodeRaw = parsed.reason_code
    const out: {
      policyDecision?: 'allow' | 'deny' | 'prompt'
      memoryScope?: 'global' | 'project' | 'session'
      memoryReasonCode?: string
    } = {}
    if (decisionRaw === 'allow' || decisionRaw === 'deny' || decisionRaw === 'prompt') {
      out.policyDecision = decisionRaw
    }
    if (scopeRaw === 'global' || scopeRaw === 'project' || scopeRaw === 'session') {
      out.memoryScope = scopeRaw
    }
    if (typeof reasonCodeRaw === 'string') {
      out.memoryReasonCode = reasonCodeRaw
    }
    return Object.keys(out).length > 0 ? out : null
  }

  const buildLoopCompletionStatus = (
    messages: Message[],
    streamId: string | undefined,
    taskOutcome?: Message['taskOutcome'],
    degradedReason?: string
  ): { label: string; kind: NonNullable<Message['statusKind']> } => {
    const scopedToolMessages = messages.filter((msg) => {
      if (msg.role !== 'tool') return false
      if (!streamId) return true
      return msg.streamId === streamId
    })
    const total = scopedToolMessages.length
    const completed = scopedToolMessages.filter((msg) => msg.toolStatus === 'completed').length
    const failed = scopedToolMessages.filter((msg) => msg.toolStatus === 'error').length

    if (taskOutcome === 'partial_success') {
      if (degradedReason?.includes('max_iterations_reached')) {
        return {
          label: total > 0
            ? `本轮已完成 ${completed}/${total} 个步骤，达到迭代上限，可继续未完成部分`
            : '达到迭代上限，可继续未完成部分',
          kind: 'partial',
        }
      }
      return {
        label: total > 0
          ? `本轮部分完成：已完成 ${completed}/${total} 个步骤`
          : '本轮任务部分完成，可继续补全',
        kind: 'partial',
      }
    }

    if (taskOutcome === 'failed') {
      return {
        label: total > 0
          ? `本轮执行失败：已完成 ${completed}/${total} 个步骤`
          : '本轮执行失败',
        kind: 'failed',
      }
    }

    if (total === 0) return { label: '本轮执行完成', kind: 'success' }
    if (failed > 0) {
      return {
        label: `本轮执行完成：成功 ${completed} 个，失败 ${failed} 个`,
        kind: 'partial',
      }
    }
    return {
      label: `本轮执行完成：共完成 ${completed} 个步骤`,
      kind: 'success',
    }
  }

  const normalizeSessionTitleSource = (raw: string): string => {
    const cleaned = raw
      .replace(/\[resume_cursor\][\s\S]*$/gi, '')
      .replace(/^(请|帮我|麻烦|继续|继续帮我|继续把|我想|想要|我要|需要|请先|先帮我)\s*/u, '')
      .replace(/(一下|一下子|好吗|可以吗|吧|谢谢|thanks|thank you)\s*$/giu, '')
      .replace(/`+/g, '')
      .replace(/[#>*_\-\[\]]/g, ' ')
      .replace(/\s+/g, ' ')
      .trim()

    return cleaned
  }

  // P3: accept the owning conversation so [resume_cursor] falls back to its title,
  // not the currently-visible activeConv (which may differ mid-rename).
  const formatSessionTitle = (raw: string, conv?: Conversation): string => {
    if (raw.includes('[resume_cursor]')) {
      return conv?.title ?? activeConv?.title ?? '继续当前任务'
    }
    const cleaned = normalizeSessionTitleSource(raw)

    if (!cleaned) return PLACEHOLDER_SESSION_TITLE
    const firstLine = cleaned.split(/[\n。！？!?]/).find((segment) => segment.trim())?.trim() ?? cleaned
    return firstLine.slice(0, 30) || PLACEHOLDER_SESSION_TITLE
  }

  const normalizeTitleComparison = (value: string): string =>
    value.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '')

  const areTitlesSimilar = (a: string, b: string): boolean => {
    const normalizedA = normalizeTitleComparison(a)
    const normalizedB = normalizeTitleComparison(b)
    if (!normalizedA || !normalizedB) return false
    return (
      normalizedA === normalizedB ||
      normalizedA.includes(normalizedB) ||
      normalizedB.includes(normalizedA)
    )
  }

  const isMeaningfulUserMessage = (content: string): boolean => {
    const normalized = normalizeSessionTitleSource(content)
    if (!normalized) return false
    if (normalized.includes('[resume_cursor]')) return false
    if (normalized.length < 2) return false
    const lower = normalized.toLowerCase()
    return !GENERIC_USER_PROMPTS.some((prompt) => lower === prompt)
  }

  const getMeaningfulUserMessages = (messages: Message[]): Message[] =>
    messages.filter((message) => message.role === 'user' && isMeaningfulUserMessage(message.content))

  const getInitialSessionTitleCandidate = (messages: Message[], conv?: Conversation): string | null => {
    const meaningfulMessages = getMeaningfulUserMessages(messages)
    const seedMessage =
      meaningfulMessages.find((message) => formatSessionTitle(message.content, conv) !== PLACEHOLDER_SESSION_TITLE)
      ?? meaningfulMessages[0]
    if (!seedMessage) return null
    const nextTitle = formatSessionTitle(seedMessage.content, conv)
    return nextTitle === PLACEHOLDER_SESSION_TITLE ? null : nextTitle
  }

  const getCorrectionTitleCandidate = (messages: Message[], currentTitle: string, conv?: Conversation): string | null => {
    const userMessages = getMeaningfulUserMessages(messages)
    if (userMessages.length < 2) return null
    const recentCandidates = userMessages
      .slice(-2)
      .map((message) => formatSessionTitle(message.content, conv))
      .filter((candidate) => candidate && candidate !== PLACEHOLDER_SESSION_TITLE)
    if (recentCandidates.length === 0) return null
    const [previousCandidate, latestCandidate] = recentCandidates
    const resolvedCandidate = latestCandidate ?? previousCandidate
    if (!resolvedCandidate) return null
    if (areTitlesSimilar(resolvedCandidate, currentTitle)) return null
    return resolvedCandidate
  }

  const getInitialSessionTitleState = (
    title: string,
    existingMessages: Message[] = []
  ): SessionTitleState => {
    if (title && title !== PLACEHOLDER_SESSION_TITLE) {
      return {
        stage: 'locked',
        autoRenameCount: MAX_AUTO_RENAME_COUNT,
      }
    }

    const userTurnCount = getMeaningfulUserMessages(existingMessages).length
    return {
      stage: userTurnCount === 0 ? 'placeholder' : 'provisional',
      autoRenameCount: 0,
    }
  }

  const maybeAutoRenameSession = (
    projectId: string,
    sessionId: string,
    conversation: Conversation
  ) => {
    const titleState =
      sessionTitleStates[sessionId] ?? getInitialSessionTitleState(conversation.title, conversation.messages)

    // P0: manual stage = user-initiated rename; never auto-override
    if (titleState.stage === 'manual' || titleState.stage === 'locked') return

    const meaningfulUserMessages = getMeaningfulUserMessages(conversation.messages)
    const meaningfulTurnCount = meaningfulUserMessages.length

    if (meaningfulTurnCount === 0) return

    // P2: wait until AI has responded at least once before setting any title,
    // so the session title reflects real context rather than just the first user prompt.
    const hasAiReply = conversation.messages.some((m) => m.role === 'assistant')
    if (!hasAiReply) return

    if (meaningfulTurnCount > MAX_AUTO_TITLE_TURNS) {
      setSessionTitleStates((prev) => ({
        ...prev,
        [sessionId]: {
          ...titleState,
          stage: 'locked',
          autoRenameCount: Math.max(titleState.autoRenameCount, 1),
        },
      }))
      return
    }

    const currentTitle = conversation.title || PLACEHOLDER_SESSION_TITLE
    // P3: pass the owning conversation so [resume_cursor] resolves correctly
    const initialCandidate = getInitialSessionTitleCandidate(conversation.messages, conversation)
    if (
      titleState.stage === 'placeholder' &&
      initialCandidate &&
      !areTitlesSimilar(initialCandidate, currentTitle)
    ) {
      syncSessionTitle(projectId, sessionId, initialCandidate)
      setSessionTitleStates((prev) => ({
        ...prev,
        [sessionId]: {
          stage: 'provisional',
          autoRenameCount: 1,
        },
      }))
      return
    }

    if (
      titleState.stage === 'provisional' &&
      titleState.autoRenameCount < MAX_AUTO_RENAME_COUNT
    ) {
      const correctionCandidate = getCorrectionTitleCandidate(conversation.messages, currentTitle, conversation)
      if (correctionCandidate) {
        syncSessionTitle(projectId, sessionId, correctionCandidate)
        setSessionTitleStates((prev) => ({
          ...prev,
          [sessionId]: {
            stage: 'locked',
            autoRenameCount: titleState.autoRenameCount + 1,
          },
        }))
        return
      }

      if (meaningfulTurnCount >= MAX_AUTO_TITLE_TURNS) {
        setSessionTitleStates((prev) => ({
          ...prev,
          [sessionId]: {
            ...titleState,
            stage: 'locked',
          },
        }))
      }
    }
  }

  const syncSessionTitle = (projectId: string, sessionId: string, title: string) => {
    const nextTitle = title.trim()
    if (!nextTitle) return

    // Capture previous title for rollback before the optimistic update
    const previousTitle = conversations[sessionId]?.title ?? PLACEHOLDER_SESSION_TITLE

    setConversations((prev) => {
      const conversation = prev[sessionId]
      if (!conversation || conversation.title === nextTitle) return prev
      return {
        ...prev,
        [sessionId]: {
          ...conversation,
          title: nextTitle,
        },
      }
    })

    setProjectSessions((prev) => {
      const sessions = prev[projectId]
      if (!sessions) return prev
      let changed = false
      const nextSessions = sessions.map((session) => {
        if (session.id !== sessionId || session.title === nextTitle) return session
        changed = true
        return { ...session, title: nextTitle }
      })
      return changed ? { ...prev, [projectId]: nextSessions } : prev
    })

    // P1: rollback optimistic update and show error toast on persistence failure
    void renameSession(sessionId, nextTitle).catch((err) => {
      console.error('Failed to rename session:', err)
      toast.error('重命名失败，已恢复原名称', { duration: 3000 })
      setConversations((prev) => {
        const conversation = prev[sessionId]
        if (!conversation || conversation.title !== nextTitle) return prev
        return { ...prev, [sessionId]: { ...conversation, title: previousTitle } }
      })
      setProjectSessions((prev) => {
        const sessions = prev[projectId]
        if (!sessions) return prev
        return {
          ...prev,
          [projectId]: sessions.map((s) =>
            s.id === sessionId && s.title === nextTitle ? { ...s, title: previousTitle } : s
          ),
        }
      })
    })
  }

  const activeConv = activeSessionId ? conversations[activeSessionId] : null

  // Build the last-3 sessions list for the HomeScreen suggestion row
  const recentSessions = useMemo((): RecentSession[] => {
    const all: RecentSession[] = []
    for (const [projectId, sessions] of Object.entries(projectSessions)) {
      const project = projects.find((p) => p.id === projectId)
      for (const s of sessions) {
        all.push({
          sessionId: s.id,
          projectId,
          projectName: project?.name ?? projectId,
          title: s.title,
          updatedAt: s.updated_at,
        })
      }
    }
    all.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    return all.slice(0, 3)
  }, [projectSessions, projects])
  const isActiveSessionLoading = activeSessionId ? Boolean(sessionLoading[activeSessionId]) : false
  const activeTitle = activeConv?.title ?? '新对话'
  const todos = activeSessionId ? sessionTodos[activeSessionId] ?? [] : []
  const branchLabel = 'feature/consolidate-codebase'
  const minLeftPaneWidth = 280
  const maxLeftPaneWidth = 520
  const activeMessages = useMemo(
    () =>
      activeConv?.messages
        .filter((msg) => !(msg.role === 'user' && msg.content.includes('[resume_cursor]')))
        .map((msg) => ({
          ...msg,
          content: msg.content || ' ',
        })) ?? [],
    [activeConv?.messages]
  )
  const runningSessionIds = Object.entries(sessionLoading)
    .filter(([, running]) => running)
    .map(([sessionId]) => sessionId)


  useEffect(() => {
    if (activeProjectId && activeSessionId) {
      localStorage.setItem('lastActiveProjectId', activeProjectId)
      localStorage.setItem('lastActiveSessionId', activeSessionId)
    }
  }, [activeProjectId, activeSessionId])

  useEffect(() => {
    localStorage.setItem('lastActiveSection', activeSection)
  }, [activeSection])

  useEffect(() => {
    localStorage.setItem('permissionMode', permissionMode)
  }, [permissionMode])

  useEffect(() => {
    sessionLoadingRef.current = sessionLoading
  }, [sessionLoading])

  // P1: compute stable counters so the rename effect only fires when truly necessary,
  // not on every streaming token that updates message content.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const activeMeaningfulUserMsgCount = useMemo(
    () =>
      activeConv
        ? activeConv.messages.filter(
            (m) => m.role === 'user' && isMeaningfulUserMessage(m.content)
          ).length
        : 0,
    // isMeaningfulUserMessage is a stable pure function defined in the same render scope
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [activeConv?.messages]
  )
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const activeAiReplyCount = useMemo(
    () => activeConv?.messages.filter((m) => m.role === 'assistant').length ?? 0,
    [activeConv?.messages]
  )

  // Only re-run when meaningful counters change — avoids firing on every streaming update
  useEffect(() => {
    if (!activeConv || !activeSessionId) return
    maybeAutoRenameSession(activeConv.projectId, activeSessionId, activeConv)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeSessionId, activeMeaningfulUserMsgCount, activeAiReplyCount])

  // Phase M2.8 — direct `listenToPermissionRequests` removed.
  // Permission prompts now arrive via the projection bridge (see
  // `wireRuntimeProjectionListeners`) into `snapshot.approvals`,
  // which `permissionPrompt` (above) reads via
  // `useRuntimeProjectionSelector`.

  // Phase M2.4 — wire the canonical runtime projection pipeline.
  // The bridge subscribes broadly to `agent-token` /
  // `permission-request` / `memory_event`, normalises each via the
  // translator family and feeds the projection store.  Existing
  // per-stream listeners and the legacy permission/memory wiring
  // continue to drive the current ChatWorkspace + TelemetryDrawer
  // surfaces in parallel — M2.5+ will swap consumers over to the
  // projection store and retire the per-stream callbacks.
  useEffect(() => {
    const unwire = wireRuntimeProjectionListeners()
    return () => unwire()
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | undefined
    listenToChatPrefill((payload) => {
      setActiveSection('chat')
      if (typeof payload?.prompt === 'string' && payload.prompt.trim()) {
        setInput(payload.prompt)
      }
    })
      .then((dispose) => {
        unlisten = dispose
      })
      .catch((err) => {
        console.error('Failed to listen chat prefill event:', err)
      })
    return () => {
      if (unlisten) unlisten()
    }
  }, [])

  // Cmd+, (macOS) / Ctrl+, (Win/Linux) — universal shortcut to open settings
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === ',') {
        e.preventDefault()
        void openSettingsWindow()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  // ⌘+Shift+D (macOS) / Ctrl+Shift+D — developer shortcut that toggles
  // the TelemetryDrawer for the active session.  Drives the Phase 6E
  // harness observability surface visible from the chat workspace.
  const [isTelemetryDrawerOpen, setIsTelemetryDrawerOpen] = useState(false)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && (e.key === 'D' || e.key === 'd')) {
        e.preventDefault()
        setIsTelemetryDrawerOpen((prev) => !prev)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const loadProjects = async (): Promise<ProjectMeta[]> => {
    try {
      setLoading(true)
      const projectList = await listProjects()
      setProjects(projectList)

      const sessionsMap: Record<string, SessionMeta[]> = {}
      for (const project of projectList) {
        sessionsMap[project.id] = await listProjectSessions(project.id)
      }
      setProjectSessions(sessionsMap)
      return projectList
    } catch (err) {
      console.error('Failed to load projects:', err)
      return []
    } finally {
      setLoading(false)
    }
  }

  // P0: monotonic merge — keep in-memory title when our local state is provisional/locked/manual,
  // preventing a stale backend response from overwriting an optimistic title update.
  const refreshProjectSessions = async (projectId: string) => {
    const sessions = await listProjectSessions(projectId)
    setProjectSessions((prev) => {
      const existing = prev[projectId] ?? []
      const titleStates = sessionTitleStatesRef.current
      const merged = sessions.map((s) => {
        const inMem = existing.find((e) => e.id === s.id)
        if (!inMem) return s
        const ts = titleStates[s.id]
        // Keep the in-memory title when we've already set it and the backend may not have caught up
        if (
          ts &&
          (ts.stage === 'provisional' || ts.stage === 'locked' || ts.stage === 'manual') &&
          inMem.title &&
          inMem.title !== PLACEHOLDER_SESSION_TITLE
        ) {
          return { ...s, title: inMem.title }
        }
        return s
      })
      return { ...prev, [projectId]: merged }
    })
  }

  const handleSelectProject = async (projectId: string) => {
    setActiveSection('chat')
    setActiveProjectId(projectId)
    const project = projects.find((p) => p.id === projectId)
    if (project) {
      setCurrentProject({
        id: project.id,
        name: project.name,
        workdir: project.workdir,
        created_at: project.created_at,
        updated_at: '',
      })
    }
    setActiveSessionId(null)
  }

  const handleSelectSession = async (
    projectId: string,
    sessionId: string,
    projectOverride?: ProjectMeta
  ) => {
    setActiveSection('chat')
    setActiveProjectId(projectId)
    setActiveSessionId(sessionId)
    setSessionTodos((prev) => ({ ...prev, [sessionId]: [] }))

    const project = projectOverride ?? projects.find((item) => item.id === projectId)
    if (project) {
      setCurrentProject({
        id: project.id,
        name: project.name,
        workdir: project.workdir,
        created_at: project.created_at,
        updated_at: '',
      })
    }

    if (conversations[sessionId]) return

    const sessionMeta = projectSessions[projectId]?.find((session) => session.id === sessionId)

    try {
      const fullSession = await getSession(sessionId)
      const baseTimestamp = new Date(fullSession.updated_at)
      const convertedMessages: Message[] = []
      const toolMessageIndexById = new Map<string, number>()
      let recoveredTodos: TodoItem[] = []

      const upsertToolMessage = (toolCallId: string, nextMessage: Message) => {
        const existingIndex = toolMessageIndexById.get(toolCallId)
        if (existingIndex !== undefined) {
          convertedMessages[existingIndex] = {
            ...convertedMessages[existingIndex],
            ...nextMessage,
            id: convertedMessages[existingIndex].id,
            toolCallId,
          }
          return
        }

        const index = convertedMessages.push(nextMessage) - 1
        toolMessageIndexById.set(toolCallId, index)
      }

      for (const msg of fullSession.messages) {
        if (msg.role === "system") {
          continue
        }
        const messageOutcome = {
          requestId: msg.request_id,
          taskOutcome: msg.task_outcome,
          degradedReason: msg.degraded_reason,
          resumeAvailable: msg.resume_available,
          resumeCursor: msg.resume_cursor,
        }
        let pushedAssistantText = false
        for (const block of msg.blocks) {
          if (block.type === "tool_use" && block.tool_use_block) {
            const toolCallId = block.tool_use_block.id
            upsertToolMessage(toolCallId, {
              id: `tool-use-${toolCallId}`,
              role: "tool",
              content: "",
              timestamp: baseTimestamp,
              toolCallId,
              toolName: block.tool_use_block.name,
              toolArgs: block.tool_use_block.input as Record<string, unknown> | undefined,
              toolStatus: "running",
              policyDecision: "prompt",
              evidenceId: toolCallId,
              effectiveWorkdir: project?.workdir,
              disableAnimation: true,
              ...messageOutcome,
            })
            continue
          }

          if (block.type === "tool_result" && block.tool_use_id) {
            const toolCallId = block.tool_use_id
            const existingIndex = toolMessageIndexById.get(toolCallId)
            const toolArgs =
              existingIndex !== undefined ? convertedMessages[existingIndex]?.toolArgs : undefined
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
              effectiveWorkdir: project?.workdir,
              isError: false,
              disableAnimation: true,
              ...messageOutcome,
            })
            const isTodoWriteResult =
              (block.tool_name ?? '').trim() === 'TodoWrite' ||
              (existingIndex !== undefined &&
                convertedMessages[existingIndex]?.toolName?.trim() === 'TodoWrite')
            if (isTodoWriteResult) {
              const nextTodos = extractTodosFromToolResult(block.output)
              if (nextTodos) {
                recoveredTodos = nextTodos
              }
            }
            continue
          }

          if (block.type === "text" && block.text) {
            pushedAssistantText = true
            convertedMessages.push({
              id: `${msg.role}-${crypto.randomUUID()}`,
              role: msg.role as "user" | "assistant",
              content: block.text,
              timestamp: baseTimestamp,
              thinking: msg.thinking,
              disableAnimation: true,
              ...messageOutcome,
            })
          }
        }

        if (
          msg.role === "assistant" &&
          msg.thinking &&
          !pushedAssistantText
        ) {
          convertedMessages.push({
            id: `${msg.role}-${crypto.randomUUID()}`,
            role: "assistant",
            content: "",
            timestamp: baseTimestamp,
            thinking: msg.thinking,
            disableAnimation: true,
            ...messageOutcome,
          })
        }
      }

      setConversations((prev) => ({
        ...prev,
        [sessionId]: {
          id: sessionId,
          projectId,
          title: fullSession.title || sessionMeta?.title || PLACEHOLDER_SESSION_TITLE,
          messages: convertedMessages,
          updatedAt: new Date(fullSession.updated_at),
        },
      }))
      setSessionTitleStates((prev) => ({
        ...prev,
        [sessionId]: getInitialSessionTitleState(
          fullSession.title || sessionMeta?.title || PLACEHOLDER_SESSION_TITLE,
          convertedMessages
        ),
      }))
      setSessionTodos((prev) => ({ ...prev, [sessionId]: recoveredTodos }))
    } catch (err) {
      console.error('Failed to load session:', err)
      setConversations((prev) => ({
        ...prev,
        [sessionId]: {
          id: sessionId,
          projectId,
          title: sessionMeta?.title || PLACEHOLDER_SESSION_TITLE,
          messages: [],
          updatedAt: new Date(),
        },
      }))
      setSessionTitleStates((prev) => ({
        ...prev,
        [sessionId]: getInitialSessionTitleState(sessionMeta?.title || PLACEHOLDER_SESSION_TITLE),
      }))
      setSessionTodos((prev) => ({ ...prev, [sessionId]: [] }))
    }
  }

  const handleNewChat = async (projectId: string): Promise<string | null> => {
    try {
      setActiveSection('chat')
      const session = await createSession(projectId, PLACEHOLDER_SESSION_TITLE)
      const sessions = await listProjectSessions(projectId)
      setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }))

      const project = projects.find((item) => item.id === projectId)
      if (project) {
        setCurrentProject({
          id: project.id,
          name: project.name,
          workdir: project.workdir,
          created_at: project.created_at,
          updated_at: '',
        })
      }

      setActiveProjectId(projectId)
      setActiveSessionId(session.id)
      setConversations((prev) => ({
        ...prev,
        [session.id]: {
          id: session.id,
          projectId,
          title: PLACEHOLDER_SESSION_TITLE,
          messages: [],
          updatedAt: new Date(),
        },
      }))
      setSessionTitleStates((prev) => ({
        ...prev,
        [session.id]: {
          stage: 'placeholder',
          autoRenameCount: 0,
        },
      }))
      setSessionTodos((prev) => ({ ...prev, [session.id]: [] }))
      return session.id
    } catch (err) {
      console.error('Failed to create session:', err)
      return null
    }
  }

  /// 点击「新线程」—— 清空当前 session，显示「开始构建」界面
  const handleNewThread = () => {
    setActiveSessionId(null)
    setActiveProjectId(null)
    setCurrentProject(null)
  }

  /// Home 页面切换所选项目（不立即创建 session）
  const handleHomeProjectSelect = (projectId: string | null) => {
    setActiveProjectId(projectId)
    setActiveSessionId(null)
    if (!projectId) {
      setCurrentProject(null)
      return
    }
    const project = projects.find((p) => p.id === projectId)
    if (project) {
      setCurrentProject({
        id: project.id,
        name: project.name,
        workdir: project.workdir,
        created_at: project.created_at,
        updated_at: '',
      })
    }
  }

  /// Home 页面发送第一条消息 → 先创建 session 再发送
  const handleSendMessage = (text: string) => {
    void sendMessage(text)
  }

  /// 在 Home 页面点击「添加新项目」→ 打开文件夹选择器 → 创建项目 → 选中并留在 Home
  const handlePickFolderAndCreateProject = async () => {
    try {
      const folderPath = await pickFolderDialog()
      if (!folderPath) return // user cancelled

      const folderName = folderPath.split('/').filter(Boolean).pop() ?? '新项目'
      const newProject = await createProject(folderName, folderPath)

      // Refresh project list so the new project appears in the dropdown
      await loadProjects()

      // Select the new project on the Home screen — no session created yet
      handleHomeProjectSelect(newProject.id)
    } catch (err) {
      console.error('[handlePickFolderAndCreateProject] Failed:', err)
    }
  }

  const handleDeleteProject = async (projectId: string) => {
    try {
      await deleteProject(projectId)
      await loadProjects()
      if (activeProjectId === projectId) {
        setActiveProjectId(null)
        setActiveSessionId(null)
        setCurrentProject(null)
      }
    } catch (err) {
      console.error('Failed to delete project:', err)
    }
  }

  const handleRenameProject = async (projectId: string, newName: string) => {
    try {
      await renameProject(projectId, newName)
      await loadProjects()
      if (currentProject?.id === projectId) {
        setCurrentProject((prev) => (prev ? { ...prev, name: newName } : null))
      }
    } catch (err) {
      console.error('Failed to rename project:', err)
    }
  }

  const handleDeleteSession = async (projectId: string, sessionId: string) => {
    try {
      await deleteSession(sessionId)
      const sessions = await listProjectSessions(projectId)
      setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }))
      setConversations((prev) => {
        const next = { ...prev }
        delete next[sessionId]
        return next
      })
      setSessionTitleStates((prev) => {
        const next = { ...prev }
        delete next[sessionId]
        return next
      })
      setSessionTodos((prev) => {
        const next = { ...prev }
        delete next[sessionId]
        return next
      })
      if (activeSessionId === sessionId) {
        setActiveSessionId(null)
      }
    } catch (err) {
      console.error('Failed to delete session:', err)
    }
  }

  // P0: user-initiated rename — marks stage as 'manual' so auto-rename never overrides again
  const handleRenameSession = (sessionId: string, newTitle: string) => {
    const conv = conversations[sessionId]
    if (!conv) return
    // Lock the stage first so any in-flight auto-rename is a no-op once it checks the stage
    setSessionTitleStates((prev) => ({
      ...prev,
      [sessionId]: {
        stage: 'manual',
        autoRenameCount: MAX_AUTO_RENAME_COUNT,
      },
    }))
    syncSessionTitle(conv.projectId, sessionId, newTitle)
  }

  const handleTogglePinSession = async (projectId: string, sessionId: string, pinned: boolean) => {
    try {
      await setSessionPinned(sessionId, !pinned)
      const sessions = await listProjectSessions(projectId)
      setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }))
    } catch (err) {
      console.error('Failed to toggle session pin:', err)
    }
  }

  const handleCreateProject = async (name: string, workdir: string) => {
    try {
      const newProject = await createProject(name, workdir)
      await loadProjects()
      const session = await createSession(newProject.id, PLACEHOLDER_SESSION_TITLE)

      setCurrentProject(newProject)
      setActiveProjectId(newProject.id)
      setActiveSessionId(session.id)
      setConversations((prev) => ({
        ...prev,
        [session.id]: {
          id: session.id,
          projectId: newProject.id,
          title: PLACEHOLDER_SESSION_TITLE,
          messages: [],
          updatedAt: new Date(),
        },
      }))
      setSessionTitleStates((prev) => ({
        ...prev,
        [session.id]: {
          stage: 'placeholder',
          autoRenameCount: 0,
        },
      }))
      setSessionTodos((prev) => ({ ...prev, [session.id]: [] }))
      setIsCreateProjectOpen(false)
    } catch (err) {
      console.error('Failed to create project:', err)
    }
  }

  const stopAgentStream = async (sessionIdOverride?: string) => {
    const sessionId = sessionIdOverride ?? activeSessionId
    if (!sessionId) return
    const streamAbortHandle = streamAbortHandles[sessionId]
    if (streamAbortHandle) {
      try {
        await invoke<string>('stop_agent_stream', { streamId: streamAbortHandle })
      } catch (err) {
        console.error('Failed to stop stream:', err)
      }
      setStreamAbortHandles((prev) => {
        const { [sessionId]: _removed, ...rest } = prev
        return rest
      })
      setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
    }
  }

  const handlePermissionDecision = async (
    decision: 'allow' | 'deny',
    scope: 'once' | 'session' = 'once'
  ) => {
    if (!permissionPrompt) return
    const sessionId = permissionPrompt.session_id
    try {
      await respondPermission(sessionId, decision, {
        toolName: permissionPrompt.tool_name,
        scope,
      })
    } catch (err) {
      console.error('Failed to respond permission:', err)
    } finally {
      // Phase M2.8 — clear the projection-store approval so the
      // dialog closes.  Local `setPermissionPrompt(null)` removed.
      runtimeProjectionStore.dispatch({
        kind: 'permission_resolved',
        sessionId,
        decision,
        scope,
        receivedAt: Date.now(),
      })
    }
  }

  const buildResumePrompt = (resumeCursor: string) =>
    `[resume_cursor] ${resumeCursor}\n` +
    '请从该游标继续完成上一次任务，仅补全未完成步骤，禁止重复已确认的副作用操作。'

  const sendMessage = async (
    overrideText?: string,
    options?: {
      sessionIdOverride?: string
      isInternalResume?: boolean
      resumeCursor?: string
    }
  ) => {
    const messageText = (overrideText ?? input).trim()
    let targetSessionId = options?.sessionIdOverride ?? activeSessionId
    let freshProjectId: string | undefined
    if (!targetSessionId && !options?.sessionIdOverride) {
      const fallbackProjectId = activeProjectId ?? projects[0]?.id ?? null
      if (fallbackProjectId) {
        freshProjectId = fallbackProjectId
        targetSessionId = await handleNewChat(fallbackProjectId)
      }
    }
    if (!messageText || !targetSessionId) return
    const sessionId = targetSessionId
    if (sessionLoadingRef.current[sessionId]) return
    if (!options?.isInternalResume) {
      setSessionTodos((prev) => ({ ...prev, [sessionId]: [] }))
    }

    // When handleNewChat just created this session the React state hasn't
    // re-rendered yet, so conversations[sessionId] is still undefined.
    // Construct a synthetic conv for new sessions rather than bailing out.
    const conv =
      conversations[sessionId] ??
      (freshProjectId
        ? {
            id: sessionId,
            projectId: freshProjectId,
            title: PLACEHOLDER_SESSION_TITLE,
            messages: [],
            updatedAt: new Date(),
          }
        : null)
    if (!conv) return

    if (!options?.isInternalResume) {
      autoResumeAttemptsRef.current[sessionId] = 0
    } else if (options.resumeCursor) {
      setRecoveryStateForCursor(sessionId, options.resumeCursor, true)
    }

    const userMsg: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      content: messageText,
      timestamp: new Date(),
    }

    const updatedConv = {
      ...conv,
      messages: [...conv.messages, userMsg],
      updatedAt: new Date(),
    }

    setConversations((prev) => ({ ...prev, [sessionId]: updatedConv }))
    maybeAutoRenameSession(conv.projectId, sessionId, updatedConv)
    if (!overrideText) {
      setInput('')
    }
    setSessionLoading((prev) => ({ ...prev, [sessionId]: true }))

    const startTime = Date.now()
    let accumulatedText = ''
    let accumulatedThinking = ''
    let assistantMsgId: string | null = null
    let hasPendingTextDelta = false
    let hasPendingThinkingDelta = false
    let streamRafId: number | null = null
    const seenToolCallIds = new Set<string>()

    const createAssistantMessage = () => {
      if (assistantMsgId) return
      assistantMsgId = crypto.randomUUID()
      const assistantMsg: Message = {
        id: assistantMsgId,
        role: 'assistant',
        content: '',
        timestamp: new Date(),
        isStreaming: true,
        statusLabel: options?.isInternalResume ? '正在恢复未完成任务…' : undefined,
        statusKind: options?.isInternalResume ? 'info' : undefined,
      }

      setConversations((prev) => {
        const currentConv = prev[sessionId]
        if (!currentConv) return prev
        return {
          ...prev,
          [sessionId]: {
            ...currentConv,
            messages: [...currentConv.messages, assistantMsg],
          },
        }
      })
    }

    const finalizeCurrentAssistantSegment = () => {
      if (!assistantMsgId) return

      const finalizedAssistantId = assistantMsgId
      setConversations((prev) => {
        const currentConv = prev[activeSessionId]
        if (!currentConv) return prev

        return {
          ...prev,
          [activeSessionId]: {
            ...currentConv,
            messages: currentConv.messages.map((msg) =>
              msg.id === finalizedAssistantId
                ? { ...msg, isStreaming: false, statusLabel: undefined, statusKind: undefined }
                : msg
            ),
          },
        }
      })

      assistantMsgId = null
      accumulatedText = ''
      accumulatedThinking = ''
    }

    const ensureAssistantMessage = () => {
      if (!assistantMsgId) {
        createAssistantMessage()
      }

      return assistantMsgId
    }

    const flushAssistantDeltas = () => {
      if (!assistantMsgId) return
      if (!hasPendingTextDelta && !hasPendingThinkingDelta) return
      const currentAssistantId = assistantMsgId
      const nextText = accumulatedText
      const nextThinking = accumulatedThinking

      hasPendingTextDelta = false
      hasPendingThinkingDelta = false

      setConversations((prev) => {
        const currentConv = prev[sessionId]
        if (!currentConv) return prev
        return {
          ...prev,
          [sessionId]: {
            ...currentConv,
            messages: currentConv.messages.map((msg) =>
              msg.id === currentAssistantId
                ? { ...msg, content: nextText, thinking: nextThinking || msg.thinking, statusLabel: undefined, statusKind: undefined }
                : msg
            ),
          },
        }
      })
    }

    const cancelScheduledAssistantFlush = () => {
      if (streamRafId !== null) {
        window.cancelAnimationFrame(streamRafId)
        streamRafId = null
      }
    }

    const scheduleAssistantFlush = () => {
      if (streamRafId !== null) return
      streamRafId = window.requestAnimationFrame(() => {
        streamRafId = null
        flushAssistantDeltas()
      })
    }

    // Detect slash commands
    if (messageText.startsWith('/')) {
      const cmdPrefix = messageText.split(/\s+/)[0]
      const suggestions = await suggestSlashCommands(cmdPrefix, 1)
      if (suggestions.length > 0) {
        // Check if this is a skill-based slash command: resolve the invocation
        // message (contains full SKILL.md) and send it to the agent for LLM
        // processing.  Builtin commands (/help, /clear, /skills, /agents) always
        // start with a known prefix that does NOT resolve via resolveSkillSlash,
        // so they fall through to executeSlashCommand as before.
        const cwd = currentProject?.workdir
        const skillInvocation = await resolveSkillSlash(messageText, cwd)
        if (skillInvocation) {
          // Skill slash: route through the agent so the LLM activates the skill
          // via the skill() tool and then responds according to its instructions.
          try {
            createAssistantMessage()
            const streamId = await startAgentStream(sessionId, skillInvocation, permissionMode)
            setStreamAbortHandles((prev) => ({ ...prev, [sessionId]: streamId }))
            const unlisten = await listenToStream(streamId, (payload: StreamTokenPayload) => {
              if (payload.event_type === 'text_delta' && payload.text) {
                ensureAssistantMessage()
                if (!accumulatedText && !accumulatedThinking && assistantMsgId) {
                  setConversations((prev) => {
                    const currentConv = prev[sessionId]
                    if (!currentConv) return prev
                    return {
                      ...prev,
                      [sessionId]: {
                        ...currentConv,
                        messages: currentConv.messages.map((msg) =>
                          msg.id === assistantMsgId ? { ...msg, statusLabel: undefined, statusKind: undefined } : msg
                        ),
                      },
                    }
                  })
                }
                accumulatedText += payload.text
                hasPendingTextDelta = true
                scheduleAssistantFlush()
                // Phase TTS-D / P1：把 text_delta 喂给 voice bridge（hook 内部
                // 自己判断 enabled / voice / sentence boundary）
                agentVoice.feed(payload.text)
              } else if (payload.event_type === 'thinking_delta' && payload.thinking) {
                ensureAssistantMessage()
                accumulatedThinking += payload.thinking
                hasPendingThinkingDelta = true
                scheduleAssistantFlush()
              } else if (payload.event_type === 'tool_call_update') {
                cancelScheduledAssistantFlush()
                flushAssistantDeltas()
                const toolCallId = payload.tool_call_id
                if (toolCallId) {
                  const nextStatus = payload.tool_status ?? 'running'
                  if (!seenToolCallIds.has(toolCallId)) {
                    seenToolCallIds.add(toolCallId)
                    finalizeCurrentAssistantSegment()
                  }
                  setConversations((prev) => {
                    const currentConv = prev[sessionId]
                    if (!currentConv) return prev
                    const existingIndex = currentConv.messages.findIndex(
                      (msg) => msg.role === 'tool' && msg.toolCallId === toolCallId
                    )
                    const existingMessage = existingIndex >= 0 ? currentConv.messages[existingIndex] : undefined
                    // For `memory_store` results we extract the structured
                    // policy fields from the tool result JSON so deny / prompt
                    // states surface in the UI even though the control-plane
                    // permission `policy_decision` is always 'allow'.
                    const memoryFields =
                      payload.tool_name === 'memory_store'
                        ? extractMemoryStoreFields(payload.tool_result ?? null)
                        : null
                    const updatedToolMessage: Message = {
                      id: existingMessage?.id ?? `tool-${toolCallId}-${Date.now()}`,
                      role: 'tool',
                      content: nextStatus === 'queued' || nextStatus === 'running'
                        ? existingMessage?.content ?? ''
                        : (payload.tool_result || existingMessage?.content || ''),
                      timestamp: existingMessage?.timestamp ?? new Date(),
                      toolCallId,
                      streamId: payload.stream_id ?? existingMessage?.streamId,
                      toolName: payload.tool_name ?? existingMessage?.toolName ?? 'unknown',
                      toolArgs: payload.tool_args ?? existingMessage?.toolArgs,
                      toolDurationMs: payload.tool_duration_ms ?? existingMessage?.toolDurationMs,
                      isError: nextStatus === 'error' || existingMessage?.isError,
                      toolStatus: nextStatus,
                      effectiveWorkdir: payload.effective_workdir ?? existingMessage?.effectiveWorkdir,
                      policyDecision:
                        memoryFields?.policyDecision ??
                        payload.policy_decision ??
                        existingMessage?.policyDecision,
                      memoryScope: memoryFields?.memoryScope ?? existingMessage?.memoryScope,
                      memoryReasonCode:
                        memoryFields?.memoryReasonCode ?? existingMessage?.memoryReasonCode,
                      evidenceId: payload.evidence_id ?? existingMessage?.evidenceId,
                      requestId: payload.request_id ?? existingMessage?.requestId,
                      taskOutcome: payload.task_outcome ?? existingMessage?.taskOutcome,
                      degradedReason: payload.degraded_reason ?? existingMessage?.degradedReason,
                      resumeAvailable: payload.resume_available ?? existingMessage?.resumeAvailable,
                      resumeCursor: payload.resume_cursor ?? existingMessage?.resumeCursor,
                      disableAnimation: true,
                    }
                    const nextMessages =
                      existingIndex >= 0
                        ? currentConv.messages.map((msg, index) => (index === existingIndex ? updatedToolMessage : msg))
                        : [...currentConv.messages, updatedToolMessage]
                    return {
                      ...prev,
                      [sessionId]: { ...currentConv, messages: nextMessages },
                    }
                  })
                  if (payload.tool_name === 'TodoWrite' && payload.tool_result) {
                    const nextTodos = extractTodosFromToolResult(payload.tool_result)
                    if (nextTodos) {
                      setSessionTodos((prev) => ({ ...prev, [sessionId]: nextTodos }))
                    }
                  }
                }
              } else if (payload.event_type === 'stream_complete') {
                cancelScheduledAssistantFlush()
                flushAssistantDeltas()
                // Phase TTS-D / P1：flush 残留半句让 voice bridge 合成完
                void agentVoice.flushAndStop()
                setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
                setStreamAbortHandles((prev) => {
                  const { [sessionId]: _removed, ...rest } = prev
                  return rest
                })
                void refreshProjectSessions(conv.projectId).catch(() => {})
                if (assistantMsgId) {
                  setConversations((prev) => {
                    const currentConv = prev[sessionId]
                    if (!currentConv) return prev
                    return {
                      ...prev,
                      [sessionId]: {
                        ...currentConv,
                        messages: currentConv.messages.map((msg) => {
                          if (msg.id === assistantMsgId) {
                            return {
                              ...msg,
                              isStreaming: false,
                              thinkingTime: Date.now() - startTime,
                              taskOutcome: payload.task_outcome ?? msg.taskOutcome ?? 'completed',
                              degradedReason: payload.degraded_reason ?? msg.degradedReason,
                              resumeAvailable: payload.resume_available ?? msg.resumeAvailable ?? false,
                              resumeCursor: payload.resume_cursor ?? msg.resumeCursor,
                              memoryContext: payload.memory_context ?? msg.memoryContext,
                              contextBudgetUsage: payload.context_budget_usage ?? msg.contextBudgetUsage,
                            }
                          }
                          if (msg.role === 'tool' && (msg.toolStatus === 'queued' || msg.toolStatus === 'running')) {
                            return { ...msg, toolStatus: 'error', isError: true }
                          }
                          return msg
                        }),
                      },
                    }
                  })
                }
                unlisten()
              } else if (payload.event_type === 'stream_error') {
                cancelScheduledAssistantFlush()
                flushAssistantDeltas()
                unlisten()
                setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
                setStreamAbortHandles((prev) => {
                  const { [sessionId]: _removed, ...rest } = prev
                  return rest
                })
              }
            })
          } catch (err) {
            setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
          }
          return
        }

        // Builtin slash command: execute and display result as a static message
        setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
        try {
          const result = await executeSlashCommand(messageText, sessionId)
          const assistantMsgId = crypto.randomUUID()
          const assistantMsg: Message = {
            id: assistantMsgId,
            role: 'assistant',
            content: result,
            timestamp: new Date(),
            isStreaming: false,
          }
          setConversations((prev) => {
            const currentConv = prev[sessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [sessionId]: {
                ...currentConv,
                messages: [...currentConv.messages, assistantMsg],
              },
            }
          })
        } catch (err) {
          const errorMsg: Message = {
            id: crypto.randomUUID(),
            role: 'assistant',
            content: String(err),
            timestamp: new Date(),
            isStreaming: false,
          }
          setConversations((prev) => {
            const currentConv = prev[sessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [sessionId]: {
                ...currentConv,
                messages: [...currentConv.messages, errorMsg],
              },
            }
          })
        }
        return
      }
    }

    try {
      createAssistantMessage()
      const streamId = await startAgentStream(sessionId, userMsg.content, permissionMode)
      setStreamAbortHandles((prev) => ({ ...prev, [sessionId]: streamId }))

      const unlisten = await listenToStream(streamId, (payload: StreamTokenPayload) => {
        if (payload.event_type === 'text_delta' && payload.text) {
          ensureAssistantMessage()
          if (!accumulatedText && !accumulatedThinking && assistantMsgId) {
            setConversations((prev) => {
              const currentConv = prev[sessionId]
              if (!currentConv) return prev
              return {
                ...prev,
                [sessionId]: {
                  ...currentConv,
                  messages: currentConv.messages.map((msg) =>
                    msg.id === assistantMsgId ? { ...msg, statusLabel: undefined, statusKind: undefined } : msg
                  ),
                },
              }
            })
          }
          accumulatedText += payload.text
          hasPendingTextDelta = true
          scheduleAssistantFlush()
          // Phase TTS-D / P1：第二条流路径同样喂给 voice bridge
          agentVoice.feed(payload.text)
        } else if (payload.event_type === 'thinking_delta' && payload.thinking) {
          ensureAssistantMessage()
          if (!accumulatedText && !accumulatedThinking && assistantMsgId) {
            setConversations((prev) => {
              const currentConv = prev[sessionId]
              if (!currentConv) return prev
              return {
                ...prev,
                [sessionId]: {
                  ...currentConv,
                  messages: currentConv.messages.map((msg) =>
                    msg.id === assistantMsgId ? { ...msg, statusLabel: undefined, statusKind: undefined } : msg
                  ),
                },
              }
            })
          }
          accumulatedThinking += payload.thinking
          hasPendingThinkingDelta = true
          scheduleAssistantFlush()
        } else if (payload.event_type === 'tool_call_update') {
          cancelScheduledAssistantFlush()
          flushAssistantDeltas()
          const toolCallId = payload.tool_call_id
          if (toolCallId) {
            const nextStatus = payload.tool_status ?? 'running'

            if (!seenToolCallIds.has(toolCallId)) {
              seenToolCallIds.add(toolCallId)
              finalizeCurrentAssistantSegment()
            }

            setConversations((prev) => {
              const currentConv = prev[sessionId]
              if (!currentConv) return prev

              const existingIndex = currentConv.messages.findIndex(
                (msg) => msg.role === 'tool' && msg.toolCallId === toolCallId
              )
              const existingMessage = existingIndex >= 0 ? currentConv.messages[existingIndex] : undefined

              // Mirror of the skill-invocation branch above: pull deny / prompt
              // state out of the structured `memory_store` JSON so the UI can
              // render the highlighted MemoryWriteCard for those decisions.
              const memoryFields =
                payload.tool_name === 'memory_store'
                  ? extractMemoryStoreFields(payload.tool_result ?? null)
                  : null

              const updatedToolMessage: Message = {
                id: existingMessage?.id ?? `tool-${toolCallId}-${Date.now()}`,
                role: 'tool',
                content: nextStatus === 'queued' || nextStatus === 'running'
                  ? existingMessage?.content ?? ''
                  : (payload.tool_result || existingMessage?.content || ''),
                timestamp: existingMessage?.timestamp ?? new Date(),
                toolCallId,
                streamId: payload.stream_id ?? existingMessage?.streamId,
                toolName: payload.tool_name ?? existingMessage?.toolName ?? 'unknown',
                toolArgs: payload.tool_args ?? existingMessage?.toolArgs,
                toolDurationMs: payload.tool_duration_ms ?? existingMessage?.toolDurationMs,
                isError: nextStatus === 'error' || existingMessage?.isError,
                toolStatus: nextStatus,
                effectiveWorkdir: payload.effective_workdir ?? existingMessage?.effectiveWorkdir,
                policyDecision:
                  memoryFields?.policyDecision ??
                  payload.policy_decision ??
                  existingMessage?.policyDecision,
                memoryScope: memoryFields?.memoryScope ?? existingMessage?.memoryScope,
                memoryReasonCode:
                  memoryFields?.memoryReasonCode ?? existingMessage?.memoryReasonCode,
                evidenceId: payload.evidence_id ?? existingMessage?.evidenceId,
                requestId: payload.request_id ?? existingMessage?.requestId,
                taskOutcome: payload.task_outcome ?? existingMessage?.taskOutcome,
                degradedReason: payload.degraded_reason ?? existingMessage?.degradedReason,
                resumeAvailable: payload.resume_available ?? existingMessage?.resumeAvailable,
                resumeCursor: payload.resume_cursor ?? existingMessage?.resumeCursor,
                disableAnimation: true,
              }

              const nextMessages =
                existingIndex >= 0
                  ? currentConv.messages.map((msg, index) => (index === existingIndex ? updatedToolMessage : msg))
                  : [...currentConv.messages, updatedToolMessage]

              return {
                ...prev,
                [sessionId]: {
                  ...currentConv,
                  messages: nextMessages,
                },
              }
            })
          }
          // Parse TodoWrite SSE events
          if (payload.tool_name === 'TodoWrite' && payload.tool_result) {
            const nextTodos = extractTodosFromToolResult(payload.tool_result)
            if (nextTodos) {
              setSessionTodos((prev) => ({
                ...prev,
                [sessionId]: nextTodos,
              }))
            }
          }
        } else if (payload.event_type === 'final_text_override' && payload.text) {
          cancelScheduledAssistantFlush()
          const currentAssistantId = ensureAssistantMessage()
          accumulatedText = payload.text
          hasPendingTextDelta = false
          hasPendingThinkingDelta = false
          setConversations((prev) => {
            const currentConv = prev[activeSessionId]
            if (!currentConv) return prev
            return {
              ...prev,
                [activeSessionId]: {
                  ...currentConv,
                  messages: currentConv.messages.map((msg) =>
                    msg.id === currentAssistantId
                      ? { ...msg, content: accumulatedText, statusLabel: undefined, statusKind: undefined }
                      : msg
                  ),
                },
            }
          })
        } else if (payload.event_type === 'stream_complete') {
          cancelScheduledAssistantFlush()
          flushAssistantDeltas()
          // Phase TTS-D / P1：第二条流路径同样要 flush 残留半句
          void agentVoice.flushAndStop()
          setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
          setStreamAbortHandles((prev) => {
            const { [sessionId]: _removed, ...rest } = prev
            return rest
          })
          void refreshProjectSessions(conv.projectId).catch((error) => {
            console.error('Failed to refresh session counts:', error)
          })
          autoResumeAttemptsRef.current[sessionId] = 0
          attemptedAutoResumeCursorsRef.current.clear()
          setConversations((prev) => {
            const currentConv = prev[sessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [sessionId]: {
                ...currentConv,
                messages: currentConv.messages.map((msg) =>
                  msg.isRecovering ? { ...msg, isRecovering: false } : msg
                ),
              },
            }
          })
          if (assistantMsgId) {
            const streamTaskOutcome = payload.task_outcome
            const streamDegradedReason = payload.degraded_reason
            const streamResumeAvailable = payload.resume_available
            const streamResumeCursor = payload.resume_cursor
            setConversations((prev) => {
              const currentConv = prev[sessionId]
              if (!currentConv) return prev
              const finalStatus = buildLoopCompletionStatus(
                currentConv.messages,
                payload.stream_id,
                streamTaskOutcome,
                streamDegradedReason
              )
              return {
                ...prev,
                [sessionId]: {
                  ...currentConv,
                  messages: currentConv.messages.map((msg) => {
                    if (msg.id === assistantMsgId) {
                      return {
                        ...msg,
                        isStreaming: false,
                        thinkingTime: Date.now() - startTime,
                        taskOutcome: streamTaskOutcome ?? msg.taskOutcome ?? 'completed',
                        degradedReason: streamDegradedReason ?? msg.degradedReason,
                        resumeAvailable: streamResumeAvailable ?? msg.resumeAvailable ?? false,
                        resumeCursor: streamResumeCursor ?? msg.resumeCursor,
                        statusLabel: finalStatus.label,
                        statusKind: finalStatus.kind,
                        memoryContext: payload.memory_context ?? msg.memoryContext,
                        contextBudgetUsage: payload.context_budget_usage ?? msg.contextBudgetUsage,
                      }
                    }
                    if (msg.role === 'tool' && (msg.toolStatus === 'queued' || msg.toolStatus === 'running')) {
                      return {
                        ...msg,
                        toolStatus: 'error',
                        isError: true,
                        content: msg.content || 'stream completed before tool reached terminal state',
                      }
                    }
                    return msg
                  }),
                },
              }
            })
          }
          unlisten()
        } else if (payload.event_type === 'stream_error') {
          cancelScheduledAssistantFlush()
          flushAssistantDeltas()
          unlisten()
          setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
          setStreamAbortHandles((prev) => {
            const { [sessionId]: _removed, ...rest } = prev
            return rest
          })
          void refreshProjectSessions(conv.projectId).catch((error) => {
            console.error('Failed to refresh session counts:', error)
          })
          setConversations((prev) => {
            const currentConv = prev[sessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [sessionId]: {
                ...currentConv,
                messages: currentConv.messages.map((msg) =>
                  msg.isRecovering ? { ...msg, isRecovering: false } : msg
                ),
              },
            }
          })
          const errMsg = payload.tool_result || 'Agent 执行失败，请稍后重试。'
          const taskOutcome = payload.task_outcome ?? 'failed'
          const degradedReason = payload.degraded_reason ?? errMsg
          const resumeAvailable = payload.resume_available ?? false
          const resumeCursor = payload.resume_cursor
          setConversations((prev) => {
            const currentConv = prev[sessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [sessionId]: {
                ...currentConv,
                messages: currentConv.messages.map((msg) => {
                  if (msg.id === assistantMsgId) {
                    const friendlyError = taskOutcome === 'partial_success'
                      ? `${errMsg}\n\n[task_outcome] partial_success`
                      : errMsg
                    return {
                      ...msg,
                      content: '',
                      isStreaming: false,
                      isError: true,
                      taskOutcome,
                      degradedReason,
                      resumeAvailable,
                      resumeCursor,
                      statusLabel: undefined,
                      statusKind: undefined,
                      toolArgs: {
                        ...(msg.toolArgs ?? {}),
                        rawError: friendlyError,
                        taskOutcome,
                        degradedReason,
                        resumeAvailable,
                        resumeCursor,
                      },
                    }
                  }
                  if (msg.role === 'tool' && (msg.toolStatus === 'queued' || msg.toolStatus === 'running')) {
                    return {
                      ...msg,
                      toolStatus: 'error',
                      isError: true,
                      content: msg.content || errMsg,
                      requestId: payload.request_id ?? msg.requestId,
                      taskOutcome: taskOutcome ?? msg.taskOutcome,
                      degradedReason: degradedReason ?? msg.degradedReason,
                      resumeAvailable: resumeAvailable ?? msg.resumeAvailable,
                      resumeCursor: resumeCursor ?? msg.resumeCursor,
                      toolArgs: {
                        ...(msg.toolArgs ?? {}),
                        rawError: errMsg,
                        taskOutcome,
                        degradedReason,
                        resumeAvailable,
                        resumeCursor,
                      },
                    }
                  }
                  return msg
                }),
              },
            }
          })
          if (taskOutcome === 'partial_success' && resumeAvailable && resumeCursor) {
            const cursorKey = `${sessionId}:${resumeCursor}`
            const attemptCount = autoResumeAttemptsRef.current[sessionId] ?? 0
            if (
              !attemptedAutoResumeCursorsRef.current.has(cursorKey)
              && attemptCount < 2
            ) {
              attemptedAutoResumeCursorsRef.current.add(cursorKey)
              autoResumeAttemptsRef.current[sessionId] = attemptCount + 1
              setRecoveryStateForCursor(sessionId, resumeCursor, true)
              window.setTimeout(() => {
                void sendMessage(buildResumePrompt(resumeCursor), {
                  sessionIdOverride: sessionId,
                  isInternalResume: true,
                  resumeCursor,
                })
              }, 80)
            }
          }
        }
      })
    } catch (err) {
      console.error('startAgentStream error:', err)

      const errorMessage =
        err && typeof err === 'object' && 'message' in err
          ? String((err as { message?: string }).message || 'Agent 执行失败，请稍后重试。')
          : err instanceof Error
            ? err.message
            : 'Agent 执行失败，请稍后重试。'

      if (assistantMsgId) {
        setConversations((prev) => {
          const currentConv = prev[sessionId]
          if (!currentConv) return prev
          return {
            ...prev,
            [sessionId]: {
              ...currentConv,
              messages: currentConv.messages.map((msg) =>
                msg.id === assistantMsgId
                  ? {
                    ...msg,
                    content: '',
                    isStreaming: false,
                    isError: true,
                    statusLabel: undefined,
                    statusKind: undefined,
                    toolArgs: { rawError: errorMessage },
                  }
                  : msg
              ),
            },
          }
        })
      } else {
        // No assistant message yet — create one with the error
        const errorMsgId = crypto.randomUUID()
        const errorMsg: Message = {
          id: errorMsgId,
          role: 'assistant',
          content: '',
          timestamp: new Date(),
          isStreaming: false,
          isError: true,
          toolArgs: { rawError: errorMessage },
        }
        setConversations((prev) => ({
          ...prev,
          [sessionId]: {
            ...updatedConv,
            messages: [...updatedConv.messages, errorMsg],
          },
        }))
      }
      setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
      void refreshProjectSessions(conv.projectId).catch((error) => {
        console.error('Failed to refresh session counts:', error)
      })
      setConversations((prev) => {
        const currentConv = prev[sessionId]
        if (!currentConv) return prev
        return {
          ...prev,
          [sessionId]: {
            ...currentConv,
            messages: currentConv.messages.map((msg) =>
              msg.isRecovering ? { ...msg, isRecovering: false } : msg
            ),
          },
        }
      })
    } finally {
      // Don't set isLoading=false here — stream_complete or stopAgentStream handles it
    }
  }

  const handleResumeFromCursor = async (resumeCursor: string) => {
    await sendMessage(buildResumePrompt(resumeCursor), {
      isInternalResume: true,
      resumeCursor,
    })
  }

  const beginResize = (startX: number, separatorEl: HTMLDivElement, pointerId?: number) => {
    resizeRef.current = {
      startX,
      startWidth: leftPaneWidth,
    }

    const handleMove = (moveEvent: MouseEvent | globalThis.PointerEvent) => {
      if (!resizeRef.current) return

      const delta = moveEvent.clientX - resizeRef.current.startX
      const nextWidth = Math.min(
        maxLeftPaneWidth,
        Math.max(minLeftPaneWidth, resizeRef.current.startWidth + delta)
      )
      setLeftPaneWidth(nextWidth)
    }

    const handleUp = () => {
      resizeRef.current = null
      window.removeEventListener('pointermove', handleMove)
      window.removeEventListener('pointerup', handleUp)
      document.body.style.userSelect = ''
      document.body.style.cursor = ''
      if (typeof pointerId === 'number') {
        try {
          separatorEl.releasePointerCapture(pointerId)
        } catch {
          // ignore release errors when pointer capture is already lost
        }
      }
    }

    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
    window.addEventListener('pointermove', handleMove)
    window.addEventListener('pointerup', handleUp)
  }

  const startResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault()
    const separatorEl = event.currentTarget
    try {
      separatorEl.setPointerCapture(event.pointerId)
    } catch {
      // ignore capture failures and fall back to window listeners
    }
    beginResize(event.clientX, separatorEl, event.pointerId)
  }

  const toggleLeftPane = () => {
    setIsLeftPaneCollapsed((value) => {
      const next = !value
      if (!next) {
        // Opening left pane → close right rail
        setIsRightRailOpen(false)
      }
      return next
    })
  }

  const toggleRightRail = () => {
    setIsRightRailOpen((value) => {
      const next = !value
      if (next) {
        // Opening right rail → close left pane
        setIsLeftPaneCollapsed(true)
      }
      return next
    })
  }

  const handlePreviewFocusChange = (active: boolean) => {
    if (active) {
      wasPreviewFocusModeRef.current = true
      leftPaneCollapsedBeforePreviewRef.current = isLeftPaneCollapsed
      if (!isLeftPaneCollapsed) {
        setIsLeftPaneCollapsed(true)
      }
      return
    }
    if (wasPreviewFocusModeRef.current && !leftPaneCollapsedBeforePreviewRef.current) {
      setIsLeftPaneCollapsed(false)
    }
    wasPreviewFocusModeRef.current = false
  }

  const startWindowDrag = async (event: ReactMouseEvent<HTMLElement>) => {
    if (event.button !== 0) return
    const target = event.target as HTMLElement | null
    if (target?.closest('[data-window-no-drag="true"]')) return
    try {
      await appWindow.startDragging()
    } catch {
      // Ignore drag failures on platforms that do not support the request in this context.
    }
  }

  // MIG-013 — `loadMainAppData` removed: its responsibilities
  // are now owned by `runBootSequence` (in
  // `src/boot/boot-orchestrator.ts`), which is invoked both on
  // first boot (via the `useEffect` at the top of this file) and
  // after onboarding completes (via `handleOnboardingComplete`).
  // Keeping a second code path would re-introduce the exact
  // "two boot orchestrators" anti-pattern MIG-013 was designed
  // to retire.

  const promptDownloadSenseVoiceAfterOnboarding = async () => {
    // 已经下载过就跳过；通过 stt_model_status 判断
    try {
      const { sttModelStatus, sttDownloadOpenflowModel, sttSaveSettings } = await import('@/lib/tauri')
      const status = await sttModelStatus()
      if (status.openflow_ready) return
      // 之前显式拒绝过，1 周内不再问
      const SKIP_KEY = 'if2ai.stt.openflow.declined_at'
      const declinedAt = Number(localStorage.getItem(SKIP_KEY) ?? 0)
      if (declinedAt > 0 && Date.now() - declinedAt < 7 * 86400_000) return

      // 用 sonner 持久 toast：行动按钮 = 立即下载 / 稍后再说
      toast('启用本地中文语音输入？', {
        description:
          '推荐下载 SenseVoice 模型（约 230MB，离线、中文识别强）。也可稍后到「设置 → STT 语音输入」手动下载。',
        duration: Infinity,
        action: {
          label: '立即下载',
          onClick: () => {
            // 立即开始后台下载并显示进度 toast
            const id = toast.loading('SenseVoice 模型下载中…', {
              description: '约 230MB，根据网速 1-5 分钟',
              duration: Infinity,
            })
            void sttDownloadOpenflowModel({ preset: 'quantized' })
              .then(async () => {
                toast.success('SenseVoice 已就绪', {
                  id,
                  description: '麦克按钮现在可以使用本地中文转写',
                  duration: 5000,
                })
                // 自动把 provider 切到 openflow
                try { await sttSaveSettings({ provider: 'openflow' }) } catch {}
              })
              .catch((e) => {
                toast.error('下载失败', { id, description: String(e), duration: 6000 })
              })
          },
        },
        cancel: {
          label: '稍后',
          onClick: () => {
            localStorage.setItem(SKIP_KEY, String(Date.now()))
          },
        },
      })
    } catch (e) {
      console.warn('[onboarding] STT prompt failed:', e)
    }
  }

  const handleOnboardingComplete = () => {
    // MIG-013 — onboarding_complete resets the store to the
    // splash state, then we re-run the canonical boot sequence
    // so `phase` properly transitions to `'main'` (via
    // `store.bootReady(...)`). Previously this called
    // `loadMainAppData()` directly, which populated project
    // state but never moved the store past `'splash'`.
    bootstrapStore.onboardingComplete()
    void runBootSequence(bootstrapStore, {
      getOnboardingState,
      ensureDefaultWorkdir,
      listProjects,
      listProjectSessions,
    })
    // Onboarding 完成后询问是否下载本地中文 STT 模型（SenseVoice 230MB）
    void promptDownloadSenseVoiceAfterOnboarding()
  }

  // MIG-013 — App.tsx renders via the canonical
  // `<AppShell>` container. Boot phase / boot surface decision
  // moved into `AppShell` (which reads the bootstrap store
  // internally); AppShell reads activation-gate state via the
  // existing `useBootRoute` hook inside its own body.
  return (
    <AppShell
      navbar={{
        activeSection,
        onSelectSection: setActiveSection,
        onOpenSettings: () => openSettingsWindow(),
        appIconSrc,
      }}
      onWindowDrag={startWindowDrag}
      onOnboardingComplete={handleOnboardingComplete}
      chatSectionOverlay={
        <AgentVoiceIndicator
          isPlaying={agentVoice.isPlaying}
          pending={agentVoice.pending}
        />
      }
      router={{
        chat: (
          <ChatWorkspace
              projects={projects}
              projectSessions={projectSessions}
              activeProjectId={activeProjectId}
              activeSessionId={activeSessionId}
              currentProject={currentProject}
              branchLabel={branchLabel}
              activeTitle={activeTitle}
              activeMessages={activeMessages}
              input={input}
              isLoading={isActiveSessionLoading}
              loading={loading}
              onSelectProject={handleSelectProject}
              onSelectSession={handleSelectSession}
              onNewChat={handleNewChat}
              onNewThread={handleNewThread}
              onHomeProjectSelect={handleHomeProjectSelect}
              onPickFolderAndCreateProject={handlePickFolderAndCreateProject}
              onSendMessage={handleSendMessage}
              recentSessions={recentSessions}
              onDeleteProject={handleDeleteProject}
              onRenameProject={handleRenameProject}
              onDeleteSession={handleDeleteSession}
              onRenameSession={handleRenameSession}
              onTogglePinSession={handleTogglePinSession}
              onOpenInFinder={openProjectInFinder}
              onCreatePermanentWorktree={createPermanentWorktree}
              onInputChange={setInput}
              onSubmit={sendMessage}
              onResumeFromCursor={handleResumeFromCursor}
              onStop={stopAgentStream}
              selectedModel={selectedModel}
              onModelChange={setSelectedModel}
              permissionMode={permissionMode}
              onPermissionModeChange={setPermissionMode}
              todos={todos}
              isRightRailOpen={isRightRailOpen}
              onToggleRightRail={toggleRightRail}
              onRightRailOpenChange={setIsRightRailOpen}
              leftPaneWidth={leftPaneWidth}
              isLeftPaneCollapsed={isLeftPaneCollapsed}
              onResizeStart={startResize}
              onToggleLeftPane={toggleLeftPane}
              onStartWindowDrag={startWindowDrag}
              onPreviewFocusChange={handlePreviewFocusChange}
              runningSessionIds={runningSessionIds}
            />
        ),
        memory: (
          <MemoryBrowser
            onStartWindowDrag={startWindowDrag}
            activeProjectId={activeProjectId}
            activeSessionId={activeSessionId}
          />
        ),
        sectionWorkspace: ({ section, onBackToChat }) => (
          <SectionWorkspace section={section} onBackToChat={onBackToChat} />
        ),
      }}
      overlays={
        <>
          <CreateProjectDialog
            isOpen={isCreateProjectOpen}
            onClose={() => setIsCreateProjectOpen(false)}
            onSubmit={handleCreateProject}
          />

          <Dialog open={Boolean(permissionPrompt)}>
            <DialogContent
              showCloseButton={false}
              onEscapeKeyDown={(e) => e.preventDefault()}
              onPointerDownOutside={(e) => e.preventDefault()}
            >
              <DialogHeader>
                <DialogTitle>权限请求</DialogTitle>
                <DialogDescription>
                  {permissionPrompt?.message ?? '该操作需要更高权限。'}
                </DialogDescription>
              </DialogHeader>
              {permissionPrompt && (
                <div className="rounded-lg border border-black/10 bg-black/[0.02] px-3 py-2 text-[12px] text-black/60">
                  工具：{permissionPrompt.tool_name} · 当前模式：{permissionPrompt.current_mode}
                </div>
              )}
              <DialogFooter className="sm:justify-between">
                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => handlePermissionDecision('deny', 'once')}
                  >
                    拒绝本次
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => handlePermissionDecision('deny', 'session')}
                  >
                    本会话拒绝
                  </Button>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    onClick={() => handlePermissionDecision('allow', 'once')}
                  >
                    允许本次
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    onClick={() => handlePermissionDecision('allow', 'session')}
                  >
                    本会话允许
                  </Button>
                </div>
              </DialogFooter>
            </DialogContent>
          </Dialog>

          <TelemetryDrawer
            sessionId={activeSessionId}
            open={isTelemetryDrawerOpen}
            onClose={() => setIsTelemetryDrawerOpen(false)}
          />
        </>
      }
    />
  )
}

export default App
