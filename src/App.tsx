import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
// MIG-012 — canonical App.tsx transport seam.
//
// Business helpers come from `@/api/*` domain facades;
// `@/lib/tauri` is only kept as the `invoke` passthrough plus
// wire-level DTO re-exports that have not yet been moved to
// `@/transport/contracts`. Every command name in this import
// block is now hidden behind a domain facade.
import {
  awaitGatewayReady,
  createPermanentWorktree,
  createProject,
  createSession,
  deleteProject,
  deleteSession,
  ensureDefaultWorkdir,
  getOnboardingState,
  listProjects,
  listProjectSessions,
  openProjectInFinder,
  openSettingsWindow,
  pickFolderDialog,
  renameProject,
  setSessionPinned,
  updateSessionIdentity,
  type ProjectMeta,
  type SessionIdentityInput,
  type SessionMeta,
} from "@/api";
import type { PermissionMode } from "@/transport/contracts";
import { toast } from "sonner";
import appIconAsset from "@/assets/app-icon.png";
import {
  projectConversationMessagesFromRuns,
  useExecutionModePreview,
  useRuntimeProjectionSelector,
} from "@/runtime-projection";
import { RuntimeProjectionWiring } from "@/app-effects/RuntimeProjectionWiring";
import { useUpdaterBanner } from "@/app-effects/useUpdaterBanner";
import { useGlobalHotkeys } from "@/app-effects/useGlobalHotkeys";
import { useChatPrefill } from "@/app-effects/useChatPrefill";
import { loadConversationHistory } from "@/session/loadConversationHistory";
import {
  PLACEHOLDER_SESSION_TITLE,
  isMeaningfulUserMessage,
} from "@/session/titleStage";
import { useSessionTitleStage } from "@/session/useSessionTitleStage";
import { useSessionRuntime } from "@/session/SessionEffects";
// MIG-013 — AppShell is the canonical top-level shell container
// (BootShell + MainShell + ContentRouter). Boot state lives in
// the bootstrap-store; App.tsx is now a data-flow host, not a
// render / boot orchestrator.
import { AppShell } from "@/modules/app-shell/AppShell";
import { runBootSequence } from "@/boot/boot-orchestrator";
import { bootstrapStore } from "@/state";
// MIG-014 + GF-03 PR-3 — chat + session store wiring lives in
// `useAppStateSetters()`; App.tsx only consumes the destructured
// shims so the `Dispatch<SetStateAction<T>>` shape is preserved
// at every existing call site without per-store imports here.
import { useAppStateSetters } from "@/stores/use-store-setter-shims";
// AppVersionWatermark moved into MainShell (Phase M2.7).
import { SectionWorkspace } from "@/modules/app-shell/components/SectionWorkspace";
import type { AppSection } from "@/modules/app-shell/types";
import { ChatWorkspace } from "@/modules/chat/components/ChatWorkspace";
import type {
  RecentSession,
  SessionTitleState,
} from "@/modules/chat/types";
import { useAgentVoiceBridge } from "@/modules/chat/useAgentVoiceBridge";
import { useMemoryWriteToasts } from "@/components/memory/useMemoryWriteToasts";
import { AgentVoiceIndicator } from "@/modules/chat/AgentVoiceIndicator";
import { broadcastChange, useCrossWindowChange } from "@/lib/crossWindowSync";
import type { PromptDiagnosticsSnapshot } from "@/modules/prompt-diagnostics/storage";
// OnboardingApp moved into BootShell (Phase M2.7).
import { MemoryBrowser } from "@/components/memory/MemoryBrowser";
// If2AiLoadingScreen moved into BootShell (Phase M2.7).
import { TelemetryDrawer } from "@/components/chat/TelemetryDrawer";
import { CreateProjectDialog } from "@/components/CreateProjectDialog";
import type { TodoItem } from "@/components/ui/TodoPanel";
import { PermissionOverlayHost } from "@/permission/PermissionOverlayHost";

const appIconSrc = appIconAsset;
// GF-03 PR-4 — title-stage constants + helpers extracted to
// `src/session/titleStage.ts` (pure) and the React surface to
// `src/session/useSessionTitleStage.ts`.

