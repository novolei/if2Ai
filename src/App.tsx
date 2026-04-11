import { useState, useRef, useEffect } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { cn } from '@/lib/utils'
import {
  runAgentTurn,
  listProjects,
  listProjectSessions,
  createProject,
  deleteProject,
  deleteSession,
  createSession,
  type Project,
  type ProjectMeta,
  type SessionMeta,
} from '@/lib/tauri'
import { ProjectRail } from '@/components/ProjectRail'
import { WelcomeScreen } from '@/components/WelcomeScreen'
import {
  Bot,
  SendHorizonal,
  Settings,
  Sparkles,
  Loader2,
} from 'lucide-react'

interface Message {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
}

interface Conversation {
  id: string
  projectId: string
  title: string
  messages: Message[]
  updatedAt: Date
}

function App() {
  // Project state
  const [projects, setProjects] = useState<ProjectMeta[]>([])
  const [projectSessions, setProjectSessions] = useState<Record<string, SessionMeta[]>>({})
  const [currentProject, setCurrentProject] = useState<Project | null>(null)
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null)
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  // Conversation state
  const [conversations, setConversations] = useState<Record<string, Conversation>>({})
  const [input, setInput] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)

  const activeConv = activeSessionId ? conversations[activeSessionId] : null

  // Load projects on mount
  useEffect(() => {
    loadProjects()
  }, [])

  // Scroll to bottom when messages change
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [activeConv?.messages])

  const loadProjects = async () => {
    try {
      setLoading(true)
      const projectList = await listProjects()
      setProjects(projectList)

      // Load sessions for each project
      const sessionsMap: Record<string, SessionMeta[]> = {}
      for (const project of projectList) {
        const sessions = await listProjectSessions(project.id)
        sessionsMap[project.id] = sessions
      }
      setProjectSessions(sessionsMap)
    } catch (err) {
      console.error('Failed to load projects:', err)
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
        updated_at: '', // Not available in ProjectMeta
      })
    }
    // Clear active session when switching projects
    setActiveSessionId(null)
  }

  const handleSelectSession = async (projectId: string, sessionId: string) => {
    setActiveProjectId(projectId)
    setActiveSessionId(sessionId)
    // Note: In a full implementation, we'd load the session messages here
  }

  const handleNewChat = async (projectId: string) => {
    try {
      const session = await createSession(projectId, '新对话')
      // Refresh sessions for this project
      const sessions = await listProjectSessions(projectId)
      setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }))

      // Set active session
      setActiveProjectId(projectId)
      setActiveSessionId(session.id)

      // Create empty conversation for the new session
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

  const handleDeleteSession = async (projectId: string, sessionId: string) => {
    try {
      await deleteSession(sessionId)
      // Refresh sessions for this project
      const sessions = await listProjectSessions(projectId)
      setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }))

      // Remove conversation
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

  const handleCreateProject = async () => {
    // For now, create a project with the current directory as workdir
    // In a full implementation, this would open a directory picker
    const name = prompt('输入项目名称:')
    if (!name) return

    const workdir = prompt('输入工作目录路径:', '.')
    if (!workdir) return

    try {
      await createProject(name, workdir)
      await loadProjects()
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

    try {
      // Call Tauri backend
      const response = await runAgentTurn(activeSessionId, userMsg.content)

      const assistantMsg: Message = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: response.message,
        timestamp: new Date(),
      }
      setConversations((prev) => ({
        ...prev,
        [activeSessionId]: {
          ...updatedConv,
          messages: [...updatedConv.messages, assistantMsg],
          updatedAt: new Date(),
        },
      }))
    } catch (err) {
      // Error display (friendly error message)
      const errorMessage = err instanceof Error ? err.message : 'Agent 执行失败，请稍后重试。'
      const assistantMsg: Message = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: `错误: ${errorMessage}`,
        timestamp: new Date(),
      }
      setConversations((prev) => ({
        ...prev,
        [activeSessionId]: {
          ...updatedConv,
          messages: [...updatedConv.messages, assistantMsg],
          updatedAt: new Date(),
        },
      }))
    } finally {
      setIsLoading(false)
    }
  }

  const handleStartNewChat = (projectId: string) => {
    handleNewChat(projectId)
  }

  return (
    <div className="flex h-screen bg-background text-foreground overflow-hidden">
      {/* Left Sidebar - ProjectRail */}
      <aside className="w-64 flex flex-col border-r border-border shrink-0">
        {/* Logo */}
        <div className="flex items-center gap-2.5 px-4 h-14 border-b border-sidebar-border">
          <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Sparkles className="h-4 w-4" />
          </div>
          <span className="font-semibold text-sm tracking-tight">If2Ai</span>
        </div>

        {/* Project navigation */}
        <ProjectRail
          projects={projects}
          projectSessions={projectSessions}
          activeProjectId={activeProjectId}
          activeSessionId={activeSessionId}
          onSelectProject={handleSelectProject}
          onSelectSession={handleSelectSession}
          onNewChat={handleNewChat}
          onDeleteProject={handleDeleteProject}
          onDeleteSession={handleDeleteSession}
          loading={loading}
        />

        {/* Settings */}
        <div className="px-3 pb-4 pt-2 border-t border-sidebar-border">
          <button className="w-full flex items-center gap-2 rounded-md px-2 py-2 text-sm text-sidebar-foreground/60 hover:bg-white/10 hover:text-sidebar-foreground transition-colors">
            <Settings className="h-4 w-4" />
            设置
          </button>
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 flex flex-col min-w-0">
        {/* Header */}
        <header className="flex items-center gap-3 px-6 h-14 border-b border-border shrink-0">
          <Bot className="h-5 w-5 text-primary" />
          <h1 className="font-medium text-sm">
            {activeConv?.title || '对话'}
          </h1>
        </header>

        {/* Messages or WelcomeScreen */}
        {activeSessionId && activeConv ? (
          <>
            {/* Messages */}
            <ScrollArea className="flex-1">
              <div ref={scrollRef} className="max-w-3xl mx-auto px-6 py-6 space-y-6">
                {activeConv.messages.length === 0 && (
                  <div className="flex flex-col items-center justify-center py-24 gap-4 text-center">
                    <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10">
                      <Sparkles className="h-8 w-8 text-primary" />
                    </div>
                    <div>
                      <h2 className="text-xl font-semibold">有什么我可以帮助你的？</h2>
                      <p className="text-muted-foreground text-sm mt-1">
                        输入你的问题，If2Ai 将为你提供智能回答
                      </p>
                    </div>
                  </div>
                )}

                {activeConv.messages.map((msg) => (
                  <div
                    key={msg.id}
                    className={cn('flex gap-3', msg.role === 'user' && 'justify-end')}
                  >
                    {msg.role === 'assistant' && (
                      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground mt-0.5">
                        <Bot className="h-4 w-4" />
                      </div>
                    )}
                    <div
                      className={cn(
                        'max-w-[80%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed',
                        msg.role === 'user'
                          ? 'bg-primary text-primary-foreground rounded-br-sm'
                          : 'bg-muted rounded-bl-sm'
                      )}
                    >
                      {msg.content}
                    </div>
                  </div>
                ))}

                {isLoading && (
                  <div className="flex gap-3">
                    <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground mt-0.5">
                      <Bot className="h-4 w-4" />
                    </div>
                    <div className="bg-muted rounded-2xl rounded-bl-sm px-4 py-3">
                      <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                    </div>
                  </div>
                )}
              </div>
            </ScrollArea>

            {/* Input bar */}
            <div className="border-t border-border px-6 py-4 shrink-0 bg-background/80 backdrop-blur">
              <form
                onSubmit={(e) => {
                  e.preventDefault()
                  sendMessage()
                }}
                className="max-w-3xl mx-auto flex gap-2"
              >
                <Input
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  placeholder="输入消息…"
                  disabled={isLoading}
                  className="flex-1 h-10 rounded-xl bg-muted border-0 focus-visible:ring-1 focus-visible:ring-primary/50"
                />
                <Button
                  type="submit"
                  size="icon"
                  disabled={!input.trim() || isLoading}
                  className="h-10 w-10 rounded-xl shrink-0"
                >
                  <SendHorizonal className="h-4 w-4" />
                </Button>
              </form>
              <p className="text-center text-xs text-muted-foreground mt-2">
                If2Ai 可能会犯错，请对重要信息进行核实。
              </p>
            </div>
          </>
        ) : (
          /* WelcomeScreen when no session is active */
          <WelcomeScreen
            currentProject={currentProject}
            projects={projects}
            onSelectProject={handleSelectProject}
            onCreateProject={handleCreateProject}
            onStartNewChat={handleStartNewChat}
            loading={loading}
          />
        )}
      </main>
    </div>
  )
}

export default App
