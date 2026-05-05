/**
 * GF-01 PR-06 — typed prop surface for {@link ComposerDock}.
 *
 * Extracted into a sibling module so the component file stays under the
 * 500-LOC hard cap. Prop names + types are copied verbatim from the
 * legacy inline declaration in `src/components/ui/chat-ui.tsx`.
 */

import type * as React from "react"
import type {
  DirectoryEntryPreview,
  PermissionMode,
} from "@/lib/tauri"
import type { ComposerDropItem } from "@/components/ui/chat-ui"

/** Snapshot of the slash-overlay state read by the composer. */
export interface SlashOverlayState {
  visible: boolean
  selectedIndex: number
  suggestions: string[]
  rawInput: string
}

/** Snapshot of the @-mention overlay state read by the composer. */
export interface AtOverlayState {
  visible: boolean
  selectedIndex: number
  entries: DirectoryEntryPreview[]
  query: string
  atPos: number
  browsePath: string | null
  breadcrumbs: Array<{ name: string; path: string | null }>
  pinned: boolean
}

/** Props for {@link ComposerDock} — preserved verbatim from the legacy declaration. */
export interface ComposerDockProps {
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
  setSlashOverlay: React.Dispatch<React.SetStateAction<SlashOverlayState | null>>
  /** Current slash overlay state (read) – used to render inline suggestions */
  slashOverlayState?: SlashOverlayState | null
  /** Called when the user selects a slash suggestion */
  onSlashSelect?: (value: string) => void
  slashTimerRef: React.RefObject<ReturnType<typeof setTimeout> | null>
  /** Current @-mention overlay state */
  atOverlayState?: AtOverlayState | null
  setAtOverlay: React.Dispatch<React.SetStateAction<AtOverlayState | null>>
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
}
