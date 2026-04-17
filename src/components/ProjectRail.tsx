import { useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from 'react'
import {
  ArrowDownUp,
  Archive,
  ChevronDown,
  ChevronsDownUp,
  ChevronsUpDown,
  EyeOff,
  Folder,
  FolderOpen,
  FolderPlus,
  MoreHorizontal,
  PencilLine,
  Pin,
  Search,
  SlidersHorizontal,
  Trash2,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { GlobalSearch } from '@/components/GlobalSearch'

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
  /** User-initiated session rename; triggers the 'manual' stage to permanently block auto-rename. */
  onRenameSession: (sessionId: string, newTitle: string) => void
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
  onNewChat: _onNewChat,
  onDeleteProject,
  onRenameProject,
  onDeleteSession,
  onRenameSession,
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
  // 'recent' = order by last-session updated_at; 'alpha' = A-Z by project name
  const [sortOrder, setSortOrder] = useState<'recent' | 'alpha'>('recent')
  // When true, projects that have zero sessions are hidden
  const [hideEmpty, setHideEmpty] = useState(false)
  // Global search modal
  const [searchOpen, setSearchOpen] = useState(false)

  // ⌘K / Ctrl+K opens the search modal from anywhere in the app
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        setSearchOpen((v) => !v)
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

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

  const sortedProjects = useMemo(() => {
    let list = hideEmpty
      ? projects.filter((p) => (projectSessions[p.id] ?? []).length > 0)
      : projects
    if (sortOrder === 'alpha') {
      list = [...list].sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }))
    } else {
      // 'recent': sort by the most-recently-updated session in each project
      list = [...list].sort((a, b) => {
        const latestA = (projectSessions[a.id] ?? []).reduce(
          (t, s) => Math.max(t, +new Date(s.updated_at)), 0
        )
        const latestB = (projectSessions[b.id] ?? []).reduce(
          (t, s) => Math.max(t, +new Date(s.updated_at)), 0
        )
        return latestB - latestA
      })
    }
    return list
  }, [projects, projectSessions, sortOrder, hideEmpty])

  /** Expand or collapse every project at once. */
  const collapseAll = () =>
    setExpandedProjects(Object.fromEntries(projects.map((p) => [p.id, false])))
  const expandAll = () =>
    setExpandedProjects(Object.fromEntries(projects.map((p) => [p.id, true])))
  const allCollapsed = projects.every((p) => expandedProjects[p.id] === false)

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
      {/* ── Top nav actions ─────────────────────────────────── */}
      <div className="shrink-0 px-3 pt-2 pb-1">
        <RailNavItem
          icon={Search}
          label="搜索"
          onClick={() => setSearchOpen(true)}
          hint="⌘K"
        />
      </div>

      {/* ── Pinned sessions (only rendered when non-empty) ─── */}
      {pinnedSessions.length > 0 && (
        <div className="shrink-0 border-t border-black/[0.06] px-3 pt-2 pb-1.5">
          {/* Section label — same style as "线程" below */}
          <div className="mb-1 flex items-center gap-1.5 px-1">
            <Pin className="size-2.5 shrink-0 rotate-45 text-muted-foreground/35" strokeWidth={2} />
            <span className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/35">
              置顶
            </span>
          </div>
          <div className="flex flex-col">
            {pinnedSessions.map((item) => (
              <PinnedItem
                key={item.sessionId}
                title={item.title}
                age={formatRelativeAge(item.updatedAt, nowMs)}
                isActive={activeSessionId === item.sessionId}
                onClick={() => onSelectSession(item.projectId, item.sessionId)}
                onUnpin={() => void onTogglePinSession(item.projectId, item.sessionId, true)}
              />
            ))}
          </div>
        </div>
      )}

      {/* ── Threads section label + toolbar ─────────────────── */}
      <div className="shrink-0 border-t border-black/[0.06] px-3 py-1.5">
        <div className="flex items-center justify-between">
          <div className="px-1 text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/35">线程</div>
          <div className="flex items-center gap-0">

            {/* Sort toggle: recent ↔ alpha */}
            <Button
              variant="ghost"
              size="icon"
              title={sortOrder === 'recent' ? '当前：最近活跃，点击切换为字母排序' : '当前：字母排序，点击切换为最近活跃'}
              onClick={() => setSortOrder((o) => o === 'recent' ? 'alpha' : 'recent')}
              className={cn(
                'size-6 cursor-pointer rounded p-0 transition-colors hover:bg-sidebar-accent hover:text-foreground',
                sortOrder === 'alpha'
                  ? 'text-primary/70'
                  : 'text-muted-foreground/40'
              )}
            >
              <ArrowDownUp className="size-3" strokeWidth={1.5} />
            </Button>

            {/* Display settings dropdown */}
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  title="显示选项"
                  className={cn(
                    'size-6 cursor-pointer rounded p-0 transition-colors hover:bg-sidebar-accent hover:text-foreground',
                    hideEmpty ? 'text-primary/70' : 'text-muted-foreground/40'
                  )}
                >
                  <SlidersHorizontal className="size-3" strokeWidth={1.5} />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent
                align="end"
                sideOffset={8}
                className="w-[172px] rounded-lg border border-border/10 bg-popover/96 p-1 text-[12px] shadow-token-lg backdrop-blur-xl"
              >
                <DropdownMenuItem
                  className="h-8 rounded-lg px-2.5 font-medium text-foreground/80 transition-colors hover:bg-accent focus:bg-accent"
                  onSelect={() => (allCollapsed ? expandAll() : collapseAll())}
                >
                  {allCollapsed ? (
                    <ChevronsUpDown className="size-3.5 shrink-0 text-muted-foreground/60" strokeWidth={1.5} />
                  ) : (
                    <ChevronsDownUp className="size-3.5 shrink-0 text-muted-foreground/60" strokeWidth={1.5} />
                  )}
                  <span>{allCollapsed ? '展开全部' : '收起全部'}</span>
                </DropdownMenuItem>
                <DropdownMenuSeparator className="my-1 bg-border/10" />
                <DropdownMenuItem
                  className={cn(
                    'h-8 rounded-lg px-2.5 font-medium transition-colors hover:bg-accent focus:bg-accent',
                    hideEmpty ? 'text-primary' : 'text-foreground/80'
                  )}
                  onSelect={() => setHideEmpty((v) => !v)}
                >
                  <EyeOff className={cn('size-3.5 shrink-0', hideEmpty ? 'text-primary/70' : 'text-muted-foreground/60')} strokeWidth={1.5} />
                  <span>隐藏空项目</span>
                  {hideEmpty && (
                    <span className="ml-auto text-[10px] text-primary/60">✓</span>
                  )}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>

          </div>
        </div>
      </div>

      <div ref={listRef} className="min-h-0 flex-1 overflow-y-auto px-3 pb-2.5">
        <div className="flex flex-col gap-0">
          {sortedProjects.length === 0 ? (
          <div className="px-2 py-6 text-center text-[12px] text-muted-foreground/30">
            {hideEmpty ? '所有项目均为空' : '暂无项目'}
          </div>
          ) : (
            sortedProjects.map((project) => (
              <ProjectGroup
                key={project.id}
                project={project}
                sessions={projectSessions[project.id] ?? []}
                activeProjectId={activeProjectId}
                activeSessionId={activeSessionId}
                expandedProjects={expandedProjects}
                setExpandedProjects={setExpandedProjects}
                onSelectSession={onSelectSession}
                onSelectProject={onSelectProject}
                onDeleteProject={onDeleteProject}
                onRenameProject={onRenameProject}
                onDeleteSession={onDeleteSession}
                onRenameSession={onRenameSession}
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

      {/* ── Global search modal ──────────────────────────── */}
      <GlobalSearch
        open={searchOpen}
        onOpenChange={setSearchOpen}
        projects={projects}
        projectSessions={projectSessions}
        activeProjectId={activeProjectId}
        onSelectSession={onSelectSession}
        onSelectProject={onSelectProject}
      />
    </div>
  )
}

function RailNavItem({
  icon: Icon,
  label,
  onClick,
  hint,
}: {
  icon: React.ComponentType<{ className?: string; strokeWidth?: number }>
  label: string
  onClick?: () => void
  hint?: string
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group flex h-8 w-full cursor-pointer items-center gap-2 rounded-xl px-2 text-left text-[13px] font-medium tracking-tight text-primary transition-colors hover:bg-primary/10 hover:text-primary"
    >
      <RailIcon icon={Icon} className="text-primary/70" />
      <span className="flex-1">{label}</span>
      {hint && (
        <span className="rounded bg-primary/8 px-1.5 py-0.5 font-mono text-[10px] text-primary/35 opacity-0 transition-opacity group-hover:opacity-100">
          {hint}
        </span>
      )}
    </button>
  )
}

/** Pinned session row — visually consistent with SessionRow but with a pin accent. */
function PinnedItem({
  title,
  age,
  isActive,
  onClick,
  onUnpin,
}: {
  title: string
  age: string
  isActive: boolean
  onClick: () => void
  onUnpin: () => void
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onClick() }
      }}
      className={cn(
        'group/pinned relative flex h-[26px] w-full cursor-pointer items-center gap-1.5 rounded-sm px-2 text-[11.5px] transition-colors',
        isActive
          ? 'bg-primary/10 text-primary font-medium'
          : 'text-foreground/45 hover:bg-black/[0.04] hover:text-foreground/65 font-normal'
      )}
    >
      {/* Pin indicator — amber accent */}
      <Pin
        className={cn(
          'size-2.5 shrink-0 rotate-45 transition-colors',
          isActive ? 'text-primary/60' : 'text-amber-400/60'
        )}
        strokeWidth={2}
      />

      <span className="min-w-0 flex-1 truncate">{title}</span>

      {/* Trailing area: age fades out on hover, unpin button fades in */}
      <div className="relative flex h-full shrink-0 items-center">
        <span className="text-[10px] text-muted-foreground/30 transition-opacity duration-100 group-hover/pinned:opacity-0">
          {age}
        </span>
        <button
          type="button"
          aria-label="取消置顶"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => { e.stopPropagation(); onUnpin() }}
          className={cn(
            'absolute right-0 top-1/2 flex size-5 -translate-y-1/2 items-center justify-center rounded',
            'opacity-0 transition-all duration-100 group-hover/pinned:opacity-100',
            'text-muted-foreground/40 hover:bg-black/[0.06] hover:text-foreground/70',
            'cursor-pointer'
          )}
        >
          <Pin className="size-2.5 rotate-45" strokeWidth={2} />
        </button>
      </div>
    </div>
  )
}

