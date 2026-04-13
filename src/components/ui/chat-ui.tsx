import * as React from "react"
import {
  ArrowDown,
  ChevronDown,
  Check,
  Copy,
  MoreHorizontal,
  Mic,
  Plus,
  Send,
  Sparkles,
  Square,
  AlertTriangle,
  RotateCcw,
  TerminalSquare,
} from "lucide-react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import "highlight.js/styles/github.css"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import type { PermissionMode } from "@/lib/tauri"
import { TodoPanel, type TodoItem } from "@/components/ui/TodoPanel"
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible"
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
  streamId?: string
  toolCallId?: string
  toolName?: string
  toolArgs?: Record<string, unknown>
  toolDurationMs?: number
  isError?: boolean
  toolStatus?: 'queued' | 'running' | 'completed' | 'error'
  effectiveWorkdir?: string
  policyDecision?: 'allow' | 'deny' | 'prompt'
  evidenceId?: string
  requestId?: string
  taskOutcome?: 'completed' | 'partial_success' | 'failed'
  degradedReason?: string
  resumeAvailable?: boolean
  resumeCursor?: string
}

interface ChatUIProps {
  messages: Message[]
  input: string
  onInputChange: (value: string) => void
  onSubmit: () => void
  onResumeFromCursor?: (resumeCursor: string) => void
  onStop?: () => void
  isLoading?: boolean
  sessionTitle?: string
  projectLabel?: string
  defaultWorkdir?: string
  branchLabel?: string
  selectedModel?: string
  onModelChange?: React.Dispatch<React.SetStateAction<string>>
  permissionMode?: PermissionMode
  onPermissionModeChange?: React.Dispatch<React.SetStateAction<PermissionMode>>
  todos?: TodoItem[]
}

