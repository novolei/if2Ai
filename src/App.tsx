import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  startAgentStream,
  listenToStream,
  listenToPermissionRequests,
  respondPermission,
  listProjects,
  listProjectSessions,
  createPermanentWorktree,
  createProject,
  deleteProject,
  renameProject,
  deleteSession,
  setSessionPinned,
  createSession,
  getSession,
  openProjectInFinder,
  openSettingsWindow,
  invoke,
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
import type { Conversation, Message } from '@/modules/chat/types'
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

const appIconSrc = new URL('../src-tauri/icons/icon-128.png', import.meta.url).href

function App() {
  const appWindow = getCurrentWindow()
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
  const [leftPaneWidth, setLeftPaneWidth] = useState(304)
  const [loading, setLoading] = useState(false)
  const [conversations, setConversations] = useState<Record<string, Conversation>>({})
  const [input, setInput] = useState('')
  const [sessionLoading, setSessionLoading] = useState<Record<string, boolean>>({})
  const [isCreateProjectOpen, setIsCreateProjectOpen] = useState(false)
  const [selectedModel, setSelectedModel] = useState('gpt-5.4-mini')
  const [permissionMode, setPermissionMode] = useState<PermissionMode>(() => {
    if (typeof window === 'undefined') return 'dangerFullAccess'
    const stored = localStorage.getItem('permissionMode')
    if (stored === 'readOnly' || stored === 'workspaceWrite' || stored === 'dangerFullAccess') {
      return stored
    }
    return 'dangerFullAccess'
  })
  const [todos, setTodos] = useState<TodoItem[]>([])
  const extractResumeCursor = (degradedReason?: string): string | undefined => {
    if (!degradedReason) return undefined
    const matched = degradedReason.match(/resume_cursor=([^\s;]+)/)
    return matched?.[1]
  }

  const [streamAbortHandles, setStreamAbortHandles] = useState<Record<string, string>>({})
  const [permissionPrompt, setPermissionPrompt] = useState<PermissionRequestPayload | null>(null)
  const resizeRef = useRef<{
    startX: number
    startWidth: number
  } | null>(null)

  const activeConv = activeSessionId ? conversations[activeSessionId] : null
  const isActiveSessionLoading = activeSessionId ? Boolean(sessionLoading[activeSessionId]) : false
  const activeTitle = activeConv?.title ?? '重构桌面端 UI 为 shadcn 体系'
  const branchLabel = 'feature/consolidate-codebase'
  const minLeftPaneWidth = 280
  const maxLeftPaneWidth = 520
  const activeMessages = useMemo(
    () =>
      activeConv?.messages.map((msg) => ({
        ...msg,
        content: msg.content || ' ',
      })) ?? [],
    [activeConv?.messages]
  )
  const runningSessionIds = Object.entries(sessionLoading)
    .filter(([, running]) => running)
    .map(([sessionId]) => sessionId)

  useEffect(() => {
    loadProjects().then((projectList) => {
      const lastProjectId = localStorage.getItem('lastActiveProjectId')
      const lastSessionId = localStorage.getItem('lastActiveSessionId')
      if (lastProjectId && lastSessionId) {
        const project = projectList.find((item) => item.id === lastProjectId)
        handleSelectSession(lastProjectId, lastSessionId, project)
      }
    })
  }, [])

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
            })
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
          })
        }
      }

      setConversations((prev) => ({
        ...prev,
        [sessionId]: {
          id: sessionId,
          projectId,
          title: fullSession.title || sessionMeta?.title || '新对话',
          messages: convertedMessages,
          updatedAt: new Date(fullSession.updated_at),
        },
      }))
    } catch (err) {
      console.error('Failed to load session:', err)
      setConversations((prev) => ({
        ...prev,
        [sessionId]: {
          id: sessionId,
          projectId,
          title: sessionMeta?.title || '新对话',
          messages: [],
          updatedAt: new Date(),
        },
      }))
    }
  }

  const handleNewChat = async (projectId: string) => {
    try {
      setActiveSection('chat')
      const session = await createSession(projectId, '新对话')
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
          title: '新对话',
          messages: [],
          updatedAt: new Date(),
        },
      }))
    } catch (err) {
      console.error('Failed to create session:', err)
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
      const session = await createSession(newProject.id, '新对话')

      setCurrentProject(newProject)
      setActiveProjectId(newProject.id)
      setActiveSessionId(session.id)
      setConversations((prev) => ({
        ...prev,
        [session.id]: {
          id: session.id,
          projectId: newProject.id,
          title: '新对话',
          messages: [],
          updatedAt: new Date(),
        },
      }))
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

  const sendMessage = async (overrideText?: string) => {
    const messageText = (overrideText ?? input).trim()
    if (!messageText || !activeSessionId) return
    const sessionId = activeSessionId
    if (sessionLoading[sessionId]) return

    const conv = conversations[sessionId]
    if (!conv) return

    const userMsg: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      content: messageText,
      timestamp: new Date(),
    }

    const updatedConv = {
      ...conv,
      title: conv.messages.length === 0 ? messageText.slice(0, 30) : conv.title,
      messages: [...conv.messages, userMsg],
      updatedAt: new Date(),
    }

    setConversations((prev) => ({ ...prev, [sessionId]: updatedConv }))
    if (!overrideText) {
      setInput('')
    }
    setSessionLoading((prev) => ({ ...prev, [sessionId]: true }))

    const startTime = Date.now()
    let accumulatedText = ''
    let accumulatedThinking = ''
    let assistantMsgId: string | null = null
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
              msg.id === finalizedAssistantId ? { ...msg, isStreaming: false } : msg
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

    try {
      createAssistantMessage()
      const streamId = await startAgentStream(sessionId, userMsg.content, permissionMode)
      setStreamAbortHandles((prev) => ({ ...prev, [sessionId]: streamId }))

      const unlisten = await listenToStream(streamId, (payload: StreamTokenPayload) => {
        if (payload.event_type === 'text_delta' && payload.text) {
          const currentAssistantId = ensureAssistantMessage()

          accumulatedText += payload.text
          setConversations((prev) => {
            const currentConv = prev[sessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [sessionId]: {
                ...currentConv,
                messages: currentConv.messages.map((msg) =>
                  msg.id === currentAssistantId ? { ...msg, content: accumulatedText } : msg
                ),
              },
            }
          })
        } else if (payload.event_type === 'thinking_delta' && payload.thinking) {
          const currentAssistantId = ensureAssistantMessage()

          accumulatedThinking += payload.thinking
          setConversations((prev) => {
            const currentConv = prev[sessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [sessionId]: {
                ...currentConv,
                messages: currentConv.messages.map((msg) =>
                  msg.id === currentAssistantId ? { ...msg, thinking: accumulatedThinking } : msg
                ),
              },
            }
          })
        } else if (payload.event_type === 'tool_call_update') {
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
                resumeCursor: extractResumeCursor(payload.degraded_reason) ?? existingMessage?.resumeCursor,
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
            try {
              const result = JSON.parse(payload.tool_result)
              if (result.newTodos) {
                setTodos(result.newTodos)
              }
            } catch {
              // ignore parse error
            }
          }
        } else if (payload.event_type === 'final_text_override' && payload.text) {
          const currentAssistantId = ensureAssistantMessage()
          accumulatedText = payload.text
          setConversations((prev) => {
            const currentConv = prev[activeSessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [activeSessionId]: {
                ...currentConv,
                messages: currentConv.messages.map((msg) =>
                  msg.id === currentAssistantId ? { ...msg, content: accumulatedText } : msg
                ),
              },
            }
          })
        } else if (payload.event_type === 'stream_complete') {
          setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
          setTodos([])
          setStreamAbortHandles((prev) => {
            const { [sessionId]: _removed, ...rest } = prev
            return rest
          })
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
                        resumeCursor: extractResumeCursor(payload.degraded_reason) ?? msg.resumeCursor,
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
          unlisten()
          setSessionLoading((prev) => ({ ...prev, [sessionId]: false }))
          setStreamAbortHandles((prev) => {
            const { [sessionId]: _removed, ...rest } = prev
            return rest
          })
          const errMsg = payload.tool_result || 'Agent 执行失败，请稍后重试。'
          const taskOutcome = payload.task_outcome ?? 'failed'
          const degradedReason = payload.degraded_reason ?? errMsg
          const resumeAvailable = payload.resume_available ?? false
          const resumeCursor = extractResumeCursor(degradedReason)
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
                  ? { ...msg, content: '', isStreaming: false, isError: true, toolArgs: { rawError: errorMessage } }
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
    } finally {
      // Don't set isLoading=false here — stream_complete or stopAgentStream handles it
    }
  }

  const handleResumeFromCursor = async (resumeCursor: string) => {
    // harness symbol marker: resume
    const resumePrompt =
      `[resume_cursor] ${resumeCursor}\n` +
      '请从该游标继续完成上一次任务，仅补全未完成步骤，禁止重复已确认的副作用操作。'
    await sendMessage(resumePrompt)
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

  return (
    <div className="relative isolate grid h-screen min-h-0 min-w-0 overflow-hidden bg-[#f6f7f8] text-foreground" style={{ gridTemplateColumns: '88px minmax(0, 1fr)' }}>
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
              leftPaneWidth={leftPaneWidth}
              onResizeStart={startResize}
              onStartWindowDrag={startWindowDrag}
              runningSessionIds={runningSessionIds}
              status={isActiveSessionLoading ? 'running' : 'idle'}
            />
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
  )
}

export default App
