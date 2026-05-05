import * as React from "react"
import { useAvailableModels } from '@/api/models'
import { ArrowDown } from "lucide-react"
import { cn } from "@/lib/utils"
import { listDirectoryPreview, openDirectoryPath, readFilePreview, type ContextBudgetUsage, type DirectoryEntryPreview, type FilePreviewPayload, type MemoryContextItem, type PermissionMode, type SessionTotals } from "@/lib/tauri"
import type { FinalRunReport } from "@/transport/contracts"

import { type TodoItem } from "@/components/ui/TodoPanel"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Textarea } from "@/components/ui/textarea"
// GF-01 PR-01 — leaf cards / utils extracted from this file.
import { MenuItemButton } from "@/components/chat/chat-ui/utils/MenuItemButton"
import {
  formatShortTime,
  redactSensitiveText,
  truncateText,
} from "@/components/chat/chat-ui/utils/text"
import {
  modelItems,
  permissionModeItems,
  permissionModeLabelFor,
} from "@/components/chat/chat-ui/utils/items"
// GF-01 PR-02 — tool-call display + diagnostic + markdown normalize utils.
import {
  isToolPendingStatus,
  pickToolString,
  type ChatToolStatus,
} from "@/components/chat/chat-ui/utils/toolCallDisplay"
// GF-01 PR-05 — ChatTranscript + ChatMessage extracted to chat-ui/transcript/.
import { ChatTranscript } from "@/components/chat/chat-ui/transcript/ChatTranscript"
// GF-01 PR-06 — ComposerDock + slash/@ overlays + PermissionModePicker extracted.
import { ComposerDock } from "@/components/chat/chat-ui/composer/ComposerDock"
// GF-01 PR-07 — sidebars + panels extracted from this file.
import { ProjectFilesRail } from "@/components/chat/chat-ui/sidebars/ProjectFilesRail"
import {
  inferPreviewLanguage,
  sortRailDirectoryEntries,
} from "@/components/chat/chat-ui/sidebars/utils"
import { TodoPanelMount } from "@/components/chat/chat-ui/panels/TodoPanelMount"
import { ContextBarMount } from "@/components/chat/chat-ui/panels/ContextBarMount"
import { ProjectPreviewMount } from "@/components/chat/chat-ui/panels/ProjectPreviewMount"
// GF-01 PR-08 — chat-ui shell hooks extracted from this file.
import { useDensityFontMode } from "@/components/chat/chat-ui/hooks/useDensityFontMode"
import { useChatScroll } from "@/components/chat/chat-ui/hooks/useChatScroll"
import { useProjectRailState } from "@/components/chat/chat-ui/hooks/useProjectRailState"
import { useProjectPreviewState } from "@/components/chat/chat-ui/hooks/useProjectPreviewState"
import { useComposerOverlays } from "@/components/chat/chat-ui/hooks/useComposerOverlays"
import { useComposerDropItems } from "@/components/chat/chat-ui/hooks/useComposerDropItems"