export function ChatUI({
  messages,
  input,
  onInputChange,
  onSubmit,
  onResumeFromCursor,
  onStop,
  isLoading,
  sessionTitle = '重构桌面端 UI 为 shadcn 体系',
  projectLabel = 'if2Ai',
  defaultWorkdir,
  branchLabel = 'feature/consolidate-codebase',
  selectedModel: selectedModelProp = 'gpt-5.4-mini',
  onModelChange: onModelChangeProp,
  permissionMode: permissionModeProp = 'dangerFullAccess',
  onPermissionModeChange: onPermissionModeChangeProp,
  todos = [],
}: ChatUIProps) {
  const bottomRef = React.useRef<HTMLDivElement>(null)
  const transcriptScrollRef = React.useRef<HTMLDivElement>(null)
  const textareaRef = React.useRef<HTMLTextAreaElement>(null)
  const composerRef = React.useRef<HTMLDivElement>(null)
  const todoPanelRef = React.useRef<HTMLDivElement>(null)
  const isAtBottomRef = React.useRef(true)
  const lastBottomOccupancyRef = React.useRef(0)
  const slashTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)
  const [selectedModel, setSelectedModel] = React.useState(selectedModelProp)
  const [selectedPermissionMode, setSelectedPermissionMode] = React.useState<PermissionMode>(permissionModeProp)
  const [selectedStrength, setSelectedStrength] = React.useState('mid')
  const [isComposerFocused, setIsComposerFocused] = React.useState(false)
  const [isAtBottom, setIsAtBottom] = React.useState(true)
  const [copiedMessageId, setCopiedMessageId] = React.useState<string | null>(null)
  const [hoveredMessageId, setHoveredMessageId] = React.useState<string | null>(null)
  const [isTodoCollapsed, setIsTodoCollapsed] = React.useState(false)
  const [composerHeight, setComposerHeight] = React.useState(196)
  const [todoPanelHeight, setTodoPanelHeight] = React.useState(0)
  const [slashOverlay, setSlashOverlay] = React.useState<{
    visible: boolean
    selectedIndex: number
    suggestions: string[]
    rawInput: string
  } | null>(null)

  // Use prop-provided model if provided, otherwise fall back to local state
  const modelValue = onModelChangeProp !== undefined ? selectedModelProp : selectedModel
  const handleModelChange: React.Dispatch<React.SetStateAction<string>> = onModelChangeProp ?? setSelectedModel
  const permissionModeValue = onPermissionModeChangeProp !== undefined ? permissionModeProp : selectedPermissionMode
  const handlePermissionModeChange: React.Dispatch<React.SetStateAction<PermissionMode>> =
    onPermissionModeChangeProp ?? setSelectedPermissionMode

  // Cleanup timer on unmount
  React.useEffect(() => {
    return () => {
      if (slashTimerRef.current) {
        clearTimeout(slashTimerRef.current)
      }
    }
  }, [])

  React.useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
  }, [messages, isLoading])

  React.useEffect(() => {
    if (!isAtBottomRef.current) return
    bottomRef.current?.scrollIntoView({ behavior: 'auto', block: 'end' })
  }, [composerHeight, todoPanelHeight, isTodoCollapsed, todos.length])

  React.useEffect(() => {
    const container = transcriptScrollRef.current
    if (!container) return
    const maxScrollTop = container.scrollHeight - container.clientHeight
    const nextIsAtBottom = maxScrollTop - container.scrollTop < 40
    isAtBottomRef.current = nextIsAtBottom
    setIsAtBottom(nextIsAtBottom)
  }, [messages, isLoading])

  React.useLayoutEffect(() => {
    const textarea = textareaRef.current
    if (!textarea) return

    textarea.style.height = 'auto'
    textarea.style.height = `${Math.min(textarea.scrollHeight, 220)}px`
  }, [input])

  React.useLayoutEffect(() => {
    const observeSize = (
      element: HTMLElement | null,
      onChange: (height: number) => void
    ) => {
      if (!element) return () => {}

      const update = () => onChange(element.getBoundingClientRect().height)
      update()

      const observer = new ResizeObserver(() => update())
      observer.observe(element)
      return () => observer.disconnect()
    }

    const cleanupComposer = observeSize(composerRef.current, setComposerHeight)
    const cleanupTodo = observeSize(todoPanelRef.current, setTodoPanelHeight)

    return () => {
      cleanupComposer()
      cleanupTodo()
    }
  }, [isTodoCollapsed, todos.length, input])

  const hasTodos = todos.length > 0
  const transcriptBottomPadding = 20
  const scrollToBottomButtonOffset = composerHeight - 10

  React.useLayoutEffect(() => {
    const container = transcriptScrollRef.current
    if (!container) return

    const nextOccupancy = composerHeight + (hasTodos ? todoPanelHeight : 0)
    const delta = nextOccupancy - lastBottomOccupancyRef.current
    lastBottomOccupancyRef.current = nextOccupancy

    if (delta === 0) return

    const distanceToBottom = container.scrollHeight - container.clientHeight - container.scrollTop
    const shouldKeepBottomAnchor = isAtBottomRef.current || distanceToBottom < 180
    if (!shouldKeepBottomAnchor) return

    container.scrollTop += delta
  }, [composerHeight, todoPanelHeight, hasTodos])

  const handleKeyDown = async (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Handle slash command overlay navigation
    if (slashOverlay?.visible) {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSlashOverlay((prev) =>
          prev ? { ...prev, selectedIndex: (prev.selectedIndex + 1) % prev.suggestions.length } : null
        )
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSlashOverlay((prev) =>
          prev
            ? { ...prev, selectedIndex: (prev.selectedIndex - 1 + prev.suggestions.length) % prev.suggestions.length }
            : null
        )
        return
      }
      if (e.key === 'Tab' || e.key === 'Enter') {
        e.preventDefault()
        const selected = slashOverlay.suggestions[slashOverlay.selectedIndex]
        onInputChange(selected)
        setSlashOverlay(null)
        return
      }
      if (e.key === 'Escape') {
        setSlashOverlay(null)
        return
      }
    }

    // Enter to submit
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
        bottomPadding={transcriptBottomPadding}
        sessionTitle={sessionTitle}
        projectLabel={projectLabel}
        defaultWorkdir={defaultWorkdir}
        bottomRef={bottomRef}
        scrollRef={transcriptScrollRef}
        onScroll={() => {
          const container = transcriptScrollRef.current
          if (!container) return
          if (container.scrollLeft !== 0) {
            container.scrollLeft = 0
          }
          const maxScrollTop = container.scrollHeight - container.clientHeight
          const nextIsAtBottom = maxScrollTop - container.scrollTop < 40
          isAtBottomRef.current = nextIsAtBottom
          setIsAtBottom(nextIsAtBottom)
        }}
        onCopyMessage={handleCopyMessage}
        onResumeFromCursor={onResumeFromCursor}
        copiedMessageId={copiedMessageId}
        hoveredMessageId={hoveredMessageId}
        onMessageHoverChange={setHoveredMessageId}
      />
      {!isAtBottom && (
        <div
          className="pointer-events-none absolute inset-x-0 z-20 flex justify-center px-6"
          style={{ bottom: `${scrollToBottomButtonOffset}px` }}
        >
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
      {todos.length > 0 && (
        <div className="relative z-0 shrink-0 px-10">
          <div className="mx-auto w-full max-w-[700px]">
            <TodoPanel
              ref={todoPanelRef}
              todos={todos}
              collapsed={isTodoCollapsed}
              onToggleCollapsed={() => setIsTodoCollapsed((value) => !value)}
              className="w-full translate-y-[8px]"
            />
          </div>
        </div>
      )}
      <ComposerDock
        containerRef={composerRef}
        input={input}
        onInputChange={onInputChange}
        onSubmit={onSubmit}
        onStop={onStop}
        isLoading={isLoading}
        selectedModel={modelValue}
        setSelectedModel={handleModelChange}
        permissionMode={permissionModeValue}
        setPermissionMode={handlePermissionModeChange}
        selectedStrength={selectedStrength}
        setSelectedStrength={setSelectedStrength}
        branchLabel={branchLabel}
        isComposerFocused={isComposerFocused}
        setIsComposerFocused={setIsComposerFocused}
        textareaRef={textareaRef}
        handleKeyDown={handleKeyDown}
        setSlashOverlay={setSlashOverlay}
        slashTimerRef={slashTimerRef}
      />
      {slashOverlay?.visible && (
        <SlashCommandSuggestions
          suggestions={slashOverlay.suggestions}
          selectedIndex={slashOverlay.selectedIndex}
          onSelect={(selected) => {
            onInputChange(selected)
            setSlashOverlay(null)
          }}
        />
      )}
    </div>
  )
}

