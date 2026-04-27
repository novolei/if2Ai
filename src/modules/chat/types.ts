import type {
  ContextBudgetUsage,
  ConversationUndoStatus,
  MemoryContextItem,
  PermissionMode,
  PromptDiagnosticsSummary,
  Project,
  ProjectMeta,
  SessionIdentityInput,
  SessionMeta,
  SessionTotals,
} from "@/lib/tauri";
import type { FinalRunReport } from "@/transport/contracts";
import type { RoutingInfo, ToolAttemptStatus, TurnCost } from "@/runtime-projection/types";
import type { Dispatch, SetStateAction } from "react";
import type { TodoItem } from "@/components/ui/TodoPanel";

/** A recent session entry shown in the Home screen suggestion list. */
export interface RecentSession {
  sessionId: string;
  projectId: string;
  projectName: string;
  title: string;
  titleIcon?: string | null;
  titlePending?: boolean;
  updatedAt: string;
}

export interface Message {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  timestamp: Date;
  thinking?: string;
  thinkingTime?: number;
  disableAnimation?: boolean;
  isStreaming?: boolean;
  // tool_call_update fields
  streamId?: string;
  toolCallId?: string;
  toolName?: string;
  toolArgs?: Record<string, unknown>;
  toolDurationMs?: number;
  isError?: boolean;
  toolStatus?: ToolAttemptStatus | "error";
  effectiveWorkdir?: string;
  policyDecision?: "allow" | "deny" | "prompt";
  /**
   * Memory-write scope reported by the `memory_store` tool result JSON
   * (one of `global` / `project` / `session`).  Only populated for
   * `memory_store` tool messages so the highlighted `MemoryWriteCard`
   * does not have to re-derive scope from `toolArgs`.
   */
  memoryScope?: "global" | "project" | "session";
  /**
   * Reason code returned by `MemoryPolicyEngine` (e.g.
   * `length_prompt_threshold`, `content_too_long`, `shadow_denied`).
   * Surfaces the precise rule that triggered a `deny` / `prompt`
   * decision in the UI tooltip, falling back to a derived label.
   */
  memoryReasonCode?: string;
  evidenceId?: string;
  requestId?: string;
  taskOutcome?: "completed" | "partial_success" | "failed";
  degradedReason?: string;
  resumeAvailable?: boolean;
  resumeCursor?: string;
  statusLabel?: string;
  statusKind?: "info" | "success" | "partial" | "failed";
  isRecovering?: boolean;
  /**
   * Memory items the agent recalled while generating this assistant response.
   * Populated from `StreamTokenPayload.memory_context` on `stream_complete`.
   * Drives the `MemoryChip` + `MemoryEvidencePanel` UI in `ChatMessage`.
   */
  memoryContext?: MemoryContextItem[];
  /**
   * Per-turn context-budget snapshot reported by the backend on completion.
   * Used by `ContextBar` (snapshot variant) and future evidence views.
   */
  contextBudgetUsage?: ContextBudgetUsage;
  /**
   * P1-7 / P2-11 — provider-billable token usage + USD cost for this turn.
   * Drives the Steward-style `[Zap] X 输入 · Y 输出 · $Z.ZZZZ` chip below
   * the assistant message.  Only populated for assistant messages.
   */
  turnCost?: TurnCost;
  /**
   * P1-8 — smart-routing decision summary for this turn (cheap vs primary
   * model + complexity bucket).  Drives the routing chip below the message.
   */
  routing?: RoutingInfo;
  /**
   * AWL-004 — canonical final work-loop report projected from runtime events.
   * Chat rendering must consume this object instead of inferring status from text.
   */
  finalRunReport?: FinalRunReport;
  /**
   * Prompt control-plane diagnostics summary for this assistant turn.
   * Carries lane / entry / reason metadata only, never raw prompt text.
   */
  promptDiagnostics?: PromptDiagnosticsSummary;
  /**
   * When this assistant message is the static result of an
   * `executeSlashCommand` invocation (e.g. `/diff`, `/branch`,
   * `/worktree`), the renderer uses a compact single-line card instead
   * of the regular markdown bubble.  Holds the original slash token
   * including the leading `/` so the card can echo it as a chip.
   */
  slashCommand?: string;
}

export interface Conversation {
  id: string;
  projectId: string;
  title: string;
  titleIcon?: string | null;
  titlePending?: boolean;
  messages: Message[];
  updatedAt: Date;
  /** P2-11 — running per-session totals (provider-billable). Hydrated from
   * `Session.session_totals` on load and refreshed from
   * `StreamTokenPayload.session_totals` on each `stream_complete`. */
  sessionTotals?: SessionTotals;
}

export type SessionTitleStage =
  | "placeholder"
  | "provisional"
  | "locked"
  | "manual";

