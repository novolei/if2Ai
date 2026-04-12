import * as React from "react"
import {
  ArrowDown,
  ChevronDown,
  Check,
  Copy,
  Mic,
  Plus,
  Send,
  Sparkles,
  Square,
} from "lucide-react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import "highlight.js/styles/github.css"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Textarea } from "@/components/ui/textarea"

interface Message {
  id: string
  role: "user" | "assistant" | "tool"
  content: string
  timestamp: Date
  thinking?: string
  thinkingTime?: number
  isStreaming?: boolean
  toolCallId?: string
  toolName?: string
  toolArgs?: Record<string, unknown>
  toolDurationMs?: number
}

interface ChatUIProps {
  messages: Message[]
  input: string
  onInputChange: (value: string) => void
  onSubmit: () => void
  isLoading?: boolean
  sessionTitle?: string
  projectLabel?: string
  branchLabel?: string
}

export function ChatUI({
  messages,
  input,
  onInputChange,
  onSubmit,
  isLoading,
  sessionTitle = '重构桌面端 UI 为 shadcn 体系',
  projectLabel = 'if2Ai',
  branchLabel = 'feature/consolidate-codebase',
}: ChatUIProps) {
  const bottomRef = React.useRef<HTMLDivElement>(null)
  const transcriptScrollRef = React.useRef<HTMLDivElement>(null)
  const textareaRef = React.useRef<HTMLTextAreaElement>(null)
  const [selectedModel, setSelectedModel] = React.useState('gpt-5.4-mini')
  const [selectedStrength, setSelectedStrength] = React.useState('mid')
  const [isComposerFocused, setIsComposerFocused] = React.useState(false)
  const [isAtBottom, setIsAtBottom] = React.useState(true)
  const [copiedMessageId, setCopiedMessageId] = React.useState<string | null>(null)
  const [hoveredMessageId, setHoveredMessageId] = React.useState<string | null>(null)

  React.useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
  }, [messages, isLoading])

  React.useEffect(() => {
    const container = transcriptScrollRef.current
    if (!container) return
    const maxScrollTop = container.scrollHeight - container.clientHeight
    setIsAtBottom(maxScrollTop - container.scrollTop < 40)
  }, [messages, isLoading])

  React.useLayoutEffect(() => {
    const textarea = textareaRef.current
    if (!textarea) return

    textarea.style.height = 'auto'
    textarea.style.height = `${Math.min(textarea.scrollHeight, 220)}px`
  }, [input])

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (input.trim() && !isLoading) onSubmit()
    }
  }

  const handleCopyMessage = React.useCallback(async (message: Message) => {
    const text = message.content.trim()
    if (!text) return

    try {
      await navigator.clipboard.writeText(text)
      setCopiedMessageId(message.id)
      window.setTimeout(() => setCopiedMessageId((current) => (current === message.id ? null : current)), 1200)
    } catch (err) {
      console.error('Failed to copy message:', err)
    }
  }, [])

  return (
    <div className="relative flex h-full min-h-0 flex-col bg-transparent">
      <ChatTranscript
        messages={messages}
        sessionTitle={sessionTitle}
        projectLabel={projectLabel}
        bottomRef={bottomRef}
        scrollRef={transcriptScrollRef}
        onScroll={() => {
          const container = transcriptScrollRef.current
          if (!container) return
          const maxScrollTop = container.scrollHeight - container.clientHeight
          setIsAtBottom(maxScrollTop - container.scrollTop < 40)
        }}
        onCopyMessage={handleCopyMessage}
        copiedMessageId={copiedMessageId}
        hoveredMessageId={hoveredMessageId}
        onMessageHoverChange={setHoveredMessageId}
      />
      {!isAtBottom && (
        <div className="pointer-events-none absolute inset-x-0 bottom-[186px] z-20 flex justify-center px-6">
          <button
            type="button"
            onClick={() => {
              transcriptScrollRef.current?.scrollTo({
                top: transcriptScrollRef.current.scrollHeight,
                behavior: 'smooth',
              })
            }}
            className="pointer-events-auto flex h-10 w-10 items-center justify-center rounded-full border border-black/8 bg-white/95 text-black/78 shadow-[0_8px_20px_rgba(15,23,42,0.08)] transition-transform duration-200 hover:-translate-y-0.5 hover:bg-white"
            aria-label="滚动到底部"
          >
            <ArrowDown className="h-[18px] w-[18px]" />
          </button>
        </div>
      )}
      <ComposerDock
        input={input}
        onInputChange={onInputChange}
        onSubmit={onSubmit}
        isLoading={isLoading}
        selectedModel={selectedModel}
        setSelectedModel={setSelectedModel}
        selectedStrength={selectedStrength}
        setSelectedStrength={setSelectedStrength}
        branchLabel={branchLabel}
        isComposerFocused={isComposerFocused}
        setIsComposerFocused={setIsComposerFocused}
        textareaRef={textareaRef}
        handleKeyDown={handleKeyDown}
      />
    </div>
  )
}

