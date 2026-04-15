import { useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from 'react'
import {
  ArrowDownUp,
  Archive,
  ChevronDown,
  Folder,
  FolderOpen,
  FolderPlus,
  MessageSquare,
  MoreHorizontal,
  PencilLine,
  Pin,
  Search,
  Settings2,
  Trash2,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'

/**
 * Unified icon wrapper for ProjectRail.
 * All lucide icons pass through here to enforce:
 *   - strokeWidth={1.5} (lighter, less heavy than default 2)
 *   - h-4 w-4 (consistent size)
 *   - default text-muted-foreground/60 with hover lift via className
 *
 * Override color via `className` on the wrapper's parent or inline.
 */
function RailIcon({
  icon: Icon,
  className,
}: {
  icon: React.ComponentType<{ className?: string; strokeWidth?: number }>
  className?: string
}) {
  return <Icon className={cn('h-3.5 w-3.5 shrink-0', className)} strokeWidth={1.5} />
}

import type { ProjectMeta, SessionMeta } from '@/lib/tauri'

export interface ProjectRailProps {
  projects: ProjectMeta[]
  projectSessions: Record<string, SessionMeta[]>
  activeProjectId: string | null
  activeSessionId: string | null
  onSelectProject: (id: string) => void
  onSelectSession: (projectId: string, sessionId: string) => void
  onNewChat: (projectId: string) => void
  onDeleteProject: (id: string) => void
  onRenameProject: (id: string, newName: string) => void
  onDeleteSession: (projectId: string, sessionId: string) => void
  onTogglePinSession: (projectId: string, sessionId: string, pinned: boolean) => void | Promise<unknown>
  onOpenInFinder: (projectId: string) => void | Promise<unknown>
  onCreatePermanentWorktree: (projectId: string) => void | Promise<unknown>
  runningSessionIds: string[]
  loading?: boolean
}

type RecentSession = {
  projectId: string
  sessionId: string
  title: string
  updatedAt: string
}

export function ProjectRail({
  projects,
  projectSessions,
  activeProjectId,
  activeSessionId,
  onSelectProject,
  onSelectSession,
  onNewChat,
  onDeleteProject,
  onRenameProject,
  onDeleteSession,
  onTogglePinSession,
  onOpenInFinder,
  onCreatePermanentWorktree,
  runningSessionIds,
  loading = false,
}: ProjectRailProps) {
  const listRef = useRef<HTMLDivElement>(null)
  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({})
  const [editingProjectId, setEditingProjectId] = useState<string | null>(null)
  const [projectDraftName, setProjectDraftName] = useState('')
  const [nowMs, setNowMs] = useState(() => Date.now())

  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), 60_000)
    return () => window.clearInterval(timer)
  }, [])

  const pinnedSessions = useMemo<RecentSession[]>(() => {
    const items = projects.flatMap((project) =>
      (projectSessions[project.id] ?? [])
        .filter((session) => session.pinned)
        .map((session) => ({
          projectId: project.id,
          sessionId: session.id,
          title: session.title,
          updatedAt: session.updated_at,
        }))
    )

    return items.slice().sort((a, b) => +new Date(b.updatedAt) - +new Date(a.updatedAt))
  }, [projectSessions, projects])

  useEffect(() => {
    listRef.current?.scrollTo({ top: 0, behavior: 'auto' })
  }, [loading, activeProjectId, activeSessionId, projects, projectSessions])

  useEffect(() => {
    if (!activeProjectId) return
    setExpandedProjects((prev) => ({
      ...prev,
      [activeProjectId]: true,
    }))
  }, [activeProjectId])

  if (loading) {
    return (
      <div className="flex h-full min-h-0 w-full flex-col overflow-hidden">
        <div className="shrink-0 px-3.5 py-3">
          <div className="flex flex-col gap-1">
            <div className="h-9 rounded-2xl bg-muted" />
            <div className="h-9 rounded-2xl bg-muted" />
            <div className="h-9 rounded-2xl bg-muted" />
            <div className="h-9 rounded-2xl bg-muted" />
          </div>
        </div>
        <div className="shrink-0 border-t border-[var(--border)]/50 px-3.5 py-2.5">
          <div className="flex items-center justify-between">
            <div className="h-3.5 w-10 rounded-full bg-muted" />
            <div className="flex gap-1">
              <div className="size-7 rounded-full bg-muted" />
              <div className="size-7 rounded-full bg-muted" />
              <div className="size-7 rounded-full bg-muted" />
            </div>
          </div>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-3.5 pb-4">
          <div className="flex flex-col gap-3">
            <div className="h-11 rounded-2xl bg-muted" />
            <div className="h-11 rounded-2xl bg-muted" />
            <div className="h-11 rounded-2xl bg-muted" />
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0 w-full flex-col overflow-hidden select-none">
      <div className="shrink-0 px-3 py-1">
        <div className="flex flex-col gap-0.5">
          <RailNavItem
            icon={PencilLine}
            label="新线程"
            onClick={() => {
              const targetProjectId = activeProjectId ?? projects[0]?.id
              if (targetProjectId) onNewChat(targetProjectId)
            }}
          />
          <RailNavItem icon={Search} label="Search" />
        </div>
        <div className="mt-1 flex flex-col gap-0.5">
          {pinnedSessions.map((item) => (
            <RecentItem
              key={item.sessionId}
              title={item.title}
              age={formatRelativeAge(item.updatedAt, nowMs)}
              onClick={() => onSelectSession(item.projectId, item.sessionId)}
              pinned
            />
          ))}
        </div>
      </div>

      <div className="shrink-0 border-t border-border/40 px-3 py-1">
        <div className="flex items-center justify-between">
          <div className="text-[10.5px] font-medium uppercase tracking-widest text-muted-foreground/35">线程</div>
          <div className="flex items-center gap-0.5 text-muted-foreground/30">
            <Button
              variant="ghost"
              size="icon"
              className="size-7 cursor-pointer rounded-full p-0 text-muted-foreground/55 transition-colors hover:bg-sidebar-accent hover:text-foreground"
            >
              <ArrowDownUp className="size-3.5" strokeWidth={1.5} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7 cursor-pointer rounded-full p-0 text-muted-foreground/55 transition-colors hover:bg-sidebar-accent hover:text-foreground"
            >
              <Settings2 className="size-3.5" strokeWidth={1.5} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7 cursor-pointer rounded-full p-0 text-muted-foreground/55 transition-colors hover:bg-sidebar-accent hover:text-foreground"
            >
              <FolderPlus className="size-3.5" strokeWidth={1.5} />
            </Button>
          </div>
        </div>
      </div>

      <div ref={listRef} className="min-h-0 flex-1 overflow-y-auto px-3 pb-2.5">
        <div className="flex flex-col gap-1">
          {projects.length === 0 ? (
          <div className="px-2 py-6 text-center text-[12px] text-muted-foreground/30">暂无项目</div>
          ) : (
            projects.map((project) => (
              <ProjectGroup
                key={project.id}
                project={project}
                sessions={projectSessions[project.id] ?? []}
                activeProjectId={activeProjectId}
                activeSessionId={activeSessionId}
                expandedProjects={expandedProjects}
                setExpandedProjects={setExpandedProjects}
                onSelectProject={onSelectProject}
                onSelectSession={onSelectSession}
                onDeleteProject={onDeleteProject}
                onRenameProject={onRenameProject}
                onDeleteSession={onDeleteSession}
                onTogglePinSession={onTogglePinSession}
                onOpenInFinder={onOpenInFinder}
                onCreatePermanentWorktree={onCreatePermanentWorktree}
                runningSessionIds={runningSessionIds}
                nowMs={nowMs}
                editingProjectId={editingProjectId}
                projectDraftName={projectDraftName}
                setEditingProjectId={setEditingProjectId}
                setProjectDraftName={setProjectDraftName}
              />
            ))
          )}
        </div>
      </div>
    </div>
  )
}

