import { useEffect, useState } from "react";
import {
  AlignJustify,
  Check,
  ChevronLeft,
  ChevronRight,
  Globe,
  Loader2,
  MessageSquare,
  MoreHorizontal,
  PanelRightClose,
  PanelRightOpen,
  Redo2,
  Rows3,
  Type,
  Undo2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { GitActionsPicker } from "@/components/chat/GitActionsPicker";
import { GitWorkbenchDialog } from "@/components/chat/GitWorkbenchDialog";
import { cn } from "@/lib/utils";
import { ChatUI } from "@/components/ui/chat-ui";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { ProjectRail } from "@/components/ProjectRail";
import type { ChatWorkspaceProps } from "../types";
import { HomeScreen } from "./HomeScreen";
import { SidebarTop } from "./SidebarTop";
import { BrowserCard } from "@/components/browser/BrowserCard";
import { useBrowserStore } from "@/stores/browser-slice";
import { refreshPendingPermission } from "@/runtime-projection";

const CHAT_DENSITY_MODE_STORAGE_KEY = "chatDensityModeV2";
const CHAT_FONT_MODE_STORAGE_KEY = "chatFontModeV2";
type DensityMode = "comfortable" | "compact";
type FontMode = "sans" | "serif";

function HeaderViewStyleControls({
  fontMode,
  densityMode,
  onFontModeChange,
  onDensityModeChange,
}: {
  fontMode: FontMode;
  densityMode: DensityMode;
  onFontModeChange: (mode: FontMode) => void;
  onDensityModeChange: (mode: DensityMode) => void;
}) {
  const fontLabel = fontMode === "serif" ? "衬线" : "无衬线";
  const densityLabel = densityMode === "compact" ? "紧凑" : "舒适";

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[12px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
          aria-label="文字显示设置"
          title={`文字显示：${fontLabel} · ${densityLabel}`}
        >
          <Type className="h-3.5 w-3.5" strokeWidth={1.8} />
          <span className="max-w-[72px] truncate">{fontLabel}</span>
          <span className="text-muted-foreground/55">·</span>
          <span>{densityLabel}</span>
          <ChevronRight className="h-3 w-3 rotate-90 opacity-70" strokeWidth={1.8} />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" sideOffset={8} className="w-[184px]">
        <DropdownMenuLabel className="px-2.5 py-1 text-[10.5px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
          字体
        </DropdownMenuLabel>
        <DropdownMenuItem
          className={cn("h-8 justify-between", fontMode === "sans" && "the-finals-selected-menu-item")}
          onSelect={() => onFontModeChange("sans")}
        >
          <span className="inline-flex items-center gap-2">
            <Type className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[12.5px] font-medium">无衬线</span>
          </span>
          {fontMode === "sans" ? <Check className="h-3.5 w-3.5 text-primary" /> : null}
        </DropdownMenuItem>
        <DropdownMenuItem
          className={cn("h-8 justify-between", fontMode === "serif" && "the-finals-selected-menu-item")}
          onSelect={() => onFontModeChange("serif")}
        >
          <span className="inline-flex items-center gap-2">
            <span className="font-serif text-[13px] font-semibold text-muted-foreground">Aa</span>
            <span className="text-[12.5px] font-medium">衬线</span>
          </span>
          {fontMode === "serif" ? <Check className="h-3.5 w-3.5 text-primary" /> : null}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuLabel className="px-2.5 py-1 text-[10.5px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
          密度
        </DropdownMenuLabel>
        <DropdownMenuItem
          className={cn("h-8 justify-between", densityMode === "comfortable" && "the-finals-selected-menu-item")}
          onSelect={() => onDensityModeChange("comfortable")}
        >
          <span className="inline-flex items-center gap-2">
            <AlignJustify className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[12.5px] font-medium">舒适</span>
          </span>
          {densityMode === "comfortable" ? <Check className="h-3.5 w-3.5 text-primary" /> : null}
        </DropdownMenuItem>
        <DropdownMenuItem
          className={cn("h-8 justify-between", densityMode === "compact" && "the-finals-selected-menu-item")}
          onSelect={() => onDensityModeChange("compact")}
        >
          <span className="inline-flex items-center gap-2">
            <Rows3 className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[12.5px] font-medium">紧凑</span>
          </span>
          {densityMode === "compact" ? <Check className="h-3.5 w-3.5 text-primary" /> : null}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function ChatWorkspace({
  projects,
  projectSessions,
  activeProjectId,
  activeSessionId,
  currentProject,
  branchLabel,
  isGitRepo,
  onGitRepoChanged,
  onBranchChange,
  onWorktreeProjectCreated,
  activeTitle,
  // GF-01 PR-09 (DT-03) — `activeMessages` is no longer forwarded
  // to <ChatUI>; the chat surface reads its own messages from the
  // runtime projection store + chat-store conversation slice. The
  // prop is still accepted (and consumed by App.tsx for the
  // title-stage rename and prompt-diagnostics surfaces) so we keep
  // it in the destructure even though this component does not
  // forward it anywhere.
  activeMessages: _activeMessages,
  activeSessionTotals,
  input,
  isLoading,
  loading,
  onSelectProject,
  onSelectSession,
  onNewChat,
  onNewThread,
  onHomeProjectSelect,
  onPickFolderAndCreateProject,
  onSendMessage,
  recentSessions,
  onDeleteProject,
  onRenameProject,
  onDeleteSession,
  onRenameSession,
  onTogglePinSession,
  onUpdateSessionIdentity,
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
  activeSessionMeta,
  conversationUndoStatus,
  onConversationUndo,
  onConversationRedo,
}: ChatWorkspaceProps) {
  // Browser store — used to show the globe badge in the header when the AI's
  // browser is actively running for the current session.
  const { browserBySession } = useBrowserStore();
  const isBrowserRunning =
    activeSessionId != null &&
    (browserBySession[activeSessionId]?.running ?? false);

  const [densityMode, setDensityMode] = useState<DensityMode>(() => {
    if (typeof window === "undefined") return "comfortable";
    try {
      const stored = window.localStorage.getItem(CHAT_DENSITY_MODE_STORAGE_KEY);
      return stored === "compact" ? "compact" : "comfortable";
    } catch {
      return "comfortable";
    }
  });
  const [fontMode, setFontMode] = useState<FontMode>(() => {
    if (typeof window === "undefined") return "sans";
    try {
      const stored = window.localStorage.getItem(CHAT_FONT_MODE_STORAGE_KEY);
      return stored === "serif" ? "serif" : "sans";
    } catch {
      return "sans";
    }
  });
  const [workbenchOpen, setWorkbenchOpen] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined") return;
    try {
      window.localStorage.setItem(CHAT_DENSITY_MODE_STORAGE_KEY, densityMode);
    } catch {
      // Ignore storage failures so layout controls never crash the main workspace.
    }
  }, [densityMode]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    try {
      window.localStorage.setItem(CHAT_FONT_MODE_STORAGE_KEY, fontMode);
    } catch {
      // Ignore storage failures so layout controls never crash the main workspace.
    }
  }, [fontMode]);

  useEffect(() => {
    if (!activeSessionId) return;
    void refreshPendingPermission(activeSessionId).catch((err) => {
      console.warn("[permission] failed to recover pending permission", err);
    });
  }, [activeSessionId]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!activeSessionId || !conversationUndoStatus) return;
      const target = e.target as HTMLElement | null;
      if (target?.closest("[data-composer-input]")) return;
      const mod = e.metaKey || e.ctrlKey;
      if (!mod || e.key.toLowerCase() !== "z") return;
      if (e.shiftKey) {
        if (!conversationUndoStatus.canRedo || !onConversationRedo) return;
        e.preventDefault();
        void onConversationRedo();
      } else {
        if (!conversationUndoStatus.canUndo || !onConversationUndo) return;
        e.preventDefault();
        void onConversationUndo();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    activeSessionId,
    conversationUndoStatus,
    onConversationUndo,
    onConversationRedo,
  ]);

  return (
    <div className="relative h-full min-h-0 min-w-0 overflow-hidden">
      <ErrorBoundary
        fallback={
          <aside
            className="absolute inset-y-0 left-0 z-20 flex min-h-0 flex-col overflow-hidden rounded-tr-[18px] rounded-br-[18px] border-r border-border/60 bg-[var(--chat-sidebar-bg-soft,rgba(238,240,241,0.56))] backdrop-blur-[2px]"
            style={{ width: leftPaneWidth }}
          >
            <SidebarTop
              onStartWindowDrag={onStartWindowDrag}
              onNewThread={onNewThread}
              activeSessionId={activeSessionId}
              activeSessionMeta={activeSessionMeta}
              activeTitle={activeTitle}
              onUpdateSessionIdentity={onUpdateSessionIdentity}
            />
            <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-[13px] text-muted-foreground">
              左侧栏加载异常
            </div>
          </aside>
        }
      >
        <aside
          className={cn(
            "absolute inset-y-0 left-0 z-20 flex min-h-0 origin-left flex-col overflow-hidden rounded-tr-[18px] rounded-br-[18px] border-r border-border/60 bg-[var(--chat-sidebar-bg,rgba(238,240,241,0.88))] shadow-[18px_0_36px_rgba(15,23,42,0.08)] backdrop-blur-[6px] transition-[transform,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]",
            isLeftPaneCollapsed
              ? "pointer-events-none -translate-x-full opacity-0"
              : "translate-x-0 opacity-100",
          )}
          style={{ width: leftPaneWidth }}
          aria-hidden={isLeftPaneCollapsed}
        >
          <div className="flex h-full min-h-0 min-w-0 flex-col">
            <SidebarTop
              onStartWindowDrag={onStartWindowDrag}
              onNewThread={onNewThread}
              activeSessionId={activeSessionId}
              activeSessionMeta={activeSessionMeta}
              activeTitle={activeTitle}
              onUpdateSessionIdentity={onUpdateSessionIdentity}
            />
            <div className="flex min-h-0 flex-1 flex-col overflow-hidden select-none">
              <ErrorBoundary
                fallback={
                  <div className="flex h-full min-h-0 flex-1 items-center justify-center px-4 text-[13px] text-muted-foreground">
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
                  onRenameSession={onRenameSession}
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
            style={{ touchAction: "none" }}
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
              <div className="max-w-md rounded-3xl border border-border bg-card px-6 py-5 text-center shadow-sm">
                <div className="text-[14px] font-semibold tracking-tight">
                  主内容加载异常
                </div>
                <div className="mt-2 text-[12px] leading-5 text-muted-foreground">
                  主工作区发生了运行时错误，但左侧栏仍然保持可用。
                </div>
                {errorMessage ? (
                  <div className="mt-3 rounded-xl border border-border bg-muted px-3 py-2 text-left font-mono text-[11px] leading-5 text-muted-foreground">
                    {errorMessage}
                  </div>
                ) : null}
              </div>
            </div>
          )}
        >
          <div className="flex min-h-0 flex-1 flex-col">
            <header
              className="window-drag flex h-14 shrink-0 items-center justify-between border-b border-border/60 px-4 select-none lg:px-5"
              onMouseDown={onStartWindowDrag}
            >
              <div className="flex min-w-0 items-center gap-3">
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground transition-all duration-200 hover:bg-accent hover:text-accent-foreground hover:shadow-[0_8px_18px_rgba(15,23,42,0.08)]"
                  data-window-no-drag="true"
                  onClick={onToggleLeftPane}
                >
                  {isLeftPaneCollapsed ? (
                    <ChevronRight className="h-4 w-4" />
                  ) : (
                    <ChevronLeft className="h-4 w-4" />
                  )}
                </Button>
                <div className="hidden h-9 w-9 items-center justify-center rounded-xl bg-muted text-[17px] leading-none text-muted-foreground lg:flex">
                  {activeSessionMeta?.title_pending ? (
                    <Loader2 className="h-4 w-4 animate-spin" strokeWidth={1.8} />
                  ) : activeSessionMeta?.title_icon ? (
                    <span className="session-emoji" aria-hidden="true">{activeSessionMeta.title_icon}</span>
                  ) : (
                    <MessageSquare className="h-4 w-4" strokeWidth={1.5} />
                  )}
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <h1 className="truncate text-[14px] font-semibold tracking-tight">
                      {activeTitle}
                    </h1>
                    <span className="text-[12px] text-muted-foreground">
                      if2Ai
                    </span>
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
                {/* Globe badge — visible while the AI's browser is running */}
                {isBrowserRunning && (
                  <div
                    className="flex h-7 items-center gap-1.5 rounded-full bg-jade/12 px-2.5 text-[11px] font-medium text-jade"
                    title="AI 浏览器运行中"
                  >
                    <Globe className="h-3 w-3" />
                    <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-jade" />
                  </div>
                )}
                {activeSessionId && conversationUndoStatus ? (
                  <div
                    className="window-no-drag flex items-center gap-0.5"
                    data-window-no-drag="true"
                  >
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 rounded-full text-muted-foreground disabled:opacity-40"
                      data-window-no-drag="true"
                      disabled={!conversationUndoStatus.canUndo}
                      title="撤销上一轮 (⌘Z)"
                      aria-label="撤销对话"
                      onClick={() => {
                        void onConversationUndo?.();
                      }}
                    >
                      <Undo2 className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 rounded-full text-muted-foreground disabled:opacity-40"
                      data-window-no-drag="true"
                      disabled={!conversationUndoStatus.canRedo}
                      title="重做 (⌘⇧Z)"
                      aria-label="重做对话"
                      onClick={() => {
                        void onConversationRedo?.();
                      }}
                    >
                      <Redo2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                ) : null}
                <div className="window-no-drag" data-window-no-drag="true">
                  <HeaderViewStyleControls
                    fontMode={fontMode}
                    densityMode={densityMode}
                    onFontModeChange={setFontMode}
                    onDensityModeChange={setDensityMode}
                  />
                </div>
                <GitActionsPicker
                  cwd={currentProject?.workdir}
                  isGitRepo={isGitRepo ?? null}
                  onGitRepoChanged={onGitRepoChanged}
                  onBranchChange={onBranchChange}
                  onOpenWorkbench={() => setWorkbenchOpen(true)}
                  onWorktreeProjectCreated={
                    onWorktreeProjectCreated
                      ? (project) =>
                          onWorktreeProjectCreated({
                            id: project.id,
                            name: project.name,
                            workdir: project.workdir,
                          })
                      : undefined
                  }
                />
                <Button
                  variant="ghost"
                  size="icon"
                  className="window-no-drag h-9 w-9 rounded-full text-muted-foreground transition-all duration-200 hover:bg-accent hover:text-accent-foreground hover:shadow-[0_8px_18px_rgba(15,23,42,0.08)]"
                  data-window-no-drag="true"
                  title={isRightRailOpen ? "收起侧边抽屉" : "展开侧边抽屉"}
                  aria-label={
                    isRightRailOpen ? "收起侧边抽屉" : "展开侧边抽屉"
                  }
                  aria-pressed={isRightRailOpen}
                  onClick={onToggleRightRail}
                >
                  {isRightRailOpen ? (
                    <PanelRightClose className="h-4 w-4" />
                  ) : (
                    <PanelRightOpen className="h-4 w-4" />
                  )}
                </Button>
              </div>
            </header>

            <div className="min-h-0 flex-1 overflow-hidden">
              {activeSessionId ? (
                <div className="relative flex h-full min-h-0 flex-col overflow-hidden">
                  {/* BrowserCard — floats over the chat area when AI browser is active */}
                  <BrowserCard sessionId={activeSessionId} />
                  <div className="min-h-0 flex-1">
                    <ChatUI
                      sessionTitle={activeTitle}
                      sessionId={activeSessionId}
                      projectLabel={currentProject?.name ?? "if2Ai"}
                      defaultWorkdir={currentProject?.workdir}
                      branchLabel={branchLabel}
                      isGitRepo={isGitRepo ?? null}
                      onGitRepoChanged={onGitRepoChanged}
                      onBranchChange={onBranchChange}
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
                      sessionTotals={activeSessionTotals}
                    />
                  </div>
                </div>
              ) : (
                <HomeScreen
                  projects={projects}
                  recentSessions={recentSessions}
                  selectedProjectId={activeProjectId}
                  selectedModel={selectedModel}
                  onModelChange={onModelChange}
                  permissionMode={permissionMode}
                  onPermissionModeChange={onPermissionModeChange}
                  onSendMessage={onSendMessage}
                  onSelectSession={(projectId, sessionId) => {
                    void onSelectSession(projectId, sessionId);
                  }}
                  onHomeProjectSelect={onHomeProjectSelect}
                  onPickFolderAndCreateProject={onPickFolderAndCreateProject}
                  isLoading={isLoading}
                  branchLabel={branchLabel}
                />
              )}
            </div>
          </div>
        </ErrorBoundary>
      </main>
      <GitWorkbenchDialog
        open={workbenchOpen}
        onOpenChange={setWorkbenchOpen}
        cwd={currentProject?.workdir}
        currentBranch={branchLabel}
      />
    </div>
  );
}
