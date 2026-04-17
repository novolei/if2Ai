import type { PermissionMode, Project, ProjectMeta, SessionMeta } from '@/lib/tauri'
import type { Dispatch, SetStateAction } from 'react'
import type { TodoItem } from '@/components/ui/TodoPanel'

/** A recent session entry shown in the Home screen suggestion list. */
export interface RecentSession {
  sessionId: string
  projectId: string
  projectName: string
  title: string
  updatedAt: string
}

export interface Message {
  id: string
  role: 'user' | 'assistant' | 'tool'
  content: string
  timestamp: Date
  thinking?: string
  thinkingTime?: number
  disableAnimation?: boolean
  isStreaming?: boolean
  // tool_call_update fields
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
  statusLabel?: string
  statusKind?: 'info' | 'success' | 'partial' | 'failed'
  isRecovering?: boolean
}

export interface Conversation {
  id: string
  projectId: string
  title: string
  messages: Message[]
  updatedAt: Date
}

export type SessionTitleStage = 'placeholder' | 'provisional' | 'locked' | 'manual'

export interface SessionTitleState {
  stage: SessionTitleStage
  autoRenameCount: number
}

export interface ChatWorkspaceProps {
  projects: ProjectMeta[]
  projectSessions: Record<string, SessionMeta[]>
  activeProjectId: string | null
  activeSessionId: string | null
  currentProject: Project | null
  branchLabel: string
  activeTitle: string
  activeMessages: Message[]
  input: string
  isLoading: boolean
  loading: boolean
  onSelectProject: (id: string) => void
  onSelectSession: (projectId: string, sessionId: string, projectOverride?: ProjectMeta) => Promise<void>
  onNewChat: (projectId: string) => void
  onNewThread: () => void
  /** Select a project for the Home screen without creating a session. */
  onHomeProjectSelect: (projectId: string | null) => void
  onPickFolderAndCreateProject: () => Promise<void>
  /** Send the first message (called from HomeScreen composer). */
  onSendMessage: (text: string) => void
  recentSessions: RecentSession[]
  onDeleteProject: (id: string) => void
  onRenameProject: (id: string, newName: string) => void
  onDeleteSession: (projectId: string, sessionId: string) => void
  /** User-initiated session rename; sets stage to 'manual' to block future auto-renames. */
  onRenameSession: (sessionId: string, newTitle: string) => void
  onTogglePinSession: (projectId: string, sessionId: string, pinned: boolean) => void | Promise<unknown>
  onOpenInFinder: (projectId: string) => void | Promise<unknown>
  onCreatePermanentWorktree: (projectId: string) => void | Promise<unknown>
  onInputChange: (value: string) => void
  onSubmit: () => void
  onResumeFromCursor?: (resumeCursor: string) => void
  onStop?: () => void
  selectedModel: string
  onModelChange: Dispatch<SetStateAction<string>>
  permissionMode: PermissionMode
  onPermissionModeChange: Dispatch<SetStateAction<PermissionMode>>
  todos: TodoItem[]
  isRightRailOpen: boolean
  onToggleRightRail: () => void
  onRightRailOpenChange: Dispatch<SetStateAction<boolean>>
  leftPaneWidth: number
  isLeftPaneCollapsed: boolean
  onResizeStart: (event: React.PointerEvent<HTMLDivElement>) => void
  onToggleLeftPane: () => void
  onStartWindowDrag: (event: React.MouseEvent<HTMLElement>) => void
  onPreviewFocusChange: (active: boolean) => void
  runningSessionIds: string[]
}