function RailNavItem({
  icon: Icon,
  label,
  onClick,
}: {
  icon: React.ComponentType<{ className?: string; strokeWidth?: number }>
  label: string
  onClick?: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex h-8 cursor-pointer items-center gap-2 rounded-xl px-2 text-left text-[13px] font-medium tracking-tight text-primary transition-colors hover:bg-primary/10 hover:text-primary"
    >
      <RailIcon icon={Icon} className="text-primary/70" />
      <span>{label}</span>
    </button>
  )
}

function RecentItem({
  title,
  age,
  pinned = false,
  onClick,
}: {
  title: string
  age: string
  pinned?: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex h-7 cursor-pointer items-center gap-1.5 rounded-xl px-0.5 text-left text-[12px] text-sidebar-foreground/55 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
    >
      <Pin className={cn('size-3 shrink-0 rotate-45 text-muted-foreground/35', pinned && 'text-muted-foreground/60')} strokeWidth={1.5} />
      <span className="min-w-0 flex-1 truncate font-medium">{title}</span>
      <span className="shrink-0 text-[10px] text-muted-foreground/40">{age}</span>
    </button>
  )
}

function ProjectGroup({
  project,
  sessions,
  activeProjectId,
  activeSessionId,
  expandedProjects,
  setExpandedProjects,
  onSelectProject,
  onSelectSession,
  onDeleteProject,
  onRenameProject,
  onDeleteSession,
  onTogglePinSession,
  onOpenInFinder,
  onCreatePermanentWorktree,
  runningSessionIds,
  nowMs,
  editingProjectId,
  projectDraftName,
  setEditingProjectId,
  setProjectDraftName,
}: {
  project: ProjectMeta
  sessions: SessionMeta[]
  activeProjectId: string | null
  activeSessionId: string | null
  expandedProjects: Record<string, boolean>
  setExpandedProjects: Dispatch<SetStateAction<Record<string, boolean>>>
  onSelectProject: (id: string) => void
  onSelectSession: (projectId: string, sessionId: string) => void
  onDeleteProject: (id: string) => void
  onRenameProject: (id: string, newName: string) => void
  onDeleteSession: (projectId: string, sessionId: string) => void
  onTogglePinSession: (projectId: string, sessionId: string, pinned: boolean) => void | Promise<unknown>
  onOpenInFinder: (projectId: string) => void | Promise<unknown>
  onCreatePermanentWorktree: (projectId: string) => void | Promise<unknown>
  runningSessionIds: string[]
  nowMs: number
  editingProjectId: string | null
  projectDraftName: string
  setEditingProjectId: Dispatch<SetStateAction<string | null>>
  setProjectDraftName: Dispatch<SetStateAction<string>>
}) {
  const hasActiveSession = sessions.some((session) => session.id === activeSessionId)
  const isActiveProject = activeProjectId === project.id
  const isExpanded = expandedProjects[project.id] ?? isActiveProject
  const isEditing = editingProjectId === project.id

  const toggleProject = () => {
    setExpandedProjects((prev) => ({
      ...prev,
      [project.id]: !isExpanded,
    }))
    onSelectProject(project.id)
  }

  const beginInlineRename = () => {
    setEditingProjectId(project.id)
    setProjectDraftName(project.name)
  }

  const commitInlineRename = () => {
    const trimmed = projectDraftName.trim()
    if (trimmed && trimmed !== project.name) {
      onRenameProject(project.id, trimmed)
    }
    setEditingProjectId(null)
    setProjectDraftName('')
  }

  const cancelInlineRename = () => {
    setEditingProjectId(null)
    setProjectDraftName('')
  }

  return (
    <div className="flex flex-col gap-0.5">
      <div className="group/project relative rounded-xl px-0.5 py-0.5 select-none">
        {isEditing ? (
          <div className="flex items-center gap-2 rounded-xl bg-accent/60 px-2.5 py-1.5 pr-2">
            <button type="button" onClick={toggleProject} className="flex shrink-0 items-center gap-2 text-foreground/70">
              <ChevronDown
                className={cn(
                  'size-3 shrink-0 text-muted-foreground/50 transition-transform',
                  !isExpanded && '-rotate-90'
                )}
                strokeWidth={1.5}
              />
              {isExpanded ? (
                <FolderOpen className={cn('size-3.5 shrink-0 text-muted-foreground/60', hasActiveSession && 'text-foreground/70')} strokeWidth={1.5} />
              ) : (
                <Folder className={cn('size-3.5 shrink-0 text-muted-foreground/60', hasActiveSession && 'text-foreground/70')} strokeWidth={1.5} />
              )}
            </button>
            <Input
              autoFocus
              value={projectDraftName}
              onChange={(event) => setProjectDraftName(event.target.value)}
              onBlur={commitInlineRename}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault()
                  commitInlineRename()
                }
                if (event.key === 'Escape') {
                  event.preventDefault()
                  cancelInlineRename()
                }
              }}
              className="h-7 flex-1 rounded-xl border border-border bg-surface px-2.5 text-[12.5px] shadow-none focus-visible:ring-0"
            />
          </div>
        ) : (
          <button
            type="button"
            onClick={toggleProject}
            className={cn(
              'relative flex w-full min-w-0 cursor-pointer items-center gap-2 rounded-xl px-3 py-2 text-left text-[13px] tracking-tight transition-colors',
              isActiveProject
                ? 'bg-sidebar-accent text-foreground font-semibold'
                : 'text-sidebar-foreground/60 hover:bg-sidebar-accent hover:text-sidebar-foreground font-medium'
            )}
          >
            <ChevronDown
              className={cn(
                'size-3.5 shrink-0 text-muted-foreground/50 transition-transform',
                !isExpanded && '-rotate-90',
                isActiveProject && 'text-foreground/60'
              )}
              strokeWidth={1.5}
            />
            {isExpanded ? (
              <FolderOpen className={cn('size-3.5 shrink-0 text-muted-foreground/60', hasActiveSession && 'text-foreground/70', isActiveProject && 'text-foreground/70')} strokeWidth={1.5} />
            ) : (
              <Folder className={cn('size-3.5 shrink-0 text-muted-foreground/60', hasActiveSession && 'text-foreground/70', isActiveProject && 'text-foreground/70')} strokeWidth={1.5} />
            )}
            <span className="min-w-0 flex-1 truncate">{project.name}</span>
          </button>
        )}

        <div className="pointer-events-none absolute inset-y-0 right-1.5 flex items-center gap-0.5 opacity-0 transition-opacity group-hover/project:opacity-100 group-focus-within/project:opacity-100">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="pointer-events-auto size-[26px] rounded-full p-0 text-muted-foreground/50 transition-colors hover:text-foreground cursor-pointer"
            onClick={(event) => {
              event.stopPropagation()
              beginInlineRename()
            }}
          >
            <PencilLine className="size-3.5" strokeWidth={1.5} />
          </Button>
          <ProjectMenu
            project={project}
            onDeleteProject={onDeleteProject}
            onRenameProject={beginInlineRename}
            onOpenInFinder={onOpenInFinder}
            onCreatePermanentWorktree={onCreatePermanentWorktree}
          />
        </div>
      </div>

      <div className={cn('flex flex-col gap-0.5', !isExpanded && 'hidden')}>
        {sessions.length === 0 ? (
          <div className="px-2 py-3 text-[11px] text-muted-foreground/20">无线程</div>
        ) : (
          <div className="flex flex-col gap-0.5 pl-1.5">
            {sessions.map((session) => (
              <SessionRow
                key={session.id}
                projectId={project.id}
                sessionId={session.id}
                title={session.title}
                age={formatRelativeAge(session.updated_at, nowMs)}
                isPinned={session.pinned}
                isRunning={runningSessionIds.includes(session.id)}
                isActive={activeSessionId === session.id}
                onSelectSession={onSelectSession}
                onTogglePinSession={onTogglePinSession}
                onDeleteSession={onDeleteSession}
              />
            ))}
          </div>
        )}
      </div>

      <Separator className="my-1 bg-border/20" />
    </div>
  )
}