function App() {
  const appWindow = getCurrentWindow();
  // MIG-013 — boot phase / project list / active project now
  // live in the bootstrap store. Local reads go through
  // `useBootstrapSelector` so App.tsx re-renders on exactly the
  // slices it consumes; writes use the store's explicit actions.
  // MIG-013 — boot phase lives in the bootstrap store; AppShell
  // reads it internally and re-renders when the phase changes.
  // App.tsx itself subscribes to specific slices below
  // (projects / activeProjectId / currentProject / projectSessions),
  // which transition together on every phase mutation, so no
  // separate phase subscription is needed here.
  // Phase TTS-D / P1：Agent 语音桥接
  const agentVoice = useAgentVoiceBridge();

  // Memory System Audit P2 #10 — surface memory writes as a subtle
  // sonner toast on the chat surface so the user can see "AI 记住了
  // X 件事" without opening Settings.
  useMemoryWriteToasts();

  // GF-03 PR-1 — runtime/effect wiring extracted to `src/app-effects/`.
  const {
    appUpdaterState,
    latestUpdaterVersion,
    updaterBannerVisible,
    dismiss: handleDismissUpdaterBanner,
    runUpdater: handleRunUpdaterFromRail,
  } = useUpdaterBanner();
  const { isTelemetryDrawerOpen, setTelemetryDrawerOpen } = useGlobalHotkeys();

  // 跨窗口监听 Onboarding 重置：设置窗口点重置后，主窗口立即跳回 Onboarding 流程
  useCrossWindowChange("cross:onboarding-reset", () => {
    console.log(
      "[App] cross-window: onboarding reset → re-entering onboarding flow",
    );
    bootstrapStore.enterOnboarding();
  });

  useCrossWindowChange<{
    sessionId: string;
    soulId: string | null;
    personaId: string | null;
  }>("cross:session-identity-changed", (payload) => {
    setProjectSessions((prev) => {
      const next: Record<string, SessionMeta[]> = {};
      for (const [projectId, sessions] of Object.entries(prev)) {
        next[projectId] = sessions.map((session) =>
          session.id === payload.sessionId
            ? {
                ...session,
                soul_id: payload.soulId,
                persona_id: payload.personaId,
              }
            : session,
        );
      }
      return next;
    });
  });

  // MIG-013 — boot orchestration moved to `src/boot/boot-orchestrator.ts`.
  // The `useEffect` below is now a thin call into the canonical
  // runner; the store transitions own every phase change.
  // GF-03 PR-1 — models-changed listener + `runtime_event` evolution
  // listener moved into `<RuntimeProjectionWiring />` (mounted in the
  // render tree below).
  useEffect(() => {
    const signal = { cancelled: false };
    void runBootSequence(bootstrapStore, {
      awaitGatewayReady,
      getOnboardingState,
      ensureDefaultWorkdir,
      listProjects,
      listProjectSessions,
      signal,
    });
    return () => {
      signal.cancelled = true;
    };
  }, []);

  const [activeSection, setActiveSection] = useState<AppSection>(() => {
    if (typeof window === "undefined") return "chat";
    const stored = localStorage.getItem("lastActiveSection");
    return stored === "skills" ||
      stored === "automation" ||
      stored === "memory" ||
      stored === "jiaochang"
      ? stored
      : "chat";
  });
  // GF-03 PR-3 — bootstrap / session / chat store setter shims
  // are composed in a single hook so App.tsx no longer carries
  // ~330 LOC of `useCallback` boilerplate. Behavior is identical
  // to the inline definitions; see `use-store-setter-shims.ts`.
  const {
    projects,
    setProjects,
    projectSessions,
    setProjectSessions,
    currentProject,
    setCurrentProject,
    activeProjectId,
    setActiveProjectId,
    activeSessionId,
    setActiveSessionId,
    conversations,
    setConversations,
    sessionLoading,
    setSessionLoading,
    sessionTodos,
    setSessionTodos,
    sessionTitleStates,
    setSessionTitleStates,
    streamAbortHandles,
    setStreamAbortHandles,
  } = useAppStateSetters();
  const [leftPaneWidth, setLeftPaneWidth] = useState(240);
  const [isLeftPaneCollapsed, setIsLeftPaneCollapsed] = useState(false);
  const [loading, setLoading] = useState(false);
  // GF-03 PR-1 — updater banner state machine moved to
  // `useUpdaterBanner()` (see top of component).
  const [input, setInput] = useState("");
  // GF-03 PR-1 — chat-prefill listener (Tauri tray / deeplink → chat).
  useChatPrefill({ setActiveSection, setInput });
  // Phase M2.6 — opt-in classifier preview. Watches the active
  // chat draft and dispatches the deterministic
  // `ExecutionModeDecision` into the projection store. Developer
  // telemetry renders the resulting judgment.
  // Honest scope: pure preview, the agent loop is NOT auto-routed.
  useExecutionModePreview(input, { sessionId: activeSessionId ?? undefined });
  const [isCreateProjectOpen, setIsCreateProjectOpen] = useState(false);
  const [selectedModel, setSelectedModel] = useState("");
  const [isRightRailOpen, setIsRightRailOpen] = useState(false);

  const [permissionMode, setPermissionMode] = useState<PermissionMode>(() => {
    if (typeof window === "undefined") return "dangerFullAccess";
    const stored = localStorage.getItem("permissionMode");
    if (
      stored === "readOnly" ||
      stored === "workspaceWrite" ||
      stored === "dangerFullAccess"
    ) {
      return stored;
    }
    return "dangerFullAccess";
  });
  // GF-03 PR-4 — sessionTitleStatesRef + pendingAutoTitleSessionIdsRef
  // are owned here but mutated by `useSessionTitleStage` (which also
  // owns the ref-sync useEffect).
  const sessionTitleStatesRef = useRef<Record<string, SessionTitleState>>({});
  const pendingAutoTitleSessionIdsRef = useRef<Set<string>>(new Set());
  const { maybeAutoRenameSession, handleRenameSession } = useSessionTitleStage(
    {
      conversations,
      projectSessions,
      sessionTitleStates,
      setConversations,
      setProjectSessions,
      setSessionTitleStates,
      sessionTitleStatesRef,
      pendingAutoTitleSessionIdsRef,
    },
  );
  // GF-03 PR-5 — permission overlay state + decide handler are
  // owned by `usePermissionOverlay()` and rendered via
  // `<PermissionOverlayHost />` in the overlays slot below.  The
  // hook continues to read `snapshot.approvals` from the canonical
  // projection store and dispatches `permission_resolved` so the
  // reducer clears the entry — behavior is unchanged.
  // GF-03 PR-6 — sessionLoadingRef / autoResumeAttemptsRef /
  // attemptedAutoResumeCursorsRef + their sync useEffect now live
  // inside `useSessionRuntime` (see `src/session/SessionEffects.tsx`).
  const leftPaneCollapsedBeforePreviewRef = useRef(false);
  const wasPreviewFocusModeRef = useRef(false);
  const resizeRef = useRef<{
    startX: number;
    startWidth: number;
  } | null>(null);

  const normalizeTodoItem = (value: unknown): TodoItem | null => {
    if (!value || typeof value !== "object") return null;
    const item = value as Record<string, unknown>;
    const content = typeof item.content === "string" ? item.content : "";
    const activeForm =
      typeof item.activeForm === "string"
        ? item.activeForm
        : typeof item.active_form === "string"
          ? item.active_form
          : content;
    const status = item.status;
    if (
      !content ||
      (status !== "pending" &&
        status !== "in_progress" &&
        status !== "completed")
    ) {
      return null;
    }
    return {
      content,
      activeForm,
      status,
    };
  };

  const extractTodosFromToolResult = (
    raw: string | null | undefined,
  ): TodoItem[] | null => {
    if (!raw) return null;
    try {
      const parsed = JSON.parse(raw) as {
        new_todos?: unknown[];
        newTodos?: unknown[];
      };
      const candidates = Array.isArray(parsed.new_todos)
        ? parsed.new_todos
        : Array.isArray(parsed.newTodos)
          ? parsed.newTodos
          : null;
      if (!candidates) return null;
      return candidates
        .map((item) => normalizeTodoItem(item))
        .filter((item): item is TodoItem => item !== null);
    } catch {
      return null;
    }
  };

  /**
   * Parse the structured `memory_store` tool result emitted by the backend
   * (`src-tauri/src/modules/tools/builtin/memory_store.rs`).  The handler
   * always returns a JSON object with `policy_decision`, `scope`,
   * `reason_code`, and `status`; we lift those onto `Message` so the
   * `MemoryStoreToolCard` highlight (deny / prompt) fires deterministically
   * instead of relying on the control-plane permission `policy_decision`,
   * which is unrelated to memory-write policy.
   *
   * Returns `null` when the result is missing, not JSON, or not produced by
   * the new structured `memory_store` handler — callers should fall back to
   * existing behaviour in that case so legacy tool flows are unaffected.
   */
  const extractMemoryStoreFields = (
    raw: string | null | undefined,
  ): {
    policyDecision?: "allow" | "deny" | "prompt";
    memoryScope?: "global" | "project" | "session";
    memoryReasonCode?: string;
  } | null => {
    if (!raw) return null;
    let parsed: Record<string, unknown>;
    try {
      const candidate = JSON.parse(raw);
      if (
        !candidate ||
        typeof candidate !== "object" ||
        Array.isArray(candidate)
      )
        return null;
      parsed = candidate as Record<string, unknown>;
    } catch {
      return null;
    }
    const decisionRaw = parsed.policy_decision;
    const scopeRaw = parsed.scope;
    const reasonCodeRaw = parsed.reason_code;
    const out: {
      policyDecision?: "allow" | "deny" | "prompt";
      memoryScope?: "global" | "project" | "session";
      memoryReasonCode?: string;
    } = {};
    if (
      decisionRaw === "allow" ||
      decisionRaw === "deny" ||
      decisionRaw === "prompt"
    ) {
      out.policyDecision = decisionRaw;
    }
    if (
      scopeRaw === "global" ||
      scopeRaw === "project" ||
      scopeRaw === "session"
    ) {
      out.memoryScope = scopeRaw;
    }
    if (typeof reasonCodeRaw === "string") {
      out.memoryReasonCode = reasonCodeRaw;
    }
    return Object.keys(out).length > 0 ? out : null;
  };


  const activeConv = activeSessionId ? conversations[activeSessionId] : null;
  const activeSessionMeta = useMemo(() => {
    if (!activeProjectId || !activeSessionId) return null;
    return (
      projectSessions[activeProjectId]?.find(
        (session) => session.id === activeSessionId,
      ) ?? null
    );
  }, [activeProjectId, activeSessionId, projectSessions]);

  // Build the last-3 sessions list for the HomeScreen suggestion row
  const recentSessions = useMemo((): RecentSession[] => {
    const all: RecentSession[] = [];
    for (const [projectId, sessions] of Object.entries(projectSessions)) {
      const project = projects.find((p) => p.id === projectId);
      for (const s of sessions) {
        all.push({
          sessionId: s.id,
          projectId,
          projectName: project?.name ?? projectId,
          title: s.title,
          titleIcon: s.title_icon ?? null,
          titlePending: Boolean(s.title_pending),
          updatedAt: s.updated_at,
        });
      }
    }
    all.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
    return all.slice(0, 3);
  }, [projectSessions, projects]);
  const isActiveSessionLoading = activeSessionId
    ? Boolean(sessionLoading[activeSessionId])
    : false;
  const activeTitle = activeConv?.title ?? "新对话";
  const todos = activeSessionId ? (sessionTodos[activeSessionId] ?? []) : [];
  // Live current branch + repo presence for the composer footer pickers.
  // Refreshed whenever the active project changes; updated optimistically
  // by `BranchPicker` / `GitActionsPicker` on successful checkout / init
  // so the UI doesn't lag behind the dropdown.
  //
  // `isGitRepo === null` = still probing (treat as "assume yes" so we
  // don't flash a disabled state while the IPC is in flight).
  const [branchLabel, setBranchLabel] = useState<string>("");
  const [isGitRepo, setIsGitRepo] = useState<boolean | null>(null);
  // Bumping `gitProbeNonce` triggers a full re-probe of branch + repo
  // status — used after `git init` to flip the pickers back on without
  // tearing down / re-mounting the active session.
  const [gitProbeNonce, setGitProbeNonce] = useState(0);
  useEffect(() => {
    let cancelled = false;
    const cwd = currentProject?.workdir;
    if (!cwd) {
      setBranchLabel("");
      setIsGitRepo(null);
      return;
    }
    setIsGitRepo(null);
    void import("@/modules/git/api").then(async ({ gitCurrentBranch, gitIsRepo }) => {
      try {
        const repo = await gitIsRepo(cwd);
        if (cancelled) return;
        setIsGitRepo(repo);
        if (!repo) {
          setBranchLabel("");
          return;
        }
        try {
          const branch = await gitCurrentBranch(cwd);
          if (!cancelled) setBranchLabel(branch);
        } catch {
          if (!cancelled) setBranchLabel("");
        }
      } catch {
        if (!cancelled) {
          setIsGitRepo(false);
          setBranchLabel("");
        }
      }
    });
    return () => {
      cancelled = true;
    };
  }, [currentProject?.workdir, gitProbeNonce]);
  const minLeftPaneWidth = 280;
  const maxLeftPaneWidth = 520;
  const projectionRuns = useRuntimeProjectionSelector((s) => s.runs);
  // P2-11 — pick the most recent projection run for the active session and
  // surface its `sessionTotals`.  When no run exists for this session yet
  // (fresh load before any turn), falls back to undefined so ContextBar
  // hides the row.  Reload-from-disk hydration is a follow-up: it needs
  // `Conversation` to carry the persisted `Session.session_totals`.
  const activeSessionTotals = useMemo(() => {
    if (!activeSessionId) return undefined;
    // 1) Live: latest projection run for this session (set by stream_complete).
    let latestAt = -1;
    let latestTotals:
      | NonNullable<(typeof projectionRuns)[string]["sessionTotals"]>
      | undefined;
    for (const run of Object.values(projectionRuns)) {
      if (run.sessionId !== activeSessionId) continue;
      if (!run.sessionTotals) continue;
      if (run.lastUpdatedAt > latestAt) {
        latestAt = run.lastUpdatedAt;
        latestTotals = run.sessionTotals;
      }
    }
    if (latestTotals) {
      return {
        input_tokens: latestTotals.inputTokens,
        output_tokens: latestTotals.outputTokens,
        cache_creation_input_tokens: latestTotals.cacheCreationInputTokens,
        cache_read_input_tokens: latestTotals.cacheReadInputTokens,
        cost_usd: latestTotals.costUsd,
        turns: latestTotals.turns,
      };
    }
    // 2) Persisted: hydrated from Session.session_totals on load.
    const conv = conversations[activeSessionId];
    return conv?.sessionTotals;
  }, [projectionRuns, activeSessionId, conversations]);
  const activeMessages = useMemo(() => {
    const hasProjectionRunsForSession =
      activeSessionId &&
      Object.values(projectionRuns).some((run) => run.sessionId === activeSessionId);
    if (!hasProjectionRunsForSession && activeConv?.messages.some((msg) => msg.role !== "user")) {
      return activeConv.messages.map((msg) => ({
        ...msg,
        content: msg.content || " ",
      }));
    }
    // T-003 (MIG-017): Chat truth cutover — assistant / tool / thinking /
    // completion for any run present in the projection store are derived from
    // canonical projection. Persisted historical messages that have no
    // matching run projection must remain visible while a new turn streams.
    const baseMessages =
      activeConv?.messages.filter((msg) => !msg.content.includes("[resume_cursor]")) ??
      [];
    return projectConversationMessagesFromRuns(
      baseMessages,
      projectionRuns,
      activeSessionId,
    ).map((msg) => ({
      ...msg,
      content: msg.content || " ",
    }));
  }, [activeConv?.messages, activeSessionId, projectionRuns]);
  const latestPromptDiagnosticsSnapshot =
    useMemo<PromptDiagnosticsSnapshot | null>(() => {
      if (!activeSessionId) return null;
      const latestAssistantMessage = [...activeMessages]
        .reverse()
        .find((msg) => msg.role === "assistant" && msg.promptDiagnostics);
      if (!latestAssistantMessage?.promptDiagnostics) return null;
      return {
        sessionId: activeSessionId,
        projectId: activeProjectId,
        assistantMessageId: latestAssistantMessage.id,
        updatedAt: latestAssistantMessage.timestamp.getTime(),
        summary: latestAssistantMessage.promptDiagnostics,
      };
    }, [activeMessages, activeProjectId, activeSessionId]);
  const runningSessionIds = Object.entries(sessionLoading)
    .filter(([, running]) => running)
    .map(([sessionId]) => sessionId);

  useEffect(() => {
    if (activeProjectId && activeSessionId) {
      localStorage.setItem("lastActiveProjectId", activeProjectId);
      localStorage.setItem("lastActiveSessionId", activeSessionId);
    }
  }, [activeProjectId, activeSessionId]);

  useEffect(() => {
    localStorage.setItem("lastActiveSection", activeSection);
  }, [activeSection]);

  useEffect(() => {
    localStorage.setItem("permissionMode", permissionMode);
  }, [permissionMode]);

  // P1: compute stable counters so the rename effect only fires when truly necessary,
  // not on every streaming token that updates message content.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const activeMeaningfulUserMsgCount = useMemo(
    () =>
      activeConv
        ? activeMessages.filter(
            (m) => m.role === "user" && isMeaningfulUserMessage(m.content),
          ).length
        : 0,
    // isMeaningfulUserMessage is a stable pure function defined in the same render scope
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [activeConv, activeMessages],
  );
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const activeAiReplyCount = useMemo(
    () =>
      activeConv ? activeMessages.filter((m) => m.role === "assistant").length : 0,
    [activeConv, activeMessages],
  );

  // Only re-run when meaningful counters change. Use activeMessages because the
  // visible assistant reply can be projected before it lands in activeConv.messages.
  useEffect(() => {
    if (!activeConv || !activeSessionId) return;
    maybeAutoRenameSession(activeConv.projectId, activeSessionId, {
      ...activeConv,
      messages: activeMessages,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeSessionId, activeMeaningfulUserMsgCount, activeAiReplyCount]);

  // Phase M2.8 — direct `listenToPermissionRequests` removed.
  // Permission prompts now arrive via the projection bridge (see
  // `wireRuntimeProjectionListeners`) into `snapshot.approvals`,
  // which `permissionPrompt` (above) reads via
  // `useRuntimeProjectionSelector`.

  // GF-03 PR-1 / ER-04 — runtime + updater + hotkeys + chat-prefill +
  // auto-compact effects moved to `src/app-effects/`.  The runtime
  // projection bridge is mounted as `<RuntimeProjectionWiring />`
  // below; it subscribes to the canonical `runtime_event` envelope
  // routed via the projection bridge (`agent-token` / permission /
  // memory channels were retired in PR D-1 / D-2 / FIX-14) and feeds
  // `snapshot.approvals` plus the evolution event store.

  const loadProjects = async (): Promise<ProjectMeta[]> => {
    try {
      setLoading(true);
      const projectList = await listProjects();
      setProjects(projectList);

      const sessionsMap: Record<string, SessionMeta[]> = {};
      for (const project of projectList) {
        sessionsMap[project.id] = await listProjectSessions(project.id);
      }
      setProjectSessions(sessionsMap);
      return projectList;
    } catch (err) {
      console.error("Failed to load projects:", err);
      return [];
    } finally {
      setLoading(false);
    }
  };

  // P0: monotonic merge — keep in-memory title when our local state is provisional/locked/manual,
  // preventing a stale backend response from overwriting an optimistic title update.
  const refreshProjectSessions = async (projectId: string) => {
    const sessions = await listProjectSessions(projectId);
    setProjectSessions((prev) => {
      const existing = prev[projectId] ?? [];
      const titleStates = sessionTitleStatesRef.current;
      const merged = sessions.map((s) => {
        const inMem = existing.find((e) => e.id === s.id);
        if (!inMem) return s;
        const ts = titleStates[s.id];
        // Keep the in-memory title when we've already set it and the backend may not have caught up
        if (
          ts &&
          (ts.stage === "provisional" ||
            ts.stage === "locked" ||
            ts.stage === "manual") &&
          inMem.title &&
          inMem.title !== PLACEHOLDER_SESSION_TITLE
        ) {
          return {
            ...s,
            title: inMem.title,
            title_icon: inMem.title_icon ?? s.title_icon ?? null,
            title_pending: inMem.title_pending ?? s.title_pending,
          };
        }
        return s;
      });
      return { ...prev, [projectId]: merged };
    });
  };

  const handleSelectProject = async (projectId: string) => {
    setActiveSection("chat");
    setActiveProjectId(projectId);
    const project = projects.find((p) => p.id === projectId);
    if (project) {
      setCurrentProject({
        id: project.id,
        name: project.name,
        workdir: project.workdir,
        created_at: project.created_at,
        updated_at: "",
      });
    }
    setActiveSessionId(null);
  };

  const handleSelectSession = async (
    projectId: string,
    sessionId: string,
    projectOverride?: ProjectMeta,
    options?: { forceReload?: boolean },
  ) => {
    setActiveSection("chat");
    setActiveProjectId(projectId);
    setActiveSessionId(sessionId);
    setSessionTodos((prev) => ({ ...prev, [sessionId]: [] }));

    const project =
      projectOverride ?? projects.find((item) => item.id === projectId);
    if (project) {
      setCurrentProject({
        id: project.id,
        name: project.name,
        workdir: project.workdir,
        created_at: project.created_at,
        updated_at: "",
      });
    }

    if (conversations[sessionId] && !options?.forceReload) return;

    const sessionMeta = projectSessions[projectId]?.find(
      (session) => session.id === sessionId,
    );

    // GF-03 PR-2 — history → Message reducer extracted to
    // `src/session/loadConversationHistory.ts`.  This handler now
    // orchestrates the three slice writes; the helper is the sole
    // owner of the (legacy `messages[]` × canonical run-log) merge.
    const { conversation, recoveredTodos, titleState } =
      await loadConversationHistory({
        sessionId,
        projectId,
        sessionMeta,
        projectWorkdir: project?.workdir,
      });
    setConversations((prev) => ({ ...prev, [sessionId]: conversation }));
    setSessionTitleStates((prev) => ({ ...prev, [sessionId]: titleState }));
    setSessionTodos((prev) => ({ ...prev, [sessionId]: recoveredTodos }));
  };

  const handleNewChat = async (projectId: string): Promise<string | null> => {
    try {
      setActiveSection("chat");
      const session = await createSession(projectId, PLACEHOLDER_SESSION_TITLE);
      const sessions = await listProjectSessions(projectId);
      setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }));

      const project = projects.find((item) => item.id === projectId);
      if (project) {
        setCurrentProject({
          id: project.id,
          name: project.name,
          workdir: project.workdir,
          created_at: project.created_at,
          updated_at: "",
        });
      }

      setActiveProjectId(projectId);
      setActiveSessionId(session.id);
      setConversations((prev) => ({
        ...prev,
        [session.id]: {
          id: session.id,
          projectId,
          title: PLACEHOLDER_SESSION_TITLE,
          titleIcon: session.title_icon ?? null,
          titlePending: Boolean(session.title_pending),
          messages: [],
          updatedAt: new Date(),
        },
      }));
      setSessionTitleStates((prev) => ({
        ...prev,
        [session.id]: {
          stage: "placeholder",
          autoRenameCount: 0,
        },
      }));
      setSessionTodos((prev) => ({ ...prev, [session.id]: [] }));
      return session.id;
    } catch (err) {
      console.error("Failed to create session:", err);
      return null;
    }
  };

  /// 点击「新线程」—— 清空当前 session，显示「开始构建」界面
  const handleNewThread = () => {
    setActiveSessionId(null);
    setActiveProjectId(null);
    setCurrentProject(null);
  };

  /// Home 页面切换所选项目（不立即创建 session）
  const handleHomeProjectSelect = (projectId: string | null) => {
    setActiveProjectId(projectId);
    setActiveSessionId(null);
    if (!projectId) {
      setCurrentProject(null);
      return;
    }
    const project = projects.find((p) => p.id === projectId);
    if (project) {
      setCurrentProject({
        id: project.id,
        name: project.name,
        workdir: project.workdir,
        created_at: project.created_at,
        updated_at: "",
      });
    }
  };

  /// Home 页面发送第一条消息 → 先创建 session 再发送
  const handleSendMessage = (text: string) => {
    void sendMessage(text);
  };

  /// 在 Home 页面点击「添加新项目」→ 打开文件夹选择器 → 创建项目 → 选中并留在 Home
  const handlePickFolderAndCreateProject = async () => {
    try {
      const folderPath = await pickFolderDialog();
      if (!folderPath) return; // user cancelled

      const folderName =
        folderPath.split("/").filter(Boolean).pop() ?? "新项目";
      const newProject = await createProject(folderName, folderPath);

      // Refresh project list so the new project appears in the dropdown
      await loadProjects();

      // Select the new project on the Home screen — no session created yet
      handleHomeProjectSelect(newProject.id);
    } catch (err) {
      console.error("[handlePickFolderAndCreateProject] Failed:", err);
    }
  };

  const handleDeleteProject = async (projectId: string) => {
    try {
      await deleteProject(projectId);
      await loadProjects();
      if (activeProjectId === projectId) {
        setActiveProjectId(null);
        setActiveSessionId(null);
        setCurrentProject(null);
      }
    } catch (err) {
      console.error("Failed to delete project:", err);
    }
  };

  const handleRenameProject = async (projectId: string, newName: string) => {
    try {
      await renameProject(projectId, newName);
      await loadProjects();
      if (currentProject?.id === projectId) {
        setCurrentProject((prev) => (prev ? { ...prev, name: newName } : null));
      }
    } catch (err) {
      console.error("Failed to rename project:", err);
    }
  };

  const handleDeleteSession = async (projectId: string, sessionId: string) => {
    try {
      await deleteSession(sessionId);
      const sessions = await listProjectSessions(projectId);
      setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }));
      setConversations((prev) => {
        const next = { ...prev };
        delete next[sessionId];
        return next;
      });
      setSessionTitleStates((prev) => {
        const next = { ...prev };
        delete next[sessionId];
        return next;
      });
      setSessionTodos((prev) => {
        const next = { ...prev };
        delete next[sessionId];
        return next;
      });
      if (activeSessionId === sessionId) {
        setActiveSessionId(null);
      }
    } catch (err) {
      console.error("Failed to delete session:", err);
    }
  };

  // GF-03 PR-4 — `handleRenameSession` moved to `useSessionTitleStage`.

  const handleTogglePinSession = async (
    projectId: string,
    sessionId: string,
    pinned: boolean,
  ) => {
    try {
      await setSessionPinned(sessionId, !pinned);
      const sessions = await listProjectSessions(projectId);
      setProjectSessions((prev) => ({ ...prev, [projectId]: sessions }));
    } catch (err) {
      console.error("Failed to toggle session pin:", err);
    }
  };

  const handleUpdateSessionIdentity = async (
    sessionId: string,
    identity: SessionIdentityInput,
  ) => {
    const updated = await updateSessionIdentity(sessionId, identity);
    setProjectSessions((prev) => {
      const next: Record<string, SessionMeta[]> = {};
      for (const [projectId, sessions] of Object.entries(prev)) {
        next[projectId] = sessions.map((session) =>
          session.id === sessionId
            ? {
                ...session,
                soul_id: updated.soul_id ?? null,
                persona_id: updated.persona_id ?? null,
              }
            : session,
        );
      }
      return next;
    });
    await broadcastChange("cross:session-identity-changed", {
      sessionId,
      soulId: updated.soul_id ?? null,
      personaId: updated.persona_id ?? null,
    });
    return updated;
  };

  const handleCreateProject = async (name: string, workdir: string) => {
    try {
      const newProject = await createProject(name, workdir);
      await loadProjects();
      const session = await createSession(
        newProject.id,
        PLACEHOLDER_SESSION_TITLE,
      );

      setCurrentProject(newProject);
      setActiveProjectId(newProject.id);
      setActiveSessionId(session.id);
      setConversations((prev) => ({
        ...prev,
        [session.id]: {
          id: session.id,
          projectId: newProject.id,
          title: PLACEHOLDER_SESSION_TITLE,
          titleIcon: session.title_icon ?? null,
          titlePending: Boolean(session.title_pending),
          messages: [],
          updatedAt: new Date(),
        },
      }));
      setSessionTitleStates((prev) => ({
        ...prev,
        [session.id]: {
          stage: "placeholder",
          autoRenameCount: 0,
        },
      }));
      setSessionTodos((prev) => ({ ...prev, [session.id]: [] }));
      setIsCreateProjectOpen(false);
    } catch (err) {
      console.error("Failed to create project:", err);
    }
  };

  // GF-03 PR-6 — sendChatTurn / stopAgentStream / undo+redo /
  // resume-from-cursor + supporting refs (sessionLoadingRef etc.)
  // moved to `useSessionRuntime` (`src/session/SessionEffects.tsx`).
  // The hook is bound below once `handleSelectSession` and
  // `refreshProjectSessions` are in scope.

  const reloadSession = useCallback(
    (projectId: string, sessionId: string) =>
      handleSelectSession(projectId, sessionId, undefined, {
        forceReload: true,
      }),
    // handleSelectSession is captured from the surrounding closure;
    // identical to App.tsx pre-extraction.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  const {
    sendChatTurn: sendMessage,
    stopAgentStream,
    handleResumeFromCursor,
    handleConversationUndo,
    handleConversationRedo,
    conversationUndoStatus,
  } = useSessionRuntime({
    activeProjectId,
    activeSessionId,
    projects,
    conversations,
    sessionLoading,
    streamAbortHandles,
    input,
    permissionMode,
    selectedModel,
    currentProjectWorkdir: currentProject?.workdir,
    agentVoice,
    setInput,
    setSessionLoading,
    setSessionTodos,
    setStreamAbortHandles,
    setConversations,
    handleNewChat,
    maybeAutoRenameSession,
    refreshProjectSessions,
    reloadSession,
    extractTodosFromToolResult,
    activeMessagesLength: activeConv?.messages.length ?? 0,
  });


  const beginResize = (
    startX: number,
    separatorEl: HTMLDivElement,
    pointerId?: number,
  ) => {
    resizeRef.current = {
      startX,
      startWidth: leftPaneWidth,
    };

    const handleMove = (moveEvent: MouseEvent | globalThis.PointerEvent) => {
      if (!resizeRef.current) return;

      const delta = moveEvent.clientX - resizeRef.current.startX;
      const nextWidth = Math.min(
        maxLeftPaneWidth,
        Math.max(minLeftPaneWidth, resizeRef.current.startWidth + delta),
      );
      setLeftPaneWidth(nextWidth);
    };

    const handleUp = () => {
      resizeRef.current = null;
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleUp);
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
      if (typeof pointerId === "number") {
        try {
          separatorEl.releasePointerCapture(pointerId);
        } catch {
          // ignore release errors when pointer capture is already lost
        }
      }
    };

    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";
    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleUp);
  };

  const startResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const separatorEl = event.currentTarget;
    try {
      separatorEl.setPointerCapture(event.pointerId);
    } catch {
      // ignore capture failures and fall back to window listeners
    }
    beginResize(event.clientX, separatorEl, event.pointerId);
  };

  const toggleLeftPane = () => {
    setIsLeftPaneCollapsed((value) => {
      const next = !value;
      if (!next) {
        // Opening left pane → close right rail
        setIsRightRailOpen(false);
      }
      return next;
    });
  };

  const toggleRightRail = () => {
    setIsRightRailOpen((value) => {
      const next = !value;
      if (next) {
        // Opening right rail → close left pane
        setIsLeftPaneCollapsed(true);
      }
      return next;
    });
  };

  const handlePreviewFocusChange = (active: boolean) => {
    if (active) {
      wasPreviewFocusModeRef.current = true;
      leftPaneCollapsedBeforePreviewRef.current = isLeftPaneCollapsed;
      if (!isLeftPaneCollapsed) {
        setIsLeftPaneCollapsed(true);
      }
      return;
    }
    if (
      wasPreviewFocusModeRef.current &&
      !leftPaneCollapsedBeforePreviewRef.current
    ) {
      setIsLeftPaneCollapsed(false);
    }
    wasPreviewFocusModeRef.current = false;
  };

  const startWindowDrag = async (event: ReactMouseEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest('[data-window-no-drag="true"]')) return;
    try {
      await appWindow.startDragging();
    } catch {
      // Ignore drag failures on platforms that do not support the request in this context.
    }
  };

  // Adapter for AppShell's simplified event signature
  const handleWindowDragForAppShell = (_event: {
    clientX: number;
    clientY: number;
  }) => {
    // AppShell only provides clientX/clientY, but we need the full event
    // for target checking. Since AppShell already handles the window-drag
    // regions, we can safely call startDragging directly.
    void appWindow.startDragging().catch(() => {
      // Ignore drag failures on platforms that do not support the request in this context.
    });
  };

  // MIG-013 — `loadMainAppData` removed: its responsibilities
  // are now owned by `runBootSequence` (in
  // `src/boot/boot-orchestrator.ts`), which is invoked both on
  // first boot (via the `useEffect` at the top of this file) and
  // after onboarding completes (via `handleOnboardingComplete`).
  // Keeping a second code path would re-introduce the exact
  // "two boot orchestrators" anti-pattern MIG-013 was designed
  // to retire.

  const promptDownloadSenseVoiceAfterOnboarding = async () => {
    // 已经下载过就跳过；通过 stt_model_status 判断
    try {
      const { sttModelStatus, sttDownloadOpenflowModel, sttSaveSettings } =
        await import("@/lib/tauri");
      const status = await sttModelStatus();
      if (status.openflow_ready) return;
      // 之前显式拒绝过，1 周内不再问
      const SKIP_KEY = "if2ai.stt.openflow.declined_at";
      const declinedAt = Number(localStorage.getItem(SKIP_KEY) ?? 0);
      if (declinedAt > 0 && Date.now() - declinedAt < 7 * 86400_000) return;

      // 用 sonner 持久 toast：行动按钮 = 立即下载 / 稍后再说
      toast("启用本地中文语音输入？", {
        description:
          "推荐下载 SenseVoice 模型（约 230MB，离线、中文识别强）。也可稍后到「设置 → STT 语音输入」手动下载。",
        duration: Infinity,
        action: {
          label: "立即下载",
          onClick: () => {
            // 立即开始后台下载并显示进度 toast
            const id = toast.loading("SenseVoice 模型下载中…", {
              description: "约 230MB，根据网速 1-5 分钟",
              duration: Infinity,
            });
            void sttDownloadOpenflowModel({ preset: "quantized" })
              .then(async () => {
                toast.success("SenseVoice 已就绪", {
                  id,
                  description: "麦克按钮现在可以使用本地中文转写",
                  duration: 5000,
                });
                // 自动把 provider 切到 openflow
                try {
                  await sttSaveSettings({ provider: "openflow" });
                } catch {}
              })
              .catch((e) => {
                toast.error("下载失败", {
                  id,
                  description: String(e),
                  duration: 6000,
                });
              });
          },
        },
        cancel: {
          label: "稍后",
          onClick: () => {
            localStorage.setItem(SKIP_KEY, String(Date.now()));
          },
        },
      });
    } catch (e) {
      console.warn("[onboarding] STT prompt failed:", e);
    }
  };

  const handleOnboardingComplete = () => {
    // MIG-013 — onboarding_complete resets the store to the
    // splash state, then we re-run the canonical boot sequence
    // so `phase` properly transitions to `'main'` (via
    // `store.bootReady(...)`). Previously this called
    // `loadMainAppData()` directly, which populated project
    // state but never moved the store past `'splash'`.
    bootstrapStore.onboardingComplete();
    void runBootSequence(bootstrapStore, {
      awaitGatewayReady,
      getOnboardingState,
      ensureDefaultWorkdir,
      listProjects,
      listProjectSessions,
    });
    // Onboarding 完成后询问是否下载本地中文 STT 模型（SenseVoice 230MB）
    void promptDownloadSenseVoiceAfterOnboarding();
  };

  // GF-03 PR-1 — `latestUpdaterVersion`, `updaterBannerVisible`,
  // `handleDismissUpdaterBanner`, `handleRunUpdaterFromRail` are now
  // returned from `useUpdaterBanner()` at the top of the component.

  // MIG-013 — App.tsx renders via the canonical
  // `<AppShell>` container. Boot phase / boot surface decision
  // moved into `AppShell` (which reads the bootstrap store
  // internally); AppShell reads activation-gate state via the
  // existing `useBootRoute` hook inside its own body.
  return (
    <>
      <RuntimeProjectionWiring onActiveModelChanged={setSelectedModel} />
    <AppShell
      navbar={{
        activeSection,
        onSelectSection: setActiveSection,
        onOpenSettings: () => openSettingsWindow(),
        onRunUpdater: handleRunUpdaterFromRail,
        updaterStatus: appUpdaterState?.status,
        updaterLatestVersion: latestUpdaterVersion,
        updaterBannerVisible,
        onDismissUpdaterBanner: handleDismissUpdaterBanner,
        appIconSrc,
      }}
      onWindowDrag={handleWindowDragForAppShell}
      onOnboardingComplete={handleOnboardingComplete}
      chatSectionOverlay={
        <AgentVoiceIndicator
          isPlaying={agentVoice.isPlaying}
          pending={agentVoice.pending}
        />
      }
      router={{
        chat: (
          <ChatWorkspace
            projects={projects}
            projectSessions={projectSessions}
            activeProjectId={activeProjectId}
            activeSessionId={activeSessionId}
            currentProject={currentProject}
            branchLabel={branchLabel}
            isGitRepo={isGitRepo}
            onGitRepoChanged={() => setGitProbeNonce((n) => n + 1)}
            onBranchChange={setBranchLabel}
            onWorktreeProjectCreated={async (project) => {
              // 把新 worktree 注册的 project 拉进 ProjectMeta 列表里
              // → 侧栏立刻看见；同时把它选成活跃 project，再开一个新
              // session 让用户直接在 worktree 里继续聊。
              await loadProjects();
              try {
                setActiveProjectId(project.id);
              } catch {
                /* setActiveProjectId 可能依赖 store；调用失败时静默降级 */
              }
            }}
            activeTitle={activeTitle}
            activeMessages={activeMessages}
            activeSessionTotals={activeSessionTotals}
            input={input}
            isLoading={isActiveSessionLoading}
            loading={loading}
            onSelectProject={handleSelectProject}
            onSelectSession={handleSelectSession}
            onNewChat={handleNewChat}
            onNewThread={handleNewThread}
            onHomeProjectSelect={handleHomeProjectSelect}
            onPickFolderAndCreateProject={handlePickFolderAndCreateProject}
            onSendMessage={handleSendMessage}
            recentSessions={recentSessions}
            onDeleteProject={handleDeleteProject}
            onRenameProject={handleRenameProject}
            onDeleteSession={handleDeleteSession}
            onRenameSession={handleRenameSession}
            onTogglePinSession={handleTogglePinSession}
            onUpdateSessionIdentity={handleUpdateSessionIdentity}
            onOpenInFinder={openProjectInFinder}
            onCreatePermanentWorktree={createPermanentWorktree}
            onInputChange={setInput}
            onSubmit={sendMessage}
            onResumeFromCursor={handleResumeFromCursor}
            onStop={stopAgentStream}
            selectedModel={selectedModel}
            onModelChange={setSelectedModel}
            permissionMode={permissionMode}
            onPermissionModeChange={setPermissionMode}
            todos={todos}
            isRightRailOpen={isRightRailOpen}
            onToggleRightRail={toggleRightRail}
            onRightRailOpenChange={setIsRightRailOpen}
            leftPaneWidth={leftPaneWidth}
            isLeftPaneCollapsed={isLeftPaneCollapsed}
            onResizeStart={startResize}
            onToggleLeftPane={toggleLeftPane}
            onStartWindowDrag={startWindowDrag}
            onPreviewFocusChange={handlePreviewFocusChange}
            runningSessionIds={runningSessionIds}
            activeSessionMeta={activeSessionMeta}
            conversationUndoStatus={conversationUndoStatus}
            onConversationUndo={handleConversationUndo}
            onConversationRedo={handleConversationRedo}
          />
        ),
        memory: (
          <MemoryBrowser
            onStartWindowDrag={startWindowDrag}
            activeProjectId={activeProjectId}
            activeSessionId={activeSessionId}
          />
        ),
        sectionWorkspace: ({ section, onBackToChat }) => (
          <SectionWorkspace section={section} onBackToChat={onBackToChat} />
        ),
      }}
      overlays={
        <>
          <CreateProjectDialog
            isOpen={isCreateProjectOpen}
            onClose={() => setIsCreateProjectOpen(false)}
            onSubmit={handleCreateProject}
          />

          <PermissionOverlayHost />

          <TelemetryDrawer
            sessionId={activeSessionId}
            latestPromptDiagnostics={latestPromptDiagnosticsSnapshot}
            open={isTelemetryDrawerOpen}
            onClose={() => setTelemetryDrawerOpen(false)}
          />
        </>
      }
    />
    </>
  );
}

export default App;