const ChatTranscript = React.memo(function ChatTranscript({
  messages,
  sessionTitle,
  projectLabel,
  bottomRef,
  scrollRef,
  onScroll,
  onCopyMessage,
  copiedMessageId,
  hoveredMessageId,
  onMessageHoverChange,
}: {
  messages: Message[]
  sessionTitle: string
  projectLabel: string
  bottomRef: React.RefObject<HTMLDivElement | null>
  scrollRef: React.RefObject<HTMLDivElement | null>
  onScroll: () => void
  onCopyMessage: (message: Message) => void
  copiedMessageId: string | null
  hoveredMessageId: string | null
  onMessageHoverChange: (messageId: string | null) => void
}) {
  return (
    <div ref={scrollRef} onScroll={onScroll} className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto flex w-full max-w-[920px] flex-col gap-6 px-10 py-6">
        {messages.length === 0 ? (
          <EmptyState sessionTitle={sessionTitle} projectLabel={projectLabel} />
        ) : (
          <div className="space-y-5">
            {messages.map((msg) => (
              <ChatMessage
                key={msg.id}
                message={msg}
                onCopyMessage={onCopyMessage}
                copiedMessageId={copiedMessageId}
                hoveredMessageId={hoveredMessageId}
                onMessageHoverChange={onMessageHoverChange}
              />
            ))}
          </div>
        )}

        <div ref={bottomRef} />
      </div>
    </div>
  )
}, (prev, next) =>
  prev.messages === next.messages &&
  prev.sessionTitle === next.sessionTitle &&
  prev.projectLabel === next.projectLabel &&
  prev.copiedMessageId === next.copiedMessageId &&
  prev.hoveredMessageId === next.hoveredMessageId &&
  prev.onCopyMessage === next.onCopyMessage
)

