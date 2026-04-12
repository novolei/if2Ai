import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  ChevronDown,
  Code2,
  MessageSquare,
  Plus,
  Settings,
  SlidersHorizontal,
  Sparkles,
  SquareTerminal,
  Play,
  MoreHorizontal,
} from 'lucide-react'
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
  type Project,
  type ProjectMeta,
  type SessionMeta,
  type StreamTokenPayload,
} from '@/lib/tauri'
import { Button } from '@/components/ui/button'
import { ProjectRail } from '@/components/ProjectRail'
import { ChatUI } from '@/components/ui/chat-ui'
import { CreateProjectDialog } from '@/components/CreateProjectDialog'
import { ErrorBoundary } from '@/components/ui/error-boundary'

const appIconSrc = new URL('../src-tauri/icons/icon-128.png', import.meta.url).href

interface Message {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
  thinking?: string
  thinkingTime?: number
  disableAnimation?: boolean
  isStreaming?: boolean
}

interface Conversation {
  id: string
  projectId: string
  title: string
  messages: Message[]
  updatedAt: Date
}

function App() {
  const appWindow = getCurrentWindow()
  const [projects, setProjects] = useState<ProjectMeta[]>([])
  const [projectSessions, setProjectSessions] = useState<Record<string, SessionMeta[]>>({})
  const [currentProject, setCurrentProject] = useState<Project | null>(null)
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null)
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null)
  const [leftPaneWidth, setLeftPaneWidth] = useState(376)
  const [loading, setLoading] = useState(false)
  const [conversations, setConversations] = useState<Record<string, Conversation>>({})
  const [input, setInput] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [isCreateProjectOpen, setIsCreateProjectOpen] = useState(false)
  const resizeRef = useRef<{
    startX: number
    startWidth: number
  } | null>(null)

  const activeConv = activeSessionId ? conversations[activeSessionId] : null
  const activeTitle = activeConv?.title ?? '重构桌面端 UI 为 shadcn 体系'
  const modelLabel = 'GPT-5.4-Mini'
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
        } else if (payload.event_type === 'stream_complete') {
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
      setIsLoading(false)
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
    <div
      className="isolate grid h-screen min-h-0 overflow-hidden bg-[#f6f7f8] text-foreground"
      style={{ gridTemplateColumns: `${leftPaneWidth}px minmax(0, 1fr)` }}
    >
      <ErrorBoundary
        fallback={
          <aside
            className="relative z-20 flex h-full min-h-0 shrink-0 flex-col overflow-hidden rounded-tr-[28px] rounded-br-[28px] border-r border-black/5 bg-[#eef0f1]"
            style={{ width: leftPaneWidth }}
          >
            <SidebarTop onCreateProject={() => setIsCreateProjectOpen(true)} />
            <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-[13px] text-black/35">
              左侧栏加载异常
            </div>
            <div className="border-t border-black/5 px-3.5 py-2.5">
              <Button
                variant="ghost"
                className="h-8 w-full justify-start gap-3 rounded-2xl px-3 text-left text-sm font-medium bg-transparent hover:bg-transparent"
                onClick={() => openSettingsWindow()}
              >
                <Settings className="h-4 w-4" />
                设置
              </Button>
            </div>
          </aside>
        }
      >
        <aside className="relative z-20 flex h-full min-h-0 min-w-0 flex-col overflow-hidden rounded-tr-[28px] rounded-br-[28px] border-r border-black/5 bg-[#eef0f1]">
          <SidebarTop onCreateProject={() => setIsCreateProjectOpen(true)} onStartWindowDrag={startWindowDrag} />
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden select-none">
            <ErrorBoundary
              fallback={
                <div className="flex h-full min-h-0 flex-1 items-center justify-center px-4 text-[13px] text-black/35">
                  左侧栏加载异常
                </div>
              }
            >
              <ProjectRail
                projects={projects}
                projectSessions={projectSessions}
                activeProjectId={activeProjectId}
                activeSessionId={activeSessionId}
                onSelectProject={handleSelectProject}
                onSelectSession={handleSelectSession}
                onNewChat={handleNewChat}
                onDeleteProject={handleDeleteProject}
                onRenameProject={handleRenameProject}
                onDeleteSession={handleDeleteSession}
                onTogglePinSession={handleTogglePinSession}
                onOpenInFinder={openProjectInFinder}
                onCreatePermanentWorktree={createPermanentWorktree}
                runningSessionIds={runningSessionIds}
                loading={loading}
              />
            </ErrorBoundary>
          </div>

          <div className="border-t border-black/5 px-3.5 py-2.5">
            <Button
              variant="ghost"
              className="h-8 w-full justify-start gap-3 rounded-2xl px-3 text-left text-sm font-medium bg-transparent hover:bg-transparent"
              onClick={() => openSettingsWindow()}
            >
              <Settings className="h-4 w-4" />
              设置
            </Button>
          </div>

          <div
            role="separator"
            aria-orientation="vertical"
            onPointerDown={startResize}
            className="absolute right-0 top-0 z-30 h-full w-4 cursor-col-resize touch-none select-none bg-transparent"
            style={{ touchAction: 'none' }}
          />
        </aside>
      </ErrorBoundary>

      <main className="relative z-10 flex min-h-0 min-w-0 flex-col overflow-hidden bg-[#fbfbfc]">
        <ErrorBoundary
          fallback={
            <div className="flex h-full min-h-0 items-center justify-center px-6">
              <div className="max-w-md rounded-3xl border border-black/5 bg-white px-6 py-5 text-center shadow-sm">
                <div className="text-[14px] font-semibold tracking-tight">主内容加载异常</div>
                <div className="mt-2 text-[12px] leading-5 text-black/45">
                  主工作区发生了运行时错误，但左侧栏仍然保持可用。
                </div>
              </div>
            </div>
          }
        >
          <div className="flex min-h-0 flex-1 flex-col">
            <header
              className="window-drag flex h-14 shrink-0 items-center justify-between border-b border-black/5 px-4 lg:px-5 select-none"
              onMouseDown={startWindowDrag}
            >
              <div className="flex min-w-0 items-center gap-3">
                <div className="hidden h-9 w-9 items-center justify-center rounded-xl bg-black/5 text-black/70 lg:flex">
                  <MessageSquare className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <h1 className="truncate text-[14px] font-semibold tracking-tight">{activeTitle}</h1>
                    <span className="text-[12px] text-muted-foreground">if2Ai</span>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="window-no-drag h-7 w-7 rounded-full text-muted-foreground"
                      data-window-no-drag="true"
                    >
                      <MoreHorizontal className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground"
                  data-window-no-drag="true"
                >
                  <Play className="h-4 w-4" />
                </Button>
                <Button
                  variant="outline"
                  className="window-no-drag h-9 rounded-full border-black/10 bg-white px-3 text-[13px] shadow-none"
                  data-window-no-drag="true"
                >
                  <Code2 className="mr-2 h-4 w-4 text-blue-500" />
                  {modelLabel}
                  <ChevronDown className="ml-2 h-3.5 w-3.5 opacity-70" />
                </Button>
                <Button
                  variant="outline"
                  className="window-no-drag h-9 rounded-full border-black/10 bg-white px-3 text-[13px] shadow-none"
                  data-window-no-drag="true"
                >
                  <SlidersHorizontal className="mr-2 h-4 w-4" />
                  提交
                  <ChevronDown className="ml-2 h-3.5 w-3.5 opacity-70" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground"
                  data-window-no-drag="true"
                >
                  <SquareTerminal className="h-4 w-4" />
                </Button>
                <div className="h-6 w-px bg-black/10" />
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground"
                  data-window-no-drag="true"
                >
                  <Plus className="h-4 w-4" />
                </Button>
                <div className="ml-1 flex items-center gap-2 text-[15px] font-medium">
                  <span className="text-emerald-600">+4,523</span>
                  <span className="text-red-600">-3,223</span>
                </div>
              </div>
            </header>

            <div className="min-h-0 flex-1 overflow-hidden">
              {activeSessionId && activeConv ? (
                <ChatUI
                  sessionTitle={activeTitle}
                  projectLabel={currentProject?.name ?? 'if2Ai'}
                  branchLabel={branchLabel}
                  messages={activeMessages}
                  input={input}
                  onInputChange={setInput}
                  onSubmit={sendMessage}
                  isLoading={isLoading}
                />
              ) : (
                <div className="flex min-h-0 h-full flex-col">
                  <div className="mx-auto flex w-full max-w-[920px] flex-1 items-center justify-center px-6 py-8">
                    <div className="max-w-xl rounded-[28px] border border-black/5 bg-white px-8 py-10 text-center shadow-[0_20px_60px_rgba(0,0,0,0.04)]">
                      <div className="mx-auto mb-5 flex h-14 w-14 items-center justify-center rounded-2xl bg-black/5">
                        <Sparkles className="h-7 w-7 text-black/60" />
                      </div>
                      <div className="space-y-3">
                        <h2 className="text-[22px] font-semibold tracking-tight">选择一个项目，开始新的线程</h2>
                        <p className="text-[13px] leading-6 text-muted-foreground">
                          左侧已经整理好项目与会话，右侧会像 Codex 一样展示变更摘要、思考过程和最终正文。
                        </p>
                      </div>
                    </div>
                  </div>
                  <ChatUI
                    sessionTitle={activeTitle}
                    projectLabel={currentProject?.name ?? 'if2Ai'}
                    branchLabel={branchLabel}
                    messages={[]}
                    input={input}
                    onInputChange={setInput}
                    onSubmit={sendMessage}
                    isLoading={isLoading}
                  />
                </div>
              )}
            </div>
          </div>
        </ErrorBoundary>
      </main>

      <CreateProjectDialog
        isOpen={isCreateProjectOpen}
        onClose={() => setIsCreateProjectOpen(false)}
        onSubmit={handleCreateProject}
      />
    </div>
  )
}

function SidebarTop({
  onCreateProject,
  onStartWindowDrag,
}: {
  onCreateProject: () => void
  onStartWindowDrag: (event: ReactMouseEvent<HTMLElement>) => void
}) {
  return (
    <div
      className="window-drag flex h-[72px] items-center justify-between border-b border-black/5 px-4 select-none"
      onMouseDown={onStartWindowDrag}
    >
      <div className="flex min-w-0 items-center gap-3">
        <img
          src={appIconSrc}
          alt="If2Ai"
          className="size-10 shrink-0 rounded-2xl border border-black/5 bg-black object-cover shadow-sm"
        />
        <div className="truncate text-[15px] font-semibold tracking-tight text-black/88">If2Ai</div>
      </div>

      <Button
        onClick={onCreateProject}
        className="window-no-drag h-9 rounded-full bg-blue-500 px-4 text-[13px] font-semibold text-white shadow-none hover:bg-blue-500/90"
        data-window-no-drag="true"
      >
        更新
      </Button>
    </div>
  )
}

export default App
