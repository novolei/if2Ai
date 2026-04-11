import { useState, useRef, useEffect, useCallback } from 'react'
import { cn } from '@/lib/utils'
import {
  startAgentStream,
  listenToStream,
  listProjects,
  listProjectSessions,
  createProject,
  deleteProject,
  renameProject,
  deleteSession,
  createSession,
  getSession,
  openSettingsWindow,
  type Project,
  type ProjectMeta,
  type SessionMeta,
  type StreamTokenPayload,
} from '@/lib/tauri'
import { open } from '@tauri-apps/plugin-dialog'
import { ProjectRail } from '@/components/ProjectRail'
import { WelcomeScreen } from '@/components/WelcomeScreen'
import { SessionStatus as SessionStatusComponent, type SessionStatus } from '@/components/SessionStatus'
import {
  Bot,
  Settings,
  Sparkles,
  ChevronDown,
  ChevronRight,
  ArrowDown,
} from 'lucide-react'
import { TypingIndicator } from '@/components/ui/typing-indicator'
import { StreamingMarkdown } from '@/components/ui/streaming-markdown'
import { InputArea } from '@/components/ui/input-area'
import chatStyles from '@/styles/chat-bubble.module.css'

interface Message {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
  thinking?: string
  thinkingTime?: number  // 思考耗时（毫秒）
  disableAnimation?: boolean  // 禁用打字机效果（用于历史消息）
  isStreaming?: boolean  // 是否正在流式输出
}

interface Conversation {
  id: string
  projectId: string
  title: string
  messages: Message[]
  updatedAt: Date
}

// 折叠的思考内容组件
function ThinkingBlock({ content, thinkingTime }: { content: string; thinkingTime?: number }) {
  const [isExpanded, setIsExpanded] = useState(false)

  const formatTime = (ms: number) => {
    if (ms < 1000) return `${ms}ms`
    return `${(ms / 1000).toFixed(1)}s`
  }

  return (
    <div className={chatStyles.thinkingBlock}>
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={chatStyles.thinkingBlockSummary}
      >
        {isExpanded ? (
          <ChevronDown className="h-3 w-3" />
        ) : (
          <ChevronRight className="h-3 w-3" />
        )}
        <span>思考过程</span>
        {thinkingTime !== undefined && (
          <span style={{ color: 'var(--color-text-tertiary)', opacity: 0.6 }}>({formatTime(thinkingTime)})</span>
        )}
      </button>
      {isExpanded && (
        <div className={chatStyles.thinkingBlockBody}>
          {content}
        </div>
      )}
    </div>
  )
}

// 打字机效果组件
function TypewriterText({ text, disableAnimation = false }: { text: string; disableAnimation?: boolean }) {
  const [displayedText, setDisplayedText] = useState('')
  const [isAnimating, setIsAnimating] = useState(!disableAnimation)
  const textRef = useRef(text)

  useEffect(() => {
    if (disableAnimation) {
      setDisplayedText(text)
      setIsAnimating(false)
      return
    }

    if (text === displayedText) {
      setIsAnimating(false)
      return
    }

    textRef.current = text
    setDisplayedText('')
    setIsAnimating(true)

    let index = 0
    const interval = setInterval(() => {
      if (index < text.length) {
        setDisplayedText(text.slice(0, index + 1))
        index++
      } else {
        clearInterval(interval)
        setIsAnimating(false)
      }
    }, 20) // 每20ms显示一个字符

    return () => clearInterval(interval)
  }, [text, disableAnimation])

  return (
    <span>
      {displayedText}
      {isAnimating && <span className="animate-pulse">▍</span>}
    </span>
  )
}