const ComposerDock = React.memo(function ComposerDock({
  input,
  onInputChange,
  onSubmit,
  isLoading,
  selectedModel,
  setSelectedModel,
  selectedStrength,
  setSelectedStrength,
  branchLabel,
  isComposerFocused,
  setIsComposerFocused,
  textareaRef,
  handleKeyDown,
}: {
  input: string
  onInputChange: (value: string) => void
  onSubmit: () => void
  isLoading?: boolean
  selectedModel: string
  setSelectedModel: React.Dispatch<React.SetStateAction<string>>
  selectedStrength: string
  setSelectedStrength: React.Dispatch<React.SetStateAction<string>>
  branchLabel: string
  isComposerFocused: boolean
  setIsComposerFocused: React.Dispatch<React.SetStateAction<boolean>>
  textareaRef: React.RefObject<HTMLTextAreaElement | null>
  handleKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void
}) {
  return (
    <div className="shrink-0 px-10 pb-2.5 pt-0">
      <div className="mx-auto flex w-full max-w-[900px] flex-col">
        <div
          className={cn(
            'rounded-[20px] border border-black/5 bg-white/42 px-4 py-2 backdrop-blur-xl transition-[box-shadow,border-color,transform,background-color,opacity] duration-300 ease-out',
            isComposerFocused
              ? 'translate-y-[-2px] scale-[1.005] border-slate-300/35 bg-white/56 shadow-[0_0_0_1px_rgba(148,163,184,0.14),0_0_0_6px_rgba(148,163,184,0.055),0_10px_24px_rgba(15,23,42,0.03),0_0_24px_rgba(255,255,255,0.42)]'
              : 'translate-y-0 scale-100 shadow-[0_4px_10px_rgba(15,23,42,0.012)]'
          )}
        >
          <div className="flex min-h-[92px] flex-col">
            <Textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => onInputChange(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={isLoading}
              rows={1}
              onFocus={() => setIsComposerFocused(true)}
              onBlur={() => setIsComposerFocused(false)}
              className="min-h-[70px] resize-none border-0 bg-transparent px-0 py-0.5 pl-3 text-[14px] leading-6 shadow-none focus-visible:ring-0"
            />

            <div className="mt-1.5 flex flex-col gap-1.5">
              <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
                <div className="flex min-w-0 items-center gap-2 overflow-visible">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7 shrink-0 rounded-md bg-transparent p-0 text-black/42 shadow-none hover:bg-black/[0.03] hover:text-black/70"
                    aria-label="添加附件"
                  >
                    <Plus className="h-4 w-4" />
                  </Button>

                  <CompactMenu
                    label={modelLabelFor(selectedModel)}
                    className="min-w-[118px]"
                    contentClassName="w-[148px]"
                    triggerIcon={<ChevronDown className="h-3 w-3 opacity-55" />}
                  >
                    {modelItems.map((item) => (
                      <MenuItemButton
                        key={item.value}
                        onClick={() => setSelectedModel(item.value)}
                        active={selectedModel === item.value}
                      >
                        {item.label}
                      </MenuItemButton>
                    ))}
                  </CompactMenu>

                  <CompactMenu
                    label={strengthLabelFor(selectedStrength)}
                    className="min-w-[60px]"
                    contentClassName="w-[92px]"
                    triggerIcon={<ChevronDown className="h-3 w-3 opacity-55" />}
                  >
                    {strengthItems.map((item) => (
                      <MenuItemButton
                        key={item.value}
                        onClick={() => setSelectedStrength(item.value)}
                        active={selectedStrength === item.value}
                      >
                        {item.label}
                      </MenuItemButton>
                    ))}
                  </CompactMenu>
                </div>

                <div className="flex shrink-0 items-center gap-1.5">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7 rounded-full text-black/42 hover:bg-black/[0.03] hover:text-black/70"
                  >
                    <Mic className="h-4 w-4" />
                  </Button>

                  <Button
                    onClick={() => {
                      if (!isLoading) onSubmit()
                    }}
                    disabled={!isLoading && !input.trim()}
                    size="icon"
                    className={cn('h-9 w-9 rounded-full shadow-none transition-colors', 'bg-black text-white hover:bg-black/90 disabled:bg-black/40')}
                    title={isLoading ? '停止生成' : '发送消息'}
                  >
                    {isLoading ? <Square className="h-4 w-4 fill-current" /> : <Send className="h-4 w-4" />}
                  </Button>
                </div>
              </div>

              <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
                <div className="flex min-w-0 items-center gap-2 overflow-visible">
                  <CompactMenu
                    label="本地"
                    className="min-w-[66px]"
                    contentClassName="w-[92px]"
                    triggerIcon={<ChevronDown className="h-3 w-3 opacity-55" />}
                  >
                    <MenuItemButton active>本地</MenuItemButton>
                    <MenuItemButton>云端</MenuItemButton>
                  </CompactMenu>

                  <CompactMenu
                    label="完全访问权限"
                    className="min-w-[96px]"
                    contentClassName="w-[120px]"
                    triggerIcon={<ChevronDown className="h-3 w-3 opacity-55" />}
                  >
                    <MenuItemButton active>完全访问权限</MenuItemButton>
                    <MenuItemButton>受限访问</MenuItemButton>
                    <MenuItemButton>只读</MenuItemButton>
                  </CompactMenu>

                  <CompactMenu
                    label={branchLabel}
                    className="min-w-[136px] max-w-[242px]"
                    contentClassName="w-[176px]"
                    triggerIcon={<ChevronDown className="h-3 w-3 opacity-55" />}
                  >
                    <MenuItemButton active>{branchLabel}</MenuItemButton>
                    <MenuItemButton>切换分支</MenuItemButton>
                    <MenuItemButton>复制分支名</MenuItemButton>
                  </CompactMenu>
                </div>

                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 shrink-0 rounded-full text-black/42 hover:bg-black/[0.03] hover:text-black/70"
                >
                  <Sparkles className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
})

function ToolCallMessage({
  message,
}: {
  message: Message
}) {
  const isError = message.toolName?.startsWith('error') ?? false
  const duration = message.toolDurationMs
    ? formatDuration(message.toolDurationMs)
    : null
  const toolName = message.toolName ?? 'unknown_tool'

  return (
    <div className="flex items-stretch gap-2">
      <div className={cn(
        'flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] font-medium',
        isError
          ? 'bg-red-500/10 text-red-500'
          : 'bg-emerald-500/10 text-emerald-500'
      )}>
        {isError ? '✗' : '✓'}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-[11px] font-mono text-black/50">
          <span className="font-medium text-black/65">{toolName}</span>
          {duration && (
            <span className="text-black/30">{duration}</span>
          )}
        </div>
        {message.content && (
          <pre className="mt-1 max-h-32 overflow-x-auto rounded-md border border-black/5 bg-white/40 px-3 py-1.5 text-[11px] font-mono leading-5 text-black/60">
            {truncateText(message.content, 600)}
          </pre>
        )}
      </div>
    </div>
  )
}

function ChatMessage({
  message,
  onCopyMessage,
  copiedMessageId,
  hoveredMessageId,
  onMessageHoverChange,
}: {
  message: Message
  onCopyMessage: (message: Message) => void
  copiedMessageId: string | null
  hoveredMessageId: string | null
  onMessageHoverChange: (messageId: string | null) => void
}) {
  const isUser = message.role === 'user'
  const isTool = message.role === 'tool'
  const hasThinking = Boolean(message.thinking?.trim())
  const hasContent = Boolean(message.content?.trim())
  const shortTime = formatShortTime(message.timestamp)
  const isCopied = copiedMessageId === message.id
  const isHovered = hoveredMessageId === message.id
  const showCopyButton = isHovered || isCopied

  if (isTool) {
    return <ToolCallMessage message={message} />
  }

  return (
    <div
      className={cn('flex flex-col gap-4', isUser ? 'items-end' : 'items-start')}
      onPointerEnter={() => onMessageHoverChange(message.id)}
      onPointerLeave={() => onMessageHoverChange(null)}
    >
      {isUser ? (
        <div className="flex max-w-[min(240px,75%)] flex-col items-end gap-1">
          <div className="rounded-[16px] bg-[#eef0f2] px-4 py-3 text-[13px] leading-6 text-black/80 shadow-[0_1px_2px_rgba(15,23,42,0.04)]">
            <pre className="whitespace-pre-wrap font-sans">{message.content}</pre>
          </div>
          <div className="flex items-center gap-1.5 pr-1">
            <MessageCopyButton
              side="left"
              copied={isCopied}
              visible={showCopyButton}
              onClick={() => onCopyMessage(message)}
            />
            <div className="text-[11px] leading-none text-black/32">{shortTime}</div>
          </div>
        </div>
      ) : (
        <div className="w-full space-y-4">
          <div className="pt-0 text-[14px] leading-6 text-black/85">
            {hasThinking && (
              <ThinkingBlock thinking={message.thinking ?? ''} thinkingTime={message.thinkingTime} />
            )}

            {!hasThinking && message.isStreaming ? (
              <div className="mb-3 flex items-start">
                <LoadingIndicator />
              </div>
            ) : null}

            {hasContent ? (
              <>
                <MarkdownContent content={message.content} />
                <div className="mt-2 flex items-center gap-1.5 pl-1">
                  <div className="text-[11px] leading-none text-black/28">{shortTime}</div>
                  <MessageCopyButton
                    side="right"
                    copied={isCopied}
                    visible={showCopyButton}
                    onClick={() => onCopyMessage(message)}
                  />
                </div>
              </>
            ) : !message.isStreaming ? (
              <div className="text-black/35">暂无正文内容</div>
            ) : null}
          </div>
        </div>
      )}
    </div>
  )
}

function MarkdownContent({ content }: { content: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeHighlight]}
      components={{
        p: ({ children }) => <p className="whitespace-pre-wrap text-[14px] leading-7">{children}</p>,
        h1: ({ children }) => <h1 className="mb-3 text-[20px] font-semibold tracking-tight">{children}</h1>,
        h2: ({ children }) => <h2 className="mb-2.5 text-[18px] font-semibold tracking-tight">{children}</h2>,
        h3: ({ children }) => <h3 className="mb-2 text-[16px] font-semibold tracking-tight">{children}</h3>,
        ul: ({ children }) => <ul className="mb-4 ml-5 list-disc space-y-1.5 text-[14px] leading-7">{children}</ul>,
        ol: ({ children }) => <ol className="mb-4 ml-5 list-decimal space-y-1.5 text-[14px] leading-7">{children}</ol>,
        li: ({ children }) => <li className="leading-7">{children}</li>,
        blockquote: ({ children }) => (
          <blockquote className="mb-4 rounded-2xl border border-primary/15 bg-primary/5 px-4 py-3 italic text-[14px] leading-7 text-muted-foreground">
            <div className="mb-2 flex items-center gap-2 text-[11px] font-medium uppercase tracking-[0.18em] text-primary/70">
              <span className="h-1.5 w-1.5 rounded-full bg-primary/50" />
              引用
            </div>
            <div className="leading-7">{children}</div>
          </blockquote>
        ),
        table: ({ children }) => (
          <div className="mb-4 overflow-hidden rounded-3xl border border-border/70 bg-background shadow-sm">
            <table className="w-full border-collapse text-[13px]">{children}</table>
          </div>
        ),
        thead: ({ children }) => <thead className="bg-muted/40">{children}</thead>,
        th: ({ children }) => (
          <th className="border-b border-border/70 px-4 py-3 text-left font-semibold">{children}</th>
        ),
        td: ({ children }) => (
          <td className="border-b border-border/50 px-4 py-3 align-top leading-7">{children}</td>
        ),
        hr: () => null,
        a: ({ children, href }) => (
          <a
            href={href}
            target="_blank"
            rel="noreferrer"
            className="text-primary underline decoration-primary/40 underline-offset-4 hover:decoration-primary"
          >
            {children}
          </a>
        ),
        code: ({ className, children, ...props }) => {
          const isBlock = className?.includes('language-')
          if (isBlock) {
            return (
              <code className={cn('block overflow-x-auto rounded-none bg-transparent p-0 font-mono text-[12px] leading-5 text-foreground', className)} {...props}>
                {children}
              </code>
            )
          }

          return (
            <code
            className="rounded-md border border-border/60 bg-muted/70 px-1.5 py-0.5 font-mono text-[12px] text-foreground"
              {...props}
            >
              {children}
            </code>
          )
        },
        pre: ({ children }) => <CodeBlock>{children}</CodeBlock>,
      }}
    >
      {content}
    </ReactMarkdown>
  )
}