const ChatTranscript = React.memo(function ChatTranscript({
  messages,
  bottomPadding,
  sessionTitle,
  projectLabel,
  defaultWorkdir,
  bottomRef,
  scrollRef,
  onScroll,
  onCopyMessage,
  onResumeFromCursor,
  copiedMessageId,
  hoveredMessageId,
  onMessageHoverChange,
}: {
  messages: Message[]
  bottomPadding: number
  sessionTitle: string
  projectLabel: string
  defaultWorkdir?: string
  bottomRef: React.RefObject<HTMLDivElement | null>
  scrollRef: React.RefObject<HTMLDivElement | null>
  onScroll: () => void
  onCopyMessage: (message: Message) => void
  onResumeFromCursor?: (resumeCursor: string) => void
  copiedMessageId: string | null
  hoveredMessageId: string | null
  onMessageHoverChange: (messageId: string | null) => void
}) {
  const primaryThinkingMessageIds = React.useMemo(() => {
    const primaryIds = new Set<string>()
    let seenThinkingThisTurn = false

    for (const item of messages) {
      if (item.role === 'user') {
        seenThinkingThisTurn = false
        continue
      }

      if (item.role === 'assistant' && item.thinking?.trim()) {
        if (!seenThinkingThisTurn) {
          primaryIds.add(item.id)
          seenThinkingThisTurn = true
        }
      }
    }

    return primaryIds
  }, [messages])

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-x-none"
      style={{ overscrollBehaviorX: 'none' }}
    >
      <div
        className="mx-auto flex w-full max-w-[920px] flex-col gap-4 px-10 pt-6"
        style={{ paddingBottom: `${bottomPadding}px` }}
      >
        {messages.length === 0 ? (
          <EmptyState sessionTitle={sessionTitle} projectLabel={projectLabel} />
        ) : (
          <div className="space-y-3">
            {messages.map((msg) => (
              <ChatMessage
                key={msg.id}
                message={msg}
                onCopyMessage={onCopyMessage}
                onResumeFromCursor={onResumeFromCursor}
                copiedMessageId={copiedMessageId}
                hoveredMessageId={hoveredMessageId}
                onMessageHoverChange={onMessageHoverChange}
                defaultWorkdir={defaultWorkdir}
                isPrimaryThinkingMessage={primaryThinkingMessageIds.has(msg.id)}
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
  prev.bottomPadding === next.bottomPadding &&
  prev.sessionTitle === next.sessionTitle &&
  prev.projectLabel === next.projectLabel &&
  prev.copiedMessageId === next.copiedMessageId &&
  prev.hoveredMessageId === next.hoveredMessageId &&
  prev.onCopyMessage === next.onCopyMessage &&
  prev.onResumeFromCursor === next.onResumeFromCursor
)

const ComposerDock = React.memo(function ComposerDock({
  input,
  onInputChange,
  onSubmit,
  onStop,
  isLoading,
  containerRef,
  selectedModel,
  setSelectedModel,
  permissionMode,
  setPermissionMode,
  selectedStrength,
  setSelectedStrength,
  branchLabel,
  isComposerFocused,
  setIsComposerFocused,
  textareaRef,
  handleKeyDown,
  setSlashOverlay,
  slashTimerRef,
}: {
  input: string
  onInputChange: (value: string) => void
  onSubmit: () => void
  onStop?: () => void
  isLoading?: boolean
  containerRef: React.RefObject<HTMLDivElement | null>
  selectedModel: string
  setSelectedModel: React.Dispatch<React.SetStateAction<string>>
  permissionMode: PermissionMode
  setPermissionMode: React.Dispatch<React.SetStateAction<PermissionMode>>
  selectedStrength: string
  setSelectedStrength: React.Dispatch<React.SetStateAction<string>>
  branchLabel: string
  isComposerFocused: boolean
  setIsComposerFocused: React.Dispatch<React.SetStateAction<boolean>>
  textareaRef: React.RefObject<HTMLTextAreaElement | null>
  handleKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void
  setSlashOverlay: React.Dispatch<
    React.SetStateAction<{
      visible: boolean
      selectedIndex: number
      suggestions: string[]
      rawInput: string
    } | null>
  >
  slashTimerRef: React.RefObject<ReturnType<typeof setTimeout> | null>
}) {
  const handleInputWithSlashDetect = (value: string) => {
    if (slashTimerRef.current) {
      clearTimeout(slashTimerRef.current)
      slashTimerRef.current = null
    }

    onInputChange(value)

    if (value.startsWith('/')) {
      const cmdPart = value.split(/\s+/)[0]
      slashTimerRef.current = setTimeout(async () => {
        try {
          const { suggestSlashCommands } = await import('@/lib/tauri')
          const suggestions = await suggestSlashCommands(cmdPart, 8)
          if (suggestions.length > 0) {
            setSlashOverlay({
              visible: true,
              selectedIndex: 0,
              suggestions,
              rawInput: cmdPart,
            })
          } else {
            setSlashOverlay(null)
          }
        } catch {
          setSlashOverlay(null)
        }
      }, 50)
    } else {
      setSlashOverlay(null)
    }
  }
  return (
    <div ref={containerRef} className="relative z-10 shrink-0 px-10 pb-2.5 pt-0">
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
              onChange={(e) => handleInputWithSlashDetect(e.target.value)}
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
                      if (isLoading && onStop) {
                        onStop()
                      } else if (!isLoading) {
                        onSubmit()
                      }
                    }}
                    disabled={!isLoading && !input.trim()}
                    size="icon"
                    className={cn('h-9 w-9 rounded-full shadow-none transition-colors', isLoading ? 'bg-red-500 text-white hover:bg-red-600' : 'bg-black text-white hover:bg-black/90 disabled:bg-black/40')}
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
                    label={permissionModeLabelFor(permissionMode)}
                    className="min-w-[96px]"
                    contentClassName="w-[120px]"
                    triggerIcon={<ChevronDown className="h-3 w-3 opacity-55" />}
                  >
                    {permissionModeItems.map((item) => (
                      <MenuItemButton
                        key={item.value}
                        active={permissionMode === item.value}
                        onClick={() => setPermissionMode(item.value)}
                      >
                        {item.label}
                      </MenuItemButton>
                    ))}
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
  defaultWorkdir,
}: {
  message: Message
  defaultWorkdir?: string
}) {
  const [expanded, setExpanded] = React.useState(false)
  const [isHovered, setIsHovered] = React.useState(false)
  const status = normalizeToolStatus(message)
  const display = buildToolCallDisplay(message, defaultWorkdir)
  const isRunning = status === 'running' || status === 'queued'
  const title = status === 'error' ? `执行失败：${display.title}` : display.title
  const hasDetails = display.details.length > 0

  return (
    <Collapsible open={expanded} onOpenChange={setExpanded}>
      <div
        className={cn(
          'group relative my-0.5 pl-4',
          status === 'error' && 'text-rose-600'
        )}
        onPointerEnter={() => setIsHovered(true)}
        onPointerLeave={() => setIsHovered(false)}
      >
        <div className={cn('absolute bottom-0 left-[5px] top-0 w-px bg-black/14', status === 'error' && 'bg-rose-200/80')} />

        <div className="flex items-start gap-1.5">
          <div className={cn('mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-black/30', status === 'error' && 'bg-rose-400')} />

          <button
            type="button"
            onClick={() => setExpanded((value) => !value)}
            className={cn(
              'group flex min-w-0 flex-1 items-center gap-1.5 rounded-none px-0 py-[2px] text-left transition-colors duration-150',
              status === 'error' ? 'hover:text-rose-600/90' : 'hover:text-black/70'
            )}
            aria-label={expanded ? '折叠工具调用' : '展开工具调用'}
          >
            <div className="relative flex size-4 shrink-0 items-center justify-center text-black/38">
              <TerminalSquare
                className={cn(
                  'absolute h-3.5 w-3.5 transition-all duration-150',
                  expanded ? 'opacity-0 scale-90' : 'opacity-100 scale-100',
                  isHovered && 'opacity-0 scale-90'
                )}
              />
              <ChevronDown
                className={cn(
                  'absolute h-3.5 w-3.5 transition-all duration-150',
                  expanded
                    ? 'opacity-100 rotate-180 scale-100'
                    : isHovered
                      ? 'opacity-100 scale-100'
                      : 'opacity-0 scale-90'
                )}
              />
            </div>

            <div className="min-w-0 flex-1">
              <div className={cn('truncate text-[13px] leading-5 tracking-[-0.01em]', status === 'error' ? 'text-rose-600/86' : 'text-black/50')}>
                {title}
              </div>
            </div>

            {isRunning ? (
              <div className="ml-1 shrink-0 opacity-70">
                <LoadingIndicator />
              </div>
            ) : null}
          </button>

          <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="size-[18px] shrink-0 rounded-full border-0 bg-transparent text-black/14 opacity-0 shadow-none transition-opacity duration-150 group-hover:opacity-100 hover:bg-transparent hover:text-black/38"
              aria-label="工具调用菜单"
              onClick={(event) => event.stopPropagation()}
            >
                <MoreHorizontal className="size-[12px]" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" sideOffset={6} className="w-[172px]">
              <DropdownMenuItem
                onSelect={() => {
                  void copyTextToClipboard(display.copyText || display.title)
                }}
              >
                复制摘要
              </DropdownMenuItem>
              {display.resultCopyText ? (
                <DropdownMenuItem
                  onSelect={() => {
                    void copyTextToClipboard(display.resultCopyText as string)
                  }}
                >
                  复制结果
                </DropdownMenuItem>
              ) : null}
              {display.diagnosticCopyText ? (
                <DropdownMenuItem
                  onSelect={() => {
                    void copyTextToClipboard(display.diagnosticCopyText as string)
                  }}
                >
                  复制诊断串
                </DropdownMenuItem>
              ) : null}
              <DropdownMenuItem
                onSelect={() => {
                  setExpanded((value) => !value)
                }}
              >
                {expanded ? '收起详情' : '展开详情'}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        <CollapsibleContent className="overflow-hidden">
          <div className="ml-[10px] border-l-[1.5px] border-black/10 pl-3 pt-1">
            {hasDetails ? (
              <div className="flex flex-col gap-0.5">
                {display.details.map((line) => (
                  <div
                    key={line}
                    className="flex min-w-0 items-start gap-1.5 text-[11.5px] leading-5 text-black/36"
                  >
                    <span className="mt-[7px] h-1 w-1 shrink-0 rounded-full bg-black/18" />
                    <span className="min-w-0 truncate font-mono">{line}</span>
                  </div>
                ))}
              </div>
            ) : status === 'error' ? (
              <div className="text-[11.5px] leading-5 text-rose-500/72">执行失败</div>
            ) : null}
          </div>
        </CollapsibleContent>
      </div>
    </Collapsible>
  )
}

function SlashCommandSuggestions({
  suggestions,
  selectedIndex,
  onSelect,
}: {
  suggestions: string[]
  selectedIndex: number
  onSelect: (value: string) => void
}) {
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-[178px] z-30 flex justify-center px-10">
      <div className="pointer-events-auto w-full max-w-[740px] rounded-xl border border-black/8 bg-white/95 shadow-[0_8px_24px_rgba(15,23,42,0.12)] backdrop-blur-sm">
        {suggestions.map((cmd, i) => (
          <button
            key={cmd}
            type="button"
            className={cn(
              'flex w-full items-center rounded-lg px-4 py-2.5 text-left text-[13px] font-mono transition-colors',
              i === selectedIndex ? 'bg-black/[0.06] text-black/90' : 'text-black/60 hover:bg-black/[0.03]'
            )}
            onClick={() => onSelect(cmd)}
          >
            <span className="text-blue-500">/</span>
            <span className="flex-1 truncate">{cmd.slice(1)}</span>
            {i === selectedIndex && (
              <span className="ml-2 text-[10px] text-muted-foreground">Tab to complete</span>
            )}
          </button>
        ))}
      </div>
    </div>
  )
}

function ChatMessage({
  message,
  onCopyMessage,
  onResumeFromCursor,
  copiedMessageId,
  hoveredMessageId,
  onMessageHoverChange,
  defaultWorkdir,
  isPrimaryThinkingMessage,
}: {
  message: Message
  onCopyMessage: (message: Message) => void
  onResumeFromCursor?: (resumeCursor: string) => void
  copiedMessageId: string | null
  hoveredMessageId: string | null
  onMessageHoverChange: (messageId: string | null) => void
  defaultWorkdir?: string
  isPrimaryThinkingMessage?: boolean
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
    return <ToolCallMessage message={message} defaultWorkdir={defaultWorkdir} />
  }

  return (
    <div
      className={cn('flex flex-col gap-2.5', isUser ? 'items-end' : 'items-start')}
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
        <div className="w-full space-y-2.5">
          {message.isError && message.toolArgs?.rawError ? (
            <ErrorCard
              error={String(message.toolArgs.rawError)}
              taskOutcome={typeof message.toolArgs.taskOutcome === 'string' ? message.toolArgs.taskOutcome : message.taskOutcome}
              resumeCursor={
                (typeof message.toolArgs.resumeCursor === 'string'
                  ? message.toolArgs.resumeCursor
                  : message.resumeCursor)
              }
              onResume={onResumeFromCursor}
            />
          ) : (
            <>
              <div className="pt-0 text-[14px] leading-6 text-black/85">
                {hasThinking && (
                  isPrimaryThinkingMessage ? (
                    <ThinkingBlock
                      thinking={message.thinking ?? ''}
                      thinkingTime={message.thinkingTime}
                      defaultOpen
                    />
                  ) : (
                    <ThinkingSummaryNode
                      thinking={message.thinking ?? ''}
                      thinkingTime={message.thinkingTime}
                    />
                  )
                )}

                {!hasThinking && message.isStreaming ? (
                  <div className="mb-2.5 flex items-start">
                    <LoadingIndicator />
                  </div>
                ) : null}

                {hasContent ? (
                  <>
                    <MarkdownContent content={message.content} />
                    <div className="mt-1.5 flex items-center gap-1.5 pl-1">
                      <div className="text-[11px] leading-none text-black/28">{shortTime}</div>
                      <MessageCopyButton
                        side="right"
                        copied={isCopied}
                        visible={showCopyButton}
                        onClick={() => onCopyMessage(message)}
                      />
                    </div>
                  </>
                ) : null}
              </div>
            </>
          )}
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

function normalizeToolStatus(message: Message): 'queued' | 'running' | 'completed' | 'error' {
  if (message.toolStatus) return message.toolStatus
  if (message.isError) return 'error'
  if (message.content.trim()) return 'completed'
  return 'running'
}

type ToolDisplay = {
  title: string
  details: string[]
  copyText: string
  resultCopyText: string | null
  diagnosticCopyText: string | null
}

function buildToolCallDisplay(message: Message, defaultWorkdir?: string): ToolDisplay {
  const toolName = (message.toolName ?? 'unknown_tool').trim()
  const args = message.toolArgs ?? {}
  const normalizedToolName = toolName.toLowerCase()
  const command = pickToolString(args, ['command', 'cmd', 'shell'])
  const path = pickToolString(args, ['path', 'file', 'file_path', 'target', 'cwd', 'directory', 'base_path', 'workdir'])
  const pattern = pickToolString(args, ['pattern', 'query', 'prompt', 'text'])
  const location = pickToolString(args, ['location', 'city', 'place'])
  const skillName = pickToolString(args, ['skill', 'name', 'title', 'path'])
  const resultSummary = summarizeToolResult(message.content)
  const titleText = pickToolHeadline(
    normalizedToolName,
    args,
    command,
    path,
    pattern,
    location,
    skillName,
    resultSummary,
    message.content,
    defaultWorkdir
  )
  const detailLines = buildToolDetailLines(
    normalizedToolName,
    args,
    command,
    path,
    pattern,
    location,
    skillName,
    resultSummary,
    defaultWorkdir
  )
  if (message.policyDecision) {
    detailLines.push(`策略：${message.policyDecision}`)
  }
  if (message.effectiveWorkdir) {
    detailLines.push(`目录：${shortenMiddle(message.effectiveWorkdir, 56)}`)
  }
  if (message.evidenceId) {
    detailLines.push(`证据：${shortenMiddle(message.evidenceId, 32)}`)
  }
  if (message.requestId) {
    detailLines.push(`请求：${shortenMiddle(message.requestId, 32)}`)
  }
  const diagnosticCopyText = buildDiagnosticCopyText(message)
  if (diagnosticCopyText) {
    detailLines.push(`诊断：${shortenMiddle(diagnosticCopyText, 56)}`)
  }

  return {
    title: titleText,
    details: detailLines.slice(0, 6),
    copyText: redactSensitiveText(titleText),
    resultCopyText: resultSummary ? redactSensitiveText(resultSummary) : null,
    diagnosticCopyText: diagnosticCopyText ? redactSensitiveText(diagnosticCopyText) : null,
  }
}

function buildDiagnosticCopyText(message: Message): string | null {
  // harness symbol marker: request_id\|diag
  if (!message.streamId || !message.evidenceId || !message.requestId) return null
  return `stream_id=${message.streamId};trace_id=${message.evidenceId};request_id=${message.requestId}`
}

function pickToolHeadline(
  toolName: string,
  args: Record<string, unknown>,
  command: string | null,
  path: string | null,
  pattern: string | null,
  location: string | null,
  skillName: string | null,
  resultSummary: string,
  content: string,
  defaultWorkdir?: string
): string {
  const declaredTitle = pickToolString(args, ['title'])
  if (declaredTitle) return truncateText(declaredTitle, 64)

  const declaredName = pickToolString(args, ['name'])
  if (declaredName && declaredName.trim() !== toolName) {
    return truncateText(declaredName, 64)
  }

  const contentPreview = content.trim()
  const looksStructured = /^[\[{]/.test(contentPreview) || /"exit_code"|"stdout"|"stderr"|"status"/.test(contentPreview)
  if (looksStructured && resultSummary) {
    return truncateText(resultSummary, 64)
  }

  if (contentPreview && contentPreview.length <= 48 && !contentPreview.includes('\n')) {
    if (!looksStructured) {
      return truncateText(contentPreview, 64)
    }
  }

  const targetPath = path || defaultWorkdir || ''

  if (toolName.includes('glob_search')) {
    return targetPath ? `搜索了 ${shortenMiddle(targetPath, 20)} 中的文件` : '搜索了当前目录中的文件'
  }

  if (toolName.includes('grep_search') || toolName.includes('content_search')) {
    const target = targetPath ? shortenMiddle(targetPath, 20) : '当前目录'
    return pattern ? `在 ${target} 中搜索 ${shortenMiddle(pattern, 20)}` : `在 ${target} 中搜索内容`
  }

  if (toolName.includes('read_file')) {
    return path ? `读取了 ${shortenMiddle(path, 28)}` : '读取了文件'
  }

  if (toolName.includes('file_write')) {
    return path ? `写入了 ${shortenMiddle(path, 28)}` : '写入了文件'
  }

  if (toolName.includes('web_search')) {
    return pattern ? `搜索了 ${shortenMiddle(pattern, 28)}` : '搜索了网页信息'
  }

  if (toolName.includes('weather')) {
    return location ? `查询了 ${shortenMiddle(location, 24)} 天气` : pattern ? `查询了 ${shortenMiddle(pattern, 24)} 天气` : '查询了天气'
  }

  if (toolName.includes('tool_search')) {
    return pattern ? `搜索了工具 ${shortenMiddle(pattern, 24)}` : '搜索了可用工具'
  }

  if (toolName.includes('skill_search')) {
    return pattern ? `搜索了技能 ${shortenMiddle(pattern, 24)}` : '搜索了可用技能'
  }

  if (toolName === 'skill' || toolName.includes('skill/') || toolName.includes('.skill')) {
    return skillName ? `调用了技能 ${shortenMiddle(skillName, 24)}` : '调用了技能'
  }

  if (command) {
    return `执行了命令 ${truncateText(redactSensitiveText(command), 46)}`
  }

  if (path) {
    return `执行了 ${shortenMiddle(path, 28)}`
  }

  if (pattern) {
    return `执行了 ${shortenMiddle(pattern, 28)}`
  }

  if (location) {
    return `执行了 ${shortenMiddle(location, 28)}`
  }

  return `执行了 ${toolName || '工具'}`
}

function buildToolDetailLines(
  toolName: string,
  args: Record<string, unknown>,
  command: string | null,
  path: string | null,
  pattern: string | null,
  location: string | null,
  skillName: string | null,
  resultSummary: string,
  defaultWorkdir?: string
): string[] {
  const detailLines: string[] = []
  const targetPath = path || defaultWorkdir || ''

  if (toolName.includes('glob_search')) {
    if (targetPath) detailLines.push(`范围：${shortenMiddle(targetPath, 56)}`)
    if (pattern) detailLines.push(`模式：${shortenMiddle(pattern, 56)}`)
  } else if (toolName.includes('grep_search') || toolName.includes('content_search')) {
    if (targetPath) detailLines.push(`路径：${shortenMiddle(targetPath, 56)}`)
    if (pattern) detailLines.push(`模式：${shortenMiddle(pattern, 56)}`)
  } else if (toolName.includes('read_file')) {
    if (path) detailLines.push(`文件：${shortenMiddle(path, 56)}`)
  } else if (toolName.includes('file_write')) {
    if (path) detailLines.push(`写入：${shortenMiddle(path, 56)}`)
  } else if (toolName.includes('web_search')) {
    if (pattern) detailLines.push(`搜索：${shortenMiddle(pattern, 56)}`)
  } else if (toolName.includes('weather')) {
    if (location) detailLines.push(`地点：${shortenMiddle(location, 56)}`)
    else if (pattern) detailLines.push(`地点：${shortenMiddle(pattern, 56)}`)
  } else if (toolName.includes('tool_search') || toolName.includes('skill_search')) {
    if (pattern) detailLines.push(`关键词：${shortenMiddle(pattern, 56)}`)
  } else if (toolName === 'skill' || toolName.includes('skill/') || toolName.includes('.skill')) {
    if (skillName) detailLines.push(`技能：${shortenMiddle(skillName, 56)}`)
  } else if (command) {
    detailLines.push(`命令：${truncateText(redactSensitiveText(command), 92)}`)
  }

  if (resultSummary) {
    detailLines.push(`结果：${truncateText(resultSummary, 96)}`)
  }

  const declaredStatus = pickToolString(args, ['status'])
  if (declaredStatus && !detailLines.some((line) => line.includes(declaredStatus))) {
    detailLines.push(`状态：${shortenMiddle(declaredStatus, 32)}`)
  }

  return detailLines.slice(0, 3)
}

function pickToolString(args: Record<string, unknown>, keys: readonly string[]): string | null {
  for (const key of keys) {
    const value = args[key]
    if (typeof value === 'string' && value.trim()) {
      return value.trim()
    }
  }

  return null
}

function summarizeToolResult(content: string): string {
  const trimmed = content.trim()
  if (!trimmed) return ''

  const parsed = tryParseJson(trimmed)
  if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
    const record = parsed as Record<string, unknown>
    const exitCode = typeof record.exit_code === 'number' ? record.exit_code : null
    const stdout = typeof record.stdout === 'string' ? record.stdout.trim() : ''
    const stderr = typeof record.stderr === 'string' ? record.stderr.trim() : ''

    if (stdout || stderr || exitCode !== null) {
      const output = stdout || stderr
      const outputSummary = output ? truncateText(firstNonEmptyLine(output), 72) : ''
      const statusText = exitCode === 0 ? '执行完成' : '执行失败'
      return outputSummary ? `${statusText}：${outputSummary}` : statusText
    }

    const count = record.count
    if (typeof count === 'number') {
      return `返回 ${count} 项结果`
    }

    for (const key of ['results', 'items', 'entries', 'files'] as const) {
      const value = record[key]
      if (Array.isArray(value)) {
        return `返回 ${value.length} 项结果`
      }
    }
  }

  if (Array.isArray(parsed)) {
    return `返回 ${parsed.length} 项结果`
  }

  const firstLine = firstNonEmptyLine(trimmed)
  if (!firstLine) return ''
  if (firstLine.length <= 120 && !firstLine.includes('\n')) {
    return truncateText(firstLine, 120)
  }

  return truncateText(firstLine, 120)
}

function tryParseJson(text: string): unknown | null {
  try {
    return JSON.parse(text)
  } catch {
    return null
  }
}

function firstNonEmptyLine(text: string): string {
  return text.split(/\r?\n/).find((line) => line.trim())?.trim() ?? ''
}

function shortenMiddle(text: string, maxLength: number): string {
  const value = text.trim()
  if (value.length <= maxLength) return value

  const visible = Math.max(8, Math.floor((maxLength - 1) / 2))
  const start = value.slice(0, visible)
  const end = value.slice(-visible)
  return `${start}…${end}`
}

async function copyTextToClipboard(text: string) {
  const value = text.trim()
  if (!value) return

  try {
    await navigator.clipboard.writeText(value)
  } catch (err) {
    console.error('Failed to copy text:', err)
  }
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

/**
 * ErrorCard — renders assistant errors with actionable messaging.
 * Supports both raw error strings and structured error hints.
 */
function ErrorCard({
  error,
  taskOutcome,
  resumeCursor,
  onResume,
  onRetry,
}: {
  error: string
  taskOutcome?: string
  resumeCursor?: string
  onResume?: (resumeCursor: string) => void
  onRetry?: () => void
}) {
  // Classify error for user-friendly messaging
  const { title, suggestion, isConnection, kindLabel } = classifyError(error, taskOutcome)

  return (
    <div className="relative w-full overflow-hidden rounded-[13px] border border-rose-200/50 bg-rose-50/32 px-3 py-2.5">
      <div className="absolute inset-y-0 left-0 w-1.5 rounded-l-[13px] bg-rose-400/90" />
      <div className="flex items-start gap-2.5 pl-2 pr-1">
        <div className="mt-0.25 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-rose-500/8">
          <AlertTriangle className="h-3.5 w-3.5 text-rose-500/92" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div className="text-[12.5px] font-medium leading-4.5 text-rose-800/90">{title}</div>
            <span className="rounded-full border border-rose-300/70 bg-rose-100/70 px-2 py-0.5 text-[10px] font-medium leading-4 text-rose-700/90">
              {kindLabel}
            </span>
          </div>
          {suggestion && (
            <div className="mt-0.5 text-[11.5px] leading-4.5 text-rose-700/68">{suggestion}</div>
          )}
          <div className="mt-1.25 break-words rounded-md bg-white/32 px-2.5 py-1.25 text-[11px] font-mono leading-4 text-rose-600/72">
            {truncateText(error, 500)}
          </div>
          {isConnection && onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="mt-1.75 flex items-center gap-1.5 rounded-md bg-rose-100/68 px-2.5 py-1 text-[11px] font-medium text-rose-700 transition-colors hover:bg-rose-200/60"
            >
              <RotateCcw className="h-3 w-3" />
              重试
            </button>
          )}
          {resumeCursor && onResume && (
            <button
              type="button"
              onClick={() => onResume(resumeCursor)}
              className="mt-1.75 ml-2 inline-flex items-center gap-1.5 rounded-md bg-rose-100/68 px-2.5 py-1 text-[11px] font-medium text-rose-700 transition-colors hover:bg-rose-200/60"
            >
              继续未完成任务
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

/**
 * Classify an error string into user-friendly messaging.
 */
function classifyError(error: string, taskOutcome?: string): {
  title: string
  suggestion: string
  isConnection: boolean
  kindLabel: string
} {
  return classifyErrorWithOutcome(error, taskOutcome)
}

function classifyErrorWithOutcome(error: string, taskOutcome?: string): {
  title: string
  suggestion: string
  isConnection: boolean
  kindLabel: string
} {
  const lower = error.toLowerCase()

  if (taskOutcome === 'partial_success' || lower.includes('task_outcome] partial_success')) {
    return {
      title: '任务部分完成',
      suggestion: '本轮已有部分工具执行成功，但流在尾段中断。可继续发送“继续完成”来补全结果。',
      isConnection: true,
      kindLabel: '部分成功',
    }
  }

  if (lower.includes('network_timeout:')) {
    return {
      title: '模型流超时',
      suggestion: '上游模型流在超时时间内未返回数据，建议重试或切换模型。',
      isConnection: true,
      kindLabel: '网络超时',
    }
  }

  if (lower.includes('permission_error:')) {
    return {
      title: '权限受限',
      suggestion: '当前权限模式不允许继续执行，请在权限弹窗中授权后重试。',
      isConnection: false,
      kindLabel: '权限错误',
    }
  }

  if (lower.includes('request_validation_error:')) {
    return {
      title: '请求格式不兼容',
      suggestion: '会话历史中的工具调用顺序与模型接口约束不一致，建议重试或新建会话。',
      isConnection: false,
      kindLabel: '请求校验失败',
    }
  }

  if (lower.includes('network_transport_error:')) {
    return {
      title: '网络传输中断',
      suggestion: '连接被中断或网络不稳定，请检查网络后重试。',
      isConnection: true,
      kindLabel: '网络中断',
    }
  }

  if (lower.includes('model_stream_error:')) {
    return {
      title: '模型流异常',
      suggestion: '模型流式输出异常终止，请稍后重试。',
      isConnection: false,
      kindLabel: '模型流断开',
    }
  }

  if (lower.includes('connection refused') || lower.includes('network') || lower.includes('dns')) {
    return {
      title: '网络连接失败',
      suggestion: '请检查网络连接和代理设置，确认 LLM 服务地址可访问。',
      isConnection: true,
      kindLabel: '网络错误',
    }
  }

  if (lower.includes('400') || lower.includes('invalidparameter') || lower.includes('invalid')) {
    return {
      title: '请求参数有误',
      suggestion: '工具定义或消息格式与服务端不兼容，请检查配置后重试。',
      isConnection: false,
      kindLabel: '请求错误',
    }
  }

  if (lower.includes('401') || lower.includes('unauthorized') || lower.includes('auth')) {
    return {
      title: '认证失败',
      suggestion: 'API Key 或认证令牌已过期，请在设置中更新。',
      isConnection: false,
      kindLabel: '认证错误',
    }
  }

  if (lower.includes('429') || lower.includes('rate limit') || lower.includes('too many requests')) {
    return {
      title: '请求频率受限',
      suggestion: 'API 调用已达上限，请稍后再试。',
      isConnection: false,
      kindLabel: '限流',
    }
  }

  if (lower.includes('500') || lower.includes('502') || lower.includes('503') || lower.includes('504')) {
    return {
      title: '服务端异常',
      suggestion: 'LLM 服务端暂时不可用，请稍后重试。',
      isConnection: true,
      kindLabel: '服务异常',
    }
  }

  if (lower.includes('missing credential') || lower.includes('missing_credentials')) {
    return {
      title: '缺少 API 配置',
      suggestion: '请在 ~/.claude/settings.json 中配置 ANTHROPIC_AUTH_TOKEN 和 ANTHROPIC_BASE_URL。',
      isConnection: false,
      kindLabel: '配置缺失',
    }
  }

  return {
    title: 'Agent 执行异常',
    suggestion: '请检查后端日志或网络配置，确认 LLM 服务可用。',
    isConnection: false,
    kindLabel: '未知错误',
  }
}

function ThinkingBlock({
  thinking,
  thinkingTime,
  defaultOpen = false,
}: {
  thinking: string
  thinkingTime?: number
  defaultOpen?: boolean
}) {
  const [open, setOpen] = React.useState(defaultOpen)
  React.useEffect(() => {
    setOpen(defaultOpen)
  }, [defaultOpen, thinking])
  const durationLabel = thinkingTime ? formatDuration(thinkingTime) : '—'

  return (
    <div className="relative mb-3 pl-4">
      <div className="absolute bottom-0 left-[5px] top-0 w-px bg-black/14" />
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="flex items-center gap-1.5 rounded-none px-0 py-[2px] text-[12px] font-light italic tracking-tight text-black/30 transition-colors hover:text-black/46"
      >
        <span className="h-1.5 w-1.5 rounded-full bg-black/30" />
        <span>已完成思考</span>
        <span className="text-black/26 not-italic"> {durationLabel}</span>
        <ChevronDown className={cn('ml-0.5 h-3 w-3 transition-transform', open && 'rotate-180')} />
      </button>

      <div
        className={cn(
          'overflow-hidden transition-[max-height,opacity] duration-200',
          open ? 'max-h-[360px] opacity-100' : 'max-h-0 opacity-0'
        )}
      >
        <div className="ml-[10px] border-l border-black/8 pl-3 pt-1">
          <div className="whitespace-pre-wrap px-1 text-[11.5px] font-light italic leading-6 text-black/44">
            {thinking}
          </div>
        </div>
      </div>
    </div>
  )
}

function ThinkingSummaryNode({
  thinking,
  thinkingTime,
}: {
  thinking: string
  thinkingTime?: number
}) {
  const durationLabel = thinkingTime ? formatDuration(thinkingTime) : ''
  const summary = summarizeThinkingText(thinking)
  const label = durationLabel ? `${summary} · ${durationLabel}` : summary

  return (
    <div className="relative mb-3 pl-4">
      <div className="absolute bottom-0 left-[5px] top-0 w-px bg-black/14" />
      <div className="flex items-center gap-1.5 rounded-none px-0 py-[2px] text-[12px] italic tracking-tight text-black/28">
        <span className="h-1.5 w-1.5 rounded-full bg-black/28" />
        <span className="truncate">{label}</span>
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

const permissionModeItems: Array<{ value: PermissionMode; label: string }> = [
  { value: 'dangerFullAccess', label: '完全访问权限' },
  { value: 'workspaceWrite', label: '受限访问' },
  { value: 'readOnly', label: '只读' },
]

function modelLabelFor(value: string) {
  return modelItems.find((item) => item.value === value)?.label ?? 'GPT-5.4-Mini'
}

function strengthLabelFor(value: string) {
  return strengthItems.find((item) => item.value === value)?.label ?? '中'
}

function permissionModeLabelFor(value: PermissionMode) {
  return permissionModeItems.find((item) => item.value === value)?.label ?? '完全访问权限'
}

function summarizeThinkingText(thinking: string): string {
  const firstLine = firstNonEmptyLine(thinking)
  if (!firstLine) return '已完成思考'
  return truncateText(firstLine.replace(/\s+/g, ' '), 44)
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

function redactSensitiveText(text: string): string {
  if (!text) return text
  return text
    .replace(/(token|password|secret|api[_-]?key)\s*[:=]\s*[^\s]+/gi, '$1=<redacted>')
    .replace(/Bearer\s+[A-Za-z0-9._-]+/g, 'Bearer <redacted>')
}
