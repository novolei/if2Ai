import { useEffect, useMemo, useState } from 'react'
import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Code2,
  MessageSquare,
  MoreHorizontal,
  Play,
  SlidersHorizontal,
  Sparkles,
  SquareTerminal,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { ChatUI } from '@/components/ui/chat-ui'
import { ErrorBoundary } from '@/components/ui/error-boundary'
import { ProjectRail } from '@/components/ProjectRail'
import type { ChatWorkspaceProps } from '../types'
import { SidebarTop } from './SidebarTop'

const modelItems = [
  { value: 'gpt-5.4-mini', label: 'GPT-5.4-Mini' },
  { value: 'gpt-5.4', label: 'GPT-5.4' },
  { value: 'gpt-4.1', label: 'GPT-4.1' },
]

const CHAT_DENSITY_MODE_STORAGE_KEY = 'chatDensityModeV2'
const CHAT_FONT_MODE_STORAGE_KEY = 'chatFontModeV2'
type DensityMode = 'comfortable' | 'compact'
type FontMode = 'sans' | 'serif'

function formatModelName(model: string): string {
  return model.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
}

function HeaderViewStyleControls({
  fontMode,
  densityMode,
  onFontModeChange,
  onDensityModeChange,
}: {
  fontMode: FontMode
  densityMode: DensityMode
  onFontModeChange: (mode: FontMode) => void
  onDensityModeChange: (mode: DensityMode) => void
}) {
  return (
    <div className="inline-flex items-center gap-2 px-1 py-0.5 text-[12px] text-black/62">
      <div className="inline-flex items-center gap-1.5">
        <span className="select-none text-[11px] font-medium text-black/50">字体</span>
        <button
          type="button"
          onClick={() => onFontModeChange('sans')}
          className={cn(
            'inline-flex h-7 min-w-7 items-center justify-center rounded-md px-2 text-[12px] font-semibold transition-colors',
            fontMode === 'sans' ? 'text-black/92' : 'text-black/42 hover:text-black/70'
          )}
        >
          Aa
        </button>
        <button
          type="button"
          onClick={() => onFontModeChange('serif')}
          className={cn(
            'inline-flex h-7 min-w-7 items-center justify-center rounded-md px-2 text-[12px] font-semibold transition-colors',
            fontMode === 'serif' ? 'font-serif text-black/92' : 'font-serif text-black/42 hover:text-black/70'
          )}
        >
          Aa
        </button>
      </div>
      <div className="h-5 w-px bg-black/10" />
      <div className="inline-flex items-center gap-1.5">
        <span className="select-none text-[11px] font-medium text-black/50">密度</span>
        <button
          type="button"
          onClick={() => onDensityModeChange('compact')}
          className={cn(
            'inline-flex h-7 min-w-7 items-center justify-center rounded-md px-2 text-[12px] transition-colors',
            densityMode === 'compact' ? 'text-black/92' : 'text-black/42 hover:text-black/70'
          )}
        >
          ≡
        </button>
        <button
          type="button"
          onClick={() => onDensityModeChange('comfortable')}
          className={cn(
            'inline-flex h-7 min-w-7 items-center justify-center rounded-md px-2 text-[12px] transition-colors',
            densityMode === 'comfortable' ? 'text-black/92' : 'text-black/42 hover:text-black/70'
          )}
        >
          ☰
        </button>
      </div>
    </div>
  )
}