export interface SessionTitleState {
  stage: SessionTitleStage;
  autoRenameCount: number;
}

export interface ChatWorkspaceProps {
  projects: ProjectMeta[];
  projectSessions: Record<string, SessionMeta[]>;
  activeProjectId: string | null;
  activeSessionId: string | null;
  currentProject: Project | null;
  branchLabel: string;
  /** Tri-state Git presence:
   *   - `true`  → cwd is inside a git working tree, all pickers active
   *   - `false` → cwd has no `.git`; pickers render disabled with a
   *     "无 Git 仓库" hint and offer an "init now" affordance
   *   - `null`  → still probing; UI assumes "active" optimistically so
   *     the disabled state doesn't flash on every project switch */
  isGitRepo?: boolean | null;
  /** Fired after `git init` (or any other event that may have changed
   *  the repo presence).  Parent should re-probe `gitIsRepo` and
   *  refresh `branchLabel`. */
  onGitRepoChanged?: () => void;
  /** Optional callback invoked by `BranchPicker` after a successful
   *  checkout / create.  Parent should refresh its `branchLabel` so the
   *  composer pill reflects reality. */
  onBranchChange?: (newBranch: string) => void;
  /** Optional callback fired after `gitCreateWorktreeProject` registers
   *  a worktree as a new project.  Parent should reload the project
   *  list and (optionally) switch the active session into it. */
  onWorktreeProjectCreated?: (project: {
    id: string;
    name: string;
    workdir: string;
  }) => void;
  activeTitle: string;
  activeMessages: Message[];
  /** P2-11 — running per-session totals (provider-billable). Forwarded to
   * `ChatUI` → `ContextBar` so the「本会话累计」row reflects the active session. */
  activeSessionTotals?: SessionTotals;
  input: string;
  isLoading: boolean;
  loading: boolean;
  onSelectProject: (id: string) => void;
  onSelectSession: (
    projectId: string,
    sessionId: string,
    projectOverride?: ProjectMeta,
    options?: { forceReload?: boolean },
  ) => Promise<void>;
  onNewChat: (projectId: string) => void;
  onNewThread: () => void;
  /** Select a project for the Home screen without creating a session. */
  onHomeProjectSelect: (projectId: string | null) => void;
  onPickFolderAndCreateProject: () => Promise<void>;
  /** Send the first message (called from HomeScreen composer). */
  onSendMessage: (text: string) => void;
  recentSessions: RecentSession[];
  onDeleteProject: (id: string) => void;
  onRenameProject: (id: string, newName: string) => void;
  onDeleteSession: (projectId: string, sessionId: string) => void;
  /** User-initiated session rename; sets stage to 'manual' to block future auto-renames. */
  onRenameSession: (sessionId: string, newTitle: string) => void;
  onTogglePinSession: (
    projectId: string,
    sessionId: string,
    pinned: boolean,
  ) => void | Promise<unknown>;
  onUpdateSessionIdentity: (
    sessionId: string,
    identity: SessionIdentityInput,
  ) => Promise<SessionMeta | void>;
  onOpenInFinder: (projectId: string) => void | Promise<unknown>;
  onCreatePermanentWorktree: (projectId: string) => void | Promise<unknown>;
  onInputChange: (value: string) => void;
  onSubmit: () => void;
  onResumeFromCursor?: (resumeCursor: string) => void;
  onStop?: () => void;
  selectedModel: string;
  onModelChange: Dispatch<SetStateAction<string>>;
  permissionMode: PermissionMode;
  onPermissionModeChange: Dispatch<SetStateAction<PermissionMode>>;
  todos: TodoItem[];
  isRightRailOpen: boolean;
  onToggleRightRail: () => void;
  onRightRailOpenChange: Dispatch<SetStateAction<boolean>>;
  leftPaneWidth: number;
  isLeftPaneCollapsed: boolean;
  onResizeStart: (event: React.PointerEvent<HTMLDivElement>) => void;
  onToggleLeftPane: () => void;
  onStartWindowDrag: (event: React.MouseEvent<HTMLElement>) => void;
  onPreviewFocusChange: (active: boolean) => void;
  runningSessionIds: string[];
  activeSessionMeta?: SessionMeta | null;
  /**
   * Most recent context-budget snapshot emitted by the streaming agent for
   * the active session. Drives the live `ContextBar` rendered above the
   * composer. `null` when no turn has completed yet for this session.
   */
  latestContextBudgetUsage?: ContextBudgetUsage | null;
  /** Backend undo stack availability for the active session. */
  conversationUndoStatus?: ConversationUndoStatus | null;
  /** Restore previous transcript snapshot (session_undo). */
  onConversationUndo?: () => void | Promise<void>;
  /** Re-apply last undone snapshot (session_redo). */
  onConversationRedo?: () => void | Promise<void>;
}
