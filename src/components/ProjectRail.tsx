import { useEffect, useMemo, useRef, useState, type ComponentType, type Dispatch, type SetStateAction } from 'react'
import {
  ArrowDownUp,
  Archive,
  ChevronDown,
  Circle,
  Folder,
  FolderOpen,
  FolderPlus,
  MoreHorizontal,
  PencilLine,
  Pin,
  Search,
  Settings2,
  Trash2,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
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
            <div className="h-9 rounded-2xl bg-black/5" />
            <div className="h-9 rounded-2xl bg-black/5" />
            <div className="h-9 rounded-2xl bg-black/5" />
            <div className="h-9 rounded-2xl bg-black/5" />
          </div>
        </div>
        <div className="shrink-0 border-t border-black/5 px-3.5 py-2.5">
          <div className="flex items-center justify-between">
            <div className="h-3.5 w-10 rounded-full bg-black/5" />
            <div className="flex gap-1">
              <div className="size-7 rounded-full bg-black/5" />
              <div className="size-7 rounded-full bg-black/5" />
              <div className="size-7 rounded-full bg-black/5" />
            </div>
          </div>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-3.5 pb-4">
          <div className="flex flex-col gap-3">
            <div className="h-11 rounded-2xl bg-black/5" />
            <div className="h-11 rounded-2xl bg-black/5" />
            <div className="h-11 rounded-2xl bg-black/5" />
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0 w-full flex-col overflow-hidden select-none">
      <div className="shrink-0 px-3 py-2.5">
        <div className="flex flex-col gap-1.5">
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
        <div className="mt-2 flex flex-col gap-1">
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

      <div className="shrink-0 border-t border-black/5 px-3 py-[9px]">
        <div className="flex items-center justify-between">
          <div className="text-[12px] font-medium tracking-tight text-black/35">线程</div>
          <div className="flex items-center gap-1 text-black/35">
            <Button
              variant="ghost"
              size="icon"
              className="size-7 cursor-pointer rounded-full p-0 text-black/40 hover:bg-black/[0.03] hover:text-black/70"
            >
              <ArrowDownUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7 cursor-pointer rounded-full p-0 text-black/40 hover:bg-black/[0.03] hover:text-black/70"
            >
              <Settings2 className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7 cursor-pointer rounded-full p-0 text-black/40 hover:bg-black/[0.03] hover:text-black/70"
            >
              <FolderPlus className="size-4" />
            </Button>
          </div>
        </div>
      </div>

      <div ref={listRef} className="min-h-0 flex-1 overflow-y-auto px-3 pb-4">
        <div className="flex flex-col gap-2.5">
          {projects.length === 0 ? (
            <div className="pl-2 text-[13px] text-black/30">暂无项目</div>
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
  icon: ComponentType<{ className?: string }>
  label: string
  onClick?: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex h-9 cursor-pointer items-center gap-2.5 rounded-2xl px-2 text-left text-[13px] font-medium tracking-tight text-black/80 transition-colors hover:bg-black/[0.03] active:bg-black/[0.05]"
    >
      <Icon className="size-4 shrink-0 text-black/70" />
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
      className="flex h-9 cursor-pointer items-center gap-2.5 rounded-2xl px-2 text-left text-[13px] font-medium tracking-tight text-black/78 transition-colors hover:bg-black/[0.03] active:bg-black/[0.05]"
    >
      <Pin className={cn('size-3.5 shrink-0 rotate-45 text-black/35', pinned && 'text-black/40')} />
      <span className="min-w-0 flex-1 truncate">{title}</span>
      <span className="shrink-0 text-[12px] font-medium text-black/35">{age}</span>
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
}) {
  const hasActiveSession = sessions.some((session) => session.id === activeSessionId)
  const isActiveProject = activeProjectId === project.id
  const isExpanded = expandedProjects[project.id] ?? isActiveProject

  const toggleProject = () => {
    setExpandedProjects((prev) => ({
      ...prev,
      [project.id]: !isExpanded,
    }))
    onSelectProject(project.id)
  }

  return (
    <div className="flex flex-col gap-1.5">
      <div className="group/project relative rounded-[16px] px-0.5 py-0.5 transition-colors hover:bg-black/[0.03] select-none">
        <button
          type="button"
          onClick={toggleProject}
          className={cn(
            'flex w-full min-w-0 cursor-pointer items-center gap-2 rounded-[14px] px-2.5 py-[7px] pr-12 text-left text-[13px] font-medium tracking-tight transition-colors',
            isActiveProject ? 'bg-black/[0.04] text-black/90' : 'text-black/72 hover:text-black/90'
          )}
        >
          <ChevronDown
            className={cn(
              'size-3.5 shrink-0 text-black/30 transition-transform',
              !isExpanded && '-rotate-90'
            )}
          />
          {isExpanded ? (
            <FolderOpen className={cn('size-4 shrink-0 text-black/45', hasActiveSession && 'text-black/60')} />
          ) : (
            <Folder className={cn('size-4 shrink-0 text-black/45', hasActiveSession && 'text-black/60')} />
          )}
          <span className="min-w-0 flex-1 truncate">{project.name}</span>
        </button>

        <div className="pointer-events-none absolute inset-y-0 right-1.5 flex items-center gap-0.5 opacity-0 transition-opacity group-hover/project:opacity-100 group-focus-within/project:opacity-100">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="pointer-events-auto size-[26px] rounded-full p-0 text-black/25 opacity-90 transition-colors hover:text-black/70 cursor-pointer"
            onClick={(event) => {
              event.stopPropagation()
              const nextName = window.prompt('重命名项目', project.name)?.trim()
              if (nextName) onRenameProject(project.id, nextName)
            }}
          >
            <PencilLine className="size-4" />
          </Button>
          <ProjectMenu
            project={project}
            onDeleteProject={onDeleteProject}
            onRenameProject={onRenameProject}
            onOpenInFinder={onOpenInFinder}
            onCreatePermanentWorktree={onCreatePermanentWorktree}
          />
        </div>
      </div>

      <div className={cn('flex flex-col gap-0.5', !isExpanded && 'hidden')}>
        {sessions.length === 0 ? (
          <div className="pl-7 text-[13px] text-black/25">无线程</div>
        ) : (
          <div className="flex flex-col gap-0.5 pl-7">
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

      <Separator className="bg-black/5" />
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
        'group/session grid h-[34px] cursor-pointer grid-cols-[18px_minmax(0,1fr)_4.25rem] items-center gap-2 rounded-[14px] px-2 text-[13px] font-medium tracking-tight transition-colors',
        isActive ? 'bg-black/[0.05] text-black/90' : 'text-black/78 hover:bg-black/[0.03] hover:text-black/90'
      )}
    >
      <button
        type="button"
        className="relative flex size-[18px] items-center justify-center text-black/34 transition-colors hover:text-black/60"
        aria-label={isPinned ? '取消置顶' : '置顶'}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation()
          void onTogglePinSession(projectId, sessionId, isPinned)
        }}
      >
        <Circle
          className={cn(
            'absolute size-3.5 transition-all',
            isPinned ? 'opacity-0 scale-75' : 'opacity-100 scale-100 group-hover/session:opacity-0'
          )}
        />
        <Pin
          className={cn(
            'absolute size-3.5 rotate-45 transition-all',
            isPinned
              ? 'opacity-100 scale-100 text-black/55'
              : 'opacity-0 scale-75 text-black/55 group-hover/session:opacity-100 group-hover/session:scale-100'
          )}
        />
      </button>

      <span className="min-w-0 truncate">{title}</span>

      <div className="relative flex h-full min-w-0 items-center justify-end">
        <span className="flex shrink-0 items-center justify-end gap-1.5 pr-1 text-right text-[12px] font-medium text-black/36 transition-opacity duration-150 group-hover/session:opacity-0">
          {isRunning ? (
            <span className="size-1.5 rounded-full bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)] animate-pulse" />
          ) : null}
          <span>{age}</span>
        </span>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="absolute right-0 top-1/2 size-7 -translate-y-1/2 rounded-full p-0 text-black/25 opacity-0 transition-colors hover:text-black/70 group-hover/session:opacity-100 focus-visible:opacity-100 cursor-pointer"
              onPointerDown={(event) => event.stopPropagation()}
              onClick={(event) => event.stopPropagation()}
            >
              <MoreHorizontal className="size-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            sideOffset={6}
            className="w-[156px] rounded-[18px] border border-black/8 bg-white/96 p-1 text-[12.5px] shadow-[0_10px_24px_rgba(0,0,0,0.12)] backdrop-blur-xl"
          >
            <DropdownMenuItem
              className="h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium text-black/82 transition-colors hover:bg-black/[0.045] focus:bg-black/[0.045] focus:text-black/90"
              onSelect={() => {
                void onTogglePinSession(projectId, sessionId, isPinned)
              }}
            >
              <Pin className="size-4 shrink-0 rotate-45 text-black/55" />
              <span>{isPinned ? '取消置顶' : '置顶'}</span>
            </DropdownMenuItem>
            <DropdownMenuSeparator className="my-1 bg-black/6" />
            <DropdownMenuItem
              className="h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium text-red-600 transition-colors hover:bg-red-50 focus:bg-red-50 focus:text-red-700"
              onSelect={() => {
                onDeleteSession(projectId, sessionId)
              }}
            >
              <Trash2 className="size-4 shrink-0 text-red-500" />
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
  onRenameProject: (id: string, newName: string) => void
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
          className="pointer-events-auto size-7 rounded-full p-0 text-black/25 opacity-90 transition-colors hover:text-black/70 cursor-pointer"
          onPointerDown={(event) => event.stopPropagation()}
        >
          <MoreHorizontal className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        sideOffset={8}
        className="w-[184px] rounded-[18px] border border-black/8 bg-white/96 p-1 text-[12.5px] shadow-[0_10px_24px_rgba(0,0,0,0.12)] backdrop-blur-xl"
      >
        <DropdownMenuItem
          className="h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium text-black/82 transition-colors hover:bg-black/[0.045] focus:bg-black/[0.045] focus:text-black/90"
          onSelect={() => {
            void Promise.resolve(onOpenInFinder(project.id)).catch((error) => {
              console.error('Failed to open project in Finder:', error)
            })
          }}
        >
          <FolderOpen className="size-4 shrink-0 text-black/55" />
          <span>Open in Finder</span>
        </DropdownMenuItem>
        <DropdownMenuItem
          className="h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium text-black/82 transition-colors hover:bg-black/[0.045] focus:bg-black/[0.045] focus:text-black/90"
          onSelect={() => {
            void Promise.resolve(onCreatePermanentWorktree(project.id)).catch((error) => {
              console.error('Failed to create permanent worktree:', error)
            })
          }}
        >
          <FolderPlus className="size-4 shrink-0 text-black/55" />
          <span>创建永久工作树</span>
        </DropdownMenuItem>
        <DropdownMenuItem
          className="h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium text-black/82 transition-colors hover:bg-black/[0.045] focus:bg-black/[0.045] focus:text-black/90"
          onSelect={() => {
            const nextName = window.prompt('重命名项目', project.name)?.trim()
            if (nextName) onRenameProject(project.id, nextName)
          }}
        >
          <PencilLine className="size-4 shrink-0 text-black/55" />
          <span>编辑名称</span>
        </DropdownMenuItem>
        <DropdownMenuItem
          className="h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium text-black/82 transition-colors hover:bg-black/[0.045] focus:bg-black/[0.045] focus:text-black/90"
          onSelect={() => {
          }}
        >
          <Archive className="size-4 shrink-0 text-black/55" />
          <span>Archive threads</span>
        </DropdownMenuItem>
        <DropdownMenuSeparator className="my-1.5 bg-black/6" />
        <DropdownMenuItem
          className="h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium text-red-600 transition-colors hover:bg-red-50 focus:bg-red-50 focus:text-red-700"
          onSelect={() => {
            onDeleteProject(project.id)
          }}
        >
          <Trash2 className="size-4 shrink-0 text-red-500" />
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
  if (diff < hour) return `${Math.max(1, Math.floor(diff / minute))} 分钟前`
  if (diff < day) return `${Math.max(1, Math.floor(diff / hour))} 小时前`
  if (diff < week) return `${Math.max(1, Math.floor(diff / day))} 天前`
  if (diff < month) return `${Math.max(1, Math.floor(diff / week))} 周前`
  return `${Math.max(1, Math.floor(diff / month))} 个月前`
}