export function ChatWorkspace({
  projects,
  projectSessions,
  activeProjectId,
  activeSessionId,
  currentProject,
  branchLabel,
  activeTitle,
  activeMessages,
  input,
  isLoading,
  loading,
  onSelectProject,
  onSelectSession,
  onNewChat,
  onDeleteProject,
  onRenameProject,
  onDeleteSession,
  onTogglePinSession,
  onOpenInFinder,
  onCreatePermanentWorktree,
  onInputChange,
  onSubmit,
  onResumeFromCursor,
  onStop,
  selectedModel,
  onModelChange,
  permissionMode,
  onPermissionModeChange,
  todos,
  isRightRailOpen,
  onToggleRightRail,
  onRightRailOpenChange,
  leftPaneWidth,
  isLeftPaneCollapsed,
  onResizeStart,
  onToggleLeftPane,
  onStartWindowDrag,
  onPreviewFocusChange,
  runningSessionIds,
}: ChatWorkspaceProps) {
  const [densityMode, setDensityMode] = useState<DensityMode>(() => {
    if (typeof window === 'undefined') return 'comfortable'
    try {
      const stored = window.localStorage.getItem(CHAT_DENSITY_MODE_STORAGE_KEY)
      return stored === 'compact' ? 'compact' : 'comfortable'
    } catch {
      return 'comfortable'
    }
  })
  const [fontMode, setFontMode] = useState<FontMode>(() => {
    if (typeof window === 'undefined') return 'sans'
    try {
      const stored = window.localStorage.getItem(CHAT_FONT_MODE_STORAGE_KEY)
      return stored === 'serif' ? 'serif' : 'sans'
    } catch {
      return 'sans'
    }
  })
  const openNewChat = useMemo(() => {
    return () => {
      const targetProjectId = activeProjectId ?? projects[0]?.id
      if (targetProjectId) onNewChat(targetProjectId)
    }
  }, [activeProjectId, onNewChat, projects])

  useEffect(() => {
    if (typeof window === 'undefined') return
    try {
      window.localStorage.setItem(CHAT_DENSITY_MODE_STORAGE_KEY, densityMode)
    } catch {
      // Ignore storage failures so layout controls never crash the main workspace.
    }
  }, [densityMode])

  useEffect(() => {
    if (typeof window === 'undefined') return
    try {
      window.localStorage.setItem(CHAT_FONT_MODE_STORAGE_KEY, fontMode)
    } catch {
      // Ignore storage failures so layout controls never crash the main workspace.
    }
  }, [fontMode])

  return (
    <div className="relative h-full min-h-0 min-w-0 overflow-hidden">
      <ErrorBoundary
        fallback={
          <aside
            className="absolute inset-y-0 left-0 z-20 flex min-h-0 flex-col overflow-hidden rounded-tr-[18px] rounded-br-[18px] border-r border-black/5 bg-[#eef0f1]/56 backdrop-blur-[2px]"
            style={{ width: leftPaneWidth }}
          >
            <SidebarTop onStartWindowDrag={onStartWindowDrag} onNewChat={openNewChat} />
            <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-[13px] text-black/35">
              左侧栏加载异常
            </div>
          </aside>
        }
      >
        <aside
          className={cn(
            'absolute inset-y-0 left-0 z-20 flex min-h-0 origin-left flex-col overflow-hidden rounded-tr-[18px] rounded-br-[18px] border-r border-black/5 bg-[#eef0f1]/88 shadow-[18px_0_36px_rgba(15,23,42,0.08)] backdrop-blur-[6px] transition-[transform,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]',
            isLeftPaneCollapsed ? 'pointer-events-none -translate-x-full opacity-0' : 'translate-x-0 opacity-100'
          )}
          style={{ width: leftPaneWidth }}
          aria-hidden={isLeftPaneCollapsed}
        >
          <div className="flex h-full min-h-0 min-w-0 flex-col">
            <SidebarTop onStartWindowDrag={onStartWindowDrag} onNewChat={openNewChat} />
            <div className="flex min-h-0 flex-1 flex-col overflow-hidden select-none">
              <ErrorBoundary
                fallback={
                  <div className="flex h-full min-h-0 flex-1 items-center justify-center px-4 text-[13px] text-black/35">
                    左侧栏加载异常
                  </div>
                }
              >
                <ProjectRail
                  projects={projects}
                  projectSessions={projectSessions}
                  activeProjectId={activeProjectId}
                  activeSessionId={activeSessionId}
                  onSelectProject={onSelectProject}
                  onSelectSession={onSelectSession}
                  onNewChat={onNewChat}
                  onDeleteProject={onDeleteProject}
                  onRenameProject={onRenameProject}
                  onDeleteSession={onDeleteSession}
                  onTogglePinSession={onTogglePinSession}
                  onOpenInFinder={onOpenInFinder}
                  onCreatePermanentWorktree={onCreatePermanentWorktree}
                  runningSessionIds={runningSessionIds}
                  loading={loading}
                />
              </ErrorBoundary>
            </div>
          </div>

          <div
            role="separator"
            aria-orientation="vertical"
            onPointerDown={onResizeStart}
            className="absolute right-0 top-0 z-30 h-full w-4 cursor-col-resize touch-none select-none bg-transparent"
            style={{ touchAction: 'none' }}
          />
        </aside>
      </ErrorBoundary>

      <main
        className="relative z-10 flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-transparent transition-[padding-left] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
        style={{ paddingLeft: isLeftPaneCollapsed ? 0 : leftPaneWidth }}
      >
        <ErrorBoundary
          fallback={(errorMessage) => (
            <div className="flex h-full min-h-0 items-center justify-center px-6">
                <div className="max-w-md rounded-3xl border border-black/5 bg-white px-6 py-5 text-center shadow-sm">
                <div className="text-[14px] font-semibold tracking-tight">主内容加载异常</div>
                <div className="mt-2 text-[12px] leading-5 text-black/45">
                  主工作区发生了运行时错误，但左侧栏仍然保持可用。
                </div>
                {errorMessage ? (
                  <div className="mt-3 rounded-xl border border-black/6 bg-black/[0.03] px-3 py-2 text-left font-mono text-[11px] leading-5 text-black/55">
                    {errorMessage}
                  </div>
                ) : null}
              </div>
            </div>
          )}
        >
          <div className="flex min-h-0 flex-1 flex-col">
            <header
              className="window-drag flex h-14 shrink-0 items-center justify-between border-b border-black/5 px-4 select-none lg:px-5"
              onMouseDown={onStartWindowDrag}
            >
              <div className="flex min-w-0 items-center gap-3">
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground transition-all duration-200 hover:bg-[linear-gradient(135deg,rgba(255,255,255,0.92),rgba(242,236,227,0.92))] hover:text-black/82 hover:shadow-[0_8px_18px_rgba(15,23,42,0.08)]"
                  data-window-no-drag="true"
                  onClick={onToggleLeftPane}
                >
                  {isLeftPaneCollapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
                </Button>
                <div className="hidden h-9 w-9 items-center justify-center rounded-xl bg-black/5 text-black/70 lg:flex">
                  <MessageSquare className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <h1 className="truncate text-[14px] font-semibold tracking-tight">{activeTitle}</h1>
                    <span className="text-[12px] text-muted-foreground">if2Ai</span>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="window-no-drag h-7 w-7 rounded-full text-muted-foreground"
                      data-window-no-drag="true"
                    >
                      <MoreHorizontal className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground"
                  data-window-no-drag="true"
                >
                  <Play className="h-4 w-4" />
                </Button>
                <div className="window-no-drag" data-window-no-drag="true">
                  <HeaderViewStyleControls
                    fontMode={fontMode}
                    densityMode={densityMode}
                    onFontModeChange={setFontMode}
                    onDensityModeChange={setDensityMode}
                  />
                </div>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="outline"
                      className="window-no-drag h-9 rounded-full border-black/10 bg-white px-3 text-[13px] shadow-none"
                      data-window-no-drag="true"
                    >
                      <Code2 className="mr-2 h-4 w-4 text-blue-500" />
                      {formatModelName(selectedModel)}
                      <ChevronDown className="ml-2 h-3.5 w-3.5 opacity-70" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent sideOffset={6} align="end" className="w-[148px]">
                    {modelItems.map((item) => (
                      <DropdownMenuItem
                        key={item.value}
                        className={cn(
                          'h-8 rounded-[12px] px-2.5 text-[12.5px] font-medium',
                          selectedModel === item.value
                            ? 'bg-black/[0.045] text-black/90'
                            : 'text-black/82'
                        )}
                        onSelect={() => onModelChange(item.value)}
                      >
                        {item.label}
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuContent>
                </DropdownMenu>
                <Button
                  variant="outline"
                  className="window-no-drag h-9 rounded-full border-black/10 bg-white px-3 text-[13px] shadow-none"
                  data-window-no-drag="true"
                >
                  <SlidersHorizontal className="mr-2 h-4 w-4" />
                  提交
                  <ChevronDown className="ml-2 h-3.5 w-3.5 opacity-70" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground transition-all duration-200 hover:bg-[linear-gradient(135deg,rgba(255,255,255,0.94),rgba(243,236,226,0.94))] hover:text-black/82 hover:shadow-[0_8px_18px_rgba(15,23,42,0.08)]"
                  data-window-no-drag="true"
                  title={isRightRailOpen ? '关闭右侧 Rail' : '打开右侧 Rail'}
                  aria-label={isRightRailOpen ? '关闭右侧 Rail' : '打开右侧 Rail'}
                  aria-pressed={isRightRailOpen}
                  onClick={onToggleRightRail}
                >
                  <SquareTerminal className="h-4 w-4" />
                </Button>
              </div>
            </header>

            <div className="min-h-0 flex-1 overflow-hidden">
              {activeSessionId ? (
                <div className="flex h-full min-h-0 flex-col overflow-hidden">
                  <div className="min-h-0 flex-1">
                    <ChatUI
                      sessionTitle={activeTitle}
                      projectLabel={currentProject?.name ?? 'if2Ai'}
                      defaultWorkdir={currentProject?.workdir}
                      branchLabel={branchLabel}
                      messages={activeMessages}
                      input={input}
                      onInputChange={onInputChange}
                      onSubmit={onSubmit}
                      onResumeFromCursor={onResumeFromCursor}
                      onStop={onStop}
                      isLoading={isLoading}
                      selectedModel={selectedModel}
                      onModelChange={onModelChange}
                      permissionMode={permissionMode}
                      onPermissionModeChange={onPermissionModeChange}
                      todos={todos}
                      isProjectRailOpen={isRightRailOpen}
                      onProjectRailOpenChange={onRightRailOpenChange}
                      isLeftPaneCollapsed={isLeftPaneCollapsed}
                      densityMode={densityMode}
                      fontMode={fontMode}
                      onPreviewFocusChange={onPreviewFocusChange}
                    />
                  </div>
                </div>
              ) : (
                <div className="flex h-full min-h-0 flex-col">
                  <div className="mx-auto flex w-full max-w-[920px] flex-1 items-center justify-center px-6 py-8">
                    <div className="max-w-xl rounded-[28px] border border-black/5 bg-white px-8 py-10 text-center shadow-[0_20px_60px_rgba(0,0,0,0.04)]">
                      <div className="mx-auto mb-5 flex h-14 w-14 items-center justify-center rounded-2xl bg-black/5">
                        <Sparkles className="h-7 w-7 text-black/60" />
                      </div>
                      <div className="space-y-3">
                        <h2 className="text-[22px] font-semibold tracking-tight">选择一个项目，开始新的线程</h2>
                        <p className="text-[13px] leading-6 text-muted-foreground">
                          左侧已经整理好项目与会话，右侧会像 Codex 一样展示变更摘要、思考过程和最终正文。
                        </p>
                      </div>
                    </div>
                  </div>
                  <ChatUI
                    sessionTitle={activeTitle}
                    projectLabel={currentProject?.name ?? 'if2Ai'}
                    defaultWorkdir={currentProject?.workdir}
                    branchLabel={branchLabel}
                    messages={[]}
                    input={input}
                    onInputChange={onInputChange}
                    onSubmit={onSubmit}
                    onResumeFromCursor={onResumeFromCursor}
                    onStop={onStop}
                    isLoading={isLoading}
                    selectedModel={selectedModel}
                    onModelChange={onModelChange}
                    permissionMode={permissionMode}
                    onPermissionModeChange={onPermissionModeChange}
                    todos={[]}
                    isProjectRailOpen={isRightRailOpen}
                    onProjectRailOpenChange={onRightRailOpenChange}
                    isLeftPaneCollapsed={isLeftPaneCollapsed}
                    densityMode={densityMode}
                    fontMode={fontMode}
                    onPreviewFocusChange={onPreviewFocusChange}
                  />
                </div>
              )}
            </div>
          </div>
        </ErrorBoundary>
      </main>
    </div>
  )
}