function SessionRow({
  projectId,
  sessionId,
  title,
  age,
  isPinned,
  isRunning,
  isActive,
  onSelectSession,
  onTogglePinSession,
  onDeleteSession,
}: {
  projectId: string
  sessionId: string
  title: string
  age: string
  isPinned: boolean
  isRunning: boolean
  isActive?: boolean
  onSelectSession: (projectId: string, sessionId: string) => void
  onTogglePinSession: (projectId: string, sessionId: string, pinned: boolean) => void | Promise<unknown>
  onDeleteSession: (projectId: string, sessionId: string) => void
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => onSelectSession(projectId, sessionId)}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault()
          onSelectSession(projectId, sessionId)
        }
      }}
      className={cn(
        'group/session relative grid h-7 w-full cursor-pointer grid-cols-[16px_minmax(0,1fr)_auto] items-center gap-2 rounded-lg px-1.5 text-[12px] transition-colors',
        isActive
          ? 'bg-primary/12 text-primary font-medium'
          : 'text-sidebar-foreground/55 hover:bg-sidebar-accent/60 hover:text-sidebar-foreground font-normal'
      )}
    >
      <button
        type="button"
        className="relative flex size-[18px] items-center justify-center text-muted-foreground/55 transition-colors hover:text-foreground"
        aria-label={isPinned ? '取消置顶' : '置顶'}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation()
          void onTogglePinSession(projectId, sessionId, isPinned)
        }}
      >
        <MessageSquare
          className={cn(
            'absolute size-3.5 transition-all',
            isPinned ? 'opacity-0 scale-75' : 'opacity-100 scale-100 group-hover/session:opacity-0'
          )}
          strokeWidth={1.5}
        />
        <Pin
          className={cn(
            'absolute size-3.5 rotate-45 transition-all',
            isPinned
              ? 'opacity-100 scale-100 text-muted-foreground/60'
              : 'opacity-0 scale-75 text-muted-foreground/60 group-hover/session:opacity-100 group-hover/session:scale-100'
          )}
          strokeWidth={1.5}
        />
      </button>

      <span className="min-w-0 truncate">{title}</span>

      <div className="relative flex h-full min-w-0 items-center justify-end">
        <span className="flex shrink-0 items-center justify-end gap-1 pr-0.5 text-right text-[10px] font-normal text-muted-foreground/30 transition-opacity duration-150 group-hover/session:opacity-0">
          {isRunning ? (
            <span className="inline-flex size-2 items-center justify-center">
              <span className="absolute size-1.5 rounded-full bg-primary/20 animate-pulse" />
              <span className="relative size-1 rounded-full bg-primary animate-pulse" />
            </span>
          ) : null}
          <span>{age}</span>
        </span>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="absolute right-0 top-1/2 size-7 -translate-y-1/2 rounded-full p-0 text-muted-foreground/50 opacity-0 transition-colors hover:text-foreground group-hover/session:opacity-100 focus-visible:opacity-100 cursor-pointer"
              onPointerDown={(event) => event.stopPropagation()}
              onClick={(event) => event.stopPropagation()}
            >
              <MoreHorizontal className="size-3.5" strokeWidth={1.5} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            sideOffset={6}
            className="w-[156px] rounded-lg border border-border/10 bg-popover/96 p-1 text-[12.5px] shadow-token-lg backdrop-blur-xl"
          >
            <DropdownMenuItem
              className="h-8 rounded-lg px-2.5 text-[12.5px] font-medium text-foreground/82 transition-colors hover:bg-accent focus:bg-accent focus:text-foreground/90"
              onSelect={() => {
                void onTogglePinSession(projectId, sessionId, isPinned)
              }}
            >
              <Pin className="size-3.5 shrink-0 rotate-45 text-muted-foreground/60" strokeWidth={1.5} />
              <span>{isPinned ? '取消置顶' : '置顶'}</span>
            </DropdownMenuItem>
            <DropdownMenuSeparator className="my-1 bg-border/10" />
            <DropdownMenuItem
              className="h-8 rounded-lg px-2.5 text-[12.5px] font-medium text-destructive transition-colors hover:bg-destructive/10 focus:bg-destructive/10 focus:text-destructive"
              onSelect={() => {
                onDeleteSession(projectId, sessionId)
              }}
            >
              <Trash2 className="size-3.5 shrink-0 text-destructive" />
              <span>移除</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  )
}