export interface Message {
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
  toolStatus?: import("@/transport/contracts").ToolAttemptStatus | "error"
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
  /** AWL-004 — canonical final work-loop report from runtime projection. */
  finalRunReport?: FinalRunReport
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
  /** Active session id (used by ContextBar's manual /compact button). */
  sessionId?: string
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
export type ComposerDropItem = {
  id: string
  name: string
  path: string
  kind: 'file' | 'folder'
  /** True when added via @-mention typing (vs drag-drop). Changes chip visual. */
  isMention?: boolean
}

// GF-01 PR-08 — these constants moved into the hooks that own them:
//  - BOTTOM_EPSILON_PX → useChatScroll
//  - CHAT_DENSITY_MODE_STORAGE_KEY / CHAT_FONT_MODE_STORAGE_KEY → useDensityFontMode
//  - PROJECT_RAIL_WIDTH_STORAGE_KEY / PROJECT_RAIL_NOTICE_DURATION_MS → useProjectRailState
//  - PREVIEW_AUTOSAVE_DELAY_MS → useProjectPreviewState

export function ChatUI({
  messages,
  input,
  onInputChange,
  onSubmit,
  onResumeFromCursor,
  onStop,
  isLoading,
  sessionTitle = '重构桌面端 UI 为 shadcn 体系',
  sessionId,
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
  const textareaRef = React.useRef<HTMLTextAreaElement>(null)
  const todoPanelRef = React.useRef<HTMLDivElement>(null)
  const [selectedModel, setSelectedModel] = React.useState(selectedModelProp)

  // GF-01 PR-08 / ER-02 — chat-ui shell hooks centralise the scroll,
  // density/font, project-rail, preview-state, composer overlay and
  // drop-item state machines previously inlined in this file.
  const {
    bottomRef,
    scrollRef: transcriptScrollRef,
    isAtBottomRef,
    forceAutoScrollRef,
    stickToBottomDuringStreamRef,
    lastScrollTopRef,
    lastBottomOccupancyRef,
    isAtBottom,
    setIsAtBottom,
    scrollToBottom: scrollTranscriptToBottom,
    updateBottomState,
    handleScroll: handleScrollInternal,
  } = useChatScroll()

  // ER-02 — single facade-backed subscription replacing the raw
  // `invoke('model_list_available')` + `listen('if2ai://models-changed')`.
  const { groups: availableModelGroups } = useAvailableModels()
  const availableModelItems = React.useMemo(
    () =>
      availableModelGroups
        .filter((g) => g.models.length > 0)
        .flatMap((g) =>
          g.models.map((m) => ({
            value: `${g.provider_id}/${m.model_id}`,
            label: `${g.provider_name} / ${m.name}`,
          })),
        ),
    [availableModelGroups],
  )
  // If the parent hasn't provided a real model yet, auto-select the
  // first available one so the picker always shows a real model name.
  React.useEffect(() => {
    if (!selectedModelProp && availableModelItems.length > 0 && onModelChangeProp) {
      onModelChangeProp(availableModelItems[0].value)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [availableModelItems])
  const [selectedPermissionMode, setSelectedPermissionMode] = React.useState<PermissionMode>(permissionModeProp)
  const [selectedStrength, setSelectedStrength] = React.useState('mid')
  const [isComposerFocused, setIsComposerFocused] = React.useState(false)
  const [copiedMessageId, setCopiedMessageId] = React.useState<string | null>(null)
  const [isTodoCollapsed, setIsTodoCollapsed] = React.useState(false)
  const [todoPanelHeight, setTodoPanelHeight] = React.useState(0)
  const [draftInput, setDraftInput] = React.useState(input)

  const railState = useProjectRailState({ defaultWorkdir })
  const {
    projectRailEntries,
    setProjectRailEntries,
    projectRailTree,
    setProjectRailTree,
    projectRailExpandedPaths,
    setProjectRailExpandedPaths,
    projectRailLoadingPaths,
    setProjectRailLoadingPaths,
    isProjectRailLoading,
    setIsProjectRailLoading,
    projectRailSort,
    setProjectRailSort,
    projectRailPath,
    setProjectRailPath,
    projectRailPreviewError,
    setProjectRailPreviewError,
    projectRailRefreshSeqRef,
    projectRailNoticeTimerRef,
    showProjectRailNotice,
    projectRailWidth,
    setProjectRailWidth,
  } = railState

  const dropItemsState = useComposerDropItems()
  const composerDropItems = dropItemsState.dropItems
  const setComposerDropItems = dropItemsState.setDropItems
  const handleComposerDropItem = dropItemsState.addDropItem
  const handleRemoveComposerDropItem = dropItemsState.removeDropItem
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
  const projectRailOpenBeforePreviewRef = React.useRef(true)
  const wasPreviewFocusModeRef = React.useRef(false)
  const isProjectRailOpen = isProjectRailOpenProp
  const { densityMode, fontMode } = useDensityFontMode({
    density: densityModeProp,
    font: fontModeProp,
  })
  const overlays = useComposerOverlays()
  const {
    slashOverlay,
    setSlashOverlay,
    atOverlay,
    setAtOverlay,
    slashTimerRef,
    atTimerRef,
  } = overlays

  // Use prop-provided model if provided, otherwise fall back to local state
  const modelValue = onModelChangeProp !== undefined ? selectedModelProp : selectedModel
  const handleModelChange: React.Dispatch<React.SetStateAction<string>> = onModelChangeProp ?? setSelectedModel
  const permissionModeValue = onPermissionModeChangeProp !== undefined ? permissionModeProp : selectedPermissionMode
  const handlePermissionModeChange: React.Dispatch<React.SetStateAction<PermissionMode>> =
    onPermissionModeChangeProp ?? setSelectedPermissionMode

  // GF-01 PR-08 — every per-feature unmount cleanup (slash/at debounce
  // timers, the scroll RAF, the rail notice timer, the preview autosave
  // timers) now lives inside its owning hook. `updateBottomState` is
  // returned from `useChatScroll` above.

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

  // GF-01 PR-08 — density/font + rail width persistence now live
  // inside `useDensityFontMode` / `useProjectRailState`. The
  // workdir-change reset effect lives further down, after the
  // `useProjectPreviewState` hook initialises.

  // Forward declaration: filled in once `refreshDirectoryPreview` is
  // available below; satisfied by the hook returned from
  // `useProjectPreviewState`.
  // (See the `previewState` declaration further down.)

  const railRootPath = defaultWorkdir ?? null

  // GF-01 PR-08 — `showProjectRailNotice` is now returned from
  // `useProjectRailState` (destructured above).

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

  // GF-01 PR-08 — every preview-tab state slot + callback now lives
  // here. The shell only forwards the bag down to <ProjectPreviewMount>.
  const previewState = useProjectPreviewState({
    messages,
    defaultWorkdir,
    onProjectRailOpenChange,
    refreshDirectoryPreview,
  })
  const {
    projectPreviewTabs,
    activeProjectPreviewPath,
    setActiveProjectPreviewPath,
    isProjectPreviewOpen,
    projectPreviewDrafts,
    projectPreviewSaveStates,
    projectPreviewDirtyPaths,
    openPreviewTab: openPreviewTabInternal,
    closePreviewTab,
    closePreviewPanel,
    savePreviewDraft,
    refreshPreviewTab,
    handlePreviewDraftChange,
  } = previewState

  // Wrapper preserves the original `openPreviewTab` side-effect of
  // clearing the rail's transient preview-error notice (the inline
  // version did this in the same call site).
  const openPreviewTab = React.useCallback(
    (preview: FilePreviewPayload) => {
      openPreviewTabInternal(preview)
      setProjectRailPreviewError(null)
    },
    [openPreviewTabInternal, setProjectRailPreviewError],
  )

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
    previewState.resetAll()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [defaultWorkdir])

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
    handleScrollInternal(isLoading)
  }, [handleScrollInternal, isLoading])

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

  // GF-01 PR-08 — `handleComposerDropItem` + `handleRemoveComposerDropItem`
  // are now provided by `useComposerDropItems` (destructured above).

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

  // GF-01 PR-08 — savePreviewDraft / schedulePreviewAutosave /
  // openPreviewTab / refreshPreviewTab / closePreviewTab /
  // closePreviewPanel / handlePreviewDraftChange + the
  // file_write→refresh effect now live in `useProjectPreviewState`.

  return (
    <div className="relative flex h-full min-h-0 flex-col bg-transparent text-foreground">
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
                  className="scroll-to-bottom-guard pointer-events-auto flex h-10 w-10 items-center justify-center rounded-full border border-border/70 bg-popover/95 text-popover-foreground shadow-token-lg transition-transform duration-200 hover:-translate-y-0.5 hover:bg-accent hover:text-accent-foreground"
                  aria-label="滚动到底部"
                >
                  <ArrowDown className="h-[18px] w-[18px]" />
                </button>
              </div>
            </div>
          )}
          {todos.length > 0 && (
            <TodoPanelMount
              todoPanelRef={todoPanelRef}
              todos={todos}
              isTodoCollapsed={isTodoCollapsed}
              onToggleCollapsed={() => setIsTodoCollapsed((value) => !value)}
              todoMaxWidth={todoMaxWidth}
              chatVisibleRightInset={chatVisibleRightInset}
            />
          )}
          <ContextBarMount
            usage={latestContextBudgetUsage}
            windowSize={messages.length}
            sessionTotals={sessionTotals}
            sessionId={sessionId}
            composerMaxWidth={composerMaxWidth}
            chatVisibleRightInset={chatVisibleRightInset}
          />
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
          <ProjectPreviewMount
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
