import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  startAgentStream,
  listenToStream,
  listenToPermissionRequests,
  respondPermission,
  onboarding_get_state,
  listProjects,
  listProjectSessions,
  createPermanentWorktree,
  createProject,
  deleteProject,
  renameProject,
  renameSession,
  deleteSession,
  setSessionPinned,
  createSession,
  getSession,
  openProjectInFinder,
  openSettingsWindow,
  listenToChatPrefill,
  pickFolderDialog,
  ensureDefaultWorkdir,
  invoke,
  executeSlashCommand,
  suggestSlashCommands,
  resolveSkillSlash,
  type PermissionRequestPayload,
  type PermissionMode,
  type Project,
  type ProjectMeta,
  type SessionMeta,
  type StreamTokenPayload,
} from '@/lib/tauri'
import { GlobalNavbar } from '@/modules/app-shell/components/GlobalNavbar'
import { SectionWorkspace } from '@/modules/app-shell/components/SectionWorkspace'
import type { AppSection } from '@/modules/app-shell/types'
import { ChatWorkspace } from '@/modules/chat/components/ChatWorkspace'
import type { Conversation, Message, RecentSession, SessionTitleState } from '@/modules/chat/types'
import { OnboardingApp } from '@/modules/onboarding/OnboardingApp'
import { MemoryBrowser } from '@/components/memory/MemoryBrowser'
import { If2AiLoadingScreen } from '@/components/loading/If2AiLoadingScreen'
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
  const [showSplash, setShowSplash] = useState(true)
  const [showOnboarding, setShowOnboarding] = useState(false)

  // Single coordinated startup: check onboarding during splash, then decide route
  useEffect(() => {
    let cancelled = false

    const boot = async () => {
      // Run onboarding check with timeout to avoid blocking indefinitely
      const onboardingCheck = (async () => {
        try {
          const raw = await Promise.race([
            onboarding_get_state(),
            new Promise<never>((_, reject) =>
              setTimeout(() => reject(new Error('onboarding_get_state timeout')), 3000)
            ),
          ])
          const obj = raw as Record<string, unknown>
          const tag = obj['state'] as string | undefined
          return tag === 'first_launch' || tag === 'onboarding'
        } catch (err) {
          console.warn('[boot] onboarding check failed, defaulting to no onboarding:', err)
          return false
        }
      })()

      // Enforce minimum splash duration (800ms)
      const splashTimer = new Promise<void>((resolve) => {
        setTimeout(resolve, 800)
      })

      const [isOnboarding] = await Promise.all([onboardingCheck, splashTimer])

      if (cancelled) return

      setShowOnboarding(isOnboarding)

      if (!isOnboarding) {
        // Not onboarding — load main app data before dismissing splash
        try {
          // Ensure the default workaround project exists, and get its id
          let defaultProjectId: string | null = null
          try {
            const [, projId] = await ensureDefaultWorkdir()
            defaultProjectId = projId
          } catch {
            // Non-fatal — continue without default project
          }

          const projectList = await listProjects()
          setProjects(projectList)

          const sessionsMap: Record<string, SessionMeta[]> = {}
          for (const project of projectList) {
            sessionsMap[project.id] = await listProjectSessions(project.id)
          }
          setProjectSessions(sessionsMap)

          // Always start on the Home screen — show the default project selected
          // (no session restored on launch, per design decision)
          if (defaultProjectId) {
            const project = projectList.find((item) => item.id === defaultProjectId)
            if (project) {
              setActiveProjectId(defaultProjectId)
              setCurrentProject({
                id: project.id,
                name: project.name,
                workdir: project.workdir,
                created_at: project.created_at,
                updated_at: '',
              })
            }
          }
          // activeSessionId stays null → HomeScreen is shown

          // Load the real active model from config
          try {
            const activeModel = await invoke<{ provider_id: string; model_id: string } | null>('model_get_active')
            if (activeModel) {
              setSelectedModel(`${activeModel.provider_id}/${activeModel.model_id}`)
            }
          } catch {
            // Fallback: leave empty so chat-ui shows first available model from list
          }
        } catch (err) {
          console.error('Failed to load projects during boot:', err)
        }
      }

      if (cancelled) return
      setShowSplash(false)
    }

    void boot()

    return () => {
      cancelled = true
    }
  }, [])

  const [activeSection, setActiveSection] = useState<AppSection>(() => {
    if (typeof window === 'undefined') return 'chat'
    const stored = localStorage.getItem('lastActiveSection')
    return stored === 'skills' || stored === 'automation' ? stored : 'chat'
  })
  const [projects, setProjects] = useState<ProjectMeta[]>([])
  const [projectSessions, setProjectSessions] = useState<Record<string, SessionMeta[]>>({})
  const [currentProject, setCurrentProject] = useState<Project | null>(null)
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null)
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null)
  const [leftPaneWidth, setLeftPaneWidth] = useState(240)
  const [isLeftPaneCollapsed, setIsLeftPaneCollapsed] = useState(false)
  const [loading, setLoading] = useState(false)
  const [conversations, setConversations] = useState<Record<string, Conversation>>({})
  const [input, setInput] = useState('')
  const [sessionLoading, setSessionLoading] = useState<Record<string, boolean>>({})
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
  const [sessionTodos, setSessionTodos] = useState<Record<string, TodoItem[]>>({})
  const [sessionTitleStates, setSessionTitleStates] = useState<Record<string, SessionTitleState>>({})
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

  const [streamAbortHandles, setStreamAbortHandles] = useState<Record<string, string>>({})
  const [permissionPrompt, setPermissionPrompt] = useState<PermissionRequestPayload | null>(null)
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

  const formatSessionTitle = (raw: string): string => {
    if (raw.includes('[resume_cursor]')) {
      return activeConv?.title ?? '继续当前任务'
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

  const getInitialSessionTitleCandidate = (messages: Message[]): string | null => {
    const meaningfulMessages = getMeaningfulUserMessages(messages)
    const seedMessage =
      meaningfulMessages.find((message) => formatSessionTitle(message.content) !== PLACEHOLDER_SESSION_TITLE)
      ?? meaningfulMessages[0]
    if (!seedMessage) return null
    const nextTitle = formatSessionTitle(seedMessage.content)
    return nextTitle === PLACEHOLDER_SESSION_TITLE ? null : nextTitle
  }

  const getCorrectionTitleCandidate = (messages: Message[], currentTitle: string): string | null => {
    const userMessages = getMeaningfulUserMessages(messages)
    if (userMessages.length < 2) return null
    const recentCandidates = userMessages
      .slice(-2)
      .map((message) => formatSessionTitle(message.content))
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
    const meaningfulUserMessages = getMeaningfulUserMessages(conversation.messages)
    const meaningfulTurnCount = meaningfulUserMessages.length

    if (titleState.stage === 'manual' || titleState.stage === 'locked') return

    if (meaningfulTurnCount === 0) return

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
    const initialCandidate = getInitialSessionTitleCandidate(conversation.messages)
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
      const correctionCandidate = getCorrectionTitleCandidate(conversation.messages, currentTitle)
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

    void renameSession(sessionId, nextTitle).catch((err) => {
      console.error('Failed to rename session:', err)
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

  useEffect(() => {
    if (!activeConv || !activeSessionId) return
    maybeAutoRenameSession(activeConv.projectId, activeSessionId, activeConv)
  }, [activeConv, activeSessionId])

  useEffect(() => {
    let unlisten: (() => void) | undefined

    listenToPermissionRequests((payload) => {
      setPermissionPrompt(payload)
    }).then((dispose) => {
      unlisten = dispose
    }).catch((err) => {
      console.error('Failed to listen permission requests:', err)
    })

    return () => {
      if (unlisten) unlisten()
    }
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

  const refreshProjectSessions = async (projectId: string) => {
    const sessions = await listProjectSessions(projectId)
    setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }))
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
    try {
      await respondPermission(permissionPrompt.session_id, decision, {
        toolName: permissionPrompt.tool_name,
        scope,
      })
    } catch (err) {
      console.error('Failed to respond permission:', err)
    } finally {
      setPermissionPrompt(null)
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
                      policyDecision: payload.policy_decision ?? existingMessage?.policyDecision,
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
                policyDecision: payload.policy_decision ?? existingMessage?.policyDecision,
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

  const loadMainAppData = async () => {
    try {
      // Ensure default Playground project exists
      let defaultProjectId: string | null = null
      try {
        const [, projId] = await ensureDefaultWorkdir()
        defaultProjectId = projId
      } catch {
        // Non-fatal
      }

      const projectList = await listProjects()
      setProjects(projectList)

      const sessionsMap: Record<string, SessionMeta[]> = {}
      for (const project of projectList) {
        sessionsMap[project.id] = await listProjectSessions(project.id)
      }
      setProjectSessions(sessionsMap)

      // Start on Home screen with default project pre-selected
      if (defaultProjectId) {
        const project = projectList.find((p) => p.id === defaultProjectId)
        if (project) {
          setActiveProjectId(defaultProjectId)
          setCurrentProject({
            id: project.id,
            name: project.name,
            workdir: project.workdir,
            created_at: project.created_at,
            updated_at: '',
          })
        }
      }
      setActiveSessionId(null)

      // Sync active model after onboarding completes
      try {
        const activeModel = await invoke<{ provider_id: string; model_id: string } | null>('model_get_active')
        if (activeModel) {
          setSelectedModel(`${activeModel.provider_id}/${activeModel.model_id}`)
        }
      } catch {
        // Ignore — user can select model manually
      }
    } catch (err) {
      console.error('[onboarding→main] Failed to load projects:', err)
    }
  }

  const handleOnboardingComplete = () => {
    setShowOnboarding(false)
    void loadMainAppData()
  }

  if (showOnboarding) {
    return <OnboardingApp onWindowDrag={startWindowDrag} onComplete={handleOnboardingComplete} />
  }

  return (
    <>
      {showSplash ? (
        <If2AiLoadingScreen projectName="UClaw" stageLabel="Initializing agent workspace" onWindowDrag={startWindowDrag} />
      ) : (
      <div className="relative isolate grid h-screen min-h-0 min-w-0 overflow-hidden bg-[#f6f7f8] text-foreground" style={{ gridTemplateColumns: '76px minmax(0, 1fr)' }}>
      <GlobalNavbar
        activeSection={activeSection}
        onSelectSection={setActiveSection}
        onOpenSettings={() => openSettingsWindow()}
        onStartWindowDrag={startWindowDrag}
        appIconSrc={appIconSrc}
      />

      <main className="relative z-10 flex min-h-0 min-w-0 flex-col overflow-hidden bg-[#f6f7f8]">
        <div aria-hidden="true" className="pointer-events-none absolute inset-0 z-0">
          <div className="absolute inset-0 bg-[#f6f7f8]" />
          <div className="absolute inset-0 bg-[linear-gradient(135deg,rgba(246,247,248,0)_0%,rgba(246,247,248,0.12)_46%,rgba(246,247,248,0.76)_100%)]" />
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_8%,rgba(255,255,255,0.96)_0%,rgba(255,255,255,0.76)_18%,rgba(255,255,255,0)_52%),radial-gradient(circle_at_50%_100%,rgba(242,244,246,0.94)_0%,rgba(242,244,246,0.62)_34%,rgba(242,244,246,0.18)_68%,rgba(242,244,246,0)_100%)]" />
          <div className="absolute inset-0 opacity-[0.61] [background-image:radial-gradient(rgba(169,179,189,0.46)_1px,transparent_1px)] [background-size:16px_16px] [mask-image:linear-gradient(to_bottom,transparent_0%,transparent_16%,black_50%,black_100%)]" />
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_72%_78%,rgba(255,255,255,0.5),transparent_28%),radial-gradient(circle_at_86%_90%,rgba(242,244,246,0.34),transparent_30%)]" />
        </div>

        <div className="relative z-10 flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          {activeSection === 'chat' ? (
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
          ) : activeSection === 'memory' ? (
            <MemoryBrowser />
          ) : (
            <SectionWorkspace section={activeSection} onBackToChat={() => setActiveSection('chat')} />
          )}
        </div>
      </main>

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
    </div>
      )}
    </>
  )
}

export default App
