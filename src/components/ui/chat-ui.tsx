import * as React from "react"
import {
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
  role: "user" | "assistant"
  content: string
  thinking?: string
  thinkingTime?: number
  isStreaming?: boolean
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
  const textareaRef = React.useRef<HTMLTextAreaElement>(null)
  const [selectedModel, setSelectedModel] = React.useState('gpt-5.4-mini')
  const [selectedStrength, setSelectedStrength] = React.useState('mid')
  const [isComposerFocused, setIsComposerFocused] = React.useState(false)

  React.useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
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

  return (
    <div className="flex h-full min-h-0 flex-col bg-[#fbfbfc]">
      <ChatTranscript
        messages={messages}
        isLoading={isLoading}
        sessionTitle={sessionTitle}
        projectLabel={projectLabel}
        bottomRef={bottomRef}
      />
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
  isLoading,
  sessionTitle,
  projectLabel,
  bottomRef,
}: {
  messages: Message[]
  isLoading?: boolean
  sessionTitle: string
  projectLabel: string
  bottomRef: React.RefObject<HTMLDivElement>
}) {
  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto flex w-full max-w-[920px] flex-col gap-6 px-6 py-6">
        {messages.length === 0 ? (
          <EmptyState sessionTitle={sessionTitle} projectLabel={projectLabel} />
        ) : (
          <div className="space-y-5">
            {messages.map((msg, index) => (
              <ChatMessage key={msg.id} message={msg} index={index} total={messages.length} />
            ))}
          </div>
        )}

        <div ref={bottomRef} />
      </div>
    </div>
  )
}, (prev, next) =>
  prev.messages === next.messages &&
  prev.isLoading === next.isLoading &&
  prev.sessionTitle === next.sessionTitle &&
  prev.projectLabel === next.projectLabel
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
    <div className="shrink-0 px-6 pb-4 pt-0">
      <div className="mx-auto flex w-full max-w-[900px] flex-col">
        <div
          className={cn(
            'rounded-[20px] border border-black/5 bg-white px-4 py-3 transition-[box-shadow,border-color,transform] duration-200',
            isComposerFocused
              ? 'border-slate-400/20 shadow-[0_0_0_1px_rgba(71,85,105,0.05),0_0_0_3px_rgba(71,85,105,0.035),0_10px_24px_rgba(15,23,42,0.03)]'
              : 'shadow-[0_8px_20px_rgba(15,23,42,0.025)]'
          )}
        >
          <div className="flex min-h-[128px] flex-col">
            <Textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => onInputChange(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={isLoading}
              rows={1}
              onFocus={() => setIsComposerFocused(true)}
              onBlur={() => setIsComposerFocused(false)}
              className="min-h-[102px] resize-none border-0 bg-transparent px-0 py-1 pl-3 text-[14px] leading-6 shadow-none focus-visible:ring-0"
            />

            <div className="mt-3 flex flex-col gap-2">
              <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
                <div className="flex min-w-0 items-center gap-2 overflow-hidden">
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
                <div className="flex min-w-0 items-center gap-2 overflow-hidden">
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

function ChatMessage({ message, index, total }: { message: Message; index: number; total: number }) {
  const isUser = message.role === 'user'
  const hasThinking = Boolean(message.thinking?.trim())
  const hasContent = Boolean(message.content?.trim())

  return (
    <div className={cn('flex flex-col gap-4', isUser ? 'items-end' : 'items-start')}>
      {isUser ? (
        <div className="max-w-[min(240px,75%)] rounded-3xl bg-[#eef0f2] px-4 py-3 text-[13px] leading-6 text-black/80 shadow-sm">
          <pre className="whitespace-pre-wrap font-sans">{message.content}</pre>
        </div>
      ) : (
        <div className="w-full space-y-4">
          <ChangeSummaryCard index={index} total={total} />

          <div className="border-t border-black/5 pt-4 text-[14px] leading-6 text-black/85">
            {hasThinking && (
              <ThinkingBlock thinking={message.thinking ?? ''} thinkingTime={message.thinkingTime} />
            )}

            {!hasThinking && message.isStreaming ? (
              <div className="mb-3 flex items-start">
                <LoadingIndicator />
              </div>
            ) : null}

            {hasContent ? (
              <MarkdownContent content={message.content} isStreaming={message.isStreaming} />
            ) : !message.isStreaming ? (
              <div className="text-black/35">暂无正文内容</div>
            ) : null}
          </div>
        </div>
      )}
    </div>
  )
}

function ChangeSummaryCard({ index, total }: { index: number; total: number }) {
  const positive = index === 0 ? 82 : 102
  const negative = index === 0 ? 21 : 5

  return (
    <div className="rounded-[22px] border border-black/5 bg-[#f5f6f7] px-4 py-4 shadow-[0_10px_30px_rgba(15,23,42,0.035)]">
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-1">
          <div className="text-[13px] font-medium text-black/80">1 个文件已更改</div>
          <div className="text-[13px] text-black/55">
            src/components/ui/chat-ui.tsx <span className="text-emerald-600">+{positive}</span>{' '}
            <span className="text-red-500">-{negative}</span>
          </div>
        </div>
        <div className="rounded-full bg-white px-3 py-2 text-[12px] font-medium text-black/55 shadow-sm">
          {index + 1}/{total}
        </div>
      </div>
    </div>
  )
}

function MarkdownContent({ content, isStreaming }: { content: string; isStreaming?: boolean }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeHighlight]}
      components={{
        p: ({ children }) => <p className="whitespace-pre-wrap text-[14px] leading-6">{children}</p>,
        h1: ({ children }) => <h1 className="mb-3 text-[20px] font-semibold tracking-tight">{children}</h1>,
        h2: ({ children }) => <h2 className="mb-2.5 text-[18px] font-semibold tracking-tight">{children}</h2>,
        h3: ({ children }) => <h3 className="mb-2 text-[16px] font-semibold tracking-tight">{children}</h3>,
        ul: ({ children }) => <ul className="mb-3 ml-5 list-disc space-y-1 text-[14px] leading-6">{children}</ul>,
        ol: ({ children }) => <ol className="mb-3 ml-5 list-decimal space-y-1 text-[14px] leading-6">{children}</ol>,
        li: ({ children }) => <li className="leading-7">{children}</li>,
        blockquote: ({ children }) => (
          <blockquote className="mb-3 rounded-2xl border border-primary/15 bg-primary/5 px-4 py-3 italic text-[14px] leading-6 text-muted-foreground">
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
          <td className="border-b border-border/50 px-4 py-3 align-top leading-6">{children}</td>
        ),
        hr: () => <hr className="my-5 border-border/70" />,
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

  const copyCode = async () => {
    const text = extractCodeText(children)
    if (!text.trim()) return

    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      setAnimateCopy(true)
      window.setTimeout(() => setCopied(false), 1500)
      window.setTimeout(() => setAnimateCopy(false), 420)
    } catch (err) {
      console.error('Failed to copy code:', err)
    }
  }

  const content = (
    <div className="my-4 overflow-hidden rounded-[24px] border border-black/5 bg-[#f6f8fa] shadow-[0_8px_22px_rgba(15,23,42,0.035)]">
      <div className="flex items-center justify-between border-b border-black/5 px-4 py-3">
        <div className="flex items-center gap-2 text-xs font-medium text-black/42">
          <span className="size-2.5 rounded-full bg-[#ff5f56]" />
          <span className="size-2.5 rounded-full bg-[#ffbd2e]" />
          <span className="size-2.5 rounded-full bg-[#27c93f]" />
          <span className="ml-2 text-[12px]">Code</span>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className={cn(
            'h-8 w-8 rounded-full text-black/42 transition-all hover:bg-black/[0.03] hover:text-black/70',
            animateCopy && 'scale-105 bg-black/[0.03]'
          )}
          onClick={copyCode}
          aria-label={copied ? '已复制' : '复制代码'}
        >
          {copied ? <Check className="size-4" /> : <Copy className="size-4" />}
        </Button>
      </div>
      <pre className="overflow-x-auto px-4 py-4 text-[13px] leading-6">
        {children}
      </pre>
    </div>
  )

  return content
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
            'h-7 justify-between rounded-md border-0 bg-transparent px-1.5 text-[12px] font-medium text-black/72 shadow-none hover:bg-black/[0.03] hover:text-black/88',
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
