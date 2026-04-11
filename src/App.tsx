import { useState, useRef, useEffect } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { cn } from '@/lib/utils'
import { runAgentTurn } from '@/lib/tauri'
import {
  Bot,
  SendHorizonal,
  Plus,
  Settings,
  ChevronRight,
  Sparkles,
  MessageSquare,
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
  title: string
  messages: Message[]
  updatedAt: Date
}

function App() {
  const [conversations, setConversations] = useState<Conversation[]>([
    {
      id: '1',
      title: '开始一个新对话',
      messages: [],
      updatedAt: new Date(),
    },
  ])
  const [activeId, setActiveId] = useState('1')
  const [input, setInput] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)

  const activeConv = conversations.find((c) => c.id === activeId)!

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [activeConv?.messages])

  const sendMessage = async () => {
    if (!input.trim() || isLoading) return

    const userMsg: Message = {
      id: crypto.randomUUID(),
      role: 'user',
      content: input.trim(),
      timestamp: new Date(),
    }

    const updatedConv = {
      ...activeConv,
      title: activeConv.messages.length === 0 ? input.trim().slice(0, 30) : activeConv.title,
      messages: [...activeConv.messages, userMsg],
      updatedAt: new Date(),
    }
    setConversations((prev) => prev.map((c) => (c.id === activeId ? updatedConv : c)))
    setInput('')
    setIsLoading(true)

    try {
      // 调用 Tauri 后端
      const response = await runAgentTurn(activeId, userMsg.content)

      const assistantMsg: Message = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: response.message,
        timestamp: new Date(),
      }
      setConversations((prev) =>
        prev.map((c) =>
          c.id === activeId
            ? { ...c, messages: [...updatedConv.messages, assistantMsg], updatedAt: new Date() }
            : c
        )
      )
    } catch (err) {
      // 错误展示（友好错误消息，不暴露内部错误细节）
      const errorMessage = err instanceof Error ? err.message : 'Agent 执行失败，请稍后重试。'
      const assistantMsg: Message = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: `错误: ${errorMessage}`,
        timestamp: new Date(),
      }
      setConversations((prev) =>
        prev.map((c) =>
          c.id === activeId
            ? { ...c, messages: [...updatedConv.messages, assistantMsg], updatedAt: new Date() }
            : c
        )
      )
    } finally {
      setIsLoading(false)
    }
  }

  const newConversation = () => {
    const id = crypto.randomUUID()
    setConversations((prev) => [
      { id, title: '新对话', messages: [], updatedAt: new Date() },
      ...prev,
    ])
    setActiveId(id)
  }

  return (
    <div className="flex h-screen bg-background text-foreground overflow-hidden">
      {/* Sidebar */}
      <aside className="w-64 flex flex-col border-r border-border bg-sidebar-background text-sidebar-foreground shrink-0">
        {/* Logo */}
        <div className="flex items-center gap-2.5 px-4 h-14 border-b border-sidebar-border">
          <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Sparkles className="h-4 w-4" />
          </div>
          <span className="font-semibold text-sm tracking-tight">If2Ai</span>
        </div>

        {/* New Chat */}
        <div className="px-3 py-3">
          <Button
            onClick={newConversation}
            variant="outline"
            className="w-full justify-start gap-2 border-sidebar-border bg-transparent text-sidebar-foreground hover:bg-white/10 hover:text-sidebar-foreground"
          >
            <Plus className="h-4 w-4" />
            新建对话
          </Button>
        </div>

        {/* Conversations */}
        <ScrollArea className="flex-1 px-3">
          <div className="space-y-1 pb-3">
            <p className="px-2 py-1 text-xs font-medium text-sidebar-foreground/50 uppercase tracking-wider">
              最近
            </p>
            {conversations.map((conv) => (
              <button
                key={conv.id}
                onClick={() => setActiveId(conv.id)}
                className={cn(
                  'w-full flex items-center gap-2 rounded-md px-2 py-2 text-sm text-left transition-colors',
                  conv.id === activeId
                    ? 'bg-white/15 text-white'
                    : 'text-sidebar-foreground/70 hover:bg-white/10 hover:text-sidebar-foreground'
                )}
              >
                <MessageSquare className="h-3.5 w-3.5 shrink-0 opacity-60" />
                <span className="truncate">{conv.title}</span>
                {conv.id === activeId && (
                  <ChevronRight className="ml-auto h-3.5 w-3.5 shrink-0 opacity-60" />
                )}
              </button>
            ))}
          </div>
        </ScrollArea>

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
          <h1 className="font-medium text-sm">{activeConv?.title || '对话'}</h1>
        </header>

        {/* Messages */}
        <ScrollArea className="flex-1">
          <div ref={scrollRef} className="max-w-3xl mx-auto px-6 py-6 space-y-6">
            {activeConv?.messages.length === 0 && (
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

            {activeConv?.messages.map((msg) => (
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
      </main>
    </div>
  )
}

export default App
