import React, { useEffect, useRef, useState } from 'react'
import {
  ChevronDown,
  ChevronUp,
  FolderGit2,
  FolderOpen,
  GitBranch,
  Laptop,
  Mic,
  MessageSquare,
  Plus,
  PlusCircle,
  ArrowUp,
  Search,
  X,
} from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
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
    void (async () => {
      try {
        const groups = await invoke<
          Array<{
            provider_id: string
            provider_name: string
            models: Array<{ model_id: string; name: string }>
          }>
        >('model_list_available')
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
    })()
  }, [])
  const [localInput, setLocalInput] = useState('')
  const [projectDropdownOpen, setProjectDropdownOpen] = useState(false)
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false)
  const [permissionDropdownOpen, setPermissionDropdownOpen] = useState(false)
  const [projectSearch, setProjectSearch] = useState('')
  const [isPickingFolder, setIsPickingFolder] = useState(false)
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
    <div className="relative flex h-full min-h-0 flex-col items-center overflow-y-auto px-6 pb-8 pt-0">
      {/* Vertical centering wrapper */}
      <div className="flex w-full max-w-[700px] flex-1 flex-col items-center justify-center gap-6 py-12">

        {/* ── Title ── */}
        <h1 className="text-center text-[28px] font-[450] tracking-[-0.02em] text-black/88 md:text-[32px]">
          What should we build in{' '}
          <span className="font-semibold text-black">{projectName}</span>
          ？
        </h1>

        {/* ── Composer card ── */}
        <div className="w-full rounded-2xl border border-black/[0.08] bg-white shadow-[0_2px_16px_rgba(0,0,0,0.07),0_0_0_0.5px_rgba(0,0,0,0.04)]">
          {/* Textarea */}
          <div className="px-4 pt-4 pb-2">
            <textarea
              ref={textareaRef}
              value={localInput}
              onChange={(e) => setLocalInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="向 AI 提问，@ 添加文件，/ 输入命令，$ 使用技能"
              className="w-full resize-none bg-transparent text-[14px] text-black/88 placeholder:text-black/28 focus:outline-none"
              rows={2}
              style={{ minHeight: 52 }}
            />
          </div>

          {/* Action row */}
          <div className="flex items-center justify-between border-t border-black/[0.05] px-3 py-2.5">
            {/* Left: + | Permission */}
            <div className="flex items-center gap-1">
              <button
                type="button"
                className="flex h-7 w-7 items-center justify-center rounded-lg text-black/40 transition-colors hover:bg-black/[0.05] hover:text-black/65"
                title="添加附件"
              >
                <Plus className="h-[15px] w-[15px]" />
              </button>

              <div className="relative" ref={permDropdownRef}>
                <button
                  type="button"
                  onClick={() => setPermissionDropdownOpen((v) => !v)}
                  className="flex items-center gap-1 rounded-lg px-2 py-1 text-[12px] text-black/45 transition-colors hover:bg-black/[0.05] hover:text-black/65"
                >
                  <span className="text-[13px]">🤲</span>
                  {PERMISSION_LABELS[permissionMode]}
                  <ChevronDown className="h-3 w-3" />
                </button>
                {permissionDropdownOpen && (
                  <div className="absolute left-0 top-[calc(100%+4px)] z-50 min-w-[140px] overflow-hidden rounded-xl border border-black/8 bg-white shadow-[0_8px_24px_rgba(0,0,0,0.10)] py-1">
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
                            'flex w-full items-center px-3 py-2 text-[12px] transition-colors hover:bg-black/[0.04]',
                            permissionMode === mode ? 'font-semibold text-black/88' : 'text-black/55'
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
                  className="flex items-center gap-1 rounded-lg px-2 py-1 text-[12px] text-black/45 transition-colors hover:bg-black/[0.05] hover:text-black/65"
                >
                  {modelLabel}
                  <ChevronDown className="h-3 w-3" />
                </button>
                {modelDropdownOpen && availableModelItems.length > 0 && (
                  <div className="absolute right-0 top-[calc(100%+4px)] z-50 max-h-[280px] min-w-[200px] overflow-y-auto rounded-xl border border-black/8 bg-white shadow-[0_8px_24px_rgba(0,0,0,0.10)] py-1">
                    {availableModelItems.map((item) => (
                      <button
                        key={item.value}
                        type="button"
                        onClick={() => {
                          onModelChange(item.value)
                          const parts = item.value.split('/')
                          if (parts.length === 2) {
                            void invoke('model_set_active', { providerId: parts[0], modelId: parts[1] })
                          }
                          setModelDropdownOpen(false)
                        }}
                        className={cn(
                          'flex w-full items-center px-3 py-2 text-[12px] transition-colors hover:bg-black/[0.04]',
                          selectedModel === item.value
                            ? 'font-semibold text-black/88'
                            : 'text-black/55'
                        )}
                      >
                        {item.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              <div className="h-3.5 w-px bg-black/10" />

              <button
                type="button"
                className="flex h-7 w-7 items-center justify-center rounded-lg text-black/35 transition-colors hover:bg-black/[0.05] hover:text-black/65"
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
                    ? 'bg-black text-white hover:bg-black/80 active:scale-95'
                    : 'bg-black/10 text-black/30 cursor-not-allowed'
                )}
              >
                <ArrowUp className="h-[14px] w-[14px]" />
              </button>
            </div>
          </div>
        </div>

        {/* ── Project context bar ── */}
        <div className="flex w-full items-center gap-2" ref={dropdownRef}>
          {/* Project pill (triggers dropdown) */}
          <div className="relative">
            <button
              type="button"
              onClick={() => {
                setProjectDropdownOpen((v) => !v)
                setProjectSearch('')
              }}
              className={cn(
                'flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12px] font-medium transition-all',
                projectDropdownOpen
                  ? 'border-black/20 bg-white text-black/80 shadow-sm'
                  : 'border-black/[0.08] bg-black/[0.03] text-black/55 hover:border-black/14 hover:bg-black/[0.05]'
              )}
            >
              <FolderOpen className="h-[13px] w-[13px]" />
              {projectName}
              {projectDropdownOpen ? (
                <ChevronUp className="h-3 w-3" />
              ) : (
                <ChevronDown className="h-3 w-3" />
              )}
            </button>

            {/* Project dropdown */}
            {projectDropdownOpen && (
              <div className="absolute left-0 top-[calc(100%+6px)] z-50 w-[280px] overflow-hidden rounded-xl border border-black/[0.08] bg-white shadow-[0_8px_32px_rgba(0,0,0,0.12)]">
                {/* Search */}
                <div className="flex items-center gap-2 border-b border-black/[0.06] px-3 py-2">
                  <Search className="h-3.5 w-3.5 shrink-0 text-black/30" />
                  <input
                    type="text"
                    value={projectSearch}
                    onChange={(e) => setProjectSearch(e.target.value)}
                    placeholder="搜索项目"
                    className="flex-1 bg-transparent text-[13px] text-black/80 placeholder:text-black/30 focus:outline-none"
                    autoFocus
                  />
                  {projectSearch && (
                    <button
                      type="button"
                      onClick={() => setProjectSearch('')}
                      className="text-black/30 hover:text-black/55"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  )}
                </div>

                {/* Project list */}
                <div className="max-h-[240px] overflow-y-auto py-1">
                  {filteredProjects.length === 0 ? (
                    <div className="px-4 py-3 text-[12px] text-black/35">无匹配项目</div>
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
                          'flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-black/[0.04]',
                          selectedProjectId === project.id && 'bg-black/[0.03]'
                        )}
                      >
                        <FolderGit2
                          className={cn(
                            'h-4 w-4 shrink-0',
                            selectedProjectId === project.id
                              ? 'text-black/70'
                              : 'text-black/35'
                          )}
                        />
                        <div className="min-w-0 flex-1">
                          <div
                            className={cn(
                              'truncate text-[13px]',
                              selectedProjectId === project.id
                                ? 'font-semibold text-black/88'
                                : 'text-black/70'
                            )}
                          >
                            {project.name}
                          </div>
                          <div className="truncate text-[11px] text-black/35">
                            {project.workdir.split('/').filter(Boolean).pop() ?? project.name}
                          </div>
                        </div>
                        {selectedProjectId === project.id && (
                          <div className="h-1.5 w-1.5 shrink-0 rounded-full bg-black/30" />
                        )}
                      </button>
                    ))
                  )}
                </div>

                {/* Footer actions */}
                <div className="border-t border-black/[0.06] py-1">
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
                    className="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-black/[0.04] disabled:opacity-50"
                  >
                    <PlusCircle className="h-4 w-4 shrink-0 text-black/40" />
                    <span className="text-[13px] text-black/65">
                      {isPickingFolder ? '选择文件夹中…' : '添加新项目'}
                    </span>
                  </button>

                  <button
                    type="button"
                    onClick={() => {
                      onHomeProjectSelect(null)
                      setProjectDropdownOpen(false)
                    }}
                    className="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-black/[0.04]"
                  >
                    <FolderOpen className="h-4 w-4 shrink-0 text-black/30" />
                    <span className="text-[13px] text-black/45">
                      Don&apos;t work in a project
                    </span>
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Workdir pill */}
          <button
            type="button"
            className="flex items-center gap-1.5 rounded-lg border border-black/[0.08] bg-black/[0.03] px-3 py-1.5 text-[12px] font-medium text-black/55 transition-colors hover:border-black/14 hover:bg-black/[0.05]"
          >
            <Laptop className="h-[13px] w-[13px]" />
            {workdirName}
            <ChevronDown className="h-3 w-3" />
          </button>

          {/* Branch pill */}
          <button
            type="button"
            className="flex items-center gap-1.5 rounded-lg border border-black/[0.08] bg-black/[0.03] px-3 py-1.5 text-[12px] font-medium text-black/55 transition-colors hover:border-black/14 hover:bg-black/[0.05]"
          >
            <GitBranch className="h-[13px] w-[13px]" />
            {branchLabel}
            <ChevronDown className="h-3 w-3" />
          </button>
        </div>

        {/* ── Recent sessions ── */}
        {recentSessions.length > 0 && (
          <div className="w-full space-y-1">
            {recentSessions.slice(0, 3).map((session) => (
              <button
                key={session.sessionId}
                type="button"
                onClick={() => onSelectSession(session.projectId, session.sessionId)}
                className="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-left transition-colors hover:bg-black/[0.04] active:bg-black/[0.06]"
              >
                <MessageSquare className="h-[15px] w-[15px] shrink-0 text-black/30" />
                <div className="min-w-0 flex-1">
                  <span className="block truncate text-[13px] text-black/60">
                    {session.title}
                  </span>
                </div>
                <span className="shrink-0 text-[11px] text-black/25">
                  {session.projectName}
                </span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
