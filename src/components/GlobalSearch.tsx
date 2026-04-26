import { useEffect, useMemo, useState } from 'react'
import {
  Clock,
  Brain,
  Folder,
  FolderOpen,
  Hash,
  Loader2,
  MessageSquare,
  Settings,
  SlidersHorizontal,
} from 'lucide-react'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { cn } from '@/lib/utils'
import type { ProjectMeta, SessionMeta } from '@/lib/tauri'
import { openSettingsWindow } from '@/api/window'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from '@/components/ui/command'

/** Maximum sessions shown per group when no query is typed. */
const MAX_RECENT = 8

const SETTINGS_RESULTS = [
  { id: 'settings:providers', label: '服务商配置', hint: 'Provider / API Key / Base URL', icon: SlidersHorizontal },
  { id: 'settings:models', label: '模型配置', hint: '主聊天模型 / Thinking 支持', icon: Brain },
  { id: 'settings:memory', label: '记忆设置', hint: 'Memory / 编译 / 晋升', icon: Brain },
  { id: 'settings:prompt', label: 'Prompt Diagnostics', hint: '身份 / Prompt / 诊断', icon: Settings },
]

interface GlobalSearchProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  projects: ProjectMeta[]
  projectSessions: Record<string, SessionMeta[]>
  activeProjectId: string | null
  onSelectSession: (projectId: string, sessionId: string) => void
  onSelectProject: (projectId: string) => void
}

function formatAge(value: string): string {
  const diff = Math.max(0, Date.now() - +new Date(value))
  const m = 60_000, h = 3_600_000, d = 86_400_000
  if (diff < m) return '刚刚'
  if (diff < h) return `${Math.floor(diff / m)}分钟前`
  if (diff < d) return `${Math.floor(diff / h)}小时前`
  return `${Math.floor(diff / d)}天前`
}

