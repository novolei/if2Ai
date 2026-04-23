import * as React from "react"
import { invoke } from '@tauri-apps/api/core'
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  ArrowUpRight,
  Bot,
  Cpu,
  ChevronDown,
  ChevronRight,
  Check,
  Circle,
  FileText,
  File,
  Folder,
  FolderOpen,
  Globe,
  Laptop,
  Mic,
  GripVertical,
  House,
  Lock,
  Clock3,
  Copy,
  LoaderCircle,
  MoreHorizontal,
  Plus,
  Search,
  Sparkles,
  SlidersHorizontal,
  Square,
  Paperclip,
  AlertTriangle,
  RotateCcw,
  TerminalSquare,
  UserRound,
  Wrench,
  X,
  Quote,
  ScanSearch,
} from "lucide-react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import "highlight.js/styles/github.css"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
// Phase TTS-E：消息级语音播放按钮（lazy 加载，避免 AudioContext 在入口初始化）
const MessageVoiceButtonLazy = React.lazy(() =>
  import('@/modules/chat/MessageVoiceButton').then((m) => ({ default: m.MessageVoiceButton }))
)
// 语音输入按钮（SenseVoice STT）
const SttButtonLazy = React.lazy(() =>
  import('@/modules/chat/SttButton').then((m) => ({ default: m.SttButton }))
)
import { listDirectoryPreview, openDirectoryPath, readFilePreview, writeFileContents, type ContextBudgetUsage, type DirectoryEntryPreview, type FilePreviewPayload, type MemoryContextItem, type PermissionMode, type SessionTotals } from "@/lib/tauri"
import { MemoryChip } from "@/components/memory/MemoryChip"
import { TurnCostChip } from "@/components/chat/TurnCostChip"
import { RoutingChip } from "@/components/chat/RoutingChip"
import { MemoryWriteCard } from "@/components/memory/MemoryWriteCard"
import { WriteToolDiffCard } from "@/components/chat/WriteToolDiffCard"
import { BranchPicker } from "@/components/chat/BranchPicker"
import { SlashResultCard } from "@/components/chat/SlashResultCard"
import { ContextBar } from "@/components/chat/ContextBar"
import { VirtualMessageList } from "@/components/chat/VirtualMessageList"

/**
 * Threshold above which the chat transcript switches from straight
 * `messages.map(...)` rendering to virtualised rendering.  Sessions below
 * this size keep the original DOM shape (zero behaviour change); long
 * sessions automatically opt into virtualisation for stable scroll perf.
 */
const VIRTUAL_LIST_THRESHOLD = 50
import { TodoPanel, type TodoItem } from "@/components/ui/TodoPanel"
import { WaveDotsAnimation } from "@/components/loading/WaveDotsAnimation"
import { ProjectPreviewPanel } from "@/components/ui/ProjectPreviewPanel"
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
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
  memoryScope?: 'global' | 'project' | 'session'
  memoryReasonCode?: string
  evidenceId?: string
  requestId?: string
  taskOutcome?: 'completed' | 'partial_success' | 'failed'
  degradedReason?: string
  resumeAvailable?: boolean
  resumeCursor?: string
  statusLabel?: string
  statusKind?: 'info' | 'success' | 'partial' | 'failed'
  isRecovering?: boolean
  memoryContext?: MemoryContextItem[]
  contextBudgetUsage?: ContextBudgetUsage
  /** P1-7 / P2-11 — provider-billable token + USD chip data. */
  turnCost?: import("@/runtime-projection/types").TurnCost
  /** P1-8 — smart-routing decision summary. */
  routing?: import("@/runtime-projection/types").RoutingInfo
}

interface ChatUIProps {
  messages: Message[]
  input: string
  onInputChange?: (value: string) => void
  onSubmit: (value?: string) => void
  onResumeFromCursor?: (resumeCursor: string) => void
  onStop?: () => void
  isLoading?: boolean
  sessionTitle?: string
  projectLabel?: string
  defaultWorkdir?: string
  branchLabel?: string
  onBranchChange?: (newBranch: string) => void
  isGitRepo?: boolean | null
  onGitRepoChanged?: () => void
  selectedModel?: string
  onModelChange?: React.Dispatch<React.SetStateAction<string>>
  permissionMode?: PermissionMode
  onPermissionModeChange?: React.Dispatch<React.SetStateAction<PermissionMode>>
  todos?: TodoItem[]
  isProjectRailOpen?: boolean
  onProjectRailOpenChange?: React.Dispatch<React.SetStateAction<boolean>>
  isLeftPaneCollapsed?: boolean
  densityMode?: DensityMode
  fontMode?: FontMode
  onPreviewFocusChange?: (active: boolean) => void
  /** P2-11 — running per-session totals; rendered in ContextBar's second row. */
  sessionTotals?: SessionTotals
}

type DensityMode = 'comfortable' | 'compact'
type FontMode = 'sans' | 'serif'
type RailBreadcrumb = { label: string; path: string }
type RailTreeMap = Record<string, DirectoryEntryPreview[]>
type ComposerDropItem = {
  id: string
  name: string
  path: string
  kind: 'file' | 'folder'
  /** True when added via @-mention typing (vs drag-drop). Changes chip visual. */
  isMention?: boolean
}

const BOTTOM_EPSILON_PX = 120
const CHAT_DENSITY_MODE_STORAGE_KEY = 'chatDensityModeV2'
const CHAT_FONT_MODE_STORAGE_KEY = 'chatFontModeV2'
const PROJECT_RAIL_WIDTH_STORAGE_KEY = 'projectRailWidthV1'
const PROJECT_RAIL_MIN_WIDTH = 140
const PROJECT_RAIL_MAX_WIDTH = 300
const PREVIEW_AUTOSAVE_DELAY_MS = 900
const PROJECT_RAIL_NOTICE_DURATION_MS = 2800

const SURFACE_CARD_TOKENS = {
  radius: 'rounded-[6px]',
  border: 'border border-border/50',
  background: 'bg-surface',
  headerBackground: 'bg-surface-raised',
  headerDivider: 'border-b border-border/50',
  headerLabel: 'font-mono text-[10px] tracking-tight text-muted-foreground/45',
} as const

function FontSansIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className} aria-hidden="true">
      <text
        x="2.3"
        y="11.2"
        fontSize="8.4"
        fontWeight="600"
        fontFamily="system-ui, -apple-system, Segoe UI, Roboto, sans-serif"
        fill="currentColor"
      >
        Aa
      </text>
    </svg>
  )
}

function FontSerifIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className} aria-hidden="true">
      <text
        x="2.1"
        y="11.2"
        fontSize="8.4"
        fontWeight="600"
        fontFamily="ui-serif, Georgia, Cambria, Times New Roman, serif"
        fill="currentColor"
      >
        Aa
      </text>
    </svg>
  )
}

function DensityCompactIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className} aria-hidden="true">
      <path d="M3 5h10M3 8h10M3 11h10" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  )
}

function DensityComfortableIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" className={className} aria-hidden="true">
      <path d="M3 4h10M3 8h10M3 12h10" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  )
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
  onBranchChange,
  isGitRepo = null,
  onGitRepoChanged,
  selectedModel: selectedModelProp = '',
  onModelChange: onModelChangeProp,
  permissionMode: permissionModeProp = 'dangerFullAccess',
  onPermissionModeChange: onPermissionModeChangeProp,
  todos = [],
  isProjectRailOpen: isProjectRailOpenProp = true,
  onProjectRailOpenChange,
  isLeftPaneCollapsed = false,
  densityMode: densityModeProp = 'comfortable',
  fontMode: fontModeProp = 'sans',
  onPreviewFocusChange,
  sessionTotals,
}: ChatUIProps) {
  const bottomRef = React.useRef<HTMLDivElement>(null)
  const transcriptScrollRef = React.useRef<HTMLDivElement>(null)
  const textareaRef = React.useRef<HTMLTextAreaElement>(null)
  const todoPanelRef = React.useRef<HTMLDivElement>(null)
  const isAtBottomRef = React.useRef(true)
  const forceAutoScrollRef = React.useRef(false)
  const stickToBottomDuringStreamRef = React.useRef(true)
  const lastScrollTopRef = React.useRef(0)
  const lastBottomOccupancyRef = React.useRef(0)
  const scrollRafRef = React.useRef<number | null>(null)
  const slashTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)
  const [selectedModel, setSelectedModel] = React.useState(selectedModelProp)
  // Dynamic model list state (Slice 4)
  const [availableModelItems, setAvailableModelItems] = React.useState<
    Array<{ value: string; label: string }>
  >([])
  React.useEffect(() => {
    void (async () => {
      try {
        const groups = await invoke<Array<{
          provider_id: string
          provider_name: string
          models: Array<{ model_id: string; name: string }>
        }>>('model_list_available')
        const items = groups
          .filter((g) => g.models.length > 0)
          .flatMap((g) =>
            g.models.map((m) => ({
              value: `${g.provider_id}/${m.model_id}`,
              label: `${g.provider_name} / ${m.name}`,
            }))
          )
        setAvailableModelItems(items)

        // If the parent hasn't provided a real model yet, auto-select the first
        // available one so the picker always shows a real model name.
        if (!selectedModelProp && items.length > 0 && onModelChangeProp) {
          const first = items[0]
          onModelChangeProp(first.value)
        }
      } catch {
        // Fallback: leave list empty, will use hardcoded below
      }
    })()
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  const [selectedPermissionMode, setSelectedPermissionMode] = React.useState<PermissionMode>(permissionModeProp)
  const [selectedStrength, setSelectedStrength] = React.useState('mid')
  const [isComposerFocused, setIsComposerFocused] = React.useState(false)
  const [isAtBottom, setIsAtBottom] = React.useState(true)
  const [copiedMessageId, setCopiedMessageId] = React.useState<string | null>(null)
  const [isTodoCollapsed, setIsTodoCollapsed] = React.useState(false)
  const [todoPanelHeight, setTodoPanelHeight] = React.useState(0)
  const [draftInput, setDraftInput] = React.useState(input)
  const [projectRailEntries, setProjectRailEntries] = React.useState<DirectoryEntryPreview[]>([])
  const [projectRailTree, setProjectRailTree] = React.useState<RailTreeMap>({})
  const [projectRailExpandedPaths, setProjectRailExpandedPaths] = React.useState<string[]>([])
  const [projectRailLoadingPaths, setProjectRailLoadingPaths] = React.useState<string[]>([])
  const [isProjectRailLoading, setIsProjectRailLoading] = React.useState(false)
  const [projectRailSort, setProjectRailSort] = React.useState<'recent' | 'name'>('recent')
  const [projectRailPath, setProjectRailPath] = React.useState<string | null>(defaultWorkdir ?? null)
  const [projectRailPreviewError, setProjectRailPreviewError] = React.useState<string | null>(null)
  const projectRailRefreshSeqRef = React.useRef(0)
  const projectRailNoticeTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)
  const [composerDropItems, setComposerDropItems] = React.useState<ComposerDropItem[]>([])
  // Derive the most recent context-budget snapshot from the assistant
  // messages.  This is propagated by `App.tsx` from the `stream_complete`
  // event payload.  When no turn has completed yet, the bar renders
  // nothing (see `ContextBar`).
  const latestContextBudgetUsage = React.useMemo<ContextBudgetUsage | undefined>(() => {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      const m = messages[i]
      if (m.contextBudgetUsage) return m.contextBudgetUsage
    }
    return undefined
  }, [messages])
  const [projectPreviewTabs, setProjectPreviewTabs] = React.useState<FilePreviewPayload[]>([])
  const [activeProjectPreviewPath, setActiveProjectPreviewPath] = React.useState<string | null>(null)
  const [isProjectPreviewOpen, setIsProjectPreviewOpen] = React.useState(false)
  const [projectPreviewDrafts, setProjectPreviewDrafts] = React.useState<Record<string, string>>({})
  const [projectPreviewSaveStates, setProjectPreviewSaveStates] = React.useState<Record<string, 'idle' | 'saving' | 'saved' | 'error'>>({})
  const [projectPreviewDirtyPaths, setProjectPreviewDirtyPaths] = React.useState<string[]>([])
  const projectPreviewSaveTimersRef = React.useRef<Record<string, ReturnType<typeof setTimeout>>>({})
  const projectRailOpenBeforePreviewRef = React.useRef(true)
  const wasPreviewFocusModeRef = React.useRef(false)
  const [projectRailWidth, setProjectRailWidth] = React.useState(() => {
    if (typeof window === 'undefined') return 200
    const stored = Number(window.localStorage.getItem(PROJECT_RAIL_WIDTH_STORAGE_KEY))
    if (Number.isFinite(stored)) {
      return Math.min(PROJECT_RAIL_MAX_WIDTH, Math.max(PROJECT_RAIL_MIN_WIDTH, stored))
    }
    return 200
  })
  const isProjectRailOpen = isProjectRailOpenProp
  const densityMode = densityModeProp
  const fontMode = fontModeProp
  const [slashOverlay, setSlashOverlay] = React.useState<{
    visible: boolean
    selectedIndex: number
    suggestions: string[]
    rawInput: string
  } | null>(null)

  const [atOverlay, setAtOverlay] = React.useState<{
    visible: boolean
    selectedIndex: number
    entries: DirectoryEntryPreview[]
    query: string
    /** Position of the @ character in the textarea (−1 = already cleaned) */
    atPos: number
    /** Currently browsed directory path; null = workdir root */
    browsePath: string | null
    /** Navigation history stack for back navigation */
    breadcrumbs: Array<{ name: string; path: string | null }>
    /** When true the overlay is detached from the textarea (@token already removed) */
    pinned: boolean
  } | null>(null)
  const atTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)

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
      if (atTimerRef.current) {
        clearTimeout(atTimerRef.current)
      }
      if (scrollRafRef.current !== null) {
        window.cancelAnimationFrame(scrollRafRef.current)
      }
      if (projectRailNoticeTimerRef.current) {
        clearTimeout(projectRailNoticeTimerRef.current)
      }
      Object.values(projectPreviewSaveTimersRef.current).forEach((timer) => clearTimeout(timer))
    }
  }, [])

  const updateBottomState = React.useCallback((container: HTMLDivElement) => {
    const maxScrollTop = container.scrollHeight - container.clientHeight
    const nextIsAtBottom = maxScrollTop - container.scrollTop < BOTTOM_EPSILON_PX
    isAtBottomRef.current = nextIsAtBottom
    if (nextIsAtBottom) {
      stickToBottomDuringStreamRef.current = true
    }
    setIsAtBottom((prev) => (prev === nextIsAtBottom ? prev : nextIsAtBottom))
  }, [])

  React.useEffect(() => {
    const shouldAutoScroll = isAtBottomRef.current || forceAutoScrollRef.current
      || (Boolean(isLoading) && stickToBottomDuringStreamRef.current)
    if (!shouldAutoScroll) return
    bottomRef.current?.scrollIntoView({
      behavior: forceAutoScrollRef.current || isLoading ? 'auto' : 'smooth',
      block: 'end',
    })
    if (forceAutoScrollRef.current) {
      forceAutoScrollRef.current = false
      isAtBottomRef.current = true
      setIsAtBottom(true)
    }
  }, [messages, isLoading])

  React.useEffect(() => {
    if (!isAtBottomRef.current) return
    bottomRef.current?.scrollIntoView({ behavior: 'auto', block: 'end' })
  }, [todoPanelHeight, isTodoCollapsed, todos.length])

  React.useEffect(() => {
    setDraftInput(input)
  }, [input])

  React.useEffect(() => {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(CHAT_DENSITY_MODE_STORAGE_KEY, densityMode)
  }, [densityMode])

  React.useEffect(() => {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(CHAT_FONT_MODE_STORAGE_KEY, fontMode)
  }, [fontMode])

  React.useEffect(() => {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(PROJECT_RAIL_WIDTH_STORAGE_KEY, String(projectRailWidth))
  }, [projectRailWidth])

  React.useEffect(() => {
    setProjectRailPath(defaultWorkdir ?? null)
    setProjectRailPreviewError(null)
    setComposerDropItems([])
    if (projectRailNoticeTimerRef.current) {
      clearTimeout(projectRailNoticeTimerRef.current)
      projectRailNoticeTimerRef.current = null
    }
    setProjectRailExpandedPaths([])
    setProjectRailTree({})
    setProjectPreviewTabs([])
    setActiveProjectPreviewPath(null)
    setIsProjectPreviewOpen(false)
    setProjectPreviewDrafts({})
    setProjectPreviewSaveStates({})
    setProjectPreviewDirtyPaths([])
  }, [defaultWorkdir])

  const isPreviewFocusMode = isProjectPreviewOpen && projectPreviewTabs.length > 0
  const chatVisibleRightInset = !isPreviewFocusMode && isProjectRailOpen ? projectRailWidth + 28 : 0
  const transcriptMaxWidth = isLeftPaneCollapsed
    ? (isProjectRailOpen ? 980 : 1120)
    : (isProjectRailOpen ? 900 : 1020)
  const composerMaxWidth = isLeftPaneCollapsed
    ? (isProjectRailOpen ? 940 : 1080)
    : (isProjectRailOpen ? 860 : 980)
  const todoMaxWidth = isLeftPaneCollapsed
    ? (isProjectRailOpen ? 820 : 960)
    : (isProjectRailOpen ? 760 : 860)

  React.useEffect(() => {
    onPreviewFocusChange?.(isPreviewFocusMode)
  }, [isPreviewFocusMode, onPreviewFocusChange])

  React.useEffect(() => {
    if (isPreviewFocusMode) {
      projectRailOpenBeforePreviewRef.current = isProjectRailOpen
      if (isProjectRailOpen) {
        onProjectRailOpenChange?.(false)
      }
    } else if (wasPreviewFocusModeRef.current && projectRailOpenBeforePreviewRef.current) {
      onProjectRailOpenChange?.(true)
    }
    wasPreviewFocusModeRef.current = isPreviewFocusMode
  }, [isPreviewFocusMode, isProjectRailOpen, onProjectRailOpenChange])

  const railRootPath = defaultWorkdir ?? null

  const showProjectRailNotice = React.useCallback((message: string | null) => {
    if (projectRailNoticeTimerRef.current) {
      clearTimeout(projectRailNoticeTimerRef.current)
      projectRailNoticeTimerRef.current = null
    }
    setProjectRailPreviewError(message)
    if (message) {
      projectRailNoticeTimerRef.current = window.setTimeout(() => {
        setProjectRailPreviewError((current) => current === message ? null : current)
        projectRailNoticeTimerRef.current = null
      }, PROJECT_RAIL_NOTICE_DURATION_MS)
    }
  }, [])

  const refreshDirectoryPreview = React.useCallback(async (options?: { silent?: boolean }) => {
    if (!railRootPath || !isProjectRailOpen) return

    const requestId = ++projectRailRefreshSeqRef.current
    if (!options?.silent) {
      setIsProjectRailLoading(true)
    }

    try {
      const rootEntries = await listDirectoryPreview(railRootPath, 64)
      if (projectRailRefreshSeqRef.current !== requestId) return
      const uniquePaths = Array.from(new Set(projectRailExpandedPaths)).filter((path) => path && path !== railRootPath)
      const childResults = await Promise.all(
        uniquePaths.map(async (path) => {
          try {
            const entries = await listDirectoryPreview(path, 64)
            return [path, sortRailDirectoryEntries(entries, projectRailSort)] as const
          } catch {
            return [path, []] as const
          }
        })
      )
      if (projectRailRefreshSeqRef.current !== requestId) return
      setProjectRailEntries(sortRailDirectoryEntries(rootEntries, projectRailSort))
      setProjectRailTree(Object.fromEntries(childResults))
    } catch (err) {
      console.error('Failed to load directory preview:', err)
      if (projectRailRefreshSeqRef.current === requestId && !options?.silent) {
        setProjectRailEntries([])
        setProjectRailTree({})
      }
    } finally {
      if (projectRailRefreshSeqRef.current === requestId && !options?.silent) {
        setIsProjectRailLoading(false)
      }
    }
  }, [railRootPath, isProjectRailOpen, projectRailExpandedPaths, projectRailSort])

  React.useEffect(() => {
    void refreshDirectoryPreview()
  }, [refreshDirectoryPreview])

  React.useEffect(() => {
    if (!railRootPath || !isProjectRailOpen) return
    const intervalId = window.setInterval(() => {
      void refreshDirectoryPreview({ silent: true })
    }, 2500)
    return () => window.clearInterval(intervalId)
  }, [railRootPath, isProjectRailOpen, refreshDirectoryPreview])

  React.useEffect(() => {
    if (!railRootPath || !isProjectRailOpen) return
    const timeoutId = window.setTimeout(() => {
      void refreshDirectoryPreview({ silent: true })
    }, isLoading ? 800 : 260)
    return () => window.clearTimeout(timeoutId)
  }, [messages, isLoading, railRootPath, isProjectRailOpen, refreshDirectoryPreview])

  React.useEffect(() => {
    if (!railRootPath || !isProjectRailOpen) return
    const onFocus = () => {
      void refreshDirectoryPreview({ silent: true })
    }
    window.addEventListener('focus', onFocus)
    return () => window.removeEventListener('focus', onFocus)
  }, [railRootPath, isProjectRailOpen, refreshDirectoryPreview])

  React.useEffect(() => {
    const container = transcriptScrollRef.current
    if (!container) return
    updateBottomState(container)
  }, [messages, isLoading, updateBottomState])

  React.useLayoutEffect(() => {
    const textarea = textareaRef.current
    if (!textarea) return

    textarea.style.height = 'auto'
    textarea.style.height = `${Math.min(textarea.scrollHeight, 220)}px`
  }, [draftInput])

  React.useLayoutEffect(() => {
    const observeSize = (
      element: HTMLElement | null,
      onChange: React.Dispatch<React.SetStateAction<number>>
    ) => {
      if (!element) return () => {}

      const update = () => {
        const next = Math.round(element.getBoundingClientRect().height)
        onChange((prev) => (Math.abs(prev - next) >= 1 ? next : prev))
      }
      update()

      const observer = new ResizeObserver(() => update())
      observer.observe(element)
      return () => observer.disconnect()
    }

    const cleanupTodo = observeSize(todoPanelRef.current, setTodoPanelHeight)

    return () => {
      cleanupTodo()
    }
  }, [isTodoCollapsed, todos.length])

  const hasTodos = todos.length > 0
  const transcriptBottomPadding = 20
  const scrollToBottomButtonOffset = 186

  React.useLayoutEffect(() => {
    const container = transcriptScrollRef.current
    if (!container) return

    const nextOccupancy = hasTodos ? todoPanelHeight : 0
    const delta = nextOccupancy - lastBottomOccupancyRef.current
    lastBottomOccupancyRef.current = nextOccupancy

    if (delta === 0) return

    const distanceToBottom = container.scrollHeight - container.clientHeight - container.scrollTop
    const shouldKeepBottomAnchor = isAtBottomRef.current || distanceToBottom < 180
    if (!shouldKeepBottomAnchor) return

    container.scrollTop += delta
  }, [todoPanelHeight, hasTodos])

  const submitDraft = React.useCallback(() => {
    const messageText = draftInput.trim()
    const attachmentItems = composerDropItems.filter((item) => item.kind === 'file')
    const folderReferenceItems = composerDropItems.filter((item) => item.kind === 'folder')
    const segments: string[] = []

    if (attachmentItems.length > 0) {
      segments.push([
        '附件文件：',
        ...attachmentItems.map((item) => `- \`${item.path}\``),
      ].join('\n'))
    }

    if (folderReferenceItems.length > 0) {
      segments.push([
        '引用文件夹：',
        ...folderReferenceItems.map((item) => `- \`${item.path}\``),
      ].join('\n'))
    }

    if (messageText) {
      segments.push(messageText)
    }

    const next = segments.join('\n\n').trim()
    if (!next || isLoading) return
    forceAutoScrollRef.current = true
    stickToBottomDuringStreamRef.current = true
    onSubmit(next)
    setDraftInput('')
    setComposerDropItems([])
    onInputChange?.('')
  }, [composerDropItems, draftInput, isLoading, onSubmit, onInputChange])

  const handleTranscriptScroll = React.useCallback(() => {
    const container = transcriptScrollRef.current
    if (!container) return
    if (container.scrollLeft !== 0) {
      container.scrollLeft = 0
    }
    if (scrollRafRef.current !== null) return
    scrollRafRef.current = window.requestAnimationFrame(() => {
      scrollRafRef.current = null
      const nextContainer = transcriptScrollRef.current
      if (!nextContainer) return
      const nextScrollTop = nextContainer.scrollTop
      const scrollDelta = nextScrollTop - lastScrollTopRef.current
      lastScrollTopRef.current = nextScrollTop
      if (isLoading && scrollDelta < -2) {
        // User actively scrolls upward while streaming: pause sticky auto-follow.
        stickToBottomDuringStreamRef.current = false
      }
      updateBottomState(nextContainer)
    })
  }, [isLoading, updateBottomState])

  const handleKeyDown = async (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Never intercept keystrokes while an IME composition session is active.
    // Without this guard, pressing Enter to confirm a Chinese/Japanese/Korean
    // character would also trigger submit, overlay selection, etc.
    if (e.nativeEvent.isComposing) return

    // ── @-mention overlay navigation ────────────────────────────────────────
    if (atOverlay?.visible) {
      const currentEntry = atOverlay.entries[atOverlay.selectedIndex]

      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setAtOverlay((prev) =>
          prev ? { ...prev, selectedIndex: (prev.selectedIndex + 1) % prev.entries.length } : null
        )
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setAtOverlay((prev) =>
          prev
            ? { ...prev, selectedIndex: (prev.selectedIndex - 1 + prev.entries.length) % prev.entries.length }
            : null
        )
        return
      }
      // → or Enter on a folder: navigate in
      if (e.key === 'ArrowRight' || (e.key === 'Enter' && currentEntry?.kind === 'folder')) {
        e.preventDefault()
        if (currentEntry?.kind === 'folder') {
          void navigateIntoFolder(currentEntry)
        }
        return
      }
      // ← navigate up (in browse mode); or Backspace when in browse mode with empty native query
      if (e.key === 'ArrowLeft' || (e.key === 'Backspace' && atOverlay.pinned && atOverlay.query === '')) {
        if (e.key === 'ArrowLeft') e.preventDefault()
        if (atOverlay.pinned) {
          void navigateUpFolder()
          return
        }
      }
      // Tab always selects; Enter selects files (folders handled above)
      if (e.key === 'Tab' || e.key === 'Enter') {
        e.preventDefault()
        if (currentEntry) {
          handleAtSelect(currentEntry, { atPos: atOverlay.atPos, query: atOverlay.query })
        }
        return
      }
      if (e.key === 'Escape') {
        setAtOverlay(null)
        return
      }
    }

    // ── Slash command overlay navigation ────────────────────────────────────
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
        const hasInstructionText = draftInput.trim().includes(' ')
        if (e.key === 'Enter' && hasInstructionText) {
          setSlashOverlay(null)
          submitDraft()
        } else {
          setDraftInput(selected)
          setSlashOverlay(null)
        }
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
      submitDraft()
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

  const railEntries = React.useMemo(() => sortRailDirectoryEntries(projectRailEntries, projectRailSort), [projectRailEntries, projectRailSort])

  const railTitle = React.useMemo(() => {
    if (railRootPath) {
      const trimmed = railRootPath.replace(/[\\/]+$/, '')
      const parts = trimmed.split(/[\\/]/).filter(Boolean)
      return parts.at(-1) ?? projectLabel
    }
    return projectLabel
  }, [railRootPath, projectLabel])

  const railBreadcrumbs = React.useMemo<RailBreadcrumb[]>(() => {
    if (!defaultWorkdir || !projectRailPath) return []
    const normalize = (value: string) => value.replace(/[\\/]+$/, '')
    const base = normalize(defaultWorkdir)
    const current = normalize(projectRailPath)
    const crumbs: RailBreadcrumb[] = [{ label: railTitle, path: base }]
    if (!current.startsWith(base)) return crumbs
    const suffix = current.slice(base.length).replace(/^[/\\]+/, '')
    if (!suffix) return crumbs

    let runningPath = base
    for (const segment of suffix.split(/[\\/]/).filter(Boolean)) {
      runningPath = `${runningPath}/${segment}`
      crumbs.push({ label: segment, path: runningPath })
    }
    return crumbs
  }, [defaultWorkdir, projectRailPath, railTitle])

  const appendToDraft = React.useCallback((text: string) => {
    const next = draftInput.trim().length === 0 ? text : `${draftInput.replace(/\s+$/, '')}\n${text}`
    setDraftInput(next)
    onInputChange?.(next)
  }, [draftInput, onInputChange])

  const handleQuoteProjectFile = React.useCallback((preview: FilePreviewPayload) => {
    const language = inferPreviewLanguage(preview.name, preview.kind)
    if (preview.kind === 'markdown' || preview.kind === 'code') {
      const snippet = (preview.content ?? '').slice(0, 1600).trim()
      appendToDraft([
        `请结合这个文件继续处理：\`${preview.path}\``,
        `\`\`\`${language}`,
        snippet,
        '```',
      ].join('\n'))
      return
    }
    appendToDraft(`请参考这个文件：\`${preview.path}\``)
  }, [appendToDraft])

  const handleInsertProjectFileReference = React.useCallback((path: string) => {
    appendToDraft(`请把 \`${path}\` 作为当前上下文文件一起考虑。`)
  }, [appendToDraft])

  const handleComposerDropItem = React.useCallback((item: ComposerDropItem) => {
    setComposerDropItems((current) => current.some((entry) => entry.path === item.path && entry.kind === item.kind) ? current : [...current, item])
  }, [])

  const handleRemoveComposerDropItem = React.useCallback((id: string) => {
    setComposerDropItems((current) => current.filter((item) => item.id !== id))
  }, [])

  /** Called when the user picks a file/folder from the @-mention overlay. */
  const handleAtSelect = React.useCallback((entry: DirectoryEntryPreview, overlay: { atPos: number; query: string }) => {
    // Strip the @query token from the textarea only when still anchored (atPos >= 0)
    if (overlay.atPos >= 0) {
      setDraftInput((prev) => {
        const before = prev.slice(0, overlay.atPos)
        const after = prev.slice(overlay.atPos + 1 + overlay.query.length)
        return before + after
      })
    }
    setAtOverlay(null)
    // Add as a mention chip in the drop-items area
    handleComposerDropItem({
      id: `mention:${entry.path}`,
      path: entry.path,
      name: entry.name,
      kind: entry.kind,
      isMention: true,
    })
  }, [handleComposerDropItem])

  /** Navigate into a subfolder in the @-mention overlay. Detaches overlay from textarea. */
  const navigateIntoFolder = React.useCallback(async (entry: DirectoryEntryPreview) => {
    setAtOverlay((prev) => {
      // Immediately clean up @query from textarea before going async
      if (prev && prev.atPos >= 0) {
        setDraftInput((draft) => {
          const before = draft.slice(0, prev.atPos)
          const after = draft.slice(prev.atPos + 1 + prev.query.length)
          return before + after
        })
      }
      return prev
    })

    try {
      const rawEntries = await listDirectoryPreview(entry.path, 64)
      const sorted = rawEntries
        .sort((a, b) => {
          if (a.kind !== b.kind) return a.kind === 'folder' ? -1 : 1
          return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
        })
        .slice(0, 10)

      setAtOverlay((prev) => {
        if (!prev) return null
        const parentLabel = prev.browsePath
          ? prev.browsePath.split('/').filter(Boolean).pop() ?? '…'
          : '~'
        return {
          ...prev,
          entries: sorted,
          selectedIndex: 0,
          browsePath: entry.path,
          breadcrumbs: [...prev.breadcrumbs, { name: parentLabel, path: prev.browsePath }],
          pinned: true,
          atPos: -1,
          query: '',
        }
      })
    } catch {
      // ignore – keep current overlay state
    }
  }, [])

  /** Navigate up one level in the @-mention overlay. */
  const navigateUpFolder = React.useCallback(async () => {
    setAtOverlay((prev) => {
      if (!prev) return null
      if (prev.breadcrumbs.length === 0) return null // already at root → close
      return prev // keep state while we fetch; update async below
    })

    if (!atOverlay) return
    const crumbs = atOverlay.breadcrumbs
    if (crumbs.length === 0) { setAtOverlay(null); return }

    const parent = crumbs[crumbs.length - 1]
    const parentPath = parent.path ?? defaultWorkdir
    if (!parentPath) { setAtOverlay(null); return }

    try {
      const rawEntries = await listDirectoryPreview(parentPath, 64)
      const sorted = rawEntries
        .sort((a, b) => {
          if (a.kind !== b.kind) return a.kind === 'folder' ? -1 : 1
          return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
        })
        .slice(0, 10)

      setAtOverlay((prev) => {
        if (!prev) return null
        const newCrumbs = prev.breadcrumbs.slice(0, -1)
        return {
          ...prev,
          entries: sorted,
          selectedIndex: 0,
          browsePath: parent.path,
          breadcrumbs: newCrumbs,
          pinned: newCrumbs.length > 0,
          atPos: -1,
          query: '',
        }
      })
    } catch {
      setAtOverlay(null)
    }
  }, [atOverlay, defaultWorkdir])

  const loadProjectRailChildren = React.useCallback(async (path: string, options?: { silent?: boolean }) => {
    setProjectRailLoadingPaths((current) => current.includes(path) ? current : [...current, path])
    try {
      const entries = await listDirectoryPreview(path, 64)
      setProjectRailTree((current) => ({
        ...current,
        [path]: sortRailDirectoryEntries(entries, projectRailSort),
      }))
    } catch (err) {
      console.error('Failed to load project rail children:', err)
      if (!options?.silent) {
        setProjectRailTree((current) => ({
          ...current,
          [path]: [],
        }))
      }
    } finally {
      setProjectRailLoadingPaths((current) => current.filter((item) => item !== path))
    }
  }, [projectRailSort])

  const handleToggleProjectRailFolder = React.useCallback((entry: DirectoryEntryPreview) => {
    if (entry.kind !== 'folder') return
    setProjectRailPath(entry.path)
    setProjectRailExpandedPaths((current) => {
      if (current.includes(entry.path)) {
        return current.filter((item) => item !== entry.path && !item.startsWith(`${entry.path}/`))
      }
      return [...current, entry.path]
    })
    if (!projectRailTree[entry.path]) {
      void loadProjectRailChildren(entry.path)
    }
  }, [loadProjectRailChildren, projectRailTree])

  const savePreviewDraft = React.useCallback(async (path: string, content: string) => {
    setProjectPreviewSaveStates((current) => ({ ...current, [path]: 'saving' }))
    try {
      await writeFileContents(path, content)
      setProjectPreviewTabs((current) => current.map((item) => item.path === path ? { ...item, content } : item))
      setProjectPreviewDirtyPaths((current) => current.filter((item) => item !== path))
      setProjectPreviewSaveStates((current) => ({ ...current, [path]: 'saved' }))
      window.setTimeout(() => {
        setProjectPreviewSaveStates((current) => current[path] === 'saved' ? { ...current, [path]: 'idle' } : current)
      }, 1200)
      void refreshDirectoryPreview({ silent: true })
    } catch (err) {
      console.error('Failed to save preview draft:', err)
      setProjectPreviewSaveStates((current) => ({ ...current, [path]: 'error' }))
    }
  }, [refreshDirectoryPreview])

  const schedulePreviewAutosave = React.useCallback((path: string, content: string) => {
    const timers = projectPreviewSaveTimersRef.current
    if (timers[path]) {
      clearTimeout(timers[path])
    }
    timers[path] = setTimeout(() => {
      delete timers[path]
      void savePreviewDraft(path, content)
    }, PREVIEW_AUTOSAVE_DELAY_MS)
  }, [savePreviewDraft])

  const openPreviewTab = React.useCallback((preview: FilePreviewPayload) => {
    setProjectPreviewTabs((current) => current.some((item) => item.path === preview.path) ? current.map((item) => item.path === preview.path ? preview : item) : [...current, preview])
    setActiveProjectPreviewPath(preview.path)
    setIsProjectPreviewOpen(true)
    setProjectPreviewDrafts((current) => current[preview.path] !== undefined ? current : { ...current, [preview.path]: preview.content ?? '' })
    setProjectPreviewSaveStates((current) => ({ ...current, [preview.path]: current[preview.path] ?? 'idle' }))
    setProjectRailPreviewError(null)
  }, [])

  /**
   * 重新拉取一个已经打开的 preview tab 的内容，把 tab 数据替换成最新
   * 磁盘版本。`projectPreviewDrafts[path]` 不动 —— 用户正在编辑的草稿
   * 不能被外部写入静默覆盖；新内容只刷"原文"层（preview.content / tab
   * 头里显示的元数据）。如果 path 没在打开列表里就忽略。
   */
  const refreshPreviewTab = React.useCallback(
    async (absPath: string) => {
      try {
        const fresh = await readFilePreview(absPath)
        setProjectPreviewTabs((current) => {
          if (!current.some((item) => item.path === fresh.path)) return current
          return current.map((item) => (item.path === fresh.path ? fresh : item))
        })
        // 用户没在编辑（不在 dirty 列表）才同步刷草稿；否则保留草稿，
        // 让用户决定是否手动 reset。
        setProjectPreviewDrafts((current) => {
          if (projectPreviewDirtyPaths.includes(fresh.path)) return current
          return { ...current, [fresh.path]: fresh.content ?? '' }
        })
      } catch (err) {
        console.warn('[preview] refresh failed for', absPath, err)
      }
    },
    [projectPreviewDirtyPaths],
  )

  /**
   * Auto-refresh：监听 messages 里 file_write 工具调用完成事件，
   * 当被写入的路径正好对应一个已经打开的 preview tab 时，自动拉一
   * 次最新内容刷新展示。tool 流过来的 path 既可能是绝对路径也可能
   * 是相对 workdir 的相对路径，两种 case 都比对一遍。
   *
   * processedToolIdsRef 记录已经处理过的 tool_call_id，避免同一条
   * 消息在重渲染 / 状态变化时被重复拉取。
   */
  const processedToolIdsRef = React.useRef<Set<string>>(new Set())
  React.useEffect(() => {
    if (projectPreviewTabs.length === 0) return
    for (const message of messages) {
      if (message.role !== 'tool') continue
      if (message.toolStatus !== 'completed') continue
      const toolName = message.toolName ?? ''
      if (!toolName.includes('file_write')) continue
      const toolCallId = message.toolCallId
      if (!toolCallId || processedToolIdsRef.current.has(toolCallId)) continue
      const args = (message.toolArgs ?? {}) as Record<string, unknown>
      const rawPath = typeof args.path === 'string' ? args.path : ''
      if (!rawPath) continue
      // 解析绝对路径：相对路径相对 effectiveWorkdir / defaultWorkdir
      const root = message.effectiveWorkdir ?? defaultWorkdir ?? ''
      const absCandidate = rawPath.startsWith('/')
        ? rawPath
        : root
          ? `${root.replace(/\/+$/, '')}/${rawPath.replace(/^\/+/, '')}`
          : rawPath
      const matched = projectPreviewTabs.find(
        (tab) => tab.path === absCandidate || tab.path === rawPath || tab.path.endsWith(`/${rawPath}`),
      )
      processedToolIdsRef.current.add(toolCallId)
      if (matched) {
        void refreshPreviewTab(matched.path)
      }
    }
  }, [messages, projectPreviewTabs, defaultWorkdir, refreshPreviewTab])

  const closePreviewTab = React.useCallback((path: string) => {
    const timers = projectPreviewSaveTimersRef.current
    if (timers[path]) {
      clearTimeout(timers[path])
      delete timers[path]
    }
    if (projectPreviewDirtyPaths.includes(path)) {
      const next = projectPreviewDrafts[path] ?? ''
      void savePreviewDraft(path, next)
    }
    setProjectPreviewTabs((current) => {
      const next = current.filter((item) => item.path !== path)
      setActiveProjectPreviewPath((active) => {
        if (active !== path) return active
        return next.at(-1)?.path ?? null
      })
      if (next.length === 0) {
        setIsProjectPreviewOpen(false)
        onProjectRailOpenChange?.(true)
      }
      return next
    })
    setProjectPreviewDirtyPaths((current) => current.filter((item) => item !== path))
  }, [onProjectRailOpenChange, projectPreviewDirtyPaths, projectPreviewDrafts, savePreviewDraft])

  const closePreviewPanel = React.useCallback(() => {
    Object.values(projectPreviewSaveTimersRef.current).forEach((timer) => clearTimeout(timer))
    projectPreviewSaveTimersRef.current = {}
    projectPreviewDirtyPaths.forEach((path) => {
      const next = projectPreviewDrafts[path] ?? ''
      void savePreviewDraft(path, next)
    })
    setIsProjectPreviewOpen(false)
    onProjectRailOpenChange?.(true)
  }, [onProjectRailOpenChange, projectPreviewDirtyPaths, projectPreviewDrafts, savePreviewDraft])

  const handlePreviewDraftChange = React.useCallback((path: string, value: string) => {
    setProjectPreviewDrafts((current) => ({ ...current, [path]: value }))
    setProjectPreviewDirtyPaths((current) => current.includes(path) ? current : [...current, path])
    setProjectPreviewSaveStates((current) => ({ ...current, [path]: 'idle' }))
    schedulePreviewAutosave(path, value)
  }, [schedulePreviewAutosave])

  return (
    <div className="relative flex h-full min-h-0 flex-col bg-transparent">
      <div className={cn('grid min-h-0 flex-1', isPreviewFocusMode ? 'grid-cols-2' : 'grid-cols-1')}>
        <div className="relative flex min-h-0 flex-col">
          <ChatTranscript
            messages={messages}
            bottomPadding={transcriptBottomPadding}
            sessionTitle={sessionTitle}
            projectLabel={projectLabel}
            defaultWorkdir={defaultWorkdir}
            bottomRef={bottomRef}
            scrollRef={transcriptScrollRef}
            onScroll={handleTranscriptScroll}
            onCopyMessage={handleCopyMessage}
            onResumeFromCursor={onResumeFromCursor}
            copiedMessageId={copiedMessageId}
            densityMode={densityMode}
            fontMode={fontMode}
            isLeftPaneCollapsed={isLeftPaneCollapsed}
            contentRightInset={chatVisibleRightInset}
            contentMaxWidth={transcriptMaxWidth}
          />
          {!isAtBottom && (
            <div
              className="pointer-events-none absolute inset-x-0 z-20 px-10 transition-[padding-right] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
              style={{ paddingRight: `${40 + chatVisibleRightInset}px`, bottom: `${scrollToBottomButtonOffset}px` }}
            >
              <div
                className="mx-auto flex w-full justify-center transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
                style={{ maxWidth: `${transcriptMaxWidth}px` }}
              >
                <button
                  type="button"
                  onClick={() => {
                    forceAutoScrollRef.current = true
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
            </div>
          )}
          {todos.length > 0 && (
            <div
              className="relative z-0 shrink-0 px-10 transition-[padding-right] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
              style={{ paddingRight: `${40 + chatVisibleRightInset}px` }}
            >
              <div className="mx-auto w-full transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]" style={{ maxWidth: `${todoMaxWidth}px` }}>
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
          {latestContextBudgetUsage ? (
            <div
              className="relative z-0 shrink-0 px-10 transition-[padding-right] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
              style={{ paddingRight: `${40 + chatVisibleRightInset}px` }}
            >
              <div
                className="mx-auto w-full transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
                style={{ maxWidth: `${composerMaxWidth}px` }}
              >
                <ContextBar
                  usage={latestContextBudgetUsage}
                  windowSize={messages.length}
                  sessionTotals={sessionTotals}
                />
              </div>
            </div>
          ) : null}
          <ComposerDock
            input={draftInput}
            onInputChange={setDraftInput}
            onSubmit={submitDraft}
            onStop={onStop}
            isLoading={isLoading}
            selectedModel={modelValue}
            setSelectedModel={handleModelChange}
            availableModelItems={availableModelItems}
            permissionMode={permissionModeValue}
            setPermissionMode={handlePermissionModeChange}
            selectedStrength={selectedStrength}
            setSelectedStrength={setSelectedStrength}
            branchLabel={branchLabel}
            onBranchChange={onBranchChange}
            isGitRepo={isGitRepo}
            onGitRepoChanged={onGitRepoChanged}
            isComposerFocused={isComposerFocused}
            setIsComposerFocused={setIsComposerFocused}
            isLeftPaneCollapsed={isLeftPaneCollapsed}
            contentRightInset={chatVisibleRightInset}
            contentMaxWidth={composerMaxWidth}
            textareaRef={textareaRef}
            handleKeyDown={handleKeyDown}
            setSlashOverlay={setSlashOverlay}
            slashOverlayState={slashOverlay}
            onSlashSelect={(selected) => {
              setDraftInput(selected)
              setSlashOverlay(null)
            }}
            slashTimerRef={slashTimerRef}
            atOverlayState={atOverlay}
            setAtOverlay={setAtOverlay}
            onAtSelect={handleAtSelect}
            onNavigateIntoFolder={navigateIntoFolder}
            onNavigateUpFolder={navigateUpFolder}
            atTimerRef={atTimerRef}
            defaultWorkdir={defaultWorkdir}
            dropItems={composerDropItems}
            onRemoveDropItem={handleRemoveComposerDropItem}
            onFileReferenceDrop={handleComposerDropItem}
            projectLabel={projectLabel}
            workdirLabel={
              defaultWorkdir
                ? defaultWorkdir.split('/').filter(Boolean).pop()
                : undefined
            }
          />
        </div>

        {isPreviewFocusMode ? (
          <ProjectPreviewPanel
            tabs={projectPreviewTabs}
            activeTabPath={activeProjectPreviewPath}
            drafts={projectPreviewDrafts}
            saveStates={projectPreviewSaveStates}
            dirtyPaths={projectPreviewDirtyPaths}
            onSelectTab={setActiveProjectPreviewPath}
            onCloseTab={closePreviewTab}
            onClosePanel={closePreviewPanel}
            onOpenExternally={(preview) => {
              void openDirectoryPath(preview.path)
            }}
            onQuoteIntoChat={handleQuoteProjectFile}
            onInsertIntoChat={(preview) => {
              handleInsertProjectFileReference(preview.path)
            }}
            onChangeDraft={handlePreviewDraftChange}
            onSaveNow={(path) => {
              const next = projectPreviewDrafts[path] ?? ''
              void savePreviewDraft(path, next)
            }}
            onRefresh={refreshPreviewTab}
          />
        ) : null}
      </div>

      <ProjectFilesRail
        open={!isPreviewFocusMode && isProjectRailOpen}
        width={projectRailWidth}
        title={railTitle}
        projectLabel={projectLabel}
        breadcrumbs={railBreadcrumbs}
        currentPath={projectRailPath}
        entries={railEntries}
        childEntries={projectRailTree}
        expandedPaths={projectRailExpandedPaths}
        loadingPaths={projectRailLoadingPaths}
        isLoading={isProjectRailLoading}
        sortMode={projectRailSort}
        onSortModeChange={setProjectRailSort}
        previewError={projectRailPreviewError}
        onWidthChange={setProjectRailWidth}
        onNavigateUp={() => {
          if (!projectRailPath || !defaultWorkdir) return
          const normalizedBase = defaultWorkdir.replace(/[\\/]+$/, '')
          const normalizedCurrent = projectRailPath.replace(/[\\/]+$/, '')
          if (normalizedCurrent === normalizedBase) return
          const parent = normalizedCurrent.split(/[\\/]/).slice(0, -1).join('/')
          showProjectRailNotice(null)
          const nextPath = parent || normalizedBase
          setProjectRailPath(nextPath)
          if (nextPath !== normalizedBase && !projectRailExpandedPaths.includes(nextPath)) {
            setProjectRailExpandedPaths((current) => [...current, nextPath])
            void loadProjectRailChildren(nextPath, { silent: true })
          }
        }}
        onJumpToBreadcrumb={(path) => {
          showProjectRailNotice(null)
          setProjectRailPath(path)
          if (defaultWorkdir && path !== defaultWorkdir && !projectRailExpandedPaths.includes(path)) {
            setProjectRailExpandedPaths((current) => [...current, path])
            void loadProjectRailChildren(path, { silent: true })
          }
        }}
        onOpenEntry={async (entry) => {
          if (entry.kind === 'folder') {
            handleToggleProjectRailFolder(entry)
            return
          }
          try {
            const preview = await readFilePreview(entry.path)
            openPreviewTab(preview)
            setProjectRailPath(entry.path.split(/[\\/]/).slice(0, -1).join('/') || railRootPath)
            showProjectRailNotice(null)
          } catch (err) {
            console.error('Failed to read file preview:', err)
            // 把 backend 的具体错因带出来，避免所有失败都吞成一句
            // "暂时不能预览"，让用户没法判断是文件类型不支持、还是
            // 体积超限、还是非 UTF-8 文本。
            const raw = err instanceof Error ? err.message : String(err)
            const friendly = raw.includes('too large')
              ? '文件太大，无法在面板内预览，已用「外部打开」更稳妥。'
              : raw.includes('not valid UTF-8')
                ? '这个文件不是文本，且不属于图片/视频/PDF；请用「外部打开」。'
                : '这个文件暂时不能在面板内预览。'
            showProjectRailNotice(friendly)
          }
        }}
        onToggleFolder={handleToggleProjectRailFolder}
        onOpenFolder={() => {
          if (!projectRailPath && !railRootPath) return
          void openDirectoryPath(projectRailPath ?? railRootPath!)
        }}
      />

    </div>
  )
}

const ProjectFilesRail = React.memo(function ProjectFilesRail({
  open,
  width,
  title,
  projectLabel,
  breadcrumbs,
  currentPath,
  entries,
  childEntries,
  expandedPaths,
  loadingPaths,
  isLoading,
  sortMode,
  onSortModeChange,
  previewError,
  onWidthChange,
  onNavigateUp,
  onJumpToBreadcrumb,
  onOpenEntry,
  onToggleFolder,
  onOpenFolder,
}: {
  open: boolean
  width: number
  title: string
  projectLabel: string
  breadcrumbs: RailBreadcrumb[]
  currentPath: string | null
  entries: DirectoryEntryPreview[]
  childEntries: RailTreeMap
  expandedPaths: string[]
  loadingPaths: string[]
  isLoading: boolean
  sortMode: 'recent' | 'name'
  onSortModeChange: React.Dispatch<React.SetStateAction<'recent' | 'name'>>
  previewError: string | null
  onWidthChange: React.Dispatch<React.SetStateAction<number>>
  onNavigateUp: () => void
  onJumpToBreadcrumb: (path: string) => void
  onOpenEntry: (entry: DirectoryEntryPreview) => void | Promise<void>
  onToggleFolder: (entry: DirectoryEntryPreview) => void
  onOpenFolder: () => void
}) {
  const startResize = React.useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    event.preventDefault()
    const startX = event.clientX
    const startWidth = width
    const separatorEl = event.currentTarget
    const pointerId = event.pointerId

    const handleMove = (moveEvent: PointerEvent) => {
      const delta = startX - moveEvent.clientX
      onWidthChange(Math.min(PROJECT_RAIL_MAX_WIDTH, Math.max(PROJECT_RAIL_MIN_WIDTH, startWidth + delta)))
    }

    const handleUp = () => {
      document.body.style.userSelect = ''
      document.body.style.cursor = ''
      window.removeEventListener('pointermove', handleMove)
      window.removeEventListener('pointerup', handleUp)
      try {
        separatorEl.releasePointerCapture(pointerId)
      } catch {
        // ignore pointer capture release failures
      }
    }

    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
    try {
      separatorEl.setPointerCapture(pointerId)
    } catch {
      // ignore pointer capture failures
    }
    window.addEventListener('pointermove', handleMove)
    window.addEventListener('pointerup', handleUp)
  }, [onWidthChange, width])

  return (
    <aside
      className={cn(
        'pointer-events-none absolute inset-y-2.5 right-2.5 z-20 overflow-hidden origin-right transition-[width,transform,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]',
        open ? 'translate-x-0 opacity-100' : 'translate-x-8 opacity-0'
      )}
      style={{ width: open ? `${width}px` : '0px' }}
      aria-hidden={!open}
    >
      <div
        className="pointer-events-auto absolute inset-y-5 -left-3 flex w-6 cursor-col-resize items-center justify-center"
        onPointerDown={startResize}
      >
        <div className="flex h-16 w-3 items-center justify-center rounded-full bg-surface-raised/82 shadow-[0_8px_20px_rgba(78,61,38,0.08)] backdrop-blur-sm">
          <GripVertical className="h-4 w-4 text-muted-foreground" />
        </div>
      </div>
      <div className="pointer-events-auto flex h-full flex-col justify-start">
        <div className="h-3 shrink-0" />
        <div className="min-h-0 flex-1 px-2 pb-2">
          <div className="flex h-full min-h-0 flex-col rounded-[18px] border border-border bg-surface shadow-xs">
            <div className="flex items-start justify-between gap-3 px-4 pt-4">
              <div className="min-w-0 flex-1 pr-2">
                <div
                  className="truncate text-[18px] font-medium tracking-[-0.03em] text-foreground"
                  style={{ fontFamily: '"Iowan Old Style", "Baskerville", ui-serif, Georgia, serif' }}
                >
                  {title}
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1 pt-0.5">
                <button
                  type="button"
                  onClick={onOpenFolder}
                  className="inline-flex h-7 items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.25 text-[10px] font-medium text-muted-foreground transition-all duration-200 hover:border-border hover:bg-accent hover:text-foreground"
                >
                  <Folder className="h-3.25 w-3.25 stroke-[1.85]" />
                  <span>打开文件夹</span>
                </button>
                <button
                  type="button"
                  className="inline-flex h-7 items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.25 text-[10px] font-medium text-muted-foreground transition-all duration-200 hover:border-border hover:bg-accent hover:text-foreground"
                >
                  <Sparkles className="h-3.25 w-3.25" />
                  <span>项目技能 · 2</span>
                </button>
              </div>
            </div>

            <div className="mt-2 flex items-center justify-between border-b border-border/60 px-4 pb-3">
              <div className="flex items-center gap-2">
                <div className="text-[11px] font-semibold tracking-[-0.01em] text-muted-foreground">技能</div>
                <div className="inline-flex min-w-7 items-center justify-center rounded-full bg-muted px-2 py-0.5 text-[10px] text-muted-foreground">
                  {entries.length}
                </div>
              </div>
              <button
                type="button"
                className="inline-flex h-6 w-6 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                <ChevronRight className="h-3.5 w-3.5" />
              </button>
            </div>

            <div className="flex min-h-0 flex-1 flex-col px-3 pb-3 pt-3">
              <div className="flex items-center justify-between gap-3 px-0.5">
                {breadcrumbs.length > 1 ? (
                  <div className="flex min-w-0 items-center gap-1.5 overflow-hidden rounded-[6px] bg-surface-raised px-2.5 py-1 text-[10.5px] text-muted-foreground">
                    <>
                      <button
                        type="button"
                        onClick={onNavigateUp}
                        className="inline-flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground/60 transition-colors hover:bg-muted hover:text-foreground"
                      >
                        <ChevronRight className="h-3 w-3 rotate-180" />
                      </button>
                      <div className="flex min-w-0 items-center overflow-hidden">
                        {breadcrumbs.map((crumb, index) => (
                          <React.Fragment key={crumb.path}>
                            {index > 0 ? <ChevronRight className="h-3 w-3 shrink-0 text-muted-foreground/60" /> : null}
                            <button
                              type="button"
                              onClick={() => onJumpToBreadcrumb(crumb.path)}
                              className={cn(
                                'min-w-0 shrink truncate rounded px-0.5 py-0.5 transition-colors hover:text-foreground',
                                index === breadcrumbs.length - 1 ? 'font-medium text-foreground' : 'text-muted-foreground'
                              )}
                            >
                              {crumb.label}
                            </button>
                          </React.Fragment>
                        ))}
                      </div>
                    </>
                  </div>
                ) : (
                  <div className="min-w-0 flex-1" />
                )}
                <button
                  type="button"
                  onClick={() => onSortModeChange((current) => current === 'recent' ? 'name' : 'recent')}
                  className="inline-flex shrink-0 items-center gap-1 rounded-full px-1.5 py-1 text-[11px] text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground"
                >
                  {sortMode === 'recent' ? (
                    <Clock3 className="h-3.25 w-3.25" />
                  ) : (
                    <ArrowUpDown className="h-3.25 w-3.25" />
                  )}
                  <span>{sortMode === 'recent' ? '时间' : '名称'}</span>
                </button>
              </div>

              <div className="mt-2 min-h-0 flex-1 overflow-hidden bg-transparent">
                {previewError ? (
                  <div className="mb-2 rounded-[6px] border border-border bg-muted px-3 py-2 text-[11px] text-muted-foreground">
                    {previewError}
                  </div>
                ) : null}
                {isLoading ? (
                  <div className="flex items-center gap-2 px-1 py-3 text-[11.5px] text-muted-foreground">
                    <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
                    <span>正在整理当前项目文件…</span>
                  </div>
                ) : entries.length === 0 ? (
                  <div className="px-1 py-5 text-center text-[11.5px] text-muted-foreground">
                    当前目录里还没有可显示的文件。
                  </div>
                ) : (
                  <div className="h-full min-h-0 overflow-y-auto px-0 py-0 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                    <div className="space-y-0 pb-1">
                      {entries.map((entry) => (
                        <ProjectRailTreeNode
                          key={entry.path}
                          entry={entry}
                          level={0}
                          selectedPath={currentPath}
                          childEntries={childEntries}
                          expandedPaths={expandedPaths}
                          loadingPaths={loadingPaths}
                          onOpen={onOpenEntry}
                          onToggleFolder={onToggleFolder}
                        />
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
    </aside>
  )
})

function ProjectRailGroup({
  title,
  count,
  children,
}: {
  title: string
  count: number
  children: React.ReactNode
}) {
  return (
    <section className="mb-2">
      <div className="mb-1.5 flex items-center justify-between px-1">
        <div className="text-[10px] uppercase tracking-[0.22em] text-muted-foreground/50">{title}</div>
        <div className="text-[10px] text-muted-foreground/40">{count}</div>
      </div>
      <div className="space-y-0.5">{children}</div>
    </section>
  )
}

function ProjectRailFilePreview({
  preview,
  onBack,
  onOpenExternally,
  onQuoteIntoChat,
  onInsertIntoChat,
}: {
  preview: FilePreviewPayload
  onBack: () => void
  onOpenExternally: (preview: FilePreviewPayload) => void
  onQuoteIntoChat: (preview: FilePreviewPayload) => void
  onInsertIntoChat: (preview: FilePreviewPayload) => void
}) {
  const codeFence = React.useMemo(() => {
    const raw = preview.content ?? ''
    const language = inferPreviewLanguage(preview.name, preview.kind)
    return `\`\`\`${language}\n${raw}\n\`\`\``
  }, [preview.content, preview.kind, preview.name])
  const dataUrl = React.useMemo(() => {
    if (!preview.data_base64 || !preview.mime_type) return null
    return `data:${preview.mime_type};base64,${preview.data_base64}`
  }, [preview.data_base64, preview.mime_type])

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center justify-between border-b border-border/60 px-1 pb-2.5">
        <button
          type="button"
          onClick={onBack}
          className="inline-flex items-center gap-1 rounded-full bg-muted px-2.5 py-1 text-[11px] text-muted-foreground transition-colors hover:bg-muted/80"
        >
          <ChevronRight className="h-3 w-3 rotate-180" />
          <span>返回目录</span>
        </button>
        <div className="min-w-0 truncate pl-3 text-[11px] text-muted-foreground">{preview.name}</div>
      </div>
      <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
        <button
          type="button"
          onClick={() => onOpenExternally(preview)}
          className="inline-flex items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.5 py-1.5 text-[11px] text-muted-foreground transition-all duration-150 hover:-translate-y-0.5 hover:bg-surface"
        >
          <ArrowUpRight className="h-3.5 w-3.5" />
          <span>在外部打开</span>
        </button>
        <button
          type="button"
          onClick={() => onQuoteIntoChat(preview)}
          className="inline-flex items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.5 py-1.5 text-[11px] text-muted-foreground transition-all duration-150 hover:-translate-y-0.5 hover:bg-surface"
        >
          <Quote className="h-3.5 w-3.5" />
          <span>在聊天中引用</span>
        </button>
        <button
          type="button"
          draggable
          onClick={() => onInsertIntoChat(preview)}
          onDragStart={(event) => {
            const payload = `请把 \`${preview.path}\` 作为当前上下文文件一起考虑。`
            event.dataTransfer.setData('text/plain', payload)
            event.dataTransfer.effectAllowed = 'copy'
          }}
          className="inline-flex items-center gap-1.5 rounded-[6px] border border-border bg-surface-raised px-2.5 py-1.5 text-[11px] text-muted-foreground transition-all duration-150 hover:-translate-y-0.5 hover:bg-surface"
        >
          <ScanSearch className="h-3.5 w-3.5" />
          <span>拖入上下文</span>
        </button>
      </div>
      <div className="mt-3 min-h-0 overflow-auto rounded-[6px] border border-border bg-surface/88">
        {preview.kind === 'image' && dataUrl ? (
          <div className="flex min-h-full items-start justify-center p-4">
            <img src={dataUrl} alt={preview.name} className="max-h-full max-w-full rounded-[6px] object-contain shadow-xs" />
          </div>
        ) : preview.kind === 'pdf' && dataUrl ? (
          <iframe title={preview.name} src={dataUrl} className="h-full min-h-[520px] w-full rounded-[6px]" />
        ) : preview.kind === 'markdown' ? (
          <div className="px-4 py-4 text-[12px] leading-5.5 text-foreground/70">
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {preview.content ?? ''}
            </ReactMarkdown>
          </div>
        ) : (
          <div className="px-4 py-4 text-[11px] leading-5.5 text-foreground/68">
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {codeFence}
            </ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  )
}

const ProjectRailTreeNode = React.memo(function ProjectRailTreeNode({
  entry,
  level,
  selectedPath,
  childEntries,
  expandedPaths,
  loadingPaths,
  onOpen,
  onToggleFolder,
}: {
  entry: DirectoryEntryPreview
  level: number
  selectedPath: string | null
  childEntries: RailTreeMap
  expandedPaths: string[]
  loadingPaths: string[]
  onOpen: (entry: DirectoryEntryPreview) => void | Promise<void>
  onToggleFolder: (entry: DirectoryEntryPreview) => void
}) {
  const isFolder = entry.kind === 'folder'
  const isExpanded = expandedPaths.includes(entry.path)
  const isLoading = loadingPaths.includes(entry.path)
  const children = childEntries[entry.path] ?? []
  const Icon = isFolder ? (isExpanded ? FolderOpen : Folder) : File
  const isSelected = selectedPath === entry.path

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        draggable
        onClick={() => {
          if (entry.kind === 'folder') {
            void onOpen(entry)
          }
        }}
        onDoubleClick={() => {
          if (entry.kind === 'file') {
            void onOpen(entry)
          }
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            void onOpen(entry)
          }
        }}
        onDragStart={(event) => {
          const payload = JSON.stringify({
            path: entry.path,
            name: entry.name,
            kind: entry.kind,
          })
          event.dataTransfer.setData('application/x-if2ai-rail-entry', payload)
          event.dataTransfer.setData('text/plain', entry.path)
          event.dataTransfer.effectAllowed = 'copy'
        }}
        className={cn(
          'group flex w-full cursor-pointer items-center gap-2 rounded-[6px] px-0.5 py-1 text-left text-foreground/80 transition-all duration-150 hover:bg-muted',
          isSelected && 'bg-accent',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
        )}
        style={{ paddingLeft: `${2 + level * 12}px` }}
      >
        <div className="flex h-4 w-4 shrink-0 items-center justify-center text-muted-foreground/60">
          {isFolder ? (
            <span className="inline-flex h-4 w-4 items-center justify-center text-muted-foreground/60">
              <ChevronRight className={cn('h-3.5 w-3.5 transition-transform duration-150', isExpanded && 'rotate-90')} />
            </span>
          ) : (
            <span className="h-3.5 w-3.5" />
          )}
        </div>
        <div className="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground transition-transform duration-150 group-hover:-translate-y-0.5">
          <Icon className="h-[15px] w-[15px] stroke-[1.65]" />
        </div>
        <div className="min-w-0 flex-1 truncate text-[12px] font-[380] tracking-[-0.01em] text-foreground/75 [font-feature-settings:'ss01'_1,'cv01'_1]">
          {entry.name}
        </div>
        {isLoading ? (
          <LoaderCircle className="h-3.5 w-3.5 shrink-0 animate-spin text-muted-foreground/60" />
        ) : isFolder ? (
          <div className="text-[9.5px] uppercase tracking-[0.16em] text-muted-foreground/40">{children.length > 0 ? '' : ''}</div>
        ) : (
          <ChevronRight className="h-3.5 w-3.5 shrink-0 text-muted-foreground/40 opacity-0 transition-all duration-150 group-hover:translate-x-0.5 group-hover:opacity-100" />
        )}
      </div>

      {isFolder && isExpanded ? (
        <div>
          <div className="space-y-0 pt-0.5">
            {children.length > 0 ? (
              children.map((child) => (
                <ProjectRailTreeNode
                  key={child.path}
                  entry={child}
                  level={level + 1}
                  selectedPath={selectedPath}
                  childEntries={childEntries}
                  expandedPaths={expandedPaths}
                  loadingPaths={loadingPaths}
                  onOpen={onOpen}
                  onToggleFolder={onToggleFolder}
                />
              ))
            ) : !isLoading ? (
              <div className="px-2 py-1 text-[10px] italic text-muted-foreground/50" style={{ paddingLeft: `${16 + level * 12}px` }}>
                这个文件夹目前是空的
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  )
})

function inferPreviewLanguage(fileName: string, kind?: FilePreviewPayload['kind']) {
  const ext = fileName.split('.').pop()?.toLowerCase() ?? ''
  if (kind === 'markdown' || ext === 'md' || ext === 'markdown') return 'markdown'
  if (ext === 'ts' || ext === 'tsx' || ext === 'js' || ext === 'jsx') return ext
  if (ext === 'rs' || ext === 'py' || ext === 'json' || ext === 'css' || ext === 'html' || ext === 'sh' || ext === 'md' || ext === 'sql' || ext === 'yaml' || ext === 'yml') return ext
  return 'text'
}

function sortRailDirectoryEntries(entries: DirectoryEntryPreview[], sortMode: 'recent' | 'name') {
  const next = [...entries]
  next.sort((a, b) => {
    if (a.kind !== b.kind) {
      return a.kind === 'folder' ? -1 : 1
    }
    if (sortMode === 'recent') {
      const aTime = a.modified_ms ?? 0
      const bTime = b.modified_ms ?? 0
      if (aTime !== bTime) return bTime - aTime
    }
    return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
  })
  return next
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
  densityMode,
  fontMode,
  isLeftPaneCollapsed,
  contentRightInset,
  contentMaxWidth,
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
  densityMode: DensityMode
  fontMode: FontMode
  isLeftPaneCollapsed: boolean
  contentRightInset: number
  contentMaxWidth: number
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
      className="relative min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-x-none"
      style={{ overscrollBehaviorX: 'none', paddingRight: `${contentRightInset}px` }}
    >
      <div
        className="mx-auto flex w-full flex-col gap-4 px-10 pt-6 transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
        style={{ paddingBottom: `${bottomPadding}px`, maxWidth: `${contentMaxWidth}px` }}
      >
        {messages.length === 0 ? (
          <EmptyState sessionTitle={sessionTitle} projectLabel={projectLabel} />
        ) : messages.length >= VIRTUAL_LIST_THRESHOLD ? (
          <VirtualMessageList
            messages={messages}
            scrollElementRef={scrollRef}
            estimatedItemSize={84}
            renderMessage={(msg) => (
              <div className={densityMode === 'compact' ? 'pb-2' : 'pb-3'}>
                <ChatMessage
                  message={msg}
                  onCopyMessage={onCopyMessage}
                  onResumeFromCursor={onResumeFromCursor}
                  isCopied={copiedMessageId === msg.id}
                  defaultWorkdir={defaultWorkdir}
                  isPrimaryThinkingMessage={primaryThinkingMessageIds.has(msg.id)}
                  densityMode={densityMode}
                  fontMode={fontMode}
                />
              </div>
            )}
          />
        ) : (
          <div className={densityMode === 'compact' ? 'space-y-2' : 'space-y-3'}>
            {messages.map((msg) => (
              <ChatMessage
                key={msg.id}
                message={msg}
                onCopyMessage={onCopyMessage}
                onResumeFromCursor={onResumeFromCursor}
                isCopied={copiedMessageId === msg.id}
                defaultWorkdir={defaultWorkdir}
                isPrimaryThinkingMessage={primaryThinkingMessageIds.has(msg.id)}
                densityMode={densityMode}
                fontMode={fontMode}
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
  prev.densityMode === next.densityMode &&
  prev.fontMode === next.fontMode &&
  prev.isLeftPaneCollapsed === next.isLeftPaneCollapsed &&
  prev.contentRightInset === next.contentRightInset &&
  prev.contentMaxWidth === next.contentMaxWidth &&
  prev.onCopyMessage === next.onCopyMessage &&
  prev.onResumeFromCursor === next.onResumeFromCursor
)

const ComposerDock = React.memo(function ComposerDock({
  input,
  onInputChange,
  onSubmit,
  onStop,
  isLoading,
  selectedModel,
  setSelectedModel,
  availableModelItems,
  permissionMode,
  setPermissionMode,
  selectedStrength,
  setSelectedStrength,
  branchLabel,
  onBranchChange,
  isGitRepo,
  onGitRepoChanged,
  isComposerFocused,
  setIsComposerFocused,
  isLeftPaneCollapsed,
  contentRightInset,
  contentMaxWidth,
  textareaRef,
  handleKeyDown,
  setSlashOverlay,
  slashOverlayState,
  onSlashSelect,
  slashTimerRef,
  atOverlayState,
  setAtOverlay,
  onAtSelect,
  onNavigateIntoFolder,
  onNavigateUpFolder,
  atTimerRef,
  defaultWorkdir,
  dropItems,
  onRemoveDropItem,
  onFileReferenceDrop,
  projectLabel,
  workdirLabel,
  onProjectPillClick,
}: {
  input: string
  onInputChange: (value: string) => void
  onSubmit: () => void
  onStop?: () => void
  isLoading?: boolean
  selectedModel: string
  setSelectedModel: React.Dispatch<React.SetStateAction<string>>
  availableModelItems: Array<{ value: string; label: string }>
  permissionMode: PermissionMode
  setPermissionMode: React.Dispatch<React.SetStateAction<PermissionMode>>
  selectedStrength: string
  setSelectedStrength: React.Dispatch<React.SetStateAction<string>>
  branchLabel: string
  onBranchChange?: (newBranch: string) => void
  isGitRepo?: boolean | null
  onGitRepoChanged?: () => void
  isComposerFocused: boolean
  setIsComposerFocused: React.Dispatch<React.SetStateAction<boolean>>
  isLeftPaneCollapsed: boolean
  contentRightInset: number
  contentMaxWidth: number
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
  /** Current slash overlay state (read) – used to render inline suggestions */
  slashOverlayState?: {
    visible: boolean
    selectedIndex: number
    suggestions: string[]
    rawInput: string
  } | null
  /** Called when the user selects a slash suggestion */
  onSlashSelect?: (value: string) => void
  slashTimerRef: React.RefObject<ReturnType<typeof setTimeout> | null>
  /** Current @-mention overlay state */
  atOverlayState?: {
    visible: boolean
    selectedIndex: number
    entries: DirectoryEntryPreview[]
    query: string
    atPos: number
    browsePath: string | null
    breadcrumbs: Array<{ name: string; path: string | null }>
    pinned: boolean
  } | null
  setAtOverlay: React.Dispatch<
    React.SetStateAction<{
      visible: boolean
      selectedIndex: number
      entries: DirectoryEntryPreview[]
      query: string
      atPos: number
      browsePath: string | null
      breadcrumbs: Array<{ name: string; path: string | null }>
      pinned: boolean
    } | null>
  >
  /** Called when the user picks an @-mention file/folder */
  onAtSelect?: (entry: DirectoryEntryPreview, overlay: { atPos: number; query: string }) => void
  /** Called when the user navigates into a folder in the @-mention overlay */
  onNavigateIntoFolder?: (entry: DirectoryEntryPreview) => Promise<void>
  /** Called when the user presses back in the @-mention overlay */
  onNavigateUpFolder?: () => Promise<void>
  atTimerRef: React.RefObject<ReturnType<typeof setTimeout> | null>
  /** Working directory used for @-mention file lookups */
  defaultWorkdir?: string
  dropItems: ComposerDropItem[]
  onRemoveDropItem: (id: string) => void
  onFileReferenceDrop: (item: ComposerDropItem) => void
  /** Project context metadata shown in the bottom context bar. */
  projectLabel?: string
  workdirLabel?: string
  onProjectPillClick?: () => void
}) {
  const [isDropTarget, setIsDropTarget] = React.useState(false)

  // ── IME composition guard ────────────────────────────────────────────────
  // On macOS/Electron, pressing Enter to confirm an IME candidate fires
  // `keydown` *before* `compositionend`, and `isComposing` is already `false`
  // at that moment, so `e.nativeEvent.isComposing` alone is insufficient.
  // We track composition state with a ref and delay clearing it by one tick so
  // that the confirming Enter keydown is still intercepted.
  const isComposingRef = React.useRef(false)
  const guardedHandleKeyDown = React.useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (isComposingRef.current) return
      void handleKeyDown(e)
    },
    [handleKeyDown]
  )
  const handleInputWithSlashDetect = (value: string) => {
    // Clear pending timers
    if (slashTimerRef.current) {
      clearTimeout(slashTimerRef.current)
      slashTimerRef.current = null
    }
    if (atTimerRef.current) {
      clearTimeout(atTimerRef.current)
      atTimerRef.current = null
    }

    onInputChange(value)

    // ── Slash command detection ──────────────────────────────────────────────
    if (value.startsWith('/')) {
      const cmdPart = value.split(/\s+/)[0]
      // Dismiss once the user has typed a space (started the instruction body).
      if (value.includes(' ')) {
        setSlashOverlay(null)
      } else {
        slashTimerRef.current = setTimeout(async () => {
          try {
            const { suggestSlashCommands } = await import('@/lib/tauri')
            const suggestions = await suggestSlashCommands(cmdPart, 8)
            if (suggestions.length > 0) {
              setSlashOverlay({ visible: true, selectedIndex: 0, suggestions, rawInput: cmdPart })
            } else {
              setSlashOverlay(null)
            }
          } catch {
            setSlashOverlay(null)
          }
        }, 50)
      }
    } else {
      setSlashOverlay(null)
    }

    // ── @-mention detection ──────────────────────────────────────────────────
    // When in pinned browse mode the overlay is independent of the textarea.
    if (atOverlayState?.pinned) return

    // Find the last @ that is followed by non-space text at the end of the value.
    const atIndex = value.lastIndexOf('@')
    if (atIndex >= 0 && defaultWorkdir) {
      // Ignore when @ is immediately preceded by a word character (letter / digit / . / - / +)
      // — that pattern indicates an e-mail address (e.g. user@example.com), not a file mention.
      const charBefore = atIndex > 0 ? value[atIndex - 1] : ''
      if (/[\w.\-+]/.test(charBefore)) {
        setAtOverlay(null)
        return
      }

      const textAfterAt = value.slice(atIndex + 1)
      // Only activate when no space follows the @ (i.e. the user is still typing the query)
      if (!textAfterAt.includes(' ') && !textAfterAt.includes('\n')) {
        const query = textAfterAt
        atTimerRef.current = setTimeout(async () => {
          try {
            const entries = await listDirectoryPreview(defaultWorkdir, 48)
            const filtered = query
              ? entries.filter((e) => e.name.toLowerCase().includes(query.toLowerCase()))
              : entries
            const limited = filtered.slice(0, 8)
            if (limited.length > 0) {
              setAtOverlay({ visible: true, selectedIndex: 0, entries: limited, query, atPos: atIndex, browsePath: null, breadcrumbs: [], pinned: false })
            } else {
              setAtOverlay(null)
            }
          } catch {
            setAtOverlay(null)
          }
        }, 60)
      } else {
        setAtOverlay(null)
      }
    } else {
      setAtOverlay(null)
    }
  }
  return (
    <div
      className="relative z-10 shrink-0 px-10 pb-2.5 pt-0 transition-[padding-right] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
      style={{ paddingRight: `${40 + contentRightInset}px` }}
    >
      <div className="relative mx-auto flex w-full flex-col transition-[max-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]" style={{ maxWidth: `${contentMaxWidth}px` }}>
        {/* Slash command overlay – anchored to composer top edge */}
        {slashOverlayState?.visible && onSlashSelect && (
          <SlashCommandSuggestions
            suggestions={slashOverlayState.suggestions}
            selectedIndex={slashOverlayState.selectedIndex}
            rawInput={slashOverlayState.rawInput}
            onSelect={onSlashSelect}
          />
        )}

        {/* @-mention file overlay – anchored to composer top edge */}
        {atOverlayState?.visible && onAtSelect && (
          <AtFileSuggestions
            entries={atOverlayState.entries}
            selectedIndex={atOverlayState.selectedIndex}
            query={atOverlayState.query}
            browsePath={atOverlayState.browsePath}
            breadcrumbs={atOverlayState.breadcrumbs}
            onSelect={(entry) => onAtSelect(entry, { atPos: atOverlayState.atPos, query: atOverlayState.query })}
            onNavigateInto={onNavigateIntoFolder}
            onNavigateUp={onNavigateUpFolder}
          />
        )}

        {dropItems.length > 0 ? (
          <div className="mb-3 flex flex-wrap gap-1.5 px-1">
            {dropItems.map((item) => {
              if (item.isMention) {
                const isFolder = item.kind === 'folder'
                return (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => onRemoveDropItem(item.id)}
                    className="group inline-flex max-w-full items-center gap-1.5 rounded-lg border border-jade/20 bg-jade/[0.07] px-2.5 py-1 text-left text-jade/80 transition-all duration-200 hover:border-jade/30 hover:bg-jade/[0.12] active:scale-[0.97]"
                    title={item.path}
                  >
                    {isFolder ? (
                      /* Folder badge — slightly deeper tint + folder icon */
                      <span className="flex size-[18px] shrink-0 items-center justify-center rounded-[4px] bg-jade/[0.18] text-jade">
                        <Folder className="size-2.5" />
                      </span>
                    ) : (
                      /* File badge — @ symbol */
                      <span className="flex size-[18px] shrink-0 items-center justify-center rounded-[4px] bg-jade/[0.14] text-[10px] font-bold text-jade">
                        @
                      </span>
                    )}
                    <span className="truncate text-[12px] font-semibold tracking-tight">
                      {item.name}{isFolder ? <span className="opacity-40">/</span> : null}
                    </span>
                    <span className="flex size-[14px] shrink-0 items-center justify-center rounded-full opacity-40 transition-opacity group-hover:opacity-70">
                      <X className="h-2.5 w-2.5" />
                    </span>
                  </button>
                )
              }
              // Regular drag-drop chip: neutral
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => onRemoveDropItem(item.id)}
                  className="group inline-flex max-w-full items-center gap-2 rounded-[6px] border border-border/50 bg-surface-raised px-3 py-2 text-left text-foreground/60 shadow-xs transition-all duration-200 hover:-translate-y-0.5 hover:bg-surface"
                  title={item.path}
                >
                  <span className="flex h-5 w-5 shrink-0 items-center justify-center text-muted-foreground/60">
                    {item.kind === 'file' ? <Paperclip className="h-4 w-4" /> : <Folder className="h-4 w-4" />}
                  </span>
                  <span className="truncate text-[12.5px] font-medium tracking-[-0.015em]">{item.name}</span>
                  <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full text-muted-foreground/40 transition-colors group-hover:text-muted-foreground">
                    <X className="h-3.5 w-3.5" />
                  </span>
                </button>
              )
            })}
          </div>
        ) : null}

        {/* ── Composer box ── */}
        <div
          className="flex flex-col rounded-2xl bg-card px-4 pt-2 pb-0 transition-all duration-200"
          style={{
            border: `1px solid ${isComposerFocused ? 'rgba(0,0,0,0.13)' : 'rgba(0,0,0,0.08)'}`,
            boxShadow: isComposerFocused
              ? '0 4px 6px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.09), 0 20px 48px rgba(0,0,0,0.06), 0 0 0 0.5px rgba(0,0,0,0.05)'
              : '0 2px 16px rgba(0,0,0,0.07), 0 0 0 0.5px rgba(0,0,0,0.04)',
            transform: isComposerFocused ? 'translateY(-1px)' : 'translateY(0)',
          }}
        >
          {/* Textarea (align top) */}
          <div className="flex-1 px-0 pt-1 pb-0.5">
            <Textarea
              ref={textareaRef}
              data-composer-input="true"
              value={input}
              onChange={(e) => handleInputWithSlashDetect(e.target.value)}
              onCompositionStart={() => { isComposingRef.current = true }}
              onCompositionEnd={() => {
                // Defer by one tick: the Enter that triggered compositionend fires
                // its keydown *before* this event on macOS, so delaying ensures the
                // guard is still active when that keydown is processed.
                setTimeout(() => { isComposingRef.current = false }, 0)
              }}
              onKeyDown={guardedHandleKeyDown}
              onDragOver={(event) => {
                event.preventDefault()
                event.dataTransfer.dropEffect = 'copy'
                setIsDropTarget(true)
              }}
              onDragLeave={() => setIsDropTarget(false)}
              onDrop={(event) => {
                event.preventDefault()
                const structuredPayload = event.dataTransfer.getData('application/x-if2ai-rail-entry').trim()
                if (structuredPayload) {
                  try {
                    const parsed = JSON.parse(structuredPayload) as { path: string; name: string; kind: 'file' | 'folder' }
                    if (parsed.path && parsed.name && (parsed.kind === 'file' || parsed.kind === 'folder')) {
                      onFileReferenceDrop({
                        id: `${parsed.kind}:${parsed.path}`,
                        path: parsed.path,
                        name: parsed.name,
                        kind: parsed.kind,
                      })
                    }
                  } catch (error) {
                    console.error('Failed to parse dropped rail entry:', error)
                  }
                }
                setIsDropTarget(false)
              }}
              disabled={isLoading}
              rows={1}
              onFocus={() => setIsComposerFocused(true)}
              onBlur={() => setIsComposerFocused(false)}
              className="min-h-[46px] max-h-[180px] resize-none rounded-none border-0 bg-transparent px-0 py-0 text-[13px] leading-6 shadow-none focus-visible:ring-0 placeholder:text-muted-foreground disabled:bg-transparent disabled:opacity-100"
              placeholder="向 AI 提问，@ 添加文件，/ 输入命令，$ 使用技能"
            />
          </div>

          {/* Bottom bar — mirrors Home composer: [+ | permission] … [model | | mic | send] */}
          {/* -mx-4 px-4 lets the border-t bleed to the card edges while keeping content aligned */}
          <div className="flex items-center justify-between border-t border-black/[0.06] -mx-4 px-4 pt-2 pb-2">
            {/* Left: attach + permission */}
            <div className="flex items-center gap-1">
              <button
                type="button"
                className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-black/40 transition-colors hover:bg-black/[0.05] hover:text-black/65"
                aria-label="添加附件"
                disabled={isLoading}
              >
                <Plus className="h-[15px] w-[15px]" />
              </button>

              {/* Permission mode */}
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button
                    type="button"
                    className="flex items-center gap-1 rounded-lg px-2 py-1 text-[12px] text-black/45 transition-colors hover:bg-black/[0.05] hover:text-black/65"
                    disabled={isLoading}
                  >
                    {permissionModeLabelFor(permissionMode)}
                    <ChevronDown className="h-3 w-3" />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent sideOffset={6} align="start" className="w-[148px]">
                  {permissionModeItems.map((item) => (
                    <MenuItemButton
                      key={item.value}
                      active={permissionMode === item.value}
                      onClick={() => setPermissionMode(item.value)}
                    >
                      {item.label}
                    </MenuItemButton>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            </div>

            {/* Right: model | divider | mic | send */}
            <div className="flex items-center gap-1.5">
              {/* Model selector */}
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button
                    type="button"
                    className="flex items-center gap-1 rounded-lg px-2 py-1 text-[12px] text-black/45 transition-colors hover:bg-black/[0.05] hover:text-black/65"
                    disabled={isLoading}
                  >
                    {availableModelItems.length > 0
                      ? (availableModelItems.find((i) => i.value === selectedModel)?.label ??
                         availableModelItems[0]?.label ??
                         modelLabelFor(selectedModel))
                      : (selectedModel ? modelLabelFor(selectedModel) : '选择模型')}
                    <ChevronDown className="h-3 w-3" />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent sideOffset={6} align="end" className="w-[220px]">
                  {(availableModelItems.length > 0 ? availableModelItems : modelItems).map((item) => (
                    <MenuItemButton
                      key={item.value}
                      onClick={() => {
                        setSelectedModel(item.value)
                        if (availableModelItems.length > 0) {
                          const parts = item.value.split('/')
                          if (parts.length === 2) {
                            void invoke('model_set_active', { providerId: parts[0], modelId: parts[1] })
                          }
                        }
                      }}
                      active={selectedModel === item.value}
                    >
                      {item.label}
                    </MenuItemButton>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>

              <div className="h-3.5 w-px bg-black/10" />

              {/* 语音输入按钮（SenseVoice STT） */}
              <React.Suspense fallback={null}>
                <SttButtonLazy
                  onTranscribe={(text) => {
                    // 把转写结果追加到现有 input 末尾
                    const next = input ? `${input.trim()} ${text}` : text
                    onInputChange(next)
                  }}
                />
              </React.Suspense>

              {/* Send / Stop */}
              <button
                type="button"
                onClick={() => {
                  if (isLoading && onStop) {
                    onStop()
                  } else if (!isLoading) {
                    onSubmit()
                  }
                }}
                disabled={!isLoading && !input.trim()}
                className={cn(
                  'flex h-7 w-7 shrink-0 items-center justify-center rounded-full transition-all duration-150 active:scale-95',
                  isLoading
                    ? 'bg-destructive text-white hover:opacity-90'
                    : input.trim()
                      ? 'bg-black text-white hover:bg-black/80'
                      : 'bg-black/10 text-black/30 cursor-not-allowed'
                )}
                aria-label={isLoading ? '停止生成' : '发送消息'}
              >
                {isLoading ? (
                  <Square className="h-3 w-3 fill-current" />
                ) : (
                  <ArrowUp className="h-[14px] w-[14px]" />
                )}
              </button>
            </div>
          </div>

          {/* Drop target overlay */}
          {isDropTarget && (
            <div className="pointer-events-none absolute inset-x-4 top-3 rounded-xl border border-dashed border-border bg-surface/88 px-4 py-3 text-[12px] text-muted-foreground backdrop-blur-sm">
              文件会作为附件发送，文件夹会作为引用附加到消息里。
            </div>
          )}
        </div>

        {/* ── Project context bar (project | workdir | branch) ── */}
        {(projectLabel || workdirLabel) && (
          <div className="mt-1.5 flex items-center gap-2 px-1 pb-0.5">
            {projectLabel && (
              <button
                type="button"
                onClick={onProjectPillClick}
                className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[11px] font-medium text-black/40 transition-colors hover:bg-black/[0.04] hover:text-black/60"
              >
                <FolderOpen className="h-[11px] w-[11px]" />
                {projectLabel}
              </button>
            )}
            {workdirLabel && (
              <span className="flex items-center gap-1 text-[11px] text-black/30">
                <Laptop className="h-[11px] w-[11px]" />
                {workdirLabel}
              </span>
            )}
            <BranchPicker
              cwd={defaultWorkdir}
              currentBranch={branchLabel}
              onChanged={onBranchChange}
              isGitRepo={isGitRepo ?? null}
              onInitRepo={onGitRepoChanged}
            />
          </div>
        )}
      </div>
    </div>
  )
})

const WEB_SEARCH_NO_KEY_PREFIX = '[web_search: 当前使用 DuckDuckGo 免费搜索'

function MemoryStoreToolCard({ message }: { message: Message }) {
  const args = (message.toolArgs ?? {}) as Record<string, unknown>
  // The user-supplied content is the most accurate preview; the backend
  // only echoes a 120-char content_preview for prompt decisions, and not at
  // all for allow / deny.
  const argsContent =
    typeof args.content === 'string'
      ? (args.content as string)
      : typeof args.text === 'string'
        ? (args.text as string)
        : ''
  // Backend `pending_approval` payload includes a short preview when the
  // input args aren't reachable (rare, but parses defensively).
  let backendPreview = ''
  try {
    const parsed = JSON.parse(message.content || '') as Record<string, unknown>
    if (typeof parsed.content_preview === 'string') {
      backendPreview = parsed.content_preview
    }
  } catch {
    // Not a JSON payload (legacy tool flow); fall through.
  }
  const contentText = argsContent || backendPreview || message.content || ''

  // Prefer structured fields lifted from the tool result by App.tsx; fall
  // back to args (legacy) and finally to derived defaults.
  const scopeFromArgs =
    typeof args.scope === 'string' &&
    (args.scope === 'global' || args.scope === 'project' || args.scope === 'session')
      ? (args.scope as 'global' | 'project' | 'session')
      : undefined
  const scope: 'global' | 'project' | 'session' =
    message.memoryScope ?? scopeFromArgs ?? 'session'

  const decision = message.policyDecision ?? 'allow'
  const reasonCode =
    message.memoryReasonCode ??
    (typeof args.reason_code === 'string'
      ? (args.reason_code as string)
      : decision === 'deny'
        ? 'POLICY_DENIED'
        : decision === 'prompt'
          ? 'USER_APPROVAL_REQUIRED'
          : 'ALLOWED_BY_POLICY')

  return (
    <div className="my-1.5 pl-4">
      <MemoryWriteCard
        content={contentText}
        policyDecision={decision}
        reasonCode={reasonCode}
        scope={scope}
        toolStatus={message.toolStatus}
        isStreaming={message.toolStatus === 'queued' || message.toolStatus === 'running'}
      />
    </div>
  )
}

function ToolCallMessage({
  message,
  defaultWorkdir,
}: {
  message: Message
  defaultWorkdir?: string
}) {
  const [expanded, setExpanded] = React.useState(false)
  const [isHovered, setIsHovered] = React.useState(false)

  // Specialized memory_store renderer — only used when the policy engine
  // produces a `deny` or `prompt` decision so the user notices it.  The
  // common `allow` path falls through to the generic tool-call card so
  // routine memory writes blend into the tool history like any other tool.
  if (
    message.toolName === 'memory_store' &&
    (message.policyDecision === 'deny' || message.policyDecision === 'prompt')
  ) {
    return <MemoryStoreToolCard message={message} />
  }
  const status = normalizeToolStatus(message)
  const display = buildToolCallDisplay(message, defaultWorkdir, status)
  const title = status === 'error' ? `执行失败：${display.title}` : display.title
  const hasDetails = display.details.length > 0
  const ToolGlyph = getToolCallGlyph(message.toolName, message.toolArgs)

  // Detect web_search no-key notice injected by backend.
  const showNoKeyBanner =
    message.toolName === 'web_search' &&
    typeof message.content === 'string' &&
    message.content.includes(WEB_SEARCH_NO_KEY_PREFIX)

  // file_write 专用 diff 预览：toolArgs.path + toolArgs.content 都齐
  // 时，展开后用 WriteToolDiffCard 渲染（图里那张绿色行号 +/- 卡片）。
  // 真实的 `previous_content` 由 backend 在结构化 JSON 结果里下发；
  // 解析失败 / 旧内容超出 64KiB 上限被裁掉时，回落为"全新增"渲染。
  // status === 'error' 时跳过整张卡片，避免把失败请求的 content 当作
  // "已写入"来展示。
  const writeArgs = (message.toolArgs ?? {}) as Record<string, unknown>
  const writePath = typeof writeArgs.path === 'string' ? writeArgs.path : ''
  const writeContent = typeof writeArgs.content === 'string' ? writeArgs.content : ''
  const writePreviousContent = React.useMemo(() => {
    if (typeof message.content !== 'string' || !message.content.trim()) return ''
    try {
      const parsed = JSON.parse(message.content) as Record<string, unknown>
      if (parsed && parsed.kind === 'file_write' && typeof parsed.previous_content === 'string') {
        return parsed.previous_content
      }
    } catch {
      // 旧版 backend 仍可能返回纯文本 "Successfully wrote to file: ..."；
      // 此时 previous_content 不可得，按全新增渲染。
    }
    return ''
  }, [message.content])
  const showWriteDiff =
    typeof message.toolName === 'string' &&
    message.toolName.includes('file_write') &&
    status !== 'error' &&
    Boolean(writePath) &&
    typeof writeArgs.content === 'string'

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
        <div className={cn('absolute bottom-0 left-[5px] top-0 w-px bg-border/40', status === 'error' && 'bg-rose-200/80')} />

        <div className="flex items-start gap-1.5">
          <div className={cn('mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-muted-foreground/30', status === 'error' && 'bg-rose-400')} />

          <button
            type="button"
            onClick={() => setExpanded((value) => !value)}
            className={cn(
              'group flex min-w-0 flex-1 items-center gap-2 rounded-none px-0 py-[2px] text-left transition-colors duration-150',
              status === 'error' ? 'hover:text-rose-600/90' : 'hover:text-foreground/70'
            )}
            aria-label={expanded ? '折叠工具调用' : '展开工具调用'}
          >
            <div className="flex size-4 shrink-0 items-center justify-center text-muted-foreground/35">
              <ToolGlyph className="h-3.5 w-3.5" />
            </div>

            <div className="min-w-0 flex-1">
              <div className={cn('flex items-center gap-1.5 text-[13px] leading-5 tracking-[-0.01em]', status === 'error' ? 'text-rose-600/86' : 'text-muted-foreground/50')}>
                <span className="truncate">{title}</span>
                <ToolStatusGlyph status={status} />
              </div>
            </div>

            <ChevronDown
              className={cn(
                'h-3.5 w-3.5 shrink-0 text-muted-foreground/30 transition-all duration-150',
                expanded ? 'rotate-180 text-muted-foreground/50' : isHovered ? 'text-muted-foreground/50' : 'text-muted-foreground/25'
              )}
            />
          </button>

          <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="size-[18px] shrink-0 rounded-full border-0 bg-transparent text-muted-foreground/15 opacity-0 transition-opacity duration-150 group-hover:opacity-100 hover:bg-transparent hover:text-muted-foreground/40"
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
          <div className="ml-[10px] border-l-[1.5px] border-border/40 pl-3 pt-1">
            {showWriteDiff && (
              <div className="mb-2">
                <WriteToolDiffCard
                  path={writePath}
                  newContent={writeContent}
                  oldContent={writePreviousContent}
                />
              </div>
            )}
            {showNoKeyBanner && (
              <div className="mb-2 flex items-start gap-2 rounded-lg border border-amber-200 bg-amber-50 px-2.5 py-2 text-[11.5px] leading-5 text-amber-700">
                <span className="mt-0.5 shrink-0">⚠️</span>
                <span>
                  当前使用 DuckDuckGo 免费搜索，结果质量有限。
                  在{' '}
                  <span className="font-medium underline underline-offset-2 cursor-pointer">
                    设置 → Web Search
                  </span>{' '}
                  中添加 Tavily / Brave 等服务商以获得更准确的搜索结果。
                </span>
              </div>
            )}
            {hasDetails ? (
              <div className="flex flex-col gap-0.5">
                {display.details.map((line, index) => (
                  <div
                    key={`${line}-${index}`}
                    className={cn(
                      'flex min-w-0 items-center gap-1.5 text-[11.5px] leading-5 text-muted-foreground/36',
                      index === 0 && 'px-0.5 py-0.5 text-muted-foreground/42'
                    )}
                  >
                    {index === 0 ? (
                      <div className="flex size-4 shrink-0 items-center justify-center text-muted-foreground/28">
                        <ToolGlyph className="h-3.5 w-3.5" />
                      </div>
                    ) : (
                      <span className="mt-[1px] h-1 w-1 shrink-0 rounded-full bg-muted-foreground/20" />
                    )}
                    <span className="min-w-0 truncate">
                      {index === 0 ? renderInlineToolSummary(line) : <span className="font-mono">{line}</span>}
                    </span>
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
  rawInput,
}: {
  suggestions: string[]
  selectedIndex: number
  onSelect: (value: string) => void
  rawInput?: string
}) {
  return (
    <div className="absolute inset-x-0 bottom-full z-50 mb-2.5 pointer-events-none">
      <div
        className="pointer-events-auto w-full overflow-hidden rounded-2xl"
        style={{
          background: 'rgba(255,255,255,0.97)',
          backdropFilter: 'blur(24px) saturate(180%)',
          WebkitBackdropFilter: 'blur(24px) saturate(180%)',
          border: '1px solid rgba(0,0,0,0.09)',
          boxShadow:
            '0 4px 6px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.09), 0 20px 48px rgba(0,0,0,0.06), 0 0 0 0.5px rgba(0,0,0,0.05)',
        }}
      >
        {/* Header bar */}
        <div className="flex items-center justify-between border-b border-black/[0.055] px-4 py-2.5">
          <div className="flex items-center gap-2">
            <div className="flex size-5 shrink-0 items-center justify-center rounded-md bg-jade/10 text-[11px] font-bold text-jade">
              /
            </div>
            <span className="font-mono text-[13px] font-semibold tracking-tight text-jade">
              {rawInput ?? '/'}
            </span>
          </div>
          <span className="text-[10.5px] font-medium tracking-wide text-black/28">Tab 补全</span>
        </div>

        {/* Suggestion rows */}
        <div className="py-1">
          {suggestions.map((cmd, i) => (
            <button
              key={cmd}
              type="button"
              className={cn(
                'flex w-full cursor-pointer items-center gap-3 px-4 py-[7px] text-left transition-colors',
                i === selectedIndex
                  ? 'bg-jade/[0.07] text-jade'
                  : 'text-foreground/55 hover:bg-black/[0.025] hover:text-foreground/80'
              )}
              onClick={() => onSelect(cmd)}
            >
              {/* Icon badge */}
              <div
                className={cn(
                  'flex size-[22px] shrink-0 items-center justify-center rounded-[6px] text-[11px] font-bold transition-colors',
                  i === selectedIndex ? 'bg-jade/[0.14] text-jade' : 'bg-black/[0.05] text-black/32'
                )}
              >
                /
              </div>

              {/* Command name */}
              <span className="flex-1 truncate font-mono text-[12.5px] font-medium">
                {cmd.slice(1)}
              </span>

              {/* Enter hint for selected */}
              {i === selectedIndex && (
                <kbd className="shrink-0 rounded-md border border-black/[0.08] bg-black/[0.04] px-1.5 py-[2px] font-sans text-[10px] font-medium text-black/35">
                  ↵
                </kbd>
              )}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}

/** File/folder picker overlay triggered by typing @query in the composer.
 *  Supports nested folder navigation: Enter/→ enters a folder, ←/back button goes up. */
function AtFileSuggestions({
  entries,
  selectedIndex,
  onSelect,
  onNavigateInto,
  onNavigateUp,
  query,
  browsePath,
  breadcrumbs,
}: {
  entries: DirectoryEntryPreview[]
  selectedIndex: number
  onSelect: (entry: DirectoryEntryPreview) => void
  onNavigateInto?: (entry: DirectoryEntryPreview) => void
  onNavigateUp?: () => void
  query?: string
  browsePath?: string | null
  breadcrumbs?: Array<{ name: string; path: string | null }>
}) {
  const isInSubfolder = Boolean(browsePath)
  const currentDirName = browsePath ? browsePath.split('/').filter(Boolean).pop() ?? browsePath : null

  return (
    <div className="absolute inset-x-0 bottom-full z-50 mb-2.5 pointer-events-none">
      <div
        className="pointer-events-auto w-full overflow-hidden rounded-2xl"
        style={{
          background: 'rgba(255,255,255,0.97)',
          backdropFilter: 'blur(24px) saturate(180%)',
          WebkitBackdropFilter: 'blur(24px) saturate(180%)',
          border: '1px solid rgba(0,0,0,0.09)',
          boxShadow:
            '0 4px 6px rgba(0,0,0,0.04), 0 8px 24px rgba(0,0,0,0.09), 0 20px 48px rgba(0,0,0,0.06), 0 0 0 0.5px rgba(0,0,0,0.05)',
        }}
      >
        {/* ── Header ─────────────────────────────────────────────────────── */}
        <div className="flex items-center gap-2 border-b border-black/[0.055] px-3 py-2">
          {/* Back button (shown when inside a subfolder) */}
          {isInSubfolder && onNavigateUp && (
            <button
              type="button"
              onClick={() => onNavigateUp()}
              className="flex size-[22px] shrink-0 items-center justify-center rounded-md text-black/35 transition-colors hover:bg-black/[0.05] hover:text-black/60"
              title="返回上层 (←)"
            >
              <ChevronRight className="size-3.5 rotate-180" />
            </button>
          )}

          {/* @ badge */}
          <div className="flex size-5 shrink-0 items-center justify-center rounded-md bg-jade/10 text-[11px] font-bold text-jade">
            @
          </div>

          {/* Breadcrumb path */}
          <div className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden">
            {isInSubfolder && breadcrumbs && breadcrumbs.length > 0 ? (
              <>
                {/* Ancestors (truncated to last 2) */}
                {breadcrumbs.slice(-2).map((crumb, idx, arr) => (
                  <React.Fragment key={crumb.name + idx}>
                    <span className="shrink-0 text-[11px] font-medium text-black/28">{crumb.name}</span>
                    {idx < arr.length - 1 || currentDirName ? (
                      <ChevronRight className="size-2.5 shrink-0 text-black/20" />
                    ) : null}
                  </React.Fragment>
                ))}
                {/* Current directory — highlighted */}
                <span className="truncate text-[12px] font-semibold tracking-tight text-jade">
                  {currentDirName}
                </span>
              </>
            ) : (
              /* Root level: show @query */
              <span className="truncate font-mono text-[13px] font-semibold tracking-tight text-jade">
                {query ? `@${query}` : '@'}
              </span>
            )}
          </div>

          {/* Hint */}
          <span className="ml-auto shrink-0 text-[10px] font-medium text-black/22">
            {isInSubfolder ? '← 返回  ↵ 选中  → 进入' : '↑↓ 选择  ↵/→ 进入  Tab 选中'}
          </span>
        </div>

        {/* ── Entry rows ─────────────────────────────────────────────────── */}
        <div className="py-1">
          {entries.map((entry, i) => {
            const isFolder = entry.kind === 'folder'
            const isSelected = i === selectedIndex
            return (
              <button
                key={entry.path}
                type="button"
                className={cn(
                  'flex w-full cursor-pointer items-center gap-3 px-3 py-[7px] text-left transition-colors',
                  isSelected
                    ? 'bg-jade/[0.07] text-jade'
                    : 'text-foreground/55 hover:bg-black/[0.025] hover:text-foreground/80'
                )}
                onClick={() => {
                  if (isFolder && onNavigateInto) {
                    onNavigateInto(entry)
                  } else {
                    onSelect(entry)
                  }
                }}
              >
                {/* Kind badge */}
                <div
                  className={cn(
                    'flex size-[22px] shrink-0 items-center justify-center rounded-[6px] transition-colors',
                    isSelected
                      ? isFolder
                        ? 'bg-jade/[0.14] text-jade'
                        : 'bg-jade/[0.10] text-jade'
                      : isFolder
                        ? 'bg-black/[0.05] text-black/40'
                        : 'bg-black/[0.04] text-black/32'
                  )}
                >
                  {isFolder ? <Folder className="size-3" /> : <FileText className="size-3" />}
                </div>

                {/* Name */}
                <span className="min-w-0 flex-1 truncate text-[12.5px] font-medium leading-tight">
                  {entry.name}
                </span>

                {/* Right side: chevron for folders (navigable), or ↵ for files */}
                {isFolder ? (
                  <ChevronRight
                    className={cn(
                      'size-3 shrink-0 transition-colors',
                      isSelected ? 'text-jade/60' : 'text-black/20'
                    )}
                  />
                ) : isSelected ? (
                  <kbd className="shrink-0 rounded-md border border-black/[0.08] bg-black/[0.04] px-1.5 py-[2px] font-sans text-[10px] font-medium text-black/35">
                    ↵
                  </kbd>
                ) : null}
              </button>
            )
          })}
        </div>

        {/* ── Footer hint (browse mode only) ─────────────────────────────── */}
        {isInSubfolder && (
          <div className="border-t border-black/[0.04] px-4 py-1.5">
            <span className="text-[10px] text-black/25">
              Tab 选中当前目录作为引用
            </span>
          </div>
        )}
      </div>
    </div>
  )
}

const ChatMessage = React.memo(function ChatMessage({
  message,
  onCopyMessage,
  onResumeFromCursor,
  isCopied,
  defaultWorkdir,
  isPrimaryThinkingMessage,
  densityMode,
  fontMode,
}: {
  message: Message
  onCopyMessage: (message: Message) => void
  onResumeFromCursor?: (resumeCursor: string) => void
  isCopied: boolean
  defaultWorkdir?: string
  isPrimaryThinkingMessage?: boolean
  densityMode: DensityMode
  fontMode: FontMode
}) {
  const isUser = message.role === 'user'
  const isTool = message.role === 'tool'
  const hasThinking = Boolean(message.thinking?.trim())
  const hasContent = Boolean(message.content?.trim())
  const shortTime = formatShortTime(message.timestamp)
  const showCopyButton = isCopied
  const contentHash = React.useMemo(() => hashString(message.content), [message.content])
  const showStatusLabel = !isUser && !isTool && Boolean(message.statusLabel)
  const statusMeta = getAssistantStatusMeta(message)

  if (isTool) {
    return <ToolCallMessage message={message} defaultWorkdir={defaultWorkdir} />
  }

  // Slash 命令的静态结果（来自 `executeSlashCommand`，由 App.tsx 在
  // 消息上打 `slashCommand` 标记）→ 用紧凑型单行卡片替代 markdown 气泡。
  // 只在 assistant 侧生效；isUser 早就走了 `if (isUser)` 分支。
  if (!isUser && !isTool && message.slashCommand && hasContent) {
    return (
      <div
        className={cn(
          'group flex flex-col items-start',
          densityMode === 'compact' ? 'gap-1.5' : 'gap-2.5',
        )}
      >
        <SlashResultCard
          slashCommand={message.slashCommand}
          content={message.content}
        />
        <div className="flex items-center gap-1.5 pl-1">
          <MessageCopyButton
            side="right"
            copied={isCopied}
            visible={showCopyButton}
            onClick={() => onCopyMessage(message)}
          />
          <div className="text-[11px] leading-none text-black/32">{shortTime}</div>
        </div>
      </div>
    )
  }

  return (
    <div className={cn('group flex flex-col', fontMode === 'serif' ? 'font-serif' : 'font-sans', densityMode === 'compact' ? 'gap-1.5' : 'gap-2.5', isUser ? 'items-end' : 'items-start')}>
      {isUser ? (
        <div className={cn('flex flex-col items-end gap-1', densityMode === 'compact' ? 'max-w-[min(620px,78%)]' : 'max-w-[min(680px,78%)]')}>
          <div
            className={cn(
              'rounded-lg bg-secondary px-3 text-foreground/80 shadow-xs',
              densityMode === 'compact' ? 'py-1.5 text-[12px] leading-5.5' : 'py-2 text-[13px] leading-6'
            )}
          >
            <div className={cn('whitespace-pre-wrap break-words [overflow-wrap:anywhere]', fontMode === 'serif' ? 'font-serif' : 'font-sans')}>
              {message.content}
            </div>
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
            (() => {
              const taskOutcome =
                typeof message.toolArgs.taskOutcome === 'string'
                  ? message.toolArgs.taskOutcome
                  : message.taskOutcome
              const resumeCursor =
                typeof message.toolArgs.resumeCursor === 'string'
                  ? message.toolArgs.resumeCursor
                  : message.resumeCursor
              const degradedReason =
                typeof message.toolArgs.degradedReason === 'string'
                  ? message.toolArgs.degradedReason
                  : message.degradedReason
              const rawError = String(message.toolArgs.rawError)

              return taskOutcome === 'partial_success' ? (
                <RecoveryCard
                  error={rawError}
                  degradedReason={degradedReason}
                  resumeCursor={resumeCursor}
                  isRecovering={Boolean(message.isRecovering)}
                  onResume={onResumeFromCursor}
                />
              ) : (
                <ErrorCard
                  error={rawError}
                  taskOutcome={taskOutcome}
                  resumeCursor={resumeCursor}
                  onResume={onResumeFromCursor}
                />
              )
            })()
          ) : (
            <>
              <div
                className={cn(
                  'pt-0 font-normal text-black/80',
                  densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75'
                )}
              >
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

                {showStatusLabel ? (
                  <div className={cn(
                    'mb-2 inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] leading-4',
                    statusMeta.containerClass
                  )}>
                    <span className={cn('h-1.5 w-1.5 rounded-full', statusMeta.dotClass)} />
                    <span>{message.statusLabel}</span>
                  </div>
                ) : null}

                {hasContent ? (
                  <>
                    <MarkdownContent
                      content={message.content}
                      contentHash={contentHash}
                      densityMode={densityMode}
                    />
                    <div className="mt-1.5 flex items-center gap-1.5 pl-1">
                      <div className="text-[11px] leading-none text-black/28">{shortTime}</div>
                      {/* Per-message TurnCost chip — placed before the copy
                          button so the user sees billable usage right after
                          the timestamp, mirroring Steward's `turn-cost-bar`
                          layout (see ChatArea.svelte). */}
                      {message.turnCost ? (
                        <TurnCostChip turnCost={message.turnCost} className="ml-1" />
                      ) : null}
                      <MessageCopyButton
                        side="right"
                        copied={isCopied}
                        visible={showCopyButton}
                        onClick={() => onCopyMessage(message)}
                      />
                      {/* Phase TTS-E：消息级语音播放按钮（有 agent voice 时才显示） */}
                      {!message.isStreaming ? (
                        <React.Suspense fallback={null}>
                          <MessageVoiceButtonLazy text={message.content} />
                        </React.Suspense>
                      ) : null}
                      {message.memoryContext && message.memoryContext.length > 0 ? (
                        <MemoryChip items={message.memoryContext} className="ml-1" />
                      ) : null}
                      {message.routing ? (
                        <RoutingChip routing={message.routing} className="ml-1" />
                      ) : null}
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
}, (prev, next) =>
  prev.message === next.message &&
  prev.isCopied === next.isCopied &&
  prev.defaultWorkdir === next.defaultWorkdir &&
  prev.isPrimaryThinkingMessage === next.isPrimaryThinkingMessage &&
  prev.densityMode === next.densityMode &&
  prev.fontMode === next.fontMode &&
  prev.onCopyMessage === next.onCopyMessage &&
  prev.onResumeFromCursor === next.onResumeFromCursor
)

const markdownNormalizeCache = new Map<number, { source: string; normalized: string }>()

const MarkdownContent = React.memo(function MarkdownContent({
  content,
  contentHash,
  densityMode,
}: {
  content: string
  contentHash: number
  densityMode: DensityMode
}) {
  const normalizedContent = React.useMemo(() => {
    const cached = markdownNormalizeCache.get(contentHash)
    if (cached && cached.source === content) {
      return cached.normalized
    }
    const normalized = normalizeAsciiDiagramBlocks(content)
    markdownNormalizeCache.set(contentHash, { source: content, normalized })
    if (markdownNormalizeCache.size > 300) {
      const firstKey = markdownNormalizeCache.keys().next().value
      if (firstKey !== undefined) markdownNormalizeCache.delete(firstKey)
    }
    return normalized
  }, [content, contentHash])
  const skillsReport = React.useMemo(() => parseSkillsSlashReport(content), [content])

  if (skillsReport) {
    return <SkillsSlashReport report={skillsReport} densityMode={densityMode} />
  }

  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeHighlight]}
      components={{
        p: ({ children }) => (
          <p
            className={cn(
              'my-2 whitespace-pre-wrap font-normal text-foreground/88 first:mt-0 last:mb-0',
              densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75'
            )}
          >
            {children}
          </p>
        ),
        h1: ({ children }) => <h1 className="scroll-m-20 text-xl font-semibold leading-[1.55] tracking-tight">{children}</h1>,
        h2: ({ children }) => <h2 className="scroll-m-20 border-b pb-2 text-lg font-semibold leading-[1.55] tracking-tight first:mt-0">{children}</h2>,
        h3: ({ children }) => <h3 className="scroll-m-20 text-base font-semibold leading-[1.5] tracking-tight">{children}</h3>,
        ul: ({ children }) => (
          <ul
            className={cn(
              'my-3 ml-6 list-disc marker:text-black/45',
              densityMode === 'compact' ? 'space-y-1 text-[12px] leading-5.25' : 'space-y-1.25 text-[13px] leading-5.75'
            )}
          >
            {children}
          </ul>
        ),
        ol: ({ children }) => (
          <ol
            className={cn(
              'my-3 ml-6 list-decimal marker:text-black/45',
              densityMode === 'compact' ? 'space-y-1 text-[12px] leading-5.25' : 'space-y-1.25 text-[13px] leading-5.75'
            )}
          >
            {children}
          </ol>
        ),
        li: ({ children }) => <li className={densityMode === 'compact' ? 'leading-5.25' : 'leading-5.75'}>{children}</li>,
        blockquote: ({ children }) => (
          <blockquote className={cn('mt-4 border-l-2 border-slate-300/90 pl-4 italic text-black/72', densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75')}>
            {children}
          </blockquote>
        ),
        table: ({ children }) => (
          <div className={cn('my-3 overflow-hidden shadow-none ring-0', SURFACE_CARD_TOKENS.radius, SURFACE_CARD_TOKENS.border, SURFACE_CARD_TOKENS.background)}>
            <table className={cn('w-full border-collapse text-black/75', densityMode === 'compact' ? 'text-[11.5px]' : 'text-[12px]')}>
              {children}
            </table>
          </div>
        ),
        thead: ({ children }) => <thead className={SURFACE_CARD_TOKENS.headerBackground}>{children}</thead>,
        tbody: ({ children }) => <tbody className="[&_tr:last-child_td]:border-b-0">{children}</tbody>,
        th: ({ children }) => (
          <th className={cn(
            'border-b border-black/8 px-4 text-left font-medium tracking-tight text-black/70',
            densityMode === 'compact' ? 'py-1 text-[11px]' : 'py-1.75 text-[12px]'
          )}>
            {children}
          </th>
        ),
        td: ({ children }) => (
          <td className={cn(
            'border-b border-black/7 px-4 align-top font-normal text-black/72',
            densityMode === 'compact' ? 'py-1 text-[11.5px] leading-4.5' : 'py-1.75 text-[12px] leading-5'
          )}>
            {children}
          </td>
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
              className="rounded-[3px] bg-muted px-1.5 py-0.5 font-mono text-[12px] font-normal text-muted-foreground"
              {...props}
            >
              {children}
            </code>
          )
        },
        pre: ({ children }) => <CodeBlock densityMode={densityMode}>{children}</CodeBlock>,
      }}
    >
      {normalizedContent}
    </ReactMarkdown>
  )
}, (prev, next) =>
  prev.contentHash === next.contentHash &&
  prev.content === next.content &&
  prev.densityMode === next.densityMode
)

type ParsedSkillsReport = {
  total: number
  enabled: number
  items: Array<{
    name: string
    sourceKey: string
    sourceLabel: string
    enabled: boolean
    status: string
    shadowedBy?: string
  }>
}

type GroupedSkillsReportItem = {
  name: string
  variants: ParsedSkillsReport['items']
}

function SkillsSlashReport({
  report,
  densityMode,
}: {
  report: ParsedSkillsReport
  densityMode: DensityMode
}) {
  const groupedItems = React.useMemo(() => groupSkillsReportItems(report.items), [report.items])

  return (
    <div className={cn('my-2.5 space-y-3.5', densityMode === 'compact' ? 'text-[12px]' : 'text-[13px]')}>
      <div className="flex items-center gap-2 text-black/78">
        <div className="flex size-7 items-center justify-center rounded-[6px] bg-black/[0.035] text-black/42">
          <Sparkles className="size-4" />
        </div>
        <div className="flex items-baseline gap-2">
          <span className="font-medium tracking-tight">Skills</span>
          <span className="text-black/42">{report.total} total</span>
          <span className="text-black/28">/</span>
          <span className="text-black/42">{report.enabled} enabled</span>
        </div>
      </div>

      <div className="space-y-3">
        {groupedItems.map((group) => {
          const primary = group.variants[0]
          const SourceGlyph = getSkillSourceGlyph(primary.sourceKey)
          const isActive = primary.enabled && (primary.status === 'active' || primary.status === 'review_passed')

          return (
            <div key={`${group.name}-${group.variants.length}`} className="flex gap-3">
              <div className="flex w-5 shrink-0 justify-center pt-1">
                <span className={cn(
                  'size-2 rounded-full',
                  isActive ? 'bg-emerald-500/80' : primary.enabled ? 'bg-amber-400/85' : 'bg-black/18'
                )} />
              </div>
              <div className="min-w-0 flex-1 space-y-1.5">
                <div className="flex min-w-0 items-center gap-2">
                  <div className="truncate font-medium tracking-tight text-black/78">{group.name}</div>
                  <span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-black/[0.03] px-2 py-0.5 text-[10.5px] text-black/42">
                    <SourceGlyph className="size-3" />
                    <span>{primary.sourceLabel}</span>
                  </span>
                  {group.variants.length > 1 ? (
                    <span className="shrink-0 text-[10.5px] text-black/34">{group.variants.length} variants</span>
                  ) : null}
                </div>
                <div className="space-y-1">
                  {group.variants.map((variant, index) => (
                    <SkillMetaRow
                      key={`${group.name}-${variant.sourceKey}-${variant.shadowedBy ?? 'active'}-${index}`}
                      item={variant}
                      showSource={group.variants.length > 1}
                    />
                  ))}
                </div>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function SkillMetaRow({
  item,
  showSource,
}: {
  item: ParsedSkillsReport['items'][number]
  showSource: boolean
}) {
  const SourceGlyph = getSkillSourceGlyph(item.sourceKey)

  return (
    <div className="flex min-w-0 items-start gap-2 text-[11px] text-black/38">
      {showSource ? (
        <span className="inline-flex shrink-0 items-center gap-1 text-black/32">
          <SourceGlyph className="size-3" />
          <span>{item.sourceLabel}</span>
        </span>
      ) : (
        <span className={cn('mt-[4px] size-1.5 shrink-0 rounded-full', item.enabled ? 'bg-emerald-500/55' : 'bg-black/14')} />
      )}
      <div className="min-w-0 truncate">
        <span>{item.enabled ? 'enabled' : 'disabled'}</span>
        <span className="px-1 text-black/18">·</span>
        <span>
          <span className="text-black/30">status </span>
          <span className="font-mono italic text-black/46">{item.status}</span>
        </span>
        {item.shadowedBy ? (
          <>
            <span className="px-1 text-black/18">·</span>
            <span>
              <span className="text-black/30">shadowed by </span>
              <span className="font-mono italic text-black/46">{item.shadowedBy}</span>
            </span>
          </>
        ) : null}
      </div>
    </div>
  )
}

function groupSkillsReportItems(items: ParsedSkillsReport['items']): GroupedSkillsReportItem[] {
  const groups = new Map<string, GroupedSkillsReportItem>()

  for (const item of items) {
    const key = item.name.toLowerCase()
    const existing = groups.get(key)
    if (existing) {
      existing.variants.push(item)
    } else {
      groups.set(key, { name: item.name, variants: [item] })
    }
  }

  for (const group of groups.values()) {
    group.variants.sort((a, b) => getSkillVariantPriority(b) - getSkillVariantPriority(a))
  }

  return Array.from(groups.values())
}

function getSkillVariantPriority(item: ParsedSkillsReport['items'][number]) {
  let score = 0
  if (item.enabled) score += 4
  if (item.status === 'active' || item.status === 'review_passed') score += 3
  if (!item.shadowedBy) score += 2
  if (item.sourceKey === 'workspace') score += 1.5
  else if (item.sourceKey === 'user') score += 1
  else if (item.sourceKey === 'builtin') score += 0.5
  return score
}

function parseSkillsSlashReport(content: string): ParsedSkillsReport | null {
  const normalized = content.replace(/\r\n/g, '\n').trim()
  const lines = normalized.split('\n').map((line) => line.trimEnd())
  const header = lines[0]?.match(/^📋\s+Skills\s+\((\d+)\s+total,\s+(\d+)\s+enabled\)$/)
  if (!header) return null

  const items: ParsedSkillsReport['items'] = []
  let current: ParsedSkillsReport['items'][number] | null = null

  for (const rawLine of lines.slice(2)) {
    const line = rawLine.trim()
    if (!line) continue

    const itemMatch = line.match(/^(🟢|⚪)(✅|🔒|📝|❌|⚠️)\s+(.+?)\s+(🏠 builtin|👤 user|📁 workspace|🔒 quarantine|[^\s]+)$/)
    if (itemMatch) {
      if (current) items.push(current)
      current = {
        enabled: itemMatch[1] === '🟢',
        status: normalizeSkillStatusFromEmoji(itemMatch[2]),
        name: itemMatch[3].trim(),
        sourceKey: normalizeSkillSourceKey(itemMatch[4]),
        sourceLabel: normalizeSkillSourceLabel(itemMatch[4]),
      }
      continue
    }

    if (!current) continue

    const metaMatch = line.match(/^└\s+(enabled|disabled)\s+\|\s+status:\s+(.+)$/)
    if (metaMatch) {
      current.enabled = metaMatch[1] === 'enabled'
      current.status = metaMatch[2].trim()
      continue
    }

    const shadowedMatch = line.match(/^└\s+shadowed by\s+(.+)$/)
    if (shadowedMatch) {
      current.shadowedBy = shadowedMatch[1].trim()
    }
  }

  if (current) items.push(current)
  if (!items.length) return null

  return {
    total: Number(header[1]),
    enabled: Number(header[2]),
    items,
  }
}

function normalizeSkillStatusFromEmoji(emoji: string) {
  if (emoji === '✅') return 'active'
  if (emoji === '🔒') return 'quarantine'
  if (emoji === '📝') return 'draft'
  if (emoji === '❌') return 'disabled'
  return 'warning'
}

function normalizeSkillSourceKey(sourceLabel: string) {
  if (sourceLabel.includes('builtin')) return 'builtin'
  if (sourceLabel.includes('user')) return 'user'
  if (sourceLabel.includes('workspace')) return 'workspace'
  if (sourceLabel.includes('quarantine')) return 'quarantine'
  return sourceLabel
}

function normalizeSkillSourceLabel(sourceLabel: string) {
  return sourceLabel
    .replace('🏠 ', '')
    .replace('👤 ', '')
    .replace('📁 ', '')
    .replace('🔒 ', '')
}

function getSkillSourceGlyph(sourceKey: string): React.ComponentType<{ className?: string }> {
  if (sourceKey === 'builtin') return House
  if (sourceKey === 'user') return UserRound
  if (sourceKey === 'workspace') return FolderOpen
  if (sourceKey === 'quarantine') return Lock
  return Sparkles
}

function CodeBlock({
  children,
  densityMode,
}: {
  children: React.ReactNode
  densityMode: DensityMode
}) {
  const [copied, setCopied] = React.useState(false)
  const [animateCopy, setAnimateCopy] = React.useState(false)
  const rawCode = extractCodeText(children)
  const displayCode = React.useMemo(() => normalizeCodeForDisplay(rawCode), [rawCode])

  if (looksLikeKeywordLineBlock(rawCode)) {
    return (
      <div
        className={cn(
          'my-2 text-black/76',
          densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75'
        )}
      >
        <div className="whitespace-pre-wrap [overflow-wrap:anywhere]">{normalizeKeywordLineForDisplay(displayCode.trim())}</div>
      </div>
    )
  }

  if (looksLikeNarrativeTextBlock(rawCode)) {
    return (
      <div
        className={cn(
          'my-3 pl-4 text-black/72 italic',
          densityMode === 'compact' ? 'text-[12px] leading-5.25' : 'text-[13px] leading-5.75'
        )}
      >
        <div className="whitespace-pre-wrap [overflow-wrap:anywhere]">{normalizeNarrativeTextForDisplay(displayCode.trim())}</div>
      </div>
    )
  }

  const copyCode = async () => {
    const text = displayCode
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
    <div className={cn('my-3 overflow-hidden text-[12px] shadow-none ring-0', SURFACE_CARD_TOKENS.radius, SURFACE_CARD_TOKENS.border, SURFACE_CARD_TOKENS.background)}>
      <div className={cn(
        'flex items-center justify-between px-3.5',
        SURFACE_CARD_TOKENS.headerBackground,
        SURFACE_CARD_TOKENS.headerDivider,
        densityMode === 'compact' ? 'h-6' : 'h-7'
      )}>
        <div className={SURFACE_CARD_TOKENS.headerLabel}>
          code
        </div>
        <button
          type="button"
          className={cn(
            'inline-flex h-5 items-center gap-1 rounded px-1.5 text-[11px] text-black/45 transition-all duration-150 hover:bg-black/[0.03] hover:text-black/68',
            animateCopy && 'scale-[1.03] bg-emerald-500/10 text-emerald-600'
          )}
          onClick={copyCode}
          aria-label={copied ? '已复制' : '复制代码'}
        >
          {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
          <span>{copied ? '已复制' : '复制'}</span>
        </button>
      </div>
      <div className={cn(SURFACE_CARD_TOKENS.background, densityMode === 'compact' ? 'px-3.5 pb-2.5 pt-2' : 'px-4 pb-3 pt-2.5')}>
        <pre className={cn(
          'm-0 overflow-x-auto whitespace-pre bg-transparent !bg-transparent font-mono text-black/80',
          densityMode === 'compact' ? 'text-[11.5px] leading-5.5' : 'text-[12px] leading-6'
        )}>
          {displayCode}
        </pre>
      </div>
    </div>
  )

  return content
}

function normalizeAsciiDiagramBlocks(content: string): string {
  return content.replace(/```(?:[\w-]+)?\n([\s\S]*?)```/g, (fullMatch, rawBlock) => {
    const transformed = transformAsciiRelationshipBlock(rawBlock)
    return transformed ?? fullMatch
  })
}

function transformAsciiRelationshipBlock(rawBlock: string): string | null {
  const normalized = rawBlock.replace(/\r\n/g, '\n').trim()
  if (!looksLikeRelationshipBox(normalized)) return null

  const cleanedLines = normalized
    .split('\n')
    .map((line) => stripBoxDrawingLine(line))
    .filter((line) => line.length > 0 && !looksLikeDecorativeNoiseLine(line))

  if (cleanedLines.length < 3) return null

  const firstLine = cleanedLines[0]
  const bodyLines = cleanedLines.slice(1)
  const quoteLines: string[] = [`> **${firstLine}**`, '>']

  for (const line of bodyLines) {
    const bulletText = line.replace(/^([✦•◆▪●○\-*]+)\s*/, '').trim()
    if (/^([✦•◆▪●○\-*]+)/.test(line)) {
      quoteLines.push(`> - ${bulletText}`)
      continue
    }
    if (looksLikeRelationshipHeading(line)) {
      if (quoteLines[quoteLines.length - 1] !== '>') {
        quoteLines.push('>')
      }
      quoteLines.push(`> **${line}**`)
      continue
    }
    quoteLines.push(`> ${line}`)
  }

  return quoteLines.join('\n')
}

function looksLikeRelationshipBox(text: string): boolean {
  const boxCharCount = text.match(/[│┌┐└┘─/\\_|-]/g)?.length ?? 0
  const bulletCount = text.match(/^[ \t]*[✦•◆▪●○\-*]/gm)?.length ?? 0
  const numberedCount = text.match(/^[ \t]*(?:\d+[.)]|\d+️⃣|[①②③④⑤⑥⑦⑧⑨⑩]|🔟)/gm)?.length ?? 0
  const headingCount = text
    .replace(/\r\n/g, '\n')
    .split('\n')
    .map((line) => stripBoxDrawingLine(line))
    .filter((line) => looksLikeRelationshipHeading(line)).length
  const codeSignalCount = text.match(/[{}();=<>]/g)?.length ?? 0
  return boxCharCount >= 8 && codeSignalCount < 6 && (bulletCount >= 2 || numberedCount >= 2 || headingCount >= 3)
}

function looksLikeRelationshipHeading(line: string): boolean {
  const normalized = line.trim()
  if (!normalized) return false
  return /^(?:\d+[.)]|\d+️⃣|[①②③④⑤⑥⑦⑧⑨⑩]|🔟)\s*/.test(normalized)
}

function stripBoxDrawingLine(line: string): string {
  const trimmed = line.trim()
  if (!trimmed) return ''
  if (/^[┌┐└┘─│/\\_|+\-\s]+$/.test(trimmed)) return ''

  const cleaned = trimmed
    .replace(/^[│\s]+/, '')
    .replace(/[│\s]+$/, '')
    .replace(/^[┌┐└┘─/\\_|+\-\s]+/, '')
    .replace(/[┌┐└┘─/\\_|+\-\s]+$/, '')
    .trim()
    .replace(/\s+[\/\\|]+\s*$/g, '')
    .replace(/^\s*[\/\\|]+\s+/g, '')
    .trim()

  if (looksLikeDecorativeNoiseLine(cleaned)) return ''
  return cleaned
}

function looksLikeDecorativeNoiseLine(line: string): boolean {
  const trimmed = line.trim()
  if (!trimmed) return true
  return /^[\/\\|_\-+]+$/.test(trimmed)
}

function hashString(input: string): number {
  let hash = 0x811c9dc5
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0
}

function normalizeCodeForDisplay(text: string): string {
  if (!looksLikeTreeText(text)) return text
  return text
    .replace(/\r\n/g, '\n')
    .replace(/\n[ \t]*\n(?=[ \t]*[│├└┌┐┬┼─|])/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
}

function looksLikeTreeText(text: string): boolean {
  const treeCharCount = text.match(/[│├└┌┐┬┼─]/g)?.length ?? 0
  const fileLikeCount = text.match(/([A-Za-z0-9._-]+\/|[A-Za-z0-9._-]+\.[A-Za-z0-9]+)/g)?.length ?? 0
  return treeCharCount >= 3 && fileLikeCount >= 4
}

function looksLikeNarrativeTextBlock(text: string): boolean {
  const normalized = text.replace(/\r\n/g, '\n').trim()
  if (!normalized || normalized.length < 16) return false

  const contentLines = normalized
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
  const lineCount = contentLines.length
  const punctuationSignals = normalized.match(/[「」『』。！？：…]/g)?.length ?? 0
  const dialogueSignals = normalized.match(/(^|\n)\s*[\p{Script=Han}A-Za-z0-9_-]{1,12}[:：]/gu)?.length ?? 0
  const quoteLineCount = contentLines.filter((line) => /^[[「『“"']/u.test(line)).length
  const codeSignals = normalized.match(/[{}();=<>`]/g)?.length ?? 0
  const cjkCharCount = normalized.match(/[\p{Script=Han}]/gu)?.length ?? 0
  const shortLineCount = contentLines.filter((line) => line.length <= 28).length
  const plainLineCount = contentLines.filter((line) => !/^[\-*•◆▪●○]/.test(line)).length

  if (codeSignals >= 6) return false

  if (lineCount >= 3 && punctuationSignals >= 3 && dialogueSignals >= 1) {
    return true
  }

  if (lineCount >= 2 && quoteLineCount === lineCount && punctuationSignals >= 2) {
    return true
  }

  if (lineCount >= 3 && cjkCharCount >= 12 && punctuationSignals >= 1 && shortLineCount >= 2) {
    return true
  }

  if (lineCount >= 3 && plainLineCount === lineCount && shortLineCount >= 2 && cjkCharCount >= 10) {
    return true
  }

  return lineCount >= 4 && cjkCharCount >= 16 && codeSignals === 0
}

function normalizeNarrativeTextForDisplay(text: string): string {
  return text
    .replace(/\r\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .replace(/[ \t]+\n/g, '\n')
}

function looksLikeKeywordLineBlock(text: string): boolean {
  const normalized = text.replace(/\r\n/g, '\n').trim()
  if (!normalized || normalized.includes('\n')) return false
  const separatorCount = normalized.match(/\|/g)?.length ?? 0
  const codeSignals = normalized.match(/[{}();=<>`[\]]/g)?.length ?? 0
  const cjkCharCount = normalized.match(/[\p{Script=Han}A-Za-z]/gu)?.length ?? 0
  return separatorCount >= 4 && codeSignals === 0 && cjkCharCount >= 8
}

function normalizeKeywordLineForDisplay(text: string): string {
  return text
    .replace(/\s*\|\s*/g, '  |  ')
    .replace(/\s{3,}/g, '  ')
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
        visible ? 'opacity-100' : 'opacity-0 group-hover:opacity-100',
        copied && 'opacity-100 bg-emerald-500/8 text-emerald-600 shadow-[0_0_0_1px_rgba(16,185,129,0.1)]',
        side === 'left' ? 'order-first' : 'order-last'
      )}
    >
      {copied ? <Check className="size-2.5" /> : <Copy className="size-2.5" />}
    </Button>
  )
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

function getAssistantStatusMeta(message: Message) {
  const kind = message.statusKind
    ?? (message.taskOutcome === 'partial_success'
      ? 'partial'
      : message.taskOutcome === 'failed'
        ? 'failed'
        : message.taskOutcome === 'completed'
          ? 'success'
          : 'info')
  if (kind === 'success') {
    return {
      containerClass: 'border-emerald-200/80 bg-emerald-50/75 text-emerald-800/85',
      dotClass: 'bg-emerald-500',
    }
  }
  if (kind === 'partial') {
    return {
      containerClass: 'border-amber-200/80 bg-amber-50/80 text-amber-800/85',
      dotClass: 'bg-amber-500',
    }
  }
  if (kind === 'failed') {
    return {
      containerClass: 'border-rose-200/80 bg-rose-50/80 text-rose-800/85',
      dotClass: 'bg-rose-500',
    }
  }
  return {
    containerClass: 'border-sky-200/80 bg-sky-50/80 text-sky-800/85',
    dotClass: 'bg-sky-500',
  }
}

function ToolStatusGlyph({
  status,
}: {
  status: 'queued' | 'running' | 'completed' | 'error'
}) {
  if (status === 'queued' || status === 'running') {
    return (
      <span className="relative flex h-3 w-3 shrink-0 items-center justify-center opacity-80">
        <span className="absolute inset-0 rounded-full bg-sky-400/10 animate-ping" />
        <LoaderCircle className="relative h-3 w-3 animate-spin text-sky-500/80" strokeWidth={2} />
      </span>
    )
  }

  if (status === 'completed') {
    return (
      <span className="inline-flex h-3 w-3 shrink-0 items-center justify-center rounded-full bg-emerald-500/88 text-white">
        <Check className="h-2.1 w-2.1" strokeWidth={3} />
      </span>
    )
  }

  return (
    <span className="inline-flex h-3 w-3 shrink-0 items-center justify-center rounded-full bg-rose-500/88 text-white">
      <X className="h-2.1 w-2.1" strokeWidth={3} />
    </span>
  )
}

function getToolCallGlyph(
  toolName?: string,
  args?: Record<string, unknown>
): React.ComponentType<{ className?: string }> {
  const normalized = (toolName ?? '').toLowerCase()
  const maybeCommand = pickToolString(args ?? {}, ['command', 'cmd', 'shell'])

  if (normalized === 'skill' || normalized.includes('skill/') || normalized.includes('.skill')) {
    return Sparkles
  }
  if (normalized.includes('web_search')) {
    return Globe
  }
  if (normalized.includes('glob_search') || normalized.includes('grep_search') || normalized.includes('content_search') || normalized.includes('tool_search') || normalized.includes('skill_search')) {
    return Search
  }
  if (normalized.includes('read_file')) {
    return FileText
  }
  if (normalized.includes('file_write')) {
    return FolderOpen
  }
  if (maybeCommand || normalized.includes('command') || normalized.includes('exec') || normalized.includes('shell')) {
    return Wrench
  }
  if (normalized.includes('weather')) {
    return Globe
  }
  if (normalized.includes('agent') || normalized.includes('assistant')) {
    return Bot
  }
  return TerminalSquare
}

type ToolDisplay = {
  title: string
  details: string[]
  copyText: string
  resultCopyText: string | null
  diagnosticCopyText: string | null
}

function buildToolCallDisplay(
  message: Message,
  defaultWorkdir?: string,
  status: 'queued' | 'running' | 'completed' | 'error' = 'completed'
): ToolDisplay {
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
    status,
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
  status: 'queued' | 'running' | 'completed' | 'error',
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
    return path ? `查看了 ${shortenMiddle(path, 28)}` : '查看了文件'
  }

  if (toolName.includes('file_write')) {
    if (status === 'queued' || status === 'running') {
      return path ? `正在写入 ${shortenMiddle(path, 28)}` : '正在写入文件'
    }
    return path ? `已写入 ${shortenMiddle(path, 28)}` : '已写入文件'
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
    return skillName ? `调用了 ${shortenMiddle(skillName, 24)}` : '调用了技能'
  }

  if (command) {
    return `执行了 ${truncateText(redactSensitiveText(command), 48)}`
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
  const summaryLine = buildToolCommandResultLine(toolName, command, skillName, resultSummary)

  if (summaryLine) {
    detailLines.push(summaryLine)
  }

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
  } else if (command && !summaryLine) {
    detailLines.push(`命令：${truncateText(redactSensitiveText(command), 92)}`)
  }

  if (resultSummary && !summaryLine) {
    detailLines.push(`结果：${truncateText(resultSummary, 96)}`)
  }

  const declaredStatus = pickToolString(args, ['status'])
  if (declaredStatus && !detailLines.some((line) => line.includes(declaredStatus))) {
    detailLines.push(`状态：${shortenMiddle(declaredStatus, 32)}`)
  }

  return detailLines.slice(0, 3)
}

function buildToolCommandResultLine(
  toolName: string,
  command: string | null,
  skillName: string | null,
  resultSummary: string
) {
  const toolOrCommandPart = command
    ? `命令：${truncateText(redactSensitiveText(command), resultSummary ? 68 : 108)}`
    : skillName
      ? `工具：${truncateText(skillName, resultSummary ? 34 : 72)}`
      : `工具：${truncateText(normalizeToolLabel(toolName), resultSummary ? 22 : 48)}`
  const resultPart = resultSummary
    ? `结果：${truncateText(resultSummary, command ? 24 : 52)}`
    : ''

  if (toolOrCommandPart && resultPart) {
    return `${toolOrCommandPart} · ${resultPart}`
  }

  return toolOrCommandPart || resultPart || ''
}

function normalizeToolLabel(toolName: string) {
  const normalized = toolName.trim().toLowerCase()

  if (!normalized) return 'unknown_tool'
  if (normalized.includes('glob_search')) return 'glob_search'
  if (normalized.includes('grep_search') || normalized.includes('content_search')) return 'grep_search'
  if (normalized.includes('read_file')) return 'read_file'
  if (normalized.includes('file_write')) return 'file_write'
  if (normalized.includes('web_search')) return 'web_search'
  if (normalized.includes('weather')) return 'weather'
  if (normalized.includes('tool_search')) return 'tool_search'
  if (normalized.includes('skill_search')) return 'skill_search'
  if (normalized === 'skill' || normalized.includes('skill/') || normalized.includes('.skill')) return 'skill'

  return toolName.trim()
}

function renderInlineToolSummary(line: string) {
  const segments = line.split(' · ').filter(Boolean)

  return (
    <span className="inline-flex min-w-0 max-w-full items-center gap-1.5 truncate">
      {segments.map((segment, index) => {
        const separatorNeeded = index > 0
        const colonIndex = segment.indexOf('：')
        const label = colonIndex >= 0 ? segment.slice(0, colonIndex + 1) : ''
        const value = colonIndex >= 0 ? segment.slice(colonIndex + 1).trim() : segment

        return (
          <React.Fragment key={`${segment}-${index}`}>
            {separatorNeeded ? <span className="shrink-0 text-black/24">·</span> : null}
            <span className="inline-flex min-w-0 items-center gap-1">
              {label ? <span className="shrink-0 text-black/34">{label}</span> : null}
              <span className="min-w-0 truncate rounded-[3px] bg-black/[0.04] px-1.5 py-[1px] font-mono italic text-black/48">
                {value}
              </span>
            </span>
          </React.Fragment>
        )
      })}
    </span>
  )
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

    // memory_store structured payload — emit a human readable summary
    // instead of the raw JSON ("Stored memory: <key>" replaces what the
    // legacy plain-string handler used to produce, while pending_approval
    // / denied surface their own dedicated cards above).
    if (typeof record.status === 'string' && typeof record.key === 'string') {
      const key = record.key as string
      switch (record.status) {
        case 'stored':
          return `已写入记忆：${truncateText(key, 80)}`
        case 'pending_approval':
          return `等待审批：${truncateText(key, 80)}`
        case 'denied':
          return `已拒绝写入：${truncateText(key, 80)}`
        default:
        // Fall through to generic handling.
      }
    }

    // file_write structured payload — emit a friendly one-liner so the
    // tool-card summary doesn't dump the raw JSON (which can carry up
    // to 64 KiB of pre-write content for the diff viewer).
    if (record.kind === 'file_write' && record.ok === true) {
      const path = typeof record.path === 'string' ? record.path : ''
      return path ? `已写入 ${truncateText(path, 80)}` : '已写入文件'
    }

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

    for (const key of [
      'results',
      'items',
      'entries',
      'files',
      'todos',
      'new_todos',
      'newTodos',
    ] as const) {
      const value = record[key]
      if (Array.isArray(value)) {
        return `返回 ${value.length} 项结果`
      }
    }

    // For generic JSON object outputs (like TodoWrite payloads),
    // avoid falling back to the first line "{" in tool cards.
    return '返回对象结果'
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
    <div className="flex items-center px-0.5 py-1">
      <WaveDotsAnimation
        amplitude={11.04}
        ballRadius={3}
        count={6}
        delay={0.19}
        horizontalStretch={1.10625}
        topStartColor="#fb923c"
        topEndColor="#f97316"
        bottomStartColor="#f59e0b"
        bottomEndColor="#ea580c"
        className="opacity-85"
      />
    </div>
  )
}

function RecoveryCard({
  error,
  degradedReason,
  resumeCursor,
  isRecovering,
  onResume,
}: {
  error: string
  degradedReason?: string
  resumeCursor?: string
  isRecovering?: boolean
  onResume?: (resumeCursor: string) => void
}) {
  const reasonLabel = summarizeDegradedReason(degradedReason)

  return (
    <div className="relative w-full overflow-hidden rounded-[6px] border border-emerald-200/70 bg-emerald-50/55 px-3 py-2.5">
      <div className="absolute inset-y-0 left-0 w-1.5 rounded-l-[13px] bg-emerald-400/90" />
      <div className="flex items-start gap-2.5 pl-2 pr-1">
        <div className="mt-0.25 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-emerald-500/10">
          <Check className="h-3.5 w-3.5 text-emerald-600" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div className="text-[12.5px] font-medium leading-4.5 text-emerald-900/90">任务已部分完成</div>
            <span className="rounded-full border border-emerald-300/70 bg-emerald-100/80 px-2 py-0.5 text-[10px] font-medium leading-4 text-emerald-700/90">
              {isRecovering ? '恢复中' : '可继续恢复'}
            </span>
          </div>
          <div className="mt-0.5 text-[11.5px] leading-4.5 text-emerald-800/70">
            {isRecovering
              ? '正在基于已保留的恢复点继续补全未完成部分，不会重复已确认的副作用操作。'
              : '已保留本轮已确认的执行结果。继续后只补全未完成部分，不会重复已确认的副作用操作。'}
          </div>
          {reasonLabel ? (
            <div className="mt-1 text-[11px] leading-4 text-emerald-700/75">
              中断原因：{reasonLabel}
            </div>
          ) : null}
          <div className="mt-1.25 break-words rounded-md bg-white/45 px-2.5 py-1.25 text-[11px] font-mono leading-4 text-emerald-700/70">
            {truncateText(error, 500)}
          </div>
          {resumeCursor && onResume ? (
            <button
              type="button"
              onClick={() => onResume(resumeCursor)}
              disabled={isRecovering}
              className="mt-1.75 inline-flex items-center gap-1.5 rounded-md bg-emerald-100/90 px-2.5 py-1 text-[11px] font-medium text-emerald-800 transition-colors hover:bg-emerald-200/80 disabled:cursor-default disabled:opacity-60"
            >
              <RotateCcw className="h-3 w-3" />
              {isRecovering ? '正在恢复未完成任务…' : '继续未完成任务'}
            </button>
          ) : null}
        </div>
      </div>
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
    <div className="relative w-full overflow-hidden rounded-[6px] border border-rose-200/50 bg-rose-50/32 px-3 py-2.5">
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

function summarizeDegradedReason(degradedReason?: string) {
  if (!degradedReason) return null
  const normalized = degradedReason.split(';')[0]?.trim().toLowerCase()
  if (!normalized) return null
  if (normalized.includes('network_timeout')) return '模型流超时'
  if (normalized.includes('network_transport_error')) return '网络传输中断'
  if (normalized.includes('request_validation_error')) return '请求校验失败'
  if (normalized.includes('permission_error')) return '权限受限'
  if (normalized.includes('max_iterations_reached')) return '达到迭代上限'
  if (normalized.includes('read_only_success_before_failure')) return '只读工具已完成，但回答尾段中断'
  return degradedReason.split(';')[0] ?? null
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
        'h-8 rounded-[6px] px-2.5 text-[12.5px] font-medium',
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