function App() {
  // Project state
  const [projects, setProjects] = useState<ProjectMeta[]>([])
  const [projectSessions, setProjectSessions] = useState<Record<string, SessionMeta[]>>({})
  const [currentProject, setCurrentProject] = useState<Project | null>(null)
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null)
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  // Session status for agent execution
  const [sessionStatus, setSessionStatus] = useState<SessionStatus>('idle')

  // Conversation state
  const [conversations, setConversations] = useState<Record<string, Conversation>>({})
  const [input, setInput] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [showScrollBtn, setShowScrollBtn] = useState(false)
  const [fontMode, setFontMode] = useState<'serif' | 'sans'>(() => {
    return (localStorage.getItem('fontMode') as 'serif' | 'sans') || 'serif'
  })
  const scrollRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const isAtBottom = useRef(true)

  const activeConv = activeSessionId ? conversations[activeSessionId] : null

  // Load projects on mount
  useEffect(() => {
    loadProjects().then(() => {
      // Restore last active session after projects are loaded
      const lastProjectId = localStorage.getItem('lastActiveProjectId')
      const lastSessionId = localStorage.getItem('lastActiveSessionId')
      if (lastProjectId && lastSessionId) {
        handleSelectSession(lastProjectId, lastSessionId)
      }
    })
  }, [])

  // Save active session to localStorage when it changes
  useEffect(() => {
    if (activeProjectId && activeSessionId) {
      localStorage.setItem('lastActiveProjectId', activeProjectId)
      localStorage.setItem('lastActiveSessionId', activeSessionId)
    }
  }, [activeProjectId, activeSessionId])

  // Handle scroll to detect if user is near bottom
  const handleScroll = useCallback(() => {
    const el = scrollRef.current
    if (el) {
      const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
      isAtBottom.current = distanceFromBottom < 100
      setShowScrollBtn(!isAtBottom.current)
    }
  }, [])

  // Scroll to bottom function with safety margin
  const scrollToBottom = useCallback((smooth = true) => {
    const el = scrollRef.current
    if (el) {
      const targetTop = el.scrollHeight - el.clientHeight + 20 // 20px 安全留白
      el.scrollTo({ top: targetTop, behavior: smooth ? 'smooth' : 'instant' })
      isAtBottom.current = true
    }
  }, [])

  // ResizeObserver：内容高度变化 + 在底部或AI回复中 → 自动滚
  useEffect(() => {
    const content = contentRef.current
    if (!content) return

    const ro = new ResizeObserver(() => {
      if (isAtBottom.current || isLoading) {
        scrollToBottom(true)
      }
    })
    ro.observe(content)
    return () => ro.disconnect()
  }, [isLoading, scrollToBottom])

  // 首次进入聊天页面或有新消息时 → 滚动到底部
  const prevMsgLen = useRef(0)
  useEffect(() => {
    const msgLen = activeConv?.messages.length ?? 0
    if (msgLen > prevMsgLen.current) {
      // 有新消息加入
      if (isLoading || isAtBottom.current) {
        scrollToBottom(true)
      }
    } else if (msgLen > 0 && prevMsgLen.current === 0) {
      // 首次进入聊天页面
      scrollToBottom(false) // 立即滚动，不使用 smooth
    }
    prevMsgLen.current = msgLen
  }, [activeConv?.messages, isLoading, scrollToBottom])

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

    // Initialize conversation for this session if not exists
    if (!conversations[sessionId]) {
      // Find the session title from projectSessions
      const sessionMeta = projectSessions[projectId]?.find(s => s.id === sessionId)

      // Load full session with messages from backend
      try {
        const fullSession = await getSession(sessionId)
        const convertedMessages = fullSession.messages.map((msg) => ({
          id: crypto.randomUUID(),
          role: msg.role as 'user' | 'assistant',
          content: msg.blocks.find(b => b.type === 'text')?.text || '',
          timestamp: new Date(fullSession.updated_at),
          thinking: msg.thinking,
          disableAnimation: true,  // 历史消息不需要打字机效果
        }))
        setConversations(prev => ({
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
        // Fallback to empty conversation
        setConversations(prev => ({
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

  const handleRenameProject = async (projectId: string, newName: string) => {
    try {
      await renameProject(projectId, newName)
      await loadProjects()
      // Update currentProject if it's the one being renamed
      if (currentProject?.id === projectId) {
        setCurrentProject(prev => prev ? { ...prev, name: newName } : null)
      }
    } catch (err) {
      console.error('Failed to rename project:', err)
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
    try {
      // Open directory picker
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择项目目录',
      })

      if (!selected || typeof selected !== 'string') return

      // Extract folder name as project name
      const folderName = selected.split('/').pop() || '新项目'

      // Create project
      const newProject = await createProject(folderName, selected)
      await loadProjects()

      // Create a new session for this project
      const session = await createSession(newProject.id, '新对话')

      // Set active project and session
      setCurrentProject(newProject)
      setActiveProjectId(newProject.id)
      setActiveSessionId(session.id)

      // Create empty conversation for the new session
      setConversations(prev => ({
        ...prev,
        [session.id]: {
          id: session.id,
          projectId: newProject.id,
          title: '新对话',
          messages: [],
          updatedAt: new Date(),
        },
      }))
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
    setSessionStatus('running')

    // 记录请求开始时间
    const startTime = Date.now()

    // 流式内容累加器
    let accumulatedText = ''
    let accumulatedThinking = ''
    let assistantMsgId: string | null = null

    // 创建 assistant 消息（首次收到 token 时调用）
    const createAssistantMessage = () => {
      if (assistantMsgId) return // 已创建
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
      // 调用流式 API
      console.log('[DEBUG] Calling startAgentStream with sessionId:', activeSessionId, 'message:', userMsg.content)
      const streamId = await startAgentStream(activeSessionId, userMsg.content)
      console.log('[DEBUG] startAgentStream returned streamId:', streamId)

      // 监听流式事件
      const unlisten = await listenToStream(streamId, (payload: StreamTokenPayload) => {
        console.log('[DEBUG] Stream event:', payload.event_type, payload.text || payload.thinking || '')

        if (payload.event_type === 'text_delta' && payload.text) {
          // 首次收到 token 时创建消息并关闭 loading
          if (!assistantMsgId) {
            createAssistantMessage()
            setIsLoading(false)
          }
          accumulatedText += payload.text
          // 更新消息内容
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
          // 首次收到 thinking 时创建消息并关闭 loading
          if (!assistantMsgId) {
            createAssistantMessage()
            setIsLoading(false)
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
          // 流式结束
          setSessionStatus('idle')
          if (assistantMsgId) {
            setConversations((prev) => {
              const currentConv = prev[activeSessionId]
              if (!currentConv) return prev
              const thinkingTime = Date.now() - startTime
              return {
                ...prev,
                [activeSessionId]: {
                  ...currentConv,
                  messages: currentConv.messages.map((msg) =>
                    msg.id === assistantMsgId
                      ? { ...msg, isStreaming: false, thinkingTime }
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
      console.error('[DEBUG] startAgentStream error:', err)
      setSessionStatus('error')

      // 提取错误消息
      let errorMessage = 'Agent 执行失败，请稍后重试。'
      if (err && typeof err === 'object') {
        const errorObj = err as { error?: string; message?: string }
        if (errorObj.error) {
          errorMessage = errorObj.error
        } else if (errorObj.message) {
          errorMessage = errorObj.message
        } else if ('toString' in err) {
          errorMessage = String(err)
        }
      } else if (err instanceof Error) {
        errorMessage = err.message
      }

      // 替换错误消息
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

  const handleStartNewChat = (projectId: string) => {
    handleNewChat(projectId)
  }

  return (
    <div className="flex h-screen overflow-hidden" style={{ backgroundColor: 'var(--color-bg-app)', color: 'var(--color-text-primary)' }}>
      {/* Left Sidebar - ProjectRail */}
      <aside className="w-64 flex flex-col shrink-0" style={{ backgroundColor: 'var(--color-bg-secondary)', borderRight: '1px solid var(--color-border-soft)' }}>
        {/* Logo */}
        <div className="flex items-center gap-2.5 px-4 h-14 border-b" style={{ borderColor: 'var(--color-border-soft)' }}>
          <div className="flex h-8 w-8 items-center justify-center rounded-xl" style={{ background: 'linear-gradient(135deg, var(--color-primary) 0%, var(--color-accent-mint) 100%)' }}>
            <Sparkles className="h-4 w-4" style={{ color: 'var(--color-text-inverse)' }} />
          </div>
          <span className="font-semibold text-sm tracking-tight" style={{ color: 'var(--color-text-primary)' }}>If2Ai</span>
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
          onRenameProject={handleRenameProject}
          onDeleteSession={handleDeleteSession}
          loading={loading}
        />

        {/* Settings */}
        <div className="px-3 pb-4 pt-2 border-t" style={{ borderColor: 'var(--color-border-soft)' }}>
          <button
            onClick={() => openSettingsWindow()}
            className="w-full flex items-center gap-2 rounded-lg px-3 py-2 text-sm transition-colors"
            style={{ color: 'var(--color-text-secondary)' }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = 'var(--color-primary-soft)'
              e.currentTarget.style.color = 'var(--color-text-primary)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = 'transparent'
              e.currentTarget.style.color = 'var(--color-text-secondary)'
            }}
          >
            <Settings className="h-4 w-4" />
            设置
          </button>
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 flex flex-col min-w-0 relative">
        {/* Header */}
        <header className="flex items-center justify-between px-6 h-14 shrink-0 border-b" style={{ borderColor: 'var(--color-border-soft)' }}>
          <div className="flex items-center gap-3">
            <Bot className="h-5 w-5" style={{ color: 'var(--color-primary)' }} />
            <h1 className="font-medium text-sm" style={{ color: 'var(--color-text-primary)' }}>
              {activeConv?.title || '对话'}
            </h1>
          </div>
          <SessionStatusComponent status={sessionStatus} />
        </header>

        {/* Messages or WelcomeScreen */}
        {activeSessionId && activeConv ? (
          <>
            {/* Messages */}
            <div
              ref={scrollRef}
              onScroll={handleScroll}
              className="flex-1 overflow-y-auto px-4 pt-4 pb-36"
            >
              <div ref={contentRef} className="max-w-3xl mx-auto space-y-6">
                {activeConv.messages.length === 0 && (
                  <div className="flex flex-col items-center justify-center py-24 gap-4 text-center">
                    <div className="flex h-16 w-16 items-center justify-center rounded-2xl" style={{ backgroundColor: 'var(--color-primary-soft)' }}>
                      <Sparkles className="h-8 w-8" style={{ color: 'var(--color-primary)' }} />
                    </div>
                    <div>
                      <h2 className="text-xl font-semibold" style={{ color: 'var(--color-text-primary)' }}>有什么我可以帮助你的？</h2>
                      <p className="text-sm mt-1" style={{ color: 'var(--color-text-tertiary)' }}>
                        输入你的问题，If2Ai 将为你提供智能回答
                      </p>
                    </div>
                  </div>
                )}

                {activeConv.messages.map((msg) => (
                  <div
                    key={msg.id}
                    className={cn(
                      chatStyles.messageGroup,
                      msg.role === 'user' ? chatStyles.messageGroupUser : chatStyles.messageGroupAssistant
                    )}
                  >
                    {msg.role === 'assistant' && (
                      <div className={chatStyles.avatarRow}>
                        <div className={cn(chatStyles.avatar, 'rounded-full')} style={{ background: 'linear-gradient(135deg, var(--color-primary) 0%, var(--color-accent-mint) 100%)' }}>
                          <Bot className="h-4 w-4" style={{ color: 'var(--color-text-inverse)' }} />
                        </div>
                      </div>
                    )}
                    {msg.role === 'user' && (
                      <div className={chatStyles.avatarRow + ' ' + chatStyles.avatarRowUser}>
                        <div className={cn(chatStyles.avatar, chatStyles.userAvatar)}>
                          U
                        </div>
                      </div>
                    )}
                    <div className={cn(chatStyles.message, msg.role === 'user' ? chatStyles.messageUser : chatStyles.messageAssistant)}>
                      {msg.role === 'assistant' && msg.thinking && (
                        <ThinkingBlock content={msg.thinking} thinkingTime={msg.thinkingTime} />
                      )}
                      <StreamingMarkdown
                        content={msg.content}
                        isStreaming={msg.isStreaming}
                        className={cn(fontMode === 'sans' && 'font-sans')}
                      />
                    </div>
                  </div>
                ))}

                {isLoading && (
                  <div className={chatStyles.messageGroup + ' ' + chatStyles.messageGroupAssistant}>
                    <div className={chatStyles.avatarRow}>
                      <div className={chatStyles.avatar} style={{ background: 'linear-gradient(135deg, var(--color-primary) 0%, var(--color-accent-mint) 100%)' }}>
                        <Bot className="h-4 w-4" style={{ color: 'var(--color-text-inverse)' }} />
                      </div>
                    </div>
                    <div className={chatStyles.typingIndicator}>
                      <span className={chatStyles.typingDot} />
                      <span className={chatStyles.typingDot} />
                      <span className={chatStyles.typingDot} />
                    </div>
                  </div>
                )}
              </div>
            </div>

            {/* Floating scroll to bottom button */}
            {showScrollBtn && activeConv.messages.length > 0 && (
              <button
                onClick={() => scrollToBottom(true)}
                className="fixed w-10 h-10 rounded-full flex items-center justify-center transition-colors z-50"
                style={{ right: '24px', bottom: '96px', background: 'linear-gradient(135deg, var(--color-primary) 0%, var(--color-accent-mint) 100%)', color: 'var(--color-text-inverse)', boxShadow: 'var(--shadow-mint-glow)' }}
                onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.05)'}
                onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}
              >
                <ArrowDown className="h-5 w-5" />
              </button>
            )}

            {/* Input bar */}
            <InputArea
              value={input}
              onChange={setInput}
              onSubmit={sendMessage}
              disabled={isLoading}
              isStreaming={isLoading}
              placeholder="输入消息…"
            />
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
