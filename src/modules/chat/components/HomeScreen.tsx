import React, { useEffect, useRef, useState } from 'react'
import {
  ChevronDown,
  FolderGit2,
  FolderOpen,
  GitBranch,
  Laptop,
  Loader2,
  MessageSquare,
  Mic,
  Plus,
  PlusCircle,
  ArrowUp,
  Search,
  X,
} from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { setActiveModel } from '@/api/models'
import { cn } from '@/lib/utils'
import type { PermissionMode, ProjectMeta } from '@/lib/tauri'
import type { RecentSession } from '../types'

interface HomeScreenProps {
  projects: ProjectMeta[]
  recentSessions: RecentSession[]
  selectedProjectId: string | null
  selectedModel: string
  onModelChange: (model: string) => void
  permissionMode: PermissionMode
  onPermissionModeChange: (mode: PermissionMode) => void
  onSendMessage: (text: string) => void
  onSelectSession: (projectId: string, sessionId: string) => void
  onHomeProjectSelect: (projectId: string | null) => void
  onPickFolderAndCreateProject: () => Promise<void>
  isLoading: boolean
  branchLabel: string
}

const PERMISSION_LABELS: Record<PermissionMode, string> = {
  readOnly: '只读',
  workspaceWrite: '工作区写入',
  dangerFullAccess: '默认权限',
}