function ProjectMenu({
  project,
  onDeleteProject,
  onRenameProject,
  onOpenInFinder,
  onCreatePermanentWorktree,
}: {
  project: ProjectMeta
  onDeleteProject: (id: string) => void
  onRenameProject: () => void
  onOpenInFinder: (projectId: string) => void | Promise<unknown>
  onCreatePermanentWorktree: (projectId: string) => void | Promise<unknown>
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="pointer-events-auto size-7 rounded-full p-0 text-muted-foreground/50 transition-colors hover:text-foreground cursor-pointer"
          onPointerDown={(event) => event.stopPropagation()}
        >
          <MoreHorizontal className="size-3.5" strokeWidth={1.5} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        sideOffset={8}
        className="w-[184px] rounded-lg border border-border/10 bg-popover/96 p-1 text-[12.5px] shadow-token-lg backdrop-blur-xl"
      >
        <DropdownMenuItem
          className="h-8 rounded-lg px-2.5 text-[12.5px] font-medium text-foreground/82 transition-colors hover:bg-accent focus:bg-accent focus:text-foreground/90"
          onSelect={() => {
            void Promise.resolve(onOpenInFinder(project.id)).catch((error) => {
              console.error('Failed to open project in Finder:', error)
            })
          }}
        >
          <FolderOpen className="size-3.5 shrink-0 text-muted-foreground/60" strokeWidth={1.5} />
          <span>Open in Finder</span>
        </DropdownMenuItem>
        <DropdownMenuItem
          className="h-8 rounded-lg px-2.5 text-[12.5px] font-medium text-foreground/82 transition-colors hover:bg-accent focus:bg-accent focus:text-foreground/90"
          onSelect={() => {
            void Promise.resolve(onCreatePermanentWorktree(project.id)).catch((error) => {
              console.error('Failed to create permanent worktree:', error)
            })
          }}
        >
          <FolderPlus className="size-3.5 shrink-0 text-muted-foreground/60" strokeWidth={1.5} />
          <span>创建永久工作树</span>
        </DropdownMenuItem>
        <DropdownMenuItem
          className="h-8 rounded-lg px-2.5 text-[12.5px] font-medium text-foreground/82 transition-colors hover:bg-accent focus:bg-accent focus:text-foreground/90"
          onSelect={() => {
            onRenameProject()
          }}
        >
          <PencilLine className="size-3.5 shrink-0 text-muted-foreground/60" strokeWidth={1.5} />
          <span>编辑名称</span>
        </DropdownMenuItem>
        <DropdownMenuItem
          className="h-8 rounded-lg px-2.5 text-[12.5px] font-medium text-foreground/82 transition-colors hover:bg-accent focus:bg-accent focus:text-foreground/90"
          onSelect={() => {
          }}
        >
          <Archive className="size-3.5 shrink-0 text-muted-foreground/60" strokeWidth={1.5} />
          <span>Archive threads</span>
        </DropdownMenuItem>
        <DropdownMenuSeparator className="my-1.5 bg-border/10" />
        <DropdownMenuItem
          className="h-8 rounded-lg px-2.5 text-[12.5px] font-medium text-destructive transition-colors hover:bg-destructive/10 focus:bg-destructive/10 focus:text-destructive"
          onSelect={() => {
            onDeleteProject(project.id)
          }}
        >
          <Trash2 className="size-3.5 shrink-0 text-destructive" />
          <span>移除</span>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function formatRelativeAge(value: string, nowMs: number) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '刚刚'

  const diff = Math.max(0, nowMs - date.getTime())
  const minute = 60 * 1000
  const hour = 60 * minute
  const day = 24 * hour
  const week = 7 * day
  const month = 30 * day

  if (diff < minute) return '刚刚'
  if (diff < hour) return `${Math.max(1, Math.floor(diff / minute))}分`
  if (diff < day) return `${Math.max(1, Math.floor(diff / hour))}时`
  if (diff < week) return `${Math.max(1, Math.floor(diff / day))}天`
  if (diff < month) return `${Math.max(1, Math.floor(diff / week))}周`
  return `${Math.max(1, Math.floor(diff / month))}月`
}