function CodeBlock({ children }: { children: React.ReactNode }) {
  const [copied, setCopied] = React.useState(false)
  const [animateCopy, setAnimateCopy] = React.useState(false)
  const language = extractCodeLanguage(children)

  const copyCode = async () => {
    const text = extractCodeText(children)
    if (!text.trim()) return

    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      setAnimateCopy(true)
      window.setTimeout(() => setCopied(false), 900)
      window.setTimeout(() => setAnimateCopy(false), 260)
    } catch (err) {
      console.error('Failed to copy code:', err)
    }
  }

  const content = (
    <div className="my-4 overflow-hidden rounded-[22px] border border-black/8 bg-white text-[12px] shadow-[0_1px_0_rgba(15,23,42,0.02)] ring-0">
      <div className="flex h-8 items-center justify-between bg-[#f3f4f6] px-4">
        <div className="font-mono text-[10px] font-light tracking-tight text-black/38">
          {language ?? 'Code'}
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className={cn(
            'h-6 w-6 rounded-full text-black/38 transition-all duration-150 hover:bg-black/[0.02] hover:text-black/62',
            animateCopy && 'scale-110 bg-emerald-500/10 text-emerald-600 shadow-[0_0_0_1px_rgba(16,185,129,0.12)]'
          )}
          onClick={copyCode}
          aria-label={copied ? '已复制' : '复制代码'}
        >
          {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
        </Button>
      </div>
      <div className="px-5 pb-4 pt-3">
        <pre className="m-0 overflow-x-auto whitespace-pre-wrap bg-transparent !bg-transparent pt-0 font-mono text-[11px] font-light leading-7 text-foreground [&_code]:bg-transparent [&_code]:p-0 [&_code]:text-inherit">
          {children}
        </pre>
      </div>
    </div>
  )

  return content
}