export function HomeScreen({
  projects,
  recentSessions,
  selectedProjectId,
  selectedModel,
  onModelChange,
  permissionMode,
  onPermissionModeChange,
  onSendMessage,
  onSelectSession,
  onHomeProjectSelect,
  onPickFolderAndCreateProject,
  isLoading,
  branchLabel,
}: HomeScreenProps) {
  const [availableModelItems, setAvailableModelItems] = useState<
    Array<{ value: string; label: string }>
  >([])
  useEffect(() => {
    let cancelled = false
    let unlistenModelsChanged: (() => void) | null = null
    const refresh = async () => {
      try {
        const groups = await invoke<
          Array<{
            provider_id: string
            provider_name: string
            models: Array<{ model_id: string; name: string }>
          }>
        >('model_list_available')
        if (cancelled) return
        const items = groups
          .filter((g) => g.models.length > 0)
          .flatMap((g) =>
            g.models.map((m) => ({
              value: `${g.provider_id}/${m.model_id}`,
              label: m.name,
            }))
          )
        setAvailableModelItems(items)
      } catch {
        // Fallback to empty
      }
    }
    void refresh()
    const onChanged = () => void refresh()
    window.addEventListener('if2ai:models-changed', onChanged)
    void listen('if2ai://models-changed', onChanged).then((unlisten) => {
      unlistenModelsChanged = unlisten
    })
    return () => {
      cancelled = true
      window.removeEventListener('if2ai:models-changed', onChanged)
      unlistenModelsChanged?.()
    }
  }, [])
  const [localInput, setLocalInput] = useState('')
  const [projectDropdownOpen, setProjectDropdownOpen] = useState(false)
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false)
  const [permissionDropdownOpen, setPermissionDropdownOpen] = useState(false)
  const [projectSearch, setProjectSearch] = useState('')
  const [isPickingFolder, setIsPickingFolder] = useState(false)
  const [composerFocused, setComposerFocused] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const modelDropdownRef = useRef<HTMLDivElement>(null)
  const permDropdownRef = useRef<HTMLDivElement>(null)

  const selectedProject = projects.find((p) => p.id === selectedProjectId)
  const projectName = selectedProject?.name ?? 'Workaround'
  const workdirName = selectedProject?.workdir
    ? selectedProject.workdir.split('/').filter(Boolean).pop() ?? '本地工作'
    : '本地工作'

  const filteredProjects = projectSearch.trim()
    ? projects.filter((p) =>
        p.name.toLowerCase().includes(projectSearch.toLowerCase())
      )
    : projects

  const modelLabel =
    availableModelItems.find((i) => i.value === selectedModel)?.label ??
    availableModelItems[0]?.label ??
    selectedModel.split('/')[1] ??
    '选择模型'

  // Auto-resize textarea
  useEffect(() => {
    const ta = textareaRef.current
    if (!ta) return
    ta.style.height = 'auto'
    ta.style.height = `${Math.min(ta.scrollHeight, 200)}px`
  }, [localInput])

  // Close dropdowns on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setProjectDropdownOpen(false)
        setProjectSearch('')
      }
      if (modelDropdownRef.current && !modelDropdownRef.current.contains(e.target as Node)) {
        setModelDropdownOpen(false)
      }
      if (permDropdownRef.current && !permDropdownRef.current.contains(e.target as Node)) {
        setPermissionDropdownOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  const handleSubmit = () => {
    const text = localInput.trim()
    if (!text || isLoading) return
    onSendMessage(text)
    setLocalInput('')
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSubmit()
    }
  }

  const canSend = localInput.trim().length > 0 && !isLoading

  return (
    <div className="relative flex h-full min-h-0 flex-col items-center overflow-y-auto px-6 pb-8 pt-0 text-foreground">
      {/* Vertical centering wrapper */}
      <div className="flex w-full max-w-[700px] flex-1 flex-col items-center justify-center gap-6 py-12">

        {/* ── Title ── */}
        <h1 className="text-center text-[28px] font-[450] tracking-[-0.02em] text-foreground/88 md:text-[32px]">
          今天想在{' '}
          <span className="font-semibold text-foreground">{projectName}</span>
          里完成什么？
        </h1>

        {/* ── Composer card ── */}
        <div
          className="w-full rounded-2xl border border-border/70 bg-card text-card-foreground transition-all duration-200"
          style={{
            borderColor: composerFocused
              ? 'color-mix(in srgb, var(--primary) 36%, var(--border))'
              : 'var(--border)',
            boxShadow: composerFocused ? 'var(--shadow-lg)' : 'var(--shadow-md)',
            transform: composerFocused ? 'translateY(-1px)' : 'translateY(0)',
          }}
        >
          {/* Textarea */}
          <div className="px-4 pt-4 pb-2">
            <textarea
              ref={textareaRef}
              value={localInput}
              onChange={(e) => setLocalInput(e.target.value)}
              onKeyDown={handleKeyDown}
              onFocus={() => setComposerFocused(true)}
              onBlur={() => setComposerFocused(false)}
              placeholder="向 AI 提问，@ 添加文件，/ 输入命令，$ 使用技能"
              className="w-full resize-none bg-transparent text-[14px] text-foreground/88 placeholder:text-muted-foreground/55 focus:outline-none"
              rows={2}
              style={{ minHeight: 52 }}
            />
          </div>

          {/* Action row */}
          <div className="flex items-center justify-between border-t border-border/60 px-3 py-2.5">
            {/* Left: + | Permission */}
            <div className="flex items-center gap-1">
              <button
                type="button"
                className="flex h-7 w-7 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                title="添加附件"
              >
                <Plus className="h-[15px] w-[15px]" />
              </button>

              <div className="relative" ref={permDropdownRef}>
                <button
                  type="button"
                  onClick={() => setPermissionDropdownOpen((v) => !v)}
                  className="flex items-center gap-1 rounded-lg px-2 py-1 text-[12px] text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                >
                  <span className="text-[13px]">🤲</span>
                  {PERMISSION_LABELS[permissionMode]}
                  <ChevronDown className="h-3 w-3" />
                </button>
                {permissionDropdownOpen && (
                  <div className="home-project-menu absolute left-0 top-[calc(100%+4px)] z-50 min-w-[140px] overflow-hidden rounded-xl border border-border/70 bg-popover py-1 text-popover-foreground shadow-token-lg">
                    {(['readOnly', 'workspaceWrite', 'dangerFullAccess'] as PermissionMode[]).map(
                      (mode) => (
                        <button
                          key={mode}
                          type="button"
                          onClick={() => {
                            onPermissionModeChange(mode)
                            setPermissionDropdownOpen(false)
                          }}
                          className={cn(
                            'home-project-menu-item flex w-full items-center px-3 py-2 text-[12px] transition-colors hover:bg-accent hover:text-accent-foreground',
                            permissionMode === mode ? 'home-project-menu-item-active font-semibold text-foreground' : 'text-muted-foreground'
                          )}
                        >
                          {PERMISSION_LABELS[mode]}
                        </button>
                      )
                    )}
                  </div>
                )}
              </div>
            </div>

            {/* Right: Model | Mic | Send */}
            <div className="flex items-center gap-1.5">
              {/* Model selector */}
              <div className="relative" ref={modelDropdownRef}>
                <button
                  type="button"
                  onClick={() => setModelDropdownOpen((v) => !v)}
                  className="flex items-center gap-1 rounded-lg px-2 py-1 text-[12px] text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                >
                  {modelLabel}
                  <ChevronDown className="h-3 w-3" />
                </button>
                {modelDropdownOpen && availableModelItems.length > 0 && (
                  <div className="home-project-menu absolute right-0 top-[calc(100%+4px)] z-50 max-h-[280px] min-w-[200px] overflow-y-auto rounded-xl border border-border/70 bg-popover py-1 text-popover-foreground shadow-token-lg">
                    {availableModelItems.map((item) => (
                      <button
                        key={item.value}
                        type="button"
                        onClick={() => {
                          onModelChange(item.value)
                          const parts = item.value.split('/')
                          if (parts.length === 2) {
                            void setActiveModel(parts[0], parts[1])
                          }
                          setModelDropdownOpen(false)
                        }}
                        className={cn(
                          'home-project-menu-item flex w-full items-center px-3 py-2 text-[12px] transition-colors hover:bg-accent hover:text-accent-foreground',
                          selectedModel === item.value
                            ? 'home-project-menu-item-active font-semibold text-foreground'
                            : 'text-muted-foreground'
                        )}
                      >
                        {item.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              <div className="h-3.5 w-px bg-border" />

              <button
                type="button"
                className="flex h-7 w-7 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
                title="语音输入"
              >
                <Mic className="h-[14px] w-[14px]" />
              </button>

              {/* Send button */}
              <button
                type="button"
                onClick={handleSubmit}
                disabled={!canSend}
                className={cn(
                  'flex h-7 w-7 items-center justify-center rounded-full transition-all',
                  canSend
                    ? 'bg-primary text-primary-foreground hover:bg-primary/85 active:scale-95'
                    : 'bg-muted text-muted-foreground/60 cursor-not-allowed'
                )}
              >
                <ArrowUp className="h-[14px] w-[14px]" />
              </button>
            </div>
          </div>
        </div>

        {/* ── Project context bar ── */}
        <div className="flex w-full flex-wrap items-center gap-1.5" ref={dropdownRef}>
          {/* Project pill */}
          <div className="relative">
            <button
              type="button"
              onClick={() => {
                setProjectDropdownOpen((v) => !v)
                setProjectSearch('')
              }}
              className={cn(
                'group flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12px] font-medium transition-all',
                projectDropdownOpen
                  ? 'border-primary/35 bg-card text-foreground shadow-sm'
                  : 'border-border/70 bg-muted/30 text-muted-foreground hover:border-primary/30 hover:bg-accent hover:text-accent-foreground'
              )}
            >
              <FolderOpen className={cn('h-[13px] w-[13px]', projectDropdownOpen ? 'text-foreground/70' : 'text-muted-foreground')} />
              <span>{projectName}</span>
              <ChevronDown className={cn('h-2.5 w-2.5 transition-transform', projectDropdownOpen && 'rotate-180')} />
            </button>

            {/* Project dropdown */}
            {projectDropdownOpen && (
              <div className="home-project-menu absolute left-0 top-[calc(100%+8px)] z-50 w-[280px] overflow-hidden rounded-2xl border border-border/70 bg-popover/96 text-popover-foreground shadow-token-lg backdrop-blur-xl">
                {/* Search */}
                <div className="flex items-center gap-2 border-b border-border/70 px-3.5 py-2.5">
                  <Search className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <input
                    type="text"
                    value={projectSearch}
                    onChange={(e) => setProjectSearch(e.target.value)}
                    placeholder="搜索项目…"
                    className="flex-1 bg-transparent text-[13px] text-foreground/80 placeholder:text-muted-foreground/55 focus:outline-none"
                    autoFocus
                  />
                  {projectSearch && (
                    <button
                      type="button"
                      onClick={() => setProjectSearch('')}
                      className="flex size-4 items-center justify-center rounded-full bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                      <X className="h-2.5 w-2.5" />
                    </button>
                  )}
                </div>

                {/* Project list */}
                <div className="max-h-[240px] overflow-y-auto py-1">
                  {filteredProjects.length === 0 ? (
                    <div className="px-4 py-4 text-center text-[12px] text-muted-foreground">无匹配项目</div>
                  ) : (
                    filteredProjects.map((project) => (
                      <button
                        key={project.id}
                        type="button"
                        onClick={() => {
                          onHomeProjectSelect(project.id)
                          setProjectDropdownOpen(false)
                          setProjectSearch('')
                        }}
                        className={cn(
                          'home-project-menu-item flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-accent hover:text-accent-foreground',
                          selectedProjectId === project.id && 'home-project-menu-item-active bg-accent text-accent-foreground'
                        )}
                      >
                        <FolderGit2
                          className={cn(
                            'h-4 w-4 shrink-0',
                            selectedProjectId === project.id ? 'text-foreground/70' : 'text-muted-foreground'
                          )}
                        />
                        <div className="min-w-0 flex-1">
                          <div className={cn('truncate text-[13px]', selectedProjectId === project.id ? 'font-semibold text-foreground' : 'text-foreground/70')}>
                            {project.name}
                          </div>
                          <div className="truncate text-[11px] text-muted-foreground">
                            {project.workdir.split('/').filter(Boolean).pop() ?? project.name}
                          </div>
                        </div>
                        {selectedProjectId === project.id && (
                          <div className="h-1.5 w-1.5 shrink-0 rounded-full bg-primary/60" />
                        )}
                      </button>
                    ))
                  )}
                </div>

                {/* Footer */}
                <div className="border-t border-border/70 py-1">
                  <button
                    type="button"
                    disabled={isPickingFolder}
                    onClick={async () => {
                      setProjectDropdownOpen(false)
                      setIsPickingFolder(true)
                      try {
                        await onPickFolderAndCreateProject()
                      } finally {
                        setIsPickingFolder(false)
                      }
                    }}
                    className="home-project-menu-footer flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                  >
                    <PlusCircle className="h-4 w-4 shrink-0 text-muted-foreground" />
                    <span className="text-[13px] text-muted-foreground">
                      {isPickingFolder ? '选择文件夹中…' : '添加新项目'}
                    </span>
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Workdir pill */}
          <button
            type="button"
            className="flex items-center gap-1.5 rounded-lg border border-border/70 bg-muted/30 px-3 py-1.5 text-[12px] font-medium text-muted-foreground transition-colors hover:border-primary/30 hover:bg-accent hover:text-accent-foreground"
          >
            <Laptop className="h-[13px] w-[13px]" />
            {workdirName}
            <ChevronDown className="h-3 w-3" />
          </button>

          {/* Branch pill */}
          <button
            type="button"
            className="flex items-center gap-1.5 rounded-lg border border-border/70 bg-muted/30 px-3 py-1.5 text-[12px] font-medium text-muted-foreground transition-colors hover:border-primary/30 hover:bg-accent hover:text-accent-foreground"
          >
            <GitBranch className="h-[13px] w-[13px]" />
            <span className="max-w-[140px] truncate">{branchLabel}</span>
            <ChevronDown className="h-3 w-3" />
          </button>
        </div>

        {/* ── Recent sessions ── */}
        {recentSessions.length > 0 && (
          <div className="w-full">
            {/* Section label */}
            <div className="mb-1 flex items-center justify-between px-1">
              <span className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/55">
                最近对话
              </span>
            </div>

            {/* Session rows */}
            <div className="flex flex-col">
              {recentSessions.slice(0, 3).map((session, idx) => (
                <button
                  key={session.sessionId}
                  type="button"
                  onClick={() => onSelectSession(session.projectId, session.sessionId)}
                  className={cn(
                    'group flex w-full items-center gap-3 px-3 py-2.5 text-left transition-all',
                    'rounded-xl text-muted-foreground hover:bg-accent hover:text-accent-foreground active:bg-accent/80',
                    idx === 0 && 'mt-0'
                  )}
                >
                  <span className="flex size-5 shrink-0 items-center justify-center text-[14px] leading-none text-muted-foreground/70 transition-colors group-hover:text-accent-foreground">
                    {session.titlePending ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin" strokeWidth={1.8} />
                    ) : session.titleIcon ? (
                      <span className="session-emoji" aria-hidden="true">{session.titleIcon}</span>
                    ) : (
                      <MessageSquare className="h-3.5 w-3.5" strokeWidth={1.5} />
                    )}
                  </span>

                  {/* Title */}
                  <span className="min-w-0 flex-1 truncate text-[13px] font-medium transition-colors">
                    {session.title}
                  </span>

                  {/* Project badge */}
                  <span className="shrink-0 rounded-full border border-border/60 bg-muted/30 px-2 py-[2px] text-[10.5px] text-muted-foreground transition-colors group-hover:border-primary/30 group-hover:bg-background/40 group-hover:text-accent-foreground">
                    {session.projectName}
                  </span>
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
