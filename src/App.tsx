import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  startAgentStream,
  listenToStream,
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
  const [isLoading, setIsLoading] = useState(false)
  const [isCreateProjectOpen, setIsCreateProjectOpen] = useState(false)
  const [selectedModel, setSelectedModel] = useState('gpt-5.4-mini')
  const [todos, setTodos] = useState<TodoItem[]>([])
  const [streamAbortHandle, setStreamAbortHandle] = useState<string | null>(null)
  const resizeRef = useRef<{
    startX: number
    startWidth: number
  } | null>(null)

  const activeConv = activeSessionId ? conversations[activeSessionId] : null
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
  const runningSessionIds = Object.values(conversations)
    .filter((conv) => conv.messages.some((msg) => msg.isStreaming))
    .map((conv) => conv.id)

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
      const convertedMessages = fullSession.messages.map((msg) => ({
        id: crypto.randomUUID(),
        role: msg.role as 'user' | 'assistant',
        content: msg.blocks.find((block) => block.type === 'text')?.text || '',
        timestamp: new Date(fullSession.updated_at),
        thinking: msg.thinking,
        disableAnimation: true,
      }))

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

  const stopAgentStream = async () => {
    if (streamAbortHandle) {
      try {
        await invoke<string>('stop_agent_stream', { streamId: streamAbortHandle })
      } catch (err) {
        console.error('Failed to stop stream:', err)
      }
      setStreamAbortHandle(null)
      setIsLoading(false)
    }
  }

  const sendMessage = async () => {
    if (!input.trim() || isLoading || !activeSessionId) return

    const conv = conversations[activeSessionId]
    if (!conv) return

    const userMsg: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      content: input.trim(),
      timestamp: new Date(),
    }

    const updatedConv = {
      ...conv,
      title: conv.messages.length === 0 ? input.trim().slice(0, 30) : conv.title,
      messages: [...conv.messages, userMsg],
      updatedAt: new Date(),
    }

    setConversations((prev) => ({ ...prev, [activeSessionId]: updatedConv }))
    setInput('')
    setIsLoading(true)

    const startTime = Date.now()
    let accumulatedText = ''
    let accumulatedThinking = ''
    let assistantMsgId: string | null = null

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
        const currentConv = prev[activeSessionId]
        if (!currentConv) return prev
        return {
          ...prev,
          [activeSessionId]: {
            ...currentConv,
            messages: [...currentConv.messages, assistantMsg],
          },
        }
      })
    }

    try {
      createAssistantMessage()
      const streamId = await startAgentStream(activeSessionId, userMsg.content)
      setStreamAbortHandle(streamId)

      const unlisten = await listenToStream(streamId, (payload: StreamTokenPayload) => {
        if (payload.event_type === 'text_delta' && payload.text) {
          if (!assistantMsgId) {
            createAssistantMessage()
          }

          accumulatedText += payload.text
          setConversations((prev) => {
            const currentConv = prev[activeSessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [activeSessionId]: {
                ...currentConv,
                messages: currentConv.messages.map((msg) =>
                  msg.id === assistantMsgId ? { ...msg, content: accumulatedText } : msg
                ),
              },
            }
          })
        } else if (payload.event_type === 'thinking_delta' && payload.thinking) {
          if (!assistantMsgId) {
            createAssistantMessage()
          }

          accumulatedThinking += payload.thinking
          setConversations((prev) => {
            const currentConv = prev[activeSessionId]
            if (!currentConv) return prev
            return {
              ...prev,
              [activeSessionId]: {
                ...currentConv,
                messages: currentConv.messages.map((msg) =>
                  msg.id === assistantMsgId ? { ...msg, thinking: accumulatedThinking } : msg
                ),
              },
            }
          })
        } else if (payload.event_type === 'tool_call_update') {
          // Handle tool completion/error events
          if (payload.tool_status === 'completed' || payload.tool_status === 'error') {
            const toolMsg: Message = {
              id: `tool-${payload.tool_call_id}-${Date.now()}`,
              role: 'tool',
              content: payload.tool_result || '',
              timestamp: new Date(),
              toolCallId: payload.tool_call_id,
              toolName: payload.tool_name,
              toolDurationMs: payload.tool_duration_ms,
              isError: payload.tool_status === 'error',
            }
            setConversations((prev) => {
              const currentConv = prev[activeSessionId]
              if (!currentConv) return prev
              return {
                ...prev,
                [activeSessionId]: {
                  ...currentConv,
                  messages: [...currentConv.messages, toolMsg],
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
        } else if (payload.event_type === 'stream_complete') {
          setTodos([])
          setStreamAbortHandle(null)
          if (assistantMsgId) {
            setConversations((prev) => {
              const currentConv = prev[activeSessionId]
              if (!currentConv) return prev
              return {
                ...prev,
                [activeSessionId]: {
                  ...currentConv,
                  messages: currentConv.messages.map((msg) =>
                    msg.id === assistantMsgId
                      ? { ...msg, isStreaming: false, thinkingTime: Date.now() - startTime }
                      : msg
                  ),
                },
              }
            })
          }
          unlisten()
        } else if (payload.event_type === 'stream_error') {
          unlisten()
          setIsLoading(false)
          throw new Error('Stream error occurred')
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

      setConversations((prev) => ({
        ...prev,
        [activeSessionId]: {
          ...updatedConv,
          messages: updatedConv.messages.map((msg) =>
            msg.id === assistantMsgId
              ? { ...msg, content: `错误: ${errorMessage}`, isStreaming: false }
              : msg
          ),
        },
      }))
    } finally {
      // Don't set isLoading=false here — stream_complete or stopAgentStream handles it
    }
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
              isLoading={isLoading}
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
              onStop={stopAgentStream}
              selectedModel={selectedModel}
              onModelChange={setSelectedModel}
              todos={todos}
              leftPaneWidth={leftPaneWidth}
              onResizeStart={startResize}
              onStartWindowDrag={startWindowDrag}
              runningSessionIds={runningSessionIds}
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
    </div>
  )
}

export default App