function MessageCopyButton({
  side,
  copied,
  visible,
  onClick,
}: {
  side: 'left' | 'right'
  copied: boolean
  visible: boolean
  onClick: () => void
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      onClick={onClick}
      aria-label={copied ? '已复制' : '复制消息'}
      className={cn(
        'h-4 w-4 shrink-0 rounded-full border-0 bg-transparent p-0 text-black/14 shadow-none transition-[opacity,color,background-color,box-shadow] duration-150 hover:bg-black/[0.016] hover:text-black/45',
        visible ? 'opacity-100' : 'opacity-0',
        copied && 'opacity-100 bg-emerald-500/8 text-emerald-600 shadow-[0_0_0_1px_rgba(16,185,129,0.1)]',
        side === 'left' ? 'order-first' : 'order-last'
      )}
    >
      {copied ? <Check className="size-2.5" /> : <Copy className="size-2.5" />}
    </Button>
  )
}

function extractCodeLanguage(node: React.ReactNode): string | null {
  if (!React.isValidElement(node)) return null

  const child = (node.props as { children?: React.ReactNode }).children
  if (!React.isValidElement(child)) return null

  const className = (child.props as { className?: string }).className
  const match = className?.match(/language-([a-z0-9_-]+)/i)
  return match?.[1] ?? null
}