/** A compact "+ 新聊天" row that sits at the bottom of each project's session list. */
function NewChatRow({ onClick }: { onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'group flex h-[24px] w-full cursor-pointer items-center gap-1.5 rounded-sm pl-3 pr-2',
        'text-[11px] font-medium text-muted-foreground/30',
        'transition-colors hover:bg-jade/[0.07] hover:text-jade/70',
      )}
    >
      <svg
        className="size-2.5 shrink-0 transition-transform group-hover:rotate-90"
        fill="none"
        stroke="currentColor"
        strokeWidth={2.5}
        viewBox="0 0 10 10"
      >
        <path strokeLinecap="round" d="M5 1v8M1 5h8" />
      </svg>
      <span>新聊天</span>
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
  onSelectSession,
  onSelectProject,
  onDeleteProject,
  onRenameProject,
  onDeleteSession,
  onRenameSession,
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
  onSelectSession: (projectId: string, sessionId: string) => void
  /** Navigate to Home with this project pre-selected; session created only on first send. */
  onSelectProject: (projectId: string) => void
  onDeleteProject: (id: string) => void
  onRenameProject: (id: string, newName: string) => void
  onDeleteSession: (projectId: string, sessionId: string) => void
  onRenameSession: (sessionId: string, newTitle: string) => void
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
    // Intentionally NOT calling onSelectProject here —
    // clicking the project row only expands/collapses the session list.
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
    // mt-2.5 between groups creates breathing room; first:mt-0 removes top gap for first item
    <div className="mt-1.5 flex flex-col first:mt-0.5">

      {/* ── Project header ────────────────────────────────────────────── */}
      <div className="group/project relative select-none">
        {isEditing ? (
          <div className="flex items-center gap-1.5 rounded-md bg-black/[0.04] px-2 py-[5px] pr-1.5">
            <ChevronDown
              className={cn(
                'size-2.5 shrink-0 text-foreground/30 transition-transform',
                !isExpanded && '-rotate-90'
              )}
              strokeWidth={2.5}
            />
            {isExpanded ? (
              <FolderOpen className="size-3 shrink-0 text-foreground/40" strokeWidth={1.5} />
            ) : (
              <Folder className="size-3 shrink-0 text-foreground/40" strokeWidth={1.5} />
            )}
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
              className="h-5 flex-1 rounded border-0 bg-transparent px-0 text-[11.5px] font-semibold shadow-none focus-visible:ring-0"
            />
          </div>
        ) : (
          <button
            type="button"
            onClick={toggleProject}
            className={cn(
              'relative flex w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-md px-2 py-[5px] text-left tracking-tight transition-colors',
              hasActiveSession
                ? 'text-foreground/70 hover:bg-black/[0.04]'
                : 'text-foreground/38 hover:bg-black/[0.03] hover:text-foreground/55'
            )}
          >
            {/* Tiny chevron — strong contrast vs session row */}
            <ChevronDown
              className={cn(
                'size-2.5 shrink-0 transition-transform duration-150',
                hasActiveSession ? 'text-foreground/40' : 'text-foreground/25',
                !isExpanded && '-rotate-90'
              )}
              strokeWidth={2.5}
            />
            {isExpanded ? (
              <FolderOpen
                className={cn('size-3 shrink-0', hasActiveSession ? 'text-foreground/50' : 'text-foreground/28')}
                strokeWidth={1.5}
              />
            ) : (
              <Folder
                className={cn('size-3 shrink-0', hasActiveSession ? 'text-foreground/50' : 'text-foreground/28')}
                strokeWidth={1.5}
              />
            )}
            {/* Project name: semibold + slightly larger than sessions */}
            <span className={cn(
              'min-w-0 flex-1 truncate text-[12px] font-semibold',
              hasActiveSession ? 'text-foreground/70' : 'text-foreground/40'
            )}>
              {project.name}
            </span>
          </button>
        )}

        {/* Hover actions — new chat + pencil + context menu */}
        <div className="pointer-events-none absolute inset-y-0 right-0.5 flex items-center gap-0 opacity-0 transition-opacity group-hover/project:opacity-100 group-focus-within/project:opacity-100">
          {/* + New chat shortcut */}
          <Button
            type="button"
            variant="ghost"
            size="icon"
            title="新聊天"
            className="pointer-events-auto size-6 rounded-md p-0 text-muted-foreground/35 transition-colors hover:bg-jade/10 hover:text-jade cursor-pointer"
            onClick={(event) => {
              event.stopPropagation()
              onSelectProject(project.id)
            }}
          >
            <svg
              className="size-3"
              fill="none"
              stroke="currentColor"
              strokeWidth={2.5}
              viewBox="0 0 12 12"
            >
              <path strokeLinecap="round" d="M6 2v8M2 6h8" />
            </svg>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="pointer-events-auto size-6 rounded-md p-0 text-muted-foreground/35 transition-colors hover:text-foreground cursor-pointer"
            onClick={(event) => {
              event.stopPropagation()
              beginInlineRename()
            }}
          >
            <PencilLine className="size-3" strokeWidth={1.5} />
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

      {/* ── Session list: indented + left tree-line ────────────────────── */}
      <div className={cn(!isExpanded && 'hidden')}>
        {sessions.length === 0 ? (
          // Empty state: show "no sessions" + a prominent new-chat row
          <div className="ml-[18px] border-l border-black/[0.06]">
            <div className="pb-0.5 pl-3 pt-0.5 text-[10.5px] text-muted-foreground/25">
              无线程
            </div>
            <NewChatRow onClick={() => onSelectProject(project.id)} />
          </div>
        ) : (
          // ml positions the line under the folder icon; border-l draws the connector
          <div className="ml-[18px] flex flex-col border-l border-black/[0.07] py-0.5">
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
                onRenameSession={onRenameSession}
              />
            ))}
            {/* "New chat" shortcut at the bottom of every session list */}
            <NewChatRow onClick={() => onSelectProject(project.id)} />
          </div>
        )}
      </div>
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
  onRenameSession,
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
  onRenameSession: (sessionId: string, newTitle: string) => void
}) {
  const [isEditing, setIsEditing] = useState(false)
  const [draftTitle, setDraftTitle] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  const beginEdit = () => {
    setDraftTitle(title)
    setIsEditing(true)
  }

  const commitEdit = () => {
    const trimmed = draftTitle.trim()
    if (trimmed && trimmed !== title) {
      onRenameSession(sessionId, trimmed)
    }
    setIsEditing(false)
  }

  const cancelEdit = () => {
    setIsEditing(false)
  }

  // Auto-focus input when edit mode starts
  useEffect(() => {
    if (isEditing) {
      // Delay to let the DOM render the input first
      requestAnimationFrame(() => inputRef.current?.select())
    }
  }, [isEditing])

  if (isEditing) {
    return (
      <div className="flex h-[26px] items-center gap-1.5 rounded-sm bg-black/[0.05] pl-3 pr-1.5">
        <Input
          ref={inputRef}
          value={draftTitle}
          onChange={(e) => setDraftTitle(e.target.value)}
          onBlur={commitEdit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') { e.preventDefault(); commitEdit() }
            if (e.key === 'Escape') { e.preventDefault(); cancelEdit() }
          }}
          className="h-4 flex-1 rounded border-0 bg-transparent px-0 text-[11.5px] shadow-none focus-visible:ring-0"
        />
      </div>
    )
  }

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
        // Simplified layout: no icon column — title + trailing age/menu only
        'group/session relative flex h-[26px] w-full cursor-pointer items-center rounded-sm pl-3 pr-1 text-[11.5px] transition-colors',
        isActive
          ? 'bg-primary/10 text-primary font-medium'
          : 'text-foreground/40 hover:bg-black/[0.04] hover:text-foreground/65 font-normal'
      )}
    >
      {/* Pinned dot — subtle indicator, no full icon column */}
      {isPinned && (
        <span className="mr-1.5 size-1 shrink-0 rounded-full bg-current opacity-50" />
      )}

      <span className="min-w-0 flex-1 truncate">{title}</span>

      {/* Trailing: age fades out on hover, menu fades in */}
      <div className="relative flex h-full shrink-0 items-center">
        <span className={cn(
          'flex items-center gap-1 text-[10px] text-muted-foreground/30 transition-opacity duration-100',
          'group-hover/session:opacity-0'
        )}>
          {isRunning ? (
            <span className="inline-flex size-1.5 items-center justify-center">
              <span className="absolute size-1.5 rounded-full bg-primary/30 animate-pulse" />
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
              className="absolute right-0 top-1/2 size-6 -translate-y-1/2 rounded p-0 text-muted-foreground/40 opacity-0 transition-all hover:text-foreground group-hover/session:opacity-100 focus-visible:opacity-100 cursor-pointer"
              onPointerDown={(event) => event.stopPropagation()}
              onClick={(event) => event.stopPropagation()}
            >
              <MoreHorizontal className="size-3" strokeWidth={1.5} />
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
            <DropdownMenuItem
              className="h-8 rounded-lg px-2.5 text-[12.5px] font-medium text-foreground/82 transition-colors hover:bg-accent focus:bg-accent focus:text-foreground/90"
              onSelect={beginEdit}
            >
              <PencilLine className="size-3.5 shrink-0 text-muted-foreground/60" strokeWidth={1.5} />
              <span>重命名</span>
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
