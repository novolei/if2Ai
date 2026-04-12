import { useMemo } from 'react'
import {
  ChevronDown,
  Code2,
  MessageSquare,
  MoreHorizontal,
  Play,
  Plus,
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
import { TodoPanel } from '@/components/ui/TodoPanel'
import { SessionStatus } from '@/components/SessionStatus'
import { ProjectRail } from '@/components/ProjectRail'
import type { ChatWorkspaceProps } from '../types'
import { SidebarTop } from './SidebarTop'

const modelItems = [
  { value: 'gpt-5.4-mini', label: 'GPT-5.4-Mini' },
  { value: 'gpt-5.4', label: 'GPT-5.4' },
  { value: 'gpt-4.1', label: 'GPT-4.1' },
]

function formatModelName(model: string): string {
  return model.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
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
  onStop,
  selectedModel,
  onModelChange,
  todos,
  leftPaneWidth,
  onResizeStart,
  onStartWindowDrag,
  runningSessionIds,
  status,
}: ChatWorkspaceProps) {
  const openNewChat = useMemo(() => {
    return () => {
      const targetProjectId = activeProjectId ?? projects[0]?.id
      if (targetProjectId) onNewChat(targetProjectId)
    }
  }, [activeProjectId, onNewChat, projects])

  return (
    <div
      className="grid h-full min-h-0 min-w-0 overflow-hidden"
      style={{ gridTemplateColumns: `${leftPaneWidth}px minmax(0, 1fr)` }}
    >
      <ErrorBoundary
        fallback={
          <aside
            className="relative z-20 flex h-full min-h-0 shrink-0 flex-col overflow-hidden rounded-tr-[18px] rounded-br-[18px] border-r border-black/5 bg-[#eef0f1]/56 backdrop-blur-[2px]"
            style={{ width: leftPaneWidth }}
          >
            <SidebarTop onStartWindowDrag={onStartWindowDrag} onNewChat={openNewChat} />
            <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-[13px] text-black/35">
              左侧栏加载异常
            </div>
          </aside>
        }
      >
        <aside className="relative z-20 flex h-full min-h-0 min-w-0 flex-col overflow-hidden rounded-tr-[18px] rounded-br-[18px] border-r border-black/5 bg-[#eef0f1]/56 backdrop-blur-[2px]">
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

          <div
            role="separator"
            aria-orientation="vertical"
            onPointerDown={onResizeStart}
            className="absolute right-0 top-0 z-30 h-full w-4 cursor-col-resize touch-none select-none bg-transparent"
            style={{ touchAction: 'none' }}
          />
        </aside>
      </ErrorBoundary>

      <main className="relative z-10 flex min-h-0 min-w-0 flex-col overflow-hidden bg-transparent">
        <ErrorBoundary
          fallback={
            <div className="flex h-full min-h-0 items-center justify-center px-6">
                <div className="max-w-md rounded-3xl border border-black/5 bg-white px-6 py-5 text-center shadow-sm">
                <div className="text-[14px] font-semibold tracking-tight">主内容加载异常</div>
                <div className="mt-2 text-[12px] leading-5 text-black/45">
                  主工作区发生了运行时错误，但左侧栏仍然保持可用。
                </div>
              </div>
            </div>
          }
        >
          <div className="flex min-h-0 flex-1 flex-col">
            <header
              className="window-drag flex h-14 shrink-0 items-center justify-between border-b border-black/5 px-4 select-none lg:px-5"
              onMouseDown={onStartWindowDrag}
            >
              <div className="flex min-w-0 items-center gap-3">
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
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground"
                  data-window-no-drag="true"
                >
                  <SquareTerminal className="h-4 w-4" />
                </Button>
                <div className="h-6 w-px bg-black/10" />
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground"
                  data-window-no-drag="true"
                >
                  <Plus className="h-4 w-4" />
                </Button>
                <div className="ml-1 flex items-center gap-2 text-[15px] font-medium">
                  <span className="text-muted-foreground">—</span>
                  <span className="text-muted-foreground">—</span>
                </div>
              </div>
            </header>

            <div className="min-h-0 flex-1 overflow-hidden">
              {activeSessionId && (
                <SessionStatus
                  status={status ?? (isLoading ? 'running' : 'idle')}
                />
              )}
              <TodoPanel todos={todos} />
              {activeSessionId ? (
                <ChatUI
                  sessionTitle={activeTitle}
                  projectLabel={currentProject?.name ?? 'if2Ai'}
                  branchLabel={branchLabel}
                  messages={activeMessages}
                  input={input}
                  onInputChange={onInputChange}
                  onSubmit={onSubmit}
                  onStop={onStop}
                  isLoading={isLoading}
                  selectedModel={selectedModel}
                  onModelChange={onModelChange}
                />
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
                    branchLabel={branchLabel}
                    messages={[]}
                    input={input}
                    onInputChange={onInputChange}
                    onSubmit={onSubmit}
                    onStop={onStop}
                    isLoading={isLoading}
                    selectedModel={selectedModel}
                    onModelChange={onModelChange}
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