function extractCodeText(node: React.ReactNode): string {
  if (Array.isArray(node)) return node.map(extractCodeText).join('')
  if (typeof node === 'string') return node
  if (typeof node === 'number' || typeof node === 'boolean' || node == null) return ''
  if (React.isValidElement(node)) {
    return extractCodeText((node.props as { children?: React.ReactNode }).children)
  }
  return ''
}

function formatShortTime(date: Date) {
  if (!(date instanceof Date) || Number.isNaN(date.getTime())) return ''

  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(date)
}

function EmptyState({ sessionTitle, projectLabel }: { sessionTitle: string; projectLabel: string }) {
  return (
    <div className="flex min-h-[55vh] flex-col items-center justify-center gap-6 rounded-[2rem] border border-dashed border-black/5 bg-white/60 px-8 py-16 text-center">
      <div className="flex h-18 w-18 items-center justify-center rounded-[1.75rem] bg-black/5 text-black/60 shadow-inner">
        <Sparkles className="h-8 w-8" />
      </div>
        <div className="max-w-xl space-y-3">
        <h2 className="text-[22px] font-semibold tracking-tight">{sessionTitle}</h2>
        <p className="text-[13px] leading-6 text-black/50">
          当前工作区是 <span className="text-black/80">{projectLabel}</span>。输入任务后，右侧会按照 Codex 的节奏显示变更摘要、思考过程和正文。
        </p>
      </div>
    </div>
  )
}

