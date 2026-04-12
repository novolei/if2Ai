import type { Project, ProjectMeta, SessionMeta } from '@/lib/tauri'
import type { Dispatch, SetStateAction } from 'react'
import type { TodoItem } from '@/components/ui/TodoPanel'

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
  toolCallId?: string
  toolName?: string
  toolArgs?: Record<string, unknown>
  toolDurationMs?: number
  isError?: boolean
}

export interface Conversation {
  id: string
  projectId: string
  title: string
  messages: Message[]
  updatedAt: Date
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
  onDeleteProject: (id: string) => void
  onRenameProject: (id: string, newName: string) => void
  onDeleteSession: (projectId: string, sessionId: string) => void
  onTogglePinSession: (projectId: string, sessionId: string, pinned: boolean) => void | Promise<unknown>
  onOpenInFinder: (projectId: string) => void | Promise<unknown>
  onCreatePermanentWorktree: (projectId: string) => void | Promise<unknown>
  onInputChange: (value: string) => void
  onSubmit: () => void
  onStop?: () => void
  selectedModel: string
  onModelChange: Dispatch<SetStateAction<string>>
  todos: TodoItem[]
  leftPaneWidth: number
  onResizeStart: (event: React.PointerEvent<HTMLDivElement>) => void
  onStartWindowDrag: (event: React.MouseEvent<HTMLElement>) => void
  runningSessionIds: string[]
  status?: 'idle' | 'running'
}