export function GlobalSearch({
  open,
  onOpenChange,
  projects,
  projectSessions,
  activeProjectId,
  onSelectSession,
  onSelectProject,
}: GlobalSearchProps) {
  const [query, setQuery] = useState('')

  // Reset query when closed
  useEffect(() => {
    if (!open) setQuery('')
  }, [open])

  // Flat list of all sessions with project metadata, sorted by recency
  const allSessions = useMemo(() => {
    return projects
      .flatMap((p) =>
        (projectSessions[p.id] ?? []).map((s) => ({
          ...s,
          projectId: p.id,
          projectName: p.name,
        }))
      )
      .sort((a, b) => +new Date(b.updated_at) - +new Date(a.updated_at))
  }, [projects, projectSessions])

  const q = query.trim().toLowerCase()

  const matchedSessions = useMemo(() => {
    if (!q) return allSessions.slice(0, MAX_RECENT)
    return allSessions.filter(
      (s) =>
        s.title.toLowerCase().includes(q) ||
        s.projectName.toLowerCase().includes(q)
    )
  }, [allSessions, q])

  const matchedProjects = useMemo(() => {
    if (!q) return projects
    return projects.filter((p) => p.name.toLowerCase().includes(q))
  }, [projects, q])

  const matchedSettings = useMemo(() => {
    if (!q) return SETTINGS_RESULTS.slice(0, 3)
    return SETTINGS_RESULTS.filter((item) =>
      `${item.label} ${item.hint}`.toLowerCase().includes(q),
    )
  }, [q])

  const handleSelectSession = (projectId: string, sessionId: string) => {
    onSelectSession(projectId, sessionId)
    onOpenChange(false)
  }

  const handleSelectProject = (projectId: string) => {
    onSelectProject(projectId)
    onOpenChange(false)
  }

  const hasResults = matchedSessions.length > 0 || matchedProjects.length > 0
    || matchedSettings.length > 0

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        {/* Overlay */}
        <DialogPrimitive.Overlay
          className={cn(
            'fixed inset-0 z-50 bg-black/30 backdrop-blur-sm',
            'data-[state=open]:animate-in data-[state=closed]:animate-out',
            'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0'
          )}
        />

        {/* Panel */}
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className={cn(
            'fixed left-1/2 top-[22%] z-50 w-[min(92vw,560px)] -translate-x-1/2',
            'overflow-hidden rounded-2xl border border-black/[0.07]',
            'bg-white/92 shadow-[0_20px_60px_rgba(0,0,0,0.18),0_4px_16px_rgba(0,0,0,0.08),0_0_0_0.5px_rgba(0,0,0,0.06)]',
            'backdrop-blur-2xl',
            'data-[state=open]:animate-in data-[state=closed]:animate-out',
            'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
            'data-[state=closed]:zoom-out-97 data-[state=open]:zoom-in-97',
            'data-[state=open]:slide-in-from-top-2 data-[state=closed]:slide-out-to-top-2'
          )}
        >
          <DialogPrimitive.Title className="sr-only">全局搜索</DialogPrimitive.Title>

          <Command shouldFilter={false}>
            <CommandInput
              value={query}
              onValueChange={setQuery}
              placeholder="搜索线程、项目…"
              autoFocus
            />

            <CommandList>
              {!hasResults && (
                <CommandEmpty>
                  <Hash className="size-8 text-muted-foreground/20" strokeWidth={1} />
                  <p className="text-[12.5px] text-muted-foreground/40">
                    未找到「{query}」相关内容
                  </p>
                </CommandEmpty>
              )}

              {/* ── Recent / matched sessions ──────────────────── */}
              {matchedSessions.length > 0 && (
                <CommandGroup heading={q ? '线程' : '最近线程'}>
                  {matchedSessions.map((s) => (
                    <CommandItem
                      key={s.id}
                      value={s.id}
                      onSelect={() => handleSelectSession(s.projectId, s.id)}
                    >
                      <span className="flex size-4 shrink-0 items-center justify-center text-[13px] leading-none text-muted-foreground/55">
                        {s.title_pending ? (
                          <Loader2 className="size-3.5 animate-spin" strokeWidth={1.8} />
                        ) : s.title_icon ? (
                          <span aria-hidden="true">{s.title_icon}</span>
                        ) : (
                          <MessageSquare className="size-3.5" strokeWidth={1.5} />
                        )}
                      </span>
                      <span className="min-w-0 flex-1 truncate text-foreground/75">
                        {s.title}
                      </span>
                      {/* Project badge */}
                      <span className="flex shrink-0 items-center gap-1 rounded-md bg-muted/70 px-1.5 py-0.5 text-[10.5px] text-muted-foreground/50">
                        <Folder className="size-2.5" strokeWidth={1.5} />
                        <span className="max-w-[80px] truncate">{s.projectName}</span>
                      </span>
                      {/* Age */}
                      <span className="shrink-0 text-[10.5px] text-muted-foreground/35">
                        <Clock className="mr-0.5 inline size-2.5" strokeWidth={1.5} />
                        {formatAge(s.updated_at)}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}

              {matchedSessions.length > 0 && (matchedProjects.length > 0 || matchedSettings.length > 0) && (
                <CommandSeparator />
              )}

              {matchedSettings.length > 0 && (
                <CommandGroup heading="设置与命令">
                  {matchedSettings.map((item) => {
                    const Icon = item.icon
                    return (
                      <CommandItem
                        key={item.id}
                        value={item.id}
                        onSelect={() => {
                          void openSettingsWindow()
                          onOpenChange(false)
                        }}
                      >
                        <Icon className="size-3.5 shrink-0 text-muted-foreground/40" strokeWidth={1.5} />
                        <span className="min-w-0 flex-1 truncate text-foreground/75">
                          {item.label}
                        </span>
                        <span className="max-w-[220px] shrink-0 truncate text-[10.5px] text-muted-foreground/35">
                          {item.hint}
                        </span>
                      </CommandItem>
                    )
                  })}
                </CommandGroup>
              )}

              {matchedSettings.length > 0 && matchedProjects.length > 0 && (
                <CommandSeparator />
              )}

              {/* ── Projects ───────────────────────────────────── */}
              {matchedProjects.length > 0 && (
                <CommandGroup heading="项目">
                  {matchedProjects.map((p) => {
                    const sessionCount = (projectSessions[p.id] ?? []).length
                    const isActive = p.id === activeProjectId
                    return (
                      <CommandItem
                        key={p.id}
                        value={`project:${p.id}`}
                        onSelect={() => handleSelectProject(p.id)}
                      >
                        {isActive ? (
                          <FolderOpen
                            className="size-3.5 shrink-0 text-primary/60"
                            strokeWidth={1.5}
                          />
                        ) : (
                          <Folder
                            className="size-3.5 shrink-0 text-muted-foreground/40"
                            strokeWidth={1.5}
                          />
                        )}
                        <span className={cn('min-w-0 flex-1 truncate', isActive && 'text-primary/80 font-medium')}>
                          {p.name}
                        </span>
                        {/* Session count chip */}
                        {sessionCount > 0 && (
                          <span className="shrink-0 rounded-full bg-muted/70 px-2 py-0.5 text-[10.5px] text-muted-foreground/45">
                            {sessionCount} 个线程
                          </span>
                        )}
                        {/* Workdir path */}
                        <span className="max-w-[140px] shrink-0 truncate text-right text-[10.5px] text-muted-foreground/30">
                          {p.workdir.replace(/^.*\/([^/]+\/[^/]+)$/, '…/$1')}
                        </span>
                      </CommandItem>
                    )
                  })}
                </CommandGroup>
              )}

              {/* ── Footer hint ─────────────────────────────────── */}
              <div className="flex items-center justify-end gap-3 border-t border-black/[0.05] px-3.5 py-2">
                <span className="text-[10.5px] text-muted-foreground/35">
                  <kbd className="rounded bg-black/[0.06] px-1 py-0.5 font-mono text-[10px]">↑↓</kbd>
                  {' '}导航
                </span>
                <span className="text-[10.5px] text-muted-foreground/35">
                  <kbd className="rounded bg-black/[0.06] px-1 py-0.5 font-mono text-[10px]">↵</kbd>
                  {' '}打开
                </span>
                <span className="text-[10.5px] text-muted-foreground/35">
                  <kbd className="rounded bg-black/[0.06] px-1 py-0.5 font-mono text-[10px]">Esc</kbd>
                  {' '}关闭
                </span>
              </div>
            </CommandList>
          </Command>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}