function LoadingIndicator() {
  return (
    <div className="flex items-center gap-1 px-0.5 py-0.5">
      <span className="h-1.5 w-1.5 rounded-full bg-black/32 animate-bounce [animation-delay:0ms]" />
      <span className="h-1.5 w-1.5 rounded-full bg-black/32 animate-bounce [animation-delay:150ms]" />
      <span className="h-1.5 w-1.5 rounded-full bg-black/32 animate-bounce [animation-delay:300ms]" />
    </div>
  )
}

function ThinkingBlock({ thinking, thinkingTime }: { thinking: string; thinkingTime?: number }) {
  const [open, setOpen] = React.useState(false)
  const durationLabel = thinkingTime ? formatDuration(thinkingTime) : '—'

  return (
    <div className="mb-4">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="flex items-center gap-2 px-0 text-[12px] font-light italic tracking-tight text-black/32 transition-colors hover:text-black/48"
      >
        <span className="h-1.5 w-1.5 rounded-full bg-black/28" />
        <span>思考完成</span>
        <span className="text-black/26 not-italic"> {durationLabel}</span>
        <ChevronDown className={cn('ml-0.5 h-3 w-3 transition-transform', open && 'rotate-180')} />
      </button>

      <div
        className={cn(
          'overflow-hidden transition-[max-height,opacity] duration-200',
          open ? 'max-h-[360px] opacity-100' : 'max-h-0 opacity-0'
        )}
      >
        <div className="mt-2 flex items-stretch gap-3 pl-1">
          <div className="w-[3px] rounded-full bg-black/16" />
          <div className="whitespace-pre-wrap px-1 text-[12px] font-light italic leading-6 text-black/48">
            {thinking}
          </div>
        </div>
      </div>
    </div>
  )
}

const modelItems = [
  { value: 'gpt-5.4-mini', label: 'GPT-5.4-Mini' },
  { value: 'gpt-5.4', label: 'GPT-5.4' },
  { value: 'gpt-4.1', label: 'GPT-4.1' },
]

const strengthItems = [
  { value: 'low', label: '低' },
  { value: 'mid', label: '中' },
  { value: 'high', label: '高' },
]

function modelLabelFor(value: string) {
  return modelItems.find((item) => item.value === value)?.label ?? 'GPT-5.4-Mini'
}

function strengthLabelFor(value: string) {
  return strengthItems.find((item) => item.value === value)?.label ?? '中'
}

function CompactMenu({
  label,
  className,
  contentClassName,
  triggerIcon,
  children,
}: {
  label: string
  className?: string
  contentClassName?: string
  triggerIcon?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          className={cn(
            'h-7 justify-between rounded-md border-0 bg-transparent px-1.5 text-[12px] font-medium text-black/72 shadow-none outline-none ring-0 ring-offset-0 hover:bg-black/[0.03] hover:text-black/88 focus:outline-none focus-visible:outline-none focus-visible:ring-0 focus-visible:ring-offset-0 data-[state=open]:bg-black/[0.03] data-[state=open]:text-black/88',
            className
          )}
        >
          <span className="truncate">{label}</span>
          <span className="ml-1 shrink-0 text-black/45">{triggerIcon}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent sideOffset={6} align="start" className={contentClassName ?? 'w-[176px]'}>
        {children}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function MenuItemButton({
  children,
  active = false,
  onClick,
}: {
  children: React.ReactNode
  active?: boolean
  onClick?: () => void
}) {
  return (
    <DropdownMenuItem
      className={cn(
        'h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium',
        active ? 'bg-black/[0.045] text-black/90' : 'text-black/82'
      )}
      onSelect={() => onClick?.()}
    >
      {children}
    </DropdownMenuItem>
  )
}

function formatDuration(value: number) {
  if (value < 1000) return `${value}ms`
  return `${(value / 1000).toFixed(1)}s`
}

function truncateText(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text
  return text.slice(0, maxLen) + '…'
}
