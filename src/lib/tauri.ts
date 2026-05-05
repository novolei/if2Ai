//! Tauri IPC 封装层 (Phase M2.1 — thin bridge)
//!
//! 所有前端与 Tauri 后端的交互都通过这个模块，
//! App.tsx 不直接调用 @tauri-apps/api。
//!
//! Phase M2.1 重构：runtime / memory / activation / permission /
//! execution_mode 的 wire DTO 全部迁到 `@/transport/contracts`。
//! 本文件不再声明 transport contract，只保留：
//!   1. invoke / listen facade helpers
//!   2. typed IPC helper（按 backend command 名分组）
//! 旧的 DTO 名字通过 `export type { ... } from '@/transport/contracts'`
//! 重新导出，调用方零迁移成本。

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

import { MEMORY_AFTER_TURN_EVENT } from "@/transport/contracts";
import type {
  ActivationSnapshot,
  ExecutionModeDecision,
  MemoryAfterTurnPayload,
  McpWorkbenchActivityEntry,
  McpWorkbenchDiscovery,
  McpWorkbenchGetPromptRequest,
  McpWorkbenchGetPromptResult,
  McpWorkbenchPrompt,
  McpWorkbenchReadResourceRequest,
  McpWorkbenchReadResourceResult,
  McpWorkbenchServer,
  McpWorkbenchToolCallRequest,
  McpWorkbenchToolCallResult,
  PermissionMode,
} from "@/transport/contracts";

// Re-export invoke for App.tsx stopAgentStream
export { invoke };

// ─── Phase M2.1 — re-export transport contracts for backward compat ─
// New code should import directly from `@/transport/contracts` (or
// `@/transport`); these re-exports exist purely so existing call
// sites keep compiling unchanged during the M2 cut-over.
export type {
  ActivationFailureReason,
  ActivationLicense,
  ActivationSnapshot,
  ActivationStatus,
  ActivationStatusKind,
  ClassifierEvidence,
  ComplexityLevel,
  ContextBudgetUsage,
  ExecutionMode,
  ExecutionModeDecision,
  MemoryContextItem,
  MemoryEventPayload,
  McpWorkbenchActivityEntry,
  McpWorkbenchActivityStatus,
  McpWorkbenchDiscovery,
  McpWorkbenchGetPromptRequest,
  McpWorkbenchGetPromptResult,
  McpWorkbenchPrompt,
  McpWorkbenchPromptArgument,
  McpWorkbenchReadResourceRequest,
  McpWorkbenchReadResourceResult,
  McpWorkbenchResource,
  McpWorkbenchServer,
  McpWorkbenchTool,
  McpWorkbenchToolCallRequest,
  McpWorkbenchToolCallResult,
  PermissionMode,
  PermissionRequestPayload,
  PromptDiagnosticsSummary,
  ReasonCode,
  RiskLevel,
  RouteHint,
  ScenarioProfileHint,
  StreamTokenPayload,
} from "@/transport/contracts";
export { RUNTIME_EVENT_CHANNEL } from "@/transport/contracts";

// ─── DTOs below this line stay in tauri.ts for now (M2.1 scope) ────
// Slimmer M2.4 slice will progressively migrate session / project /
// pinned / browser / harness / hub DTOs into per-feature transport
// modules under `@/transport/<feature>.ts`.  The runtime-projection
// pipeline in M2.2/M2.3 is the priority cut-over for this round.

// `MemoryEventPayload` / `MemoryContextItem` / `StreamTokenPayload`
// / `PermissionRequestPayload` / `PermissionMode` / `ContextBudgetUsage`
// definitions moved to `@/transport/contracts` in Phase M2.1.
// They remain re-exported above for backward-compat with existing
// imports from `@/lib/tauri`.

/**
 * Phase M3-C closeout — subscribe to backend `memory_after_turn`
 * Tauri events emitted by `MemoryCoordinator::after_turn` at every
 * turn end.  Replaces the M3-B `listenMemoryWriteDecision`.  Fires
 * once per turn end including when the batch is empty (the
 * envelope is the "no candidates this turn" signal).
 */
export async function listenMemoryAfterTurn(
  handler: (payload: MemoryAfterTurnPayload) => void,
): Promise<UnlistenFn> {
  return await listen<MemoryAfterTurnPayload>(
    MEMORY_AFTER_TURN_EVENT,
    (event) => {
      handler(event.payload);
    },
  );
}

/**
 * 助手消息响应
 */
export interface AgentTurnResponse {
  /** 生成的文本消息 */
  message: string;
  /** 会话 ID */
  session_id: string;
  /** 思考内容（如果有） */
  thinking?: string;
  /** 工具调用（如果有） */
  tool_calls?: ToolCall[];
  /** Token 使用量（如果有） */
  tokens?: TokenUsage;
}

/**
 * 工具调用
 */
export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

/**
 * Token 使用量
 */
export interface TokenUsage {
  input: number;
  output: number;
  cost: number;
}

/**
 * 会话元信息
 */
export interface SessionMeta {
  id: string;
  title: string;
  title_icon?: string | null;
  title_pending?: boolean;
  created_at: string;
  updated_at: string;
  pinned: boolean;
  message_count: number;
  soul_id?: string | null;
  persona_id?: string | null;
  /** Active skill names for prompt + low-trust tool attenuation */
  active_skill_ids?: string[];
}

/**
 * 项目信息
 */
export interface Project {
  id: string;
  name: string;
  workdir: string;
  created_at: string;
  updated_at: string;
}

/**
 * 项目元信息（用于列表显示）
 */
export interface ProjectMeta {
  id: string;
  name: string;
  workdir: string;
  created_at: string;
  session_count: number;
}

export interface DirectoryEntryPreview {
  name: string;
  path: string;
  kind: "folder" | "file";
  modified_ms?: number | null;
}

export interface FilePreviewPayload {
  name: string;
  path: string;
  kind: "markdown" | "code" | "image" | "pdf" | "video" | "html";
  mime_type?: string | null;
  content?: string | null;
  data_base64?: string | null;
  editable: boolean;
  language?: string | null;
}

/**
 * 运行一次 Agent 对话轮次
 *
 * @param sessionId - 会话 ID
 * @param userMessage - 用户消息
 * @returns Agent 响应
 */
export async function runAgentTurn(
  sessionId: string,
  userMessage: string,
  permissionMode?: PermissionMode,
): Promise<AgentTurnResponse> {
  return await invoke<AgentTurnResponse>("run_agent_turn", {
    sessionId,
    userMessage,
    permissionMode,
  });
}

/**
 * 开始流式 Agent 对话轮次
 *
 * @param sessionId - 会话 ID
 * @param userMessage - 用户消息
 * @returns 流 ID，用于关联事件
 */
export async function startAgentStream(
  sessionId: string,
  userMessage: string,
  permissionMode?: PermissionMode,
  selectedModel?: string,
): Promise<string> {
  const [providerId, modelId] = selectedModel?.split("/") ?? [];
  const args: Record<string, unknown> = {
    sessionId,
    userMessage,
    permissionMode,
  };
  if (providerId && modelId) {
    args.providerId = providerId;
    args.modelId = modelId;
  }
  return await invoke<string>("start_agent_stream", args);
}

/**
 * 响应后端权限请求（allow / deny）
 */
export async function respondPermission(
  sessionId: string,
  decision: "allow" | "deny",
  options?: { toolName?: string; scope?: "once" | "session" },
): Promise<void> {
  return await invoke<void>("respond_permission", {
    sessionId,
    decision,
    toolName: options?.toolName,
    scope: options?.scope,
  });
}

/**
 * 列出所有会话
 *
 * @returns 会话列表
 */
export async function listSessions(): Promise<SessionMeta[]> {
  return await invoke<SessionMeta[]>("list_sessions");
}

/**
 * 删除指定会话
 *
 * @param id - 要删除的会话 ID
 */
export async function deleteSession(id: string): Promise<void> {
  return await invoke<void>("delete_session", { id });
}

/**
 * 设置会话置顶状态
 *
 * @param id - 会话 ID
 * @param pinned - 是否置顶
 */
export async function setSessionPinned(
  id: string,
  pinned: boolean,
): Promise<void> {
  return await invoke<void>("set_session_pinned", { id, pinned });
}

/**
 * 切换 per-session memory 开关 (Phase 8A.4 / v2 §Sprint 1 / T-A4)
 *
 * `enabled = false` 会让该会话的 rolling summary / compile / fact 提取
 * 在 ticker 中被 short-circuit, 同时打 `memory_disabled_since = now`;
 * `enabled = true` 复位并打 `memory_reenabled_at = now`. 编译流水线
 * (8B+) 会按这两个时间戳过滤掉静默窗口内的摘要.
 *
 * @param sessionId - 会话 ID
 * @param enabled - true=开启 memory, false=关闭
 */
export async function memorySessionSetEnabled(
  sessionId: string,
  enabled: boolean,
): Promise<void> {
  await invoke<void>("memory_session_set_enabled", { id: sessionId, enabled });
}

/**
 * Wire-shape mirror of the backend `PinnedItemDto`
 * (`src-tauri/src/commands/pinned.rs`). Phase 8A.10 / T-F3.
 *
 * `scope` is `'project' | 'global'`; `createdByKind` is `'user'` for
 * pins added from the PinnedMemoryEditor and `'tool'` for pins added
 * by the agent via the `pin_memory` tool, in which case `toolName` and
 * `sessionId` carry the originating call.
 */
export interface PinnedItemDto {
  id: string;
  content: string;
  scope: "project" | "global";
  projectId?: string;
  createdAt: string;
  createdByKind: "user" | "tool";
  toolName?: string;
  sessionId?: string;
}

/**
 * 列出钉住的记忆 (Phase 8A.10 / T-F3)
 *
 * @param scope - `'project'` 仅当前项目, `'global'` 全局, `'both'`
 *   返回 system-prompt 注入 (8A.11) 实际看到的合并集合.
 * @param projectId - 当前 project_id; `scope='global'` 时忽略.
 */
export async function pinnedGet(
  scope: "project" | "global" | "both",
  projectId?: string,
): Promise<PinnedItemDto[]> {
  return await invoke<PinnedItemDto[]>("pinned_get", { scope, projectId });
}

/**
 * 用户从 PinnedMemoryEditor UI 新增一条 pin (Phase 8A.10 / T-F3).
 *
 * 后端会自动经过 `ThreatScanner.scan_and_redact` PII 脱敏, 触及
 * `MAX_PIN_CONTENT_CHARS` (500) 或 `MAX_PINS_PER_SCOPE` (50) 时返回
 * 错误 (在前端表现为 `Error('PinnedLimitExceeded(50)')` 之类的字符串).
 */
export async function pinnedAdd(
  content: string,
  scope: "project" | "global",
  projectId?: string,
): Promise<PinnedItemDto> {
  return await invoke<PinnedItemDto>("pinned_add", {
    content,
    scope,
    projectId,
  });
}

/**
 * 删除一条 pin (幂等 - 已不存在时返回 `false`, 不报错).
 */
export async function pinnedDelete(id: string): Promise<boolean> {
  return await invoke<boolean>("pinned_delete", { id });
}

/**
 * 重排 pin 顺序 (拖拽后调用).  按数组顺序重新打 `created_at`,
 * 不在 store 中的 id 静默跳过.
 */
export async function pinnedReorder(ids: string[]): Promise<void> {
  await invoke<void>("pinned_reorder", { ids });
}

/**
 * 创建新项目
 *
 * @param name - 项目名称
 * @param workdir - 工作目录路径
 * @returns 创建的项目信息
 */
export async function createProject(
  name: string,
  workdir: string,
): Promise<Project> {
  return await invoke<Project>("create_project", { name, workdir });
}

/**
 * 列出所有项目
 *
 * @returns 项目列表
 */
export async function listProjects(): Promise<ProjectMeta[]> {
  return await invoke<ProjectMeta[]>("list_projects");
}

/**
 * 获取指定项目
 *
 * @param id - 项目 ID
 * @returns 项目信息
 */
export async function getProject(id: string): Promise<Project> {
  return await invoke<Project>("get_project", { id });
}

/**
 * 重命名项目
 *
 * @param id - 项目 ID
 * @param newName - 新名称
 * @returns 更新后的项目信息
 */
export async function renameProject(
  id: string,
  newName: string,
): Promise<Project> {
  return await invoke<Project>("rename_project", { id, newName });
}

/**
 * 删除指定项目
 *
 * @param id - 要删除的项目 ID
 */
export async function deleteProject(id: string): Promise<void> {
  return await invoke<void>("delete_project", { id });
}

/**
 * 在系统文件管理器中打开指定项目
 *
 * @param id - 项目 ID
 */
export async function openProjectInFinder(id: string): Promise<void> {
  return await invoke<void>("open_project_in_finder", { id });
}

export async function openDirectoryPath(path: string): Promise<void> {
  return await invoke<void>("open_directory_path", { path });
}

/**
 * 打开 macOS 文件夹选择器，返回用户选中的路径。
 * 用户取消时返回 null。
 */
export async function pickFolderDialog(): Promise<string | null> {
  return await invoke<string | null>("pick_folder_dialog");
}

/**
 * 确保 ~/Documents/workaround 目录存在，并注册为默认 "Playground" 项目。
 * 返回 [workdir_path, project_id]。
 */
export async function ensureDefaultWorkdir(): Promise<[string, string]> {
  return await invoke<[string, string]>("ensure_default_workdir");
}

export async function listDirectoryPreview(
  path: string,
  limit = 32,
): Promise<DirectoryEntryPreview[]> {
  return await invoke<DirectoryEntryPreview[]>("list_directory_preview", {
    path,
    limit,
  });
}

/**
 * Read a file for inline preview.
 *
 * `maxBytes` is intentionally optional and **not defaulted** on the
 * frontend.  Leaving it `undefined` lets the backend pick a sensible,
 * **type-aware** default:
 *   - binary (image / video / pdf): 10 MiB   (clamp 32 KiB ~ 32 MiB)
 *   - text   (markdown / code / html):       128 KiB (clamp 1 KiB ~ 512 KiB)
 *
 * Hard-coding a 128 KiB cap on the frontend (the previous behaviour)
 * killed PNG / JPEG / MP4 previews even at 200 KiB, raising
 * `"file too large for inline preview"` from the backend and surfacing
 * as the misleading "这个文件暂时不能在面板内预览。" toast in the rail.
 *
 * Pass `maxBytes` explicitly only for call sites that need a stricter
 * cap (e.g. a settings preview where a multi-MB image would blow the
 * UI out).
 */
export async function readFilePreview(
  path: string,
  maxBytes?: number,
): Promise<FilePreviewPayload> {
  return await invoke<FilePreviewPayload>("read_file_preview", {
    path,
    maxBytes: maxBytes ?? null,
  });
}

export async function writeFileContents(
  path: string,
  content: string,
): Promise<void> {
  return await invoke<void>("write_file_contents", { path, content });
}

/**
 * 为项目创建永久工作树
 *
 * @param id - 项目 ID
 * @returns 创建后的工作树路径
 */
export async function createPermanentWorktree(id: string): Promise<string> {
  return await invoke<string>("create_permanent_worktree", { id });
}

/**
 * 在指定项目中创建新会话
 *
 * @param projectId - 项目 ID（空字符串表示无关联项目）
 * @param title - 会话标题
 * @returns 创建的会话信息
 */
export async function createSession(
  projectId: string,
  title: string,
  identity?: SessionIdentityInput | null,
): Promise<SessionMeta> {
  return await invoke<SessionMeta>("create_session", {
    projectId,
    title,
    identity,
  });
}

/**
 * 更新会话标题
 */
export async function renameSession(
  id: string,
  title: string,
): Promise<SessionMeta> {
  return await invoke<SessionMeta>("rename_session", { id, title });
}

export async function generateSessionTitle(
  id: string,
  titleHint?: string | null,
): Promise<SessionMeta> {
  return await invoke<SessionMeta>("generate_session_title", {
    id,
    titleHint: titleHint ?? null,
  });
}

export async function setSessionIdentity(
  id: string,
  identity: SessionIdentityInput,
): Promise<SessionMeta> {
  return await invoke<SessionMeta>("set_session_identity", { id, identity });
}

export async function setSessionActiveSkillIds(
  id: string,
  activeSkillIds: string[],
): Promise<Session> {
  return await invoke<Session>("set_session_active_skill_ids", {
    id,
    active_skill_ids: activeSkillIds,
  });
}

/**
 * 列出指定项目中的所有会话
 *
 * @param projectId - 项目 ID
 * @returns 会话列表
 */
export async function listProjectSessions(
  projectId: string,
): Promise<SessionMeta[]> {
  return await invoke<SessionMeta[]>("list_project_sessions", { projectId });
}

/**
 * 打开设置窗口
 */
export async function openSettingsWindow(): Promise<void> {
  return await invoke<void>("open_settings_window");
}

/**
 * 关闭设置窗口
 */
export async function closeSettingsWindow(): Promise<void> {
  return await invoke<void>("close_settings_window");
}

export interface ChatPrefillPayload {
  prompt: string;
}

export async function focusMainWindowAndPrefillPrompt(
  prompt: string,
): Promise<void> {
  return await invoke<void>("focus_main_window_and_prefill_prompt", { prompt });
}

export async function listenToChatPrefill(
  callback: (payload: ChatPrefillPayload) => void,
): Promise<UnlistenFn> {
  return await listen<ChatPrefillPayload>("if2ai-chat-prefill", (event) => {
    callback(event.payload);
  });
}

/**
 * 获取会话的完整信息（包括消息历史）
 *
 * @param id - 会话 ID
 * @returns 完整的会话信息
 */
export async function getSession(id: string): Promise<Session> {
  return await invoke<Session>("get_session", { id });
}

/** In-memory transcript undo/redo availability (see `session_undo` / `session_redo`). */
export interface ConversationUndoStatus {
  canUndo: boolean;
  canRedo: boolean;
}

export async function sessionUndoStatus(
  id: string,
): Promise<ConversationUndoStatus> {
  return await invoke<ConversationUndoStatus>("session_undo_status", { id });
}

/** Restore one transcript checkpoint for the session. */
export async function sessionUndo(id: string): Promise<Session> {
  return await invoke<Session>("session_undo", { id });
}

/** Re-apply the last undone transcript checkpoint. */
export async function sessionRedo(id: string): Promise<Session> {
  return await invoke<Session>("session_redo", { id });
}

/** Drain P2-12 job-monitor diagnostic lines for a session. */
export async function drainJobMonitorLines(id: string): Promise<string[]> {
  return await invoke<string[]>("drain_job_monitor_lines", { id });
}

/**
 * 会话信息（包含消息）
 */
export interface Session {
  id: string;
  project_id: string;
  title: string;
  title_icon?: string | null;
  title_pending?: boolean;
  title_request_id?: string | null;
  title_manually_renamed?: boolean;
  messages: ConversationMessage[];
  created_at: string;
  updated_at: string;
  token_count: number;
  soul_id?: string | null;
  persona_id?: string | null;
  active_skill_ids?: string[];
  /** P2-11 — per-session running totals (provider-billable). */
  session_totals?: SessionTotals;
}

/** P2-11 — mirrors Rust `SessionUsageTotals`. */
export interface SessionTotals {
  input_tokens: number;
  output_tokens: number;
  cache_creation_input_tokens: number;
  cache_read_input_tokens: number;
  cost_usd: number;
  turns: number;
}

export interface SessionIdentityInput {
  soul_id?: string | null;
  persona_id?: string | null;
}

/**
 * 对话消息
 */
export interface ConversationMessage {
  role: "system" | "user" | "assistant" | "tool";
  blocks: ContentBlock[];
  usage?: TokenUsage;
  thinking?: string;
  task_outcome?: "completed" | "partial_success" | "failed";
  degraded_reason?: string;
  resume_available?: boolean;
  resume_cursor?: string;
  request_id?: string;
}

/**
 * 内容块
 */
export interface ContentBlock {
  type: "text" | "tool_use" | "tool_result";
  text?: string;
  tool_use_id?: string;
  tool_name?: string;
  input?: string;
  output?: string;
  tool_use_block?: {
    id: string;
    name: string;
    input: Record<string, unknown>;
  };
}

/**
 * Token 使用量
 */
export interface TokenUsage {
  input_tokens: number;
  output_tokens: number;
  cache_creation_input_tokens?: number;
  cache_read_input_tokens?: number;
}

/**
 * 工具定义 (OpenAI 格式)
 */
export interface ToolDefinition {
  name: string;
  description: string;
  input_schema: object;
}

/**
 * 工具调用结果
 */
export interface ToolCallResult {
  success: boolean;
  output?: string;
  error?: string;
}

/**
 * 直接执行工具（前端调用）
 *
 * @param name - 工具名称
 * @param args - 工具参数（JSON对象）
 * @returns 工具执行结果
 */
export async function executeTool(
  name: string,
  args: Record<string, unknown>,
  permissionMode?: PermissionMode,
  sessionId?: string,
): Promise<ToolCallResult> {
  try {
    const result = await invoke<string>("execute_tool", {
      name,
      args: JSON.stringify(args),
      permissionMode,
      sessionId,
    });
    return { success: true, output: result };
  } catch (e) {
    return { success: false, error: String(e) };
  }
}

/**
 * 列出所有可用工具
 *
 * @returns 工具定义列表
 */
export async function listTools(): Promise<ToolDefinition[]> {
  return invoke<ToolDefinition[]>("list_tools");
}

/**
 * 获取工具定义（OpenAI function calling 格式）
 *
 * @param allowed - 可选的允许工具名称列表
 * @returns 工具定义列表
 */
export async function getToolDefinitions(
  allowed?: string[],
): Promise<object[]> {
  return invoke<object[]>("get_tool_definitions", { allowed });
}

/**
 * 工具集定义
 */
export interface ToolSet {
  name: string;
  description: string;
  tools: string[];
  enabled: boolean;
}

/**
 * 列出所有工具集
 *
 * @returns 工具集列表
 */
export async function listToolsets(): Promise<ToolSet[]> {
  return invoke<ToolSet[]>("list_toolsets");
}

/**
 * Suggest slash commands based on user input prefix.
 *
 * @param input - The slash command prefix typed by user
 * @param limit - Maximum number of suggestions
 * @returns List of full command strings
 */
export async function suggestSlashCommands(
  input: string,
  limit = 8,
): Promise<string[]> {
  return invoke<string[]>("suggest_slash_commands", { input, limit });
}

/**
 * Execute a slash command for the given session.
 *
 * @param input - The full slash command string
 * @param sessionId - Current session ID
 * @returns Result message from command execution
 */
export async function executeSlashCommand(
  input: string,
  sessionId: string,
): Promise<string> {
  return invoke<string>("execute_slash_command", { input, sessionId });
}

/**
 * Resolve a `/skill-name [instruction]` slash input into a full skill invocation
 * message (containing the SKILL.md content) that can be sent directly to the agent.
 * Returns null if the input does not match any installed skill.
 */
export async function resolveSkillSlash(
  input: string,
  cwd?: string,
): Promise<string | null> {
  return invoke<string | null>("resolve_skill_slash", {
    input,
    cwd: cwd ?? null,
  });
}

export interface SkillInfo {
  name: string;
  description: string;
  path: string;
  source: "workspace" | "user" | "builtin" | "remote-quarantine";
  review_status:
    | "draft"
    | "quarantine"
    | "review_passed"
    | "active"
    | "disabled";
  status: "draft" | "quarantine" | "review_passed" | "active" | "disabled";
  enabled: boolean;
  read_only: boolean;
  shadowed_by?: string;
}

export async function listSkills(cwd?: string): Promise<SkillInfo[]> {
  return invoke<SkillInfo[]>("list_skills", { cwd });
}

export async function setSkillEnabled(
  skillPath: string,
  enabled: boolean,
  sessionId = "__settings__",
): Promise<string> {
  const action = enabled ? "enable-path" : "disable-path";
  return executeSlashCommand(`/skills ${action} ${skillPath}`, sessionId);
}

export async function createSkillDraft(
  skillName: string,
  sessionId = "__settings__",
): Promise<string> {
  return executeSlashCommand(`/skills create ${skillName}`, sessionId);
}

export async function reviewSkillDraft(
  skillPath: string,
  sessionId = "__settings__",
): Promise<string> {
  return executeSlashCommand(`/skills review-path ${skillPath}`, sessionId);
}

export async function approveSkillProposal(
  skillPath: string,
  sessionId = "__settings__",
): Promise<string> {
  return executeSlashCommand(`/skills approve-path ${skillPath}`, sessionId);
}

export async function rollbackSkillProposal(
  skillPath: string,
  sessionId = "__settings__",
): Promise<string> {
  return executeSlashCommand(`/skills rollback-path ${skillPath}`, sessionId);
}

export interface SkillDistributionRequest {
  skillName: string;
  url: string;
  channel: "stable" | "canary";
  checksum: string;
  signature: string;
}

export interface SkillsMarketAuditItem {
  skill: string;
  repo: string;
  gen: string;
  socketAlerts: string;
  snykRisk: string;
}

export async function fetchSkillsMarketAudits(): Promise<
  SkillsMarketAuditItem[]
> {
  return invoke<SkillsMarketAuditItem[]>("fetch_skills_market_audits");
}

export interface HubInstallResult {
  success: boolean;
  message: string;
  skill_name?: string;
}

/** One skill result from hub_browse / hub_search. */
export interface HubSkillResult {
  name: string;
  description: string;
  source: string;
  identifier: string;
  trust_level: string;
  tags: string[];
}

export interface HubCommandResult {
  success: boolean;
  message: string;
  data?: HubSkillResult[];
}

/** Install a skill from any hub source via the Rust pipeline. */
export async function hubInstall(
  sourceId: string,
  identifier: string,
  skillsDir?: string,
): Promise<HubInstallResult> {
  return invoke<HubInstallResult>("hub_install", {
    sourceId,
    identifier,
    skillsDir: skillsDir ?? null,
  });
}

/** Browse skills from all hub sources (empty query returns defaults per source). */
export async function hubBrowse(
  sourceFilter?: string,
  limit?: number,
): Promise<HubCommandResult> {
  const result = await invoke<{
    success: boolean;
    message: string;
    data?: unknown;
  }>("hub_browse", { sourceFilter: sourceFilter ?? null, limit: limit ?? 50 });
  return {
    success: result.success,
    message: result.message,
    data: Array.isArray(result.data) ? (result.data as HubSkillResult[]) : [],
  };
}

/** Search skills across all hub sources. */
export async function hubSearch(
  query: string,
  sourceFilter?: string,
  limit?: number,
): Promise<HubCommandResult> {
  const result = await invoke<{
    success: boolean;
    message: string;
    data?: unknown;
  }>("hub_search", {
    query,
    sourceFilter: sourceFilter ?? null,
    limit: limit ?? 30,
  });
  return {
    success: result.success,
    message: result.message,
    data: Array.isArray(result.data) ? (result.data as HubSkillResult[]) : [],
  };
}

async function sha256Hex(text: string): Promise<string> {
  const data = new TextEncoder().encode(text);
  const hash = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export async function installSkillFromDistribution(
  request: SkillDistributionRequest,
  sessionId = "__settings__",
): Promise<string> {
  let effectiveSessionId = sessionId;
  if (effectiveSessionId === "__settings__") {
    const projects = await listProjects();
    if (projects.length === 0) {
      throw new Error("No project available. Please create a project first.");
    }
    const tempSession = await createSession(
      projects[0].id,
      "Skills Market Install",
    );
    effectiveSessionId = tempSession.id;
  }
  // Fail-closed: signature and checksum are required for remote installs
  // When empty, the skill will be placed in quarantine for review
  const requiresQuarantine =
    !request.signature.trim() || !request.checksum.trim();
  if (requiresQuarantine) {
    console.warn(
      "[Skills Market] No signature/checksum provided, will install to quarantine for review",
    );
  }
  const fetched = await executeTool(
    "web_fetch",
    { url: request.url },
    undefined,
    effectiveSessionId,
  );
  if (!fetched.success || !fetched.output) {
    throw new Error(fetched.error ?? "download failed");
  }
  const safeName = request.skillName.replace(/[^a-zA-Z0-9_-]/g, "-");
  const basePath = `.if2ai/skills-quarantine/${safeName}`;
  const writeSkill = await executeTool(
    "file_write",
    {
      path: `${basePath}/SKILL.md`,
      content: fetched.output,
    },
    undefined,
    effectiveSessionId,
  );
  if (!writeSkill.success) {
    throw new Error(writeSkill.error ?? "write SKILL.md failed");
  }
  const actualChecksum = await sha256Hex(fetched.output);
  // Only verify checksum if one was provided
  if (
    request.checksum.trim() &&
    request.checksum.toLowerCase() !== actualChecksum.toLowerCase()
  ) {
    throw new Error(
      `checksum mismatch: expected ${request.checksum}, actual ${actualChecksum}`,
    );
  }
  const writeManifest = await executeTool(
    "file_write",
    {
      path: `${basePath}/skill.json`,
      content: JSON.stringify(
        {
          id: safeName,
          version: "0.1.0",
          apiVersion: "v1",
          minAppVersion: "0.1.0",
          capabilities: ["custom"],
          distribution: {
            channel: request.channel,
            checksum: actualChecksum,
            signature: request.signature,
          },
          review: {
            status: "quarantine",
            riskLevel: "high",
            lastReviewedAt: "",
          },
        },
        null,
        2,
      ),
    },
    undefined,
    effectiveSessionId,
  );
  if (!writeManifest.success) {
    throw new Error(writeManifest.error ?? "write skill.json failed");
  }
  return `downloaded to quarantine: ${basePath}`;
}

// ─── Web Search Configuration ──────────────────────────────────────────────

/** A configured web search provider entry returned by the backend. */
export interface WebSearchProviderEntry {
  id: string;
  name: string;
  /** Redacted key preview shown in the UI, e.g. "tvly-abc…xyz". */
  key_preview: string | null;
  base_url: string | null;
  enabled: boolean;
}

/** Return all configured providers (keys are redacted). */
export async function getWebSearchConfig(): Promise<WebSearchProviderEntry[]> {
  return invoke<WebSearchProviderEntry[]>("get_web_search_config");
}

/** Add or update a provider.  `api_key` and `base_url` are optional. */
export async function upsertWebSearchProvider(
  id: string,
  name: string,
  apiKey?: string,
  baseUrl?: string,
  enabled?: boolean,
): Promise<WebSearchProviderEntry[]> {
  return invoke<WebSearchProviderEntry[]>("upsert_web_search_provider", {
    provider: {
      id,
      name,
      api_key: apiKey ?? null,
      base_url: baseUrl ?? null,
      enabled: enabled ?? true,
    },
  });
}

/** Remove a provider by id. */
export async function removeWebSearchProvider(
  id: string,
): Promise<WebSearchProviderEntry[]> {
  return invoke<WebSearchProviderEntry[]>("remove_web_search_provider", { id });
}

/** Reorder providers by supplying the new ordered list of ids. */
export async function reorderWebSearchProviders(
  orderedIds: string[],
): Promise<WebSearchProviderEntry[]> {
  return invoke<WebSearchProviderEntry[]>("reorder_web_search_providers", {
    orderedIds,
  });
}

/** Validate an API key / base URL for the given provider. Returns a success message or throws. */
export async function validateWebSearchKey(
  providerId: string,
  apiKey?: string,
  baseUrl?: string,
): Promise<string> {
  return invoke<string>("validate_web_search_key", {
    providerId,
    apiKey: apiKey ?? null,
    baseUrl: baseUrl ?? null,
  });
}

// ─── Memory Settings Configuration ─────────────────────────────────────────

/**
 * Memory recall mode — `lexical` keeps the legacy SQL search; `hybrid`
 * enables vector + FTS + episodic fusion.
 */
export type MemoryRecallMode = "lexical" | "hybrid";

/**
 * Memory write policy enforce mode — `shadow` audits decisions without
 * blocking, `enforce` rejects denied writes.
 */
export type MemoryPolicyEnforceMode = "shadow" | "enforce";

/**
 * Promotion thresholds — gating values for when the background scanner
 * recommends `session→project` and `project→global` upgrades.
 *
 * Mirrors the Rust `PromotionThresholds` struct (camelCase serde).
 */
export interface PromotionThresholds {
  sessionToProjectAccess: number;
  sessionToProjectImportance: number;
  projectToGlobalAccess: number;
  projectToGlobalImportance: number;
}

/** Backend-supplied default thresholds (kept in sync with `PromotionThresholds::default`). */
export const DEFAULT_PROMOTION_THRESHOLDS: PromotionThresholds = {
  sessionToProjectAccess: 3,
  sessionToProjectImportance: 0.55,
  projectToGlobalAccess: 8,
  projectToGlobalImportance: 0.7,
};

/** Memory configuration returned by the backend. */
export interface MemoryConfig {
  total_tokens: number;
  system_pct: number;
  episodic_pct: number;
  semantic_pct: number;
  working_pct: number;
  trajectory_count: number;
  /** Memory Control Plane V1 master kill-switch. */
  control_plane_v1_enabled: boolean;
  recall_mode: MemoryRecallMode;
  policy_enforce_mode: MemoryPolicyEnforceMode;
  promotion: PromotionThresholds;
  /**
   * MEM-MOD-PD0 — IANA timezone name (e.g. "Asia/Shanghai") used by
   * the logical-day pipeline. `null` means "no override" → falls back
   * to UTC.
   */
  timezone?: string | null;
  /**
   * MEM-MOD-PD0 — Logical-day cutoff hour in LOCAL `timezone` (0-23).
   * Default `4` keeps "I worked till 03:00 last night" rolled into
   * yesterday's daily aggregations.
   */
  logical_day_cutoff_hour: number;
  /**
   * MEM-MOD-PD0 (B-fix) — Read-only IANA name detected from the host
   * OS. Used by the "时间感知" UI to show what an empty `timezone`
   * actually resolves to. `null`/undefined when the OS probe failed.
   */
  detected_os_timezone?: string | null;
  /**
   * Whether conflict detection runs when storing a new memory.
   * When true, checks incoming facts against existing memories for
   * contradictions before persisting.  Default `true`.
   */
  conflict_detection_enabled: boolean;
}

/** Configuration input to persist. */
export interface MemoryConfigInput {
  total_tokens: number;
  system_pct: number;
  episodic_pct: number;
  semantic_pct: number;
  working_pct: number;
  control_plane_v1_enabled?: boolean;
  recall_mode?: MemoryRecallMode;
  policy_enforce_mode?: MemoryPolicyEnforceMode;
  promotion?: PromotionThresholds;
  /** MEM-MOD-PD0 — see {@link MemoryConfig.timezone}. */
  timezone?: string | null;
  /** MEM-MOD-PD0 — see {@link MemoryConfig.logical_day_cutoff_hour}. */
  logical_day_cutoff_hour?: number;
}

export type PromptScenarioProfile =
  | "chat"
  | "coding"
  | "research"
  | "planning"
  | "review";

export interface PromptControlSettings {
  default_scenario_profile: PromptScenarioProfile | null;
  default_soul_id: string | null;
  default_persona_id: string | null;
  /** Optional global agent name; persists across persona switches. */
  agent_name: string | null;
  /** Optional name the user wants the agent to call them. */
  user_name: string | null;
  prompt_diagnostics_enabled: boolean;
}

export interface PromptControlSettingsInput {
  default_scenario_profile?: PromptScenarioProfile | null;
  default_soul_id?: string | null;
  default_persona_id?: string | null;
  agent_name?: string | null;
  user_name?: string | null;
  prompt_diagnostics_enabled: boolean;
}

export interface PromptControlSoulOption {
  id: string;
  version: string;
  name: string;
  summary: string;
  mission: string;
  core_principles: string[];
  decision_contract: string;
  non_negotiables: string[];
}

export interface PromptControlPersonaOption {
  id: string;
  soul_id: string;
  version: string;
  name: string;
  summary: string;
  tone_rules: string[];
  collaboration_rules: string[];
  output_preferences: string[];
  /** Avatar id (matches `src/lib/persona-avatars.ts`); `null` for built-in personas. */
  avatar_id?: string | null;
}

export interface PromptControlCatalog {
  souls: PromptControlSoulOption[];
  personas: PromptControlPersonaOption[];
}

export interface SoulCustomization {
  summary?: string | null;
  mission?: string | null;
  core_principles?: string[] | null;
  decision_contract?: string | null;
  non_negotiables?: string[] | null;
}

export interface PersonaCustomization {
  summary?: string | null;
  tone_rules?: string[] | null;
  collaboration_rules?: string[] | null;
  output_preferences?: string[] | null;
}

/** Fully user-defined persona — injected into the catalog alongside built-ins. */
export interface CustomPersonaDefinition {
  soul_id: string;
  name: string;
  summary: string;
  /** Avatar id (matches `src/lib/persona-avatars.ts`); optional. */
  avatar_id?: string | null;
  tone_rules?: string[];
  collaboration_rules?: string[];
  output_preferences?: string[];
}

export interface IdentityCustomizationPack {
  souls: Record<string, SoulCustomization>;
  personas: Record<string, PersonaCustomization>;
  /** User-defined personas (added in pack v1.1; omit for legacy clients). */
  custom_personas?: Record<string, CustomPersonaDefinition>;
}

export type McpServiceTransport =
  | "stdio"
  | "sse"
  | "http"
  | "ws"
  | "sdk"
  | "claudeai-proxy";

export interface McpServiceEntry {
  name: string;
  transport: McpServiceTransport;
  scope: "user" | "project" | "local" | string;
  editable: boolean;
  manager_supported: boolean;
  command?: string | null;
  args: string[];
  env: Record<string, string>;
  url?: string | null;
  headers: Record<string, string>;
  headers_helper?: string | null;
  sdk_name?: string | null;
  proxy_id?: string | null;
}

export interface McpServiceConfig {
  user_settings_path: string;
  restart_required: boolean;
  servers: McpServiceEntry[];
}

export type McpServiceEntryInput = Omit<
  McpServiceEntry,
  "scope" | "editable" | "manager_supported"
>;

export interface McpServiceConfigInput {
  servers: McpServiceEntryInput[];
}

/** Get the current memory configuration. */
export async function getMemoryConfig(): Promise<MemoryConfig> {
  return invoke<MemoryConfig>("get_memory_config");
}

/** Save memory configuration. */
export async function setMemoryConfig(
  config: MemoryConfigInput,
): Promise<MemoryConfig> {
  return invoke<MemoryConfig>("set_memory_config", { config });
}

/** Get effective prompt control settings. */
export async function getPromptControlSettings(): Promise<PromptControlSettings> {
  return invoke<PromptControlSettings>("get_prompt_control_settings");
}

/** Load the built-in Soul / Persona catalog for the prompt control panel. */
export async function getPromptControlCatalog(): Promise<PromptControlCatalog> {
  return invoke<PromptControlCatalog>("get_prompt_control_catalog");
}

/** Load the editable identity customization pack persisted under `~/.if2ai/prompt/`. */
export async function getIdentityCustomizationPack(): Promise<IdentityCustomizationPack> {
  return invoke<IdentityCustomizationPack>("get_identity_customization_pack");
}

/** Persist the editable identity customization pack. */
export async function setIdentityCustomizationPack(
  pack: IdentityCustomizationPack,
): Promise<IdentityCustomizationPack> {
  return invoke<IdentityCustomizationPack>("set_identity_customization_pack", {
    request: pack,
  });
}

/** Load effective MCP services from user/project/local if2Ai settings. */
export async function getMcpServiceConfig(): Promise<McpServiceConfig> {
  return invoke<McpServiceConfig>("get_mcp_service_config");
}

/** Persist user-level MCP services to the if2Ai MCP settings file. */
export async function setMcpServiceConfig(
  request: McpServiceConfigInput,
): Promise<McpServiceConfig> {
  return invoke<McpServiceConfig>("set_mcp_service_config", { request });
}

export async function mcpWorkbenchListServers(): Promise<McpWorkbenchServer[]> {
  return invoke<McpWorkbenchServer[]>("mcp_workbench_list_servers");
}

export async function mcpWorkbenchDiscover(): Promise<McpWorkbenchDiscovery> {
  return invoke<McpWorkbenchDiscovery>("mcp_workbench_discover");
}

export async function mcpWorkbenchCallTool(
  request: McpWorkbenchToolCallRequest,
): Promise<McpWorkbenchToolCallResult> {
  return invoke<McpWorkbenchToolCallResult>("mcp_workbench_call_tool", {
    request,
  });
}

export async function mcpWorkbenchListResources(): Promise<
  McpWorkbenchDiscovery["resources"]
> {
  return invoke<McpWorkbenchDiscovery["resources"]>(
    "mcp_workbench_list_resources",
  );
}

export async function mcpWorkbenchReadResource(
  request: McpWorkbenchReadResourceRequest,
): Promise<McpWorkbenchReadResourceResult> {
  return invoke<McpWorkbenchReadResourceResult>(
    "mcp_workbench_read_resource",
    { request },
  );
}

export async function mcpWorkbenchListPrompts(): Promise<McpWorkbenchPrompt[]> {
  return invoke<McpWorkbenchPrompt[]>("mcp_workbench_list_prompts");
}

export async function mcpWorkbenchGetPrompt(
  request: McpWorkbenchGetPromptRequest,
): Promise<McpWorkbenchGetPromptResult> {
  return invoke<McpWorkbenchGetPromptResult>("mcp_workbench_get_prompt", {
    request,
  });
}

export async function mcpWorkbenchActivity(): Promise<
  McpWorkbenchActivityEntry[]
> {
  return invoke<McpWorkbenchActivityEntry[]>("mcp_workbench_activity");
}

/** Save prompt control settings to the dedicated prompt config directory. */
export async function setPromptControlSettings(
  request: PromptControlSettingsInput,
): Promise<PromptControlSettings> {
  return invoke<PromptControlSettings>("set_prompt_control_settings", {
    request,
  });
}

/** Export trajectories to a user-selected directory. */
export async function exportTrajectories(): Promise<string> {
  return invoke<string>("export_trajectories");
}

/** Reset onboarding state and return to Step 1. */
export async function configResetOnboarding(): Promise<void> {
  return invoke<void>("config_reset_onboarding");
}

/** Get the current app onboarding state. */
export async function onboarding_get_state(): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("onboarding_get_state");
}

/**
 * Phase M2.5 — fetch the canonical
 * [`ActivationSnapshot`](`@/transport/contracts`) from the backend
 * `activation_service`.
 *
 * Backed by `activation_get_status`：本地许可缓存 + 激活 HTTP 客户端；
 * 未设置 `IF2AI_ACTIVATION_BASE_URL` 时 Rust 侧使用公网默认端点（与
 * UClaw 共用 `license-api`）。载荷仍为 M0.4 `ActivationSnapshot` 形状。
 */
export async function activationGetStatus(): Promise<ActivationSnapshot> {
  return invoke<ActivationSnapshot>("activation_get_status");
}

// ─── Phase M2.6 — activation gate (UClaw activation server port) ─────

/** Stable installation identity returned by `activation_get_installation_id`. */
export interface InstallationIdentity {
  installationId: string;
  /** 8-char human-friendly indicator (`XXXX-XXXX`). */
  deviceIndicator: string;
}

/** Wire shape for `POST /v1/activations/request` responses. */
export interface ActivationRequestPayloadDto {
  requestId: string;
  /** Server-issued 8-char alphanumeric code (also used as OTP visual). */
  deviceRequestCode: string;
  /** `"pending"` (frontend should poll) or `"approved"` (skip polling). */
  status: string;
  expiresAt: string;
  serverTime: string;
}

/** Wire shape for `GET /v1/activations/request/:id` responses. */
export interface ActivationPollResponseDto {
  requestId: string;
  status: string;
  canRedeem: boolean;
  serverTime: string;
}

/** Stable error shape returned by activation IPCs (UI-friendly `code`). */
export interface ActivationErrorDto {
  code:
    | "transport"
    | "invalid_response"
    | "queueing"
    | "busy"
    | "invalid_app_id"
    | "server_error"
    | "decoding"
    | "not_configured";
  httpStatus?: number;
  message: string;
}

/** Tauri event payload broadcast before each retry sleep. */
export interface ActivationRetryStatusEvent {
  /** HTTP status that triggered the retry; `-1` for transport errors. */
  statusCode: number;
  attempt: number;
  maxAttempts: number;
}

export const ACTIVATION_RETRY_EVENT = "activation_retry_status";
export const ACTIVATION_STATUS_EVENT = "activation_status_changed";

export async function activationGetInstallationId(): Promise<InstallationIdentity> {
  return invoke<InstallationIdentity>("activation_get_installation_id");
}

export async function activationRequestLicense(
  installationId: string,
): Promise<ActivationRequestPayloadDto> {
  return invoke<ActivationRequestPayloadDto>("activation_request_license", {
    installationId,
  });
}

export async function activationPollRequestStatus(
  requestId: string,
): Promise<ActivationPollResponseDto> {
  return invoke<ActivationPollResponseDto>("activation_poll_request_status", {
    requestId,
  });
}

export async function activationRedeemWithRequestId(
  requestId: string,
  installationId: string,
): Promise<ActivationSnapshot> {
  return invoke<ActivationSnapshot>("activation_redeem_with_request_id", {
    requestId,
    installationId,
  });
}

export async function activationRedeemByInviteCode(
  inviteCode: string,
  installationId: string,
): Promise<ActivationSnapshot> {
  return invoke<ActivationSnapshot>("activation_redeem_by_invite_code", {
    inviteCode,
    installationId,
  });
}

export async function activationRefresh(): Promise<ActivationSnapshot> {
  return invoke<ActivationSnapshot>("activation_refresh");
}

export async function activationRevokeCheck(): Promise<ActivationSnapshot> {
  return invoke<ActivationSnapshot>("activation_revoke_check");
}

export async function activationDeactivate(): Promise<ActivationSnapshot> {
  return invoke<ActivationSnapshot>("activation_deactivate");
}

/** Subscribe to retry-status notifications.  Returns the unlisten fn. */
export function listenActivationRetryStatus(
  cb: (event: ActivationRetryStatusEvent) => void,
): Promise<UnlistenFn> {
  return listen<ActivationRetryStatusEvent>(ACTIVATION_RETRY_EVENT, (e) =>
    cb(e.payload),
  );
}

/**
 * Subscribe to backend-emitted activation snapshot transitions.
 * The Rust lifecycle loop emits this whenever the periodic
 * `revoke_check` (or refresh) returns a snapshot whose `kind` /
 * `allowsMainShell` differs from the previous tick — e.g. the
 * server revoked the license while the app was running.
 *
 * The payload mirrors the same `ActivationSnapshot` returned by
 * `activation_get_status`, so listeners typically just dispatch a
 * `refreshActivationSnapshot()` to keep the projection store in sync.
 */
export function listenActivationStatusChanged(
  cb: (snapshot: ActivationSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<ActivationSnapshot>(ACTIVATION_STATUS_EVENT, (e) =>
    cb(e.payload),
  );
}

// ─── Per-role usage dashboard (Settings ▸ 用量统计) ─────────────────

export type UsageWindow = "today" | "this_week" | "this_month" | "all_time";

export interface CallerUsage {
  caller: string;
  inputTokens: number;
  outputTokens: number;
  costUsd: number;
  turns: number;
}

export interface UsageSummary {
  window: string;
  byCaller: CallerUsage[];
  total: CallerUsage;
}

/**
 * Aggregate the durable per-role usage store for one of four time
 * windows. `window` matches the Rust `UsageWindow` snake-case enum.
 */
export async function usageSummary(window: UsageWindow): Promise<UsageSummary> {
  return invoke<UsageSummary>("usage_summary", { window });
}

// ─── Manual /compact + auto-compact event ───────────────────────────

export const CHAT_COMPACT_COMPLETED_EVENT = "chat_compact_completed";

export interface CompactReport {
  sessionId: string;
  summarizedMessages: number;
  freedTokens: number;
  summaryExcerpt: string;
  didCompact: boolean;
}

/**
 * Manually compact a session: forces RollingSummarizer to fold the
 * current message history into a summary and persist it. Bypasses
 * the COMPACT_SKIP_WINDOW_SECS throttle. Returns a report so the UI
 * can show how many messages were folded.
 */
export async function chatCompactSession(
  sessionId: string,
): Promise<CompactReport> {
  return invoke<CompactReport>("chat_compact_session", { sessionId });
}

/**
 * Subscribe to backend-emitted auto-compact events. Fires after the
 * background tick (post-finalize) lands a new summary because the
 * ContextBudget crossed the configured `IF2AI_AUTO_COMPACT_THRESHOLD`
 * (default 85%).
 */
export function listenChatCompactCompleted(
  cb: (report: CompactReport) => void,
): Promise<UnlistenFn> {
  return listen<CompactReport>(CHAT_COMPACT_COMPLETED_EVENT, (e) =>
    cb(e.payload),
  );
}

/** Phase M2.6 — wire-shape input for the deterministic classifier. */
export interface RequestIntelligenceClassifyInput {
  userMessage: string;
  sessionId?: string;
  projectId?: string;
  /** Working directory (string; backend converts to `PathBuf`). */
  workdir?: string;
}

/**
 * Phase M2.6 — run the deterministic + heuristic classifier and
 * return the canonical [`ExecutionModeDecision`].
 *
 * Honest scope: this is the same decision the backend
 * `TurnService::prepare_chat_inputs` produces, exposed as a
 * standalone IPC so the chat UI can render an honest "current
 * judgment" chip without waiting for a real `start_agent_stream`
 * call.  The decision is **advisory** — it does not auto-route
 * the agent.  The frontend MUST NOT recompute the mode locally.
 */
export async function requestIntelligenceClassify(
  input: RequestIntelligenceClassifyInput,
): Promise<ExecutionModeDecision> {
  return invoke<ExecutionModeDecision>("request_intelligence_classify", {
    input,
  });
}

// ─── Browser Control (Phase 7B) ──────────────────────────────────────────────

/** Payload of the `"browser-status"` Tauri event emitted after each browser action. */
export interface BrowserStatusEvent {
  session_id: string;
  running: boolean;
  url: string | null;
  /** Base-64 JPEG thumbnail of the current viewport, or null when unavailable. */
  thumbnail: string | null;
  backend?: "local_rust_cdp" | "browser_use_mcp" | "browser_use_cloud";
  title?: string | null;
  taken_over?: boolean;
  last_action?: string | null;
  downloads_count?: number;
  console_count?: number;
  network_error_count?: number;
  escalation_state?: "none" | "suggested" | "approval_required" | "approved" | "active" | "blocked";
}

/** Snapshot of a single active browser session returned by `get_browser_sessions`. */
export interface BrowserSessionEntry {
  session_id: string;
  running: boolean;
  url: string | null;
}

/** Response from `get_chrome_status` — reports whether Chrome is installed. */
export interface ChromeStatusPayload {
  found: boolean;
  path: string | null;
}

/** List all currently active AI-controlled browser sessions. */
export async function getBrowserSessions(): Promise<BrowserSessionEntry[]> {
  return invoke<BrowserSessionEntry[]>("get_browser_sessions");
}

/**
 * Close the browser session for `sessionId`.
 * The AI's browser process is terminated and the session is removed from the registry.
 */
export async function closeBrowserSession(sessionId: string): Promise<void> {
  return invoke<void>("close_browser_session", { sessionId });
}

/** Check whether a Chrome or Chromium binary is available on this machine. */
export async function getChromeStatus(): Promise<ChromeStatusPayload> {
  return invoke<ChromeStatusPayload>("get_chrome_status");
}

// ── Browser profile / settings management (Phase 7C, slice 7C.1 + Settings UI) ─

/** Persistent profile placement strategy.  Mirrors `BrowserProfileMode` in Rust. */
export type BrowserProfileMode =
  | "per_session_persistent"
  | "shared"
  | "ephemeral";

/** One row returned by `list_browser_profiles`. */
export interface BrowserProfileEntry {
  /** `session_id` portion of the directory name (or `"_shared"`). */
  session_id: string;
  /** Absolute path on disk. */
  path: string;
  /** Recursive directory size in bytes (best effort). */
  size_bytes: number;
  /** RFC-3339 last modified timestamp of the directory itself. */
  last_used: string | null;
}

/** Browser settings persisted in `~/.if2ai/browser.toml`. */
export interface BrowserSettings {
  /** Persisted preferred mode (next launch). */
  profile_mode: BrowserProfileMode;
  /** Soft per-profile disk cap (megabytes). */
  max_profile_disk_mb: number;
  /** Soft total disk cap across all profiles (megabytes). */
  max_total_disk_mb: number;
  /** Active env var override; non-null => env wins until cleared. */
  env_override: BrowserProfileMode | null;
  /** Mode currently used by the running registry. */
  active_mode: BrowserProfileMode;
}

/** List every persistent browser profile under `~/.if2ai/browser-profiles/`. */
export async function listBrowserProfiles(): Promise<BrowserProfileEntry[]> {
  return invoke<BrowserProfileEntry[]>("list_browser_profiles");
}

/**
 * Wipe the persistent profile directory for `sessionId` (cookies, localStorage…).
 * Throws if a `BrowserSession` is still running for that id.
 */
export async function clearBrowserProfile(sessionId: string): Promise<void> {
  return invoke<void>("clear_browser_profile", { sessionId });
}

/** Read persisted browser settings + diagnostics (env override, active mode). */
export async function getBrowserSettings(): Promise<BrowserSettings> {
  return invoke<BrowserSettings>("get_browser_settings");
}

/** Persist edited browser settings (takes effect next launch for `profile_mode`). */
export async function setBrowserSettings(
  settings: BrowserSettings,
): Promise<void> {
  return invoke<void>("set_browser_settings", { settings });
}

// ── Headed mode + user takeover (Phase 7C, slice 7C.3) ───────────────────────

/** Result returned by `request_browser_takeover` so the UI can update its
 *  address bar after the headed Chrome relaunch + auto-navigate.
 */
export interface BrowserNavigateResult {
  url: string;
  title: string;
  snapshot: string;
}

/**
 * Hand control of the browser to the human user.
 *
 * Closes the headless Chromium for `sessionId` and re-launches it as a
 * **visible** Chrome window against the same persistent profile.  While
 * this returns successfully, every subsequent AI `browser` tool call
 * surfaces a "paused" error until {@link releaseBrowserTakeover} is
 * invoked.
 */
export async function requestBrowserTakeover(
  sessionId: string,
): Promise<BrowserNavigateResult> {
  return invoke<BrowserNavigateResult>("request_browser_takeover", {
    sessionId,
  });
}

/**
 * Release the takeover flag; optionally re-launch headless so the
 * Chrome window goes away.  Cookies / login state created by the user
 * are preserved when the persistent profile mode is in use.
 */
export async function releaseBrowserTakeover(
  sessionId: string,
  backToHeadless = true,
): Promise<void> {
  return invoke<void>("release_browser_takeover", {
    sessionId,
    backToHeadless,
  });
}

export interface SmartBrowserCloudEscalationDecision {
  backend: "browser_use_cloud";
  state: "approval_required" | "approved" | "active" | "blocked";
  reason: string;
}

export async function requestSmartBrowserCloudEscalation(
  sessionId: string,
  reason: string,
): Promise<SmartBrowserCloudEscalationDecision> {
  return invoke<SmartBrowserCloudEscalationDecision>(
    "request_smart_browser_cloud_escalation",
    { sessionId, reason },
  );
}

export async function approveSmartBrowserCloudEscalation(
  sessionId: string,
  reason: string,
): Promise<SmartBrowserCloudEscalationDecision> {
  return invoke<SmartBrowserCloudEscalationDecision>(
    "approve_smart_browser_cloud_escalation",
    { sessionId, reason },
  );
}

export async function denySmartBrowserCloudEscalation(
  sessionId: string,
  reason: string,
): Promise<SmartBrowserCloudEscalationDecision> {
  return invoke<SmartBrowserCloudEscalationDecision>(
    "deny_smart_browser_cloud_escalation",
    { sessionId, reason },
  );
}

/**
 * Subscribe to `"browser-status"` Tauri events.
 * Returns an unlisten function — call it on component unmount to avoid memory leaks.
 */
export async function listenToBrowserStatus(
  handler: (payload: BrowserStatusEvent) => void,
): Promise<UnlistenFn> {
  return listen<BrowserStatusEvent>("browser-status", (event) =>
    handler(event.payload),
  );
}

/**
 * Open (or focus) the BrowserViewer window for `sessionId`.
 * The window renders an independent web view of the URL the AI is currently
 * visiting — note it does NOT share the chromiumoxide session state.
 */
export async function openBrowserViewerWindow(
  sessionId: string,
): Promise<void> {
  return invoke<void>("open_browser_viewer_window", { sessionId });
}

/**
 * Ask the backend to immediately emit a `"browser-status"` event for `sessionId`.
 * Call this when the BrowserViewer window first mounts so it can bootstrap its
 * display state without waiting for the next AI browser action.
 */
export async function requestBrowserStatus(sessionId: string): Promise<void> {
  return invoke<void>("request_browser_status", { sessionId });
}

/** Navigate the embedded live WKWebView in the viewer window to `url`. */
export async function navigateViewerWindow(
  sessionId: string,
  url: string,
): Promise<void> {
  return invoke<void>("navigate_viewer_window", { sessionId, url });
}

/** Go back in the viewer WKWebView's navigation history. */
export async function browserViewerGoBack(sessionId: string): Promise<void> {
  return invoke<void>("browser_viewer_go_back", { sessionId });
}

/** Go forward in the viewer WKWebView's navigation history. */
export async function browserViewerGoForward(sessionId: string): Promise<void> {
  return invoke<void>("browser_viewer_go_forward", { sessionId });
}

/** Reload the current page in the viewer WKWebView. */
export async function browserViewerReload(sessionId: string): Promise<void> {
  return invoke<void>("browser_viewer_reload", { sessionId });
}

// ── Harness Control IPC ────────────────────────────────────────────────────────

/**
 * Per-session telemetry snapshot emitted by the agent loop Harness.
 * Mirrors the Rust `SessionTelemetry` struct in `src-tauri/src/modules/harness/telemetry.rs`.
 */
export interface SessionTelemetry {
  session_id: string;
  turns_completed: number;
  turns_succeeded: number;
  llm_calls: number;
  input_tokens_total: number;
  output_tokens_total: number;
  /** Per-tool call counts (tool_name → count). */
  tool_calls: Record<string, number>;
  /** Per-tool success counts (tool_name → count). */
  tool_successes: Record<string, number>;
  compaction_events: number;
  permission_prompts: number;
  reflection_cycles: number;
  reflection_insights_total: number;
  /** ISO 8601 timestamp of the last agent event, or null. */
  last_event_at: string | null;
  total_turn_duration_ms: number;
}

/** Harness recording status from `get_harness_status`. */
export interface HarnessStatusResponse {
  harness_enabled: boolean;
  /** Session IDs currently being recorded. */
  active_recordings: string[];
}

/** Telemetry query result from `get_session_telemetry`. */
export interface HarnessTelemetryResponse {
  found: boolean;
  telemetry: SessionTelemetry | null;
}

/** Return the current harness status (whether recording, which sessions). */
export async function getHarnessStatus(): Promise<HarnessStatusResponse> {
  return invoke<HarnessStatusResponse>("get_harness_status");
}

/** Start recording agent events for `sessionId` to a JSONL trace file. */
export async function startHarnessRecording(sessionId: string): Promise<void> {
  return invoke<void>("start_harness_recording", { sessionId });
}

/** Stop recording for `sessionId` and flush the trace file. */
export async function stopHarnessRecording(sessionId: string): Promise<void> {
  return invoke<void>("stop_harness_recording", { sessionId });
}

/** Fetch the telemetry snapshot for `sessionId`. */
export async function getSessionTelemetry(
  sessionId: string,
): Promise<HarnessTelemetryResponse> {
  return invoke<HarnessTelemetryResponse>("get_session_telemetry", {
    sessionId,
  });
}

/** Fetch telemetry snapshots for all sessions tracked by the harness. */
export async function getAllSessionTelemetry(): Promise<SessionTelemetry[]> {
  return invoke<SessionTelemetry[]>("get_all_session_telemetry");
}

// ---------------------------------------------------------------------------
// Memory Browser commands — three-tier scope wrappers.
//
// These helpers mirror `src-tauri/src/commands/memory.rs`.  All scope-related
// arguments are optional: when `scopeKind` is omitted the backend falls back
// to the legacy unscoped library view, so existing callers (e.g. the current
// MemoryBrowser without a scope selector) continue to work unchanged.

/** Persisted memory entry as returned by Tauri commands. */
export interface MemoryEntryDto {
  key: string;
  content: string;
  category: string;
  created_at: string;
  updated_at: string;
  importance: number;
  access_count: number;
  trust_score: number;
  /** Persisted session scope tag; null = entry not session-scoped. */
  session_id: string | null;
  /** Persisted project scope tag; null = entry not project-scoped. */
  project_id: string | null;
  /** Composite quality score in [0.0, 1.0]. */
  quality_score: number;
  /** Source reliability in [0.0, 1.0]. */
  source_reliability: number;
  /** ISO-8601 timestamp of last validation, or null. */
  last_validated_at: string | null;
  /** Number of contradiction flags. */
  contradiction_count: number;
}

/** Three-tier memory scope kind matching `MemoryScopeKind` on the backend. */
export type MemoryScopeKind = "global" | "project" | "session";

/** Optional scope filter for `memoryRecall` / `memoryExport`. */
export interface MemoryScopeArgs {
  scopeKind?: MemoryScopeKind;
  sessionId?: string;
  projectId?: string;
}

/**
 * Search memory entries.  When `scopeKind` is omitted the call falls back to
 * the legacy unscoped recall path.
 */
export async function memoryRecall(args: {
  query: string;
  category?: string | null;
  limit?: number;
  scope?: MemoryScopeArgs;
}): Promise<MemoryEntryDto[]> {
  return invoke<MemoryEntryDto[]>("memory_recall", {
    query: args.query,
    category: args.category ?? null,
    limit: args.limit ?? null,
    scopeKind: args.scope?.scopeKind ?? null,
    sessionId: args.scope?.sessionId ?? null,
    projectId: args.scope?.projectId ?? null,
  });
}

/**
 * Export memory entries.  When `scopeKind` is omitted the call falls back to
 * the legacy full-library export.
 */
export async function memoryExport(
  args: {
    category?: string | null;
    scope?: MemoryScopeArgs;
  } = {},
): Promise<MemoryEntryDto[]> {
  return invoke<MemoryEntryDto[]>("memory_export", {
    category: args.category ?? null,
    scopeKind: args.scope?.scopeKind ?? null,
    sessionId: args.scope?.sessionId ?? null,
    projectId: args.scope?.projectId ?? null,
  });
}

/** Delete a memory entry by key. */
export async function memoryDelete(key: string): Promise<void> {
  return invoke<void>("memory_delete", { key });
}

/**
 * MEM-MOD-P6 — One historical snapshot of a memory entry, captured
 * just before an UPDATE / CONSOLIDATE. Returned newest-first by
 * {@link memoryHistory}.
 */
export interface MemoryHistoryEntryDto {
  key: string;
  content: string;
  category: string;
  importance: number;
  trust_score: number;
  /** RFC 3339 — when this snapshot's value was first written. */
  valid_from: string;
  /** RFC 3339 — when this snapshot was archived (= overwritten). */
  valid_to: string;
  /** `"update"` | `"consolidate"` — what triggered the snapshot. */
  source: string;
}

/**
 * MEM-MOD-P6 — Pull every historical snapshot for `key` (newest first).
 * Empty when the key has never been updated or consolidated.
 */
export async function memoryHistory(
  key: string,
): Promise<MemoryHistoryEntryDto[]> {
  return invoke<MemoryHistoryEntryDto[]>("memory_history", { key });
}

/**
 * MEM-MOD-P7 — One durable observation about the user accumulated
 * across sessions. Surfaced by {@link learnedTraitsList}; retired by
 * {@link learnedTraitsDisagree} when the user clicks "我不同意".
 */
export interface LearnedTraitDto {
  id: number;
  trait_text: string;
  evidence_count: number;
  /** [0, 1) — saturating with each repeat sighting (rises 30 % of gap). */
  confidence: number;
  /** RFC 3339 — first time the LLM extractor surfaced this trait. */
  first_seen_at: string;
  /** RFC 3339 — most recent time evidence_count was bumped. */
  last_updated_at: string;
  /** Originating session_id; informational only. */
  source_session: string | null;
}

/**
 * MEM-MOD-P7 — List active (non-disagreed) cross-session traits the
 * agent has accumulated. Newest first. `limit` defaults to 50 backend-side.
 */
export async function learnedTraitsList(
  limit?: number,
): Promise<LearnedTraitDto[]> {
  return invoke<LearnedTraitDto[]>("learned_traits_list", {
    limit: limit ?? null,
  });
}

/**
 * MEM-MOD-P7 — Mark a trait as disagreed-with so the prompt block stops
 * surfacing it. The row is preserved (audit trail), only `disagreed_at`
 * is set.
 */
export async function learnedTraitsDisagree(id: number): Promise<void> {
  return invoke<void>("learned_traits_disagree", { id });
}

/** A single promotion recommendation surfaced to the Memory Browser. */
export interface MemoryPromotionCandidateDto {
  key: string;
  category: string;
  /** `'session'` / `'project'` / `'global'` — the entry's tier today. */
  current_tier: string;
  /** `'project'` / `'global'` — where the engine recommends moving it. */
  target_tier: string;
  access_count: number;
  importance: number;
  /** Human-readable rationale (Chinese). */
  reason: string;
}

/**
 * Scan the memory library for entries that meet promotion thresholds.
 *
 * Non-destructive: returns recommendations only.  Apply with [`memoryPromote`].
 */
export async function memoryPromotionCandidates(): Promise<
  MemoryPromotionCandidateDto[]
> {
  return invoke<MemoryPromotionCandidateDto[]>("memory_promotion_candidates");
}

/**
 * Apply a promotion to a single entry.
 *
 * `targetScopeKind` must be `'project'` or `'global'`; promoting to
 * `'session'` is rejected by the backend.  When promoting to `'project'`,
 * `projectId` is required.
 */
export async function memoryPromote(args: {
  key: string;
  targetScopeKind: Exclude<MemoryScopeKind, "session">;
  projectId?: string;
}): Promise<void> {
  return invoke<void>("memory_promote", {
    key: args.key,
    targetScopeKind: args.targetScopeKind,
    projectId: args.projectId ?? null,
  });
}

/**
 * Reverse of [`memoryPromote`] — narrow an entry's visibility back down.
 *
 * `targetScopeKind` must be `'project'` or `'session'`; demoting to
 * `'global'` is rejected by the backend.  When demoting to `'project'`,
 * `projectId` is required; when demoting to `'session'`, both `sessionId`
 * and (typically) the entry's owning `projectId` are required so the entry
 * stays visible to the active context.
 *
 * Backend rejects calls that would *raise* visibility (e.g. demoting a
 * session-scoped entry to project) with an error string — call
 * `memoryPromote` for that direction instead.
 */
export async function memoryDemote(args: {
  key: string;
  targetScopeKind: Exclude<MemoryScopeKind, "global">;
  sessionId?: string;
  projectId?: string;
}): Promise<void> {
  return invoke<void>("memory_demote", {
    key: args.key,
    targetScopeKind: args.targetScopeKind,
    sessionId: args.sessionId ?? null,
    projectId: args.projectId ?? null,
  });
}

/**
 * Wipe **every** memory entry across all categories and scopes.
 *
 * Drives the "Clear all memories" affordance on the Memory Settings
 * page.  The frontend MUST present a confirmation dialog before
 * invoking this — the backend performs no extra confirmation so it
 * stays scriptable from harness tooling.
 *
 * Returns the number of rows the backend reports as removed.  A
 * `memory_cleared` event is emitted on the `memory_event` channel even
 * when the count is zero, so the Telemetry Drawer can correlate the
 * action.
 */
export async function memoryClearAll(): Promise<number> {
  return invoke<number>("memory_clear_all");
}

// ─── Memory Compile Pipeline (Phase 8B.5 / T-C5) ────────────────────────────
//
// Tauri wrappers for `memory_compile_now` / `memory_compiled_read` /
// `memory_compiled_clear`.  The four-stage compile pipeline
// (today / week / longterm / facts → assemble) lives entirely in
// `src-tauri/src/modules/memory/compiler/` and is normally driven by
// the Phase 8B ticker (8B.6+); these commands let the frontend
// CompiledMemoryViewer (8B.10) and operator scripts trigger / inspect /
// drop the compiled cache on demand.
//
// `scope` is one of `'current' | 'all' | 'project' | 'global'`.  Until
// multi-scope sidecars land in 8B.x every value collapses onto the
// shared `<data_local_dir>/.if2ai/memory/` root on the backend; the
// string union is kept so the wire-shape stays stable for future
// project / session sidecars.

/**
 * MEM-MOD-WIRE-FIX-3 — `CompileResult::Skipped` now carries a reason
 * payload so the Memory Debug UI can explain *why* a stage didn't
 * recompile. Wire shape mirrors `serde(tag = "kind")` over the Rust
 * enum:
 *   { kind: "compiled" }
 *   { kind: "skipped", reason: "cache_hit" | ... }
 */
export type CompileSkipReason =
  | "cache_hit"
  | "upstream_missing"
  | "empty_input"
  | "llm_degraded";

export type CompileResult =
  | { kind: "compiled" }
  | { kind: "skipped"; reason: CompileSkipReason };

/** Frontend mirror of the backend `CompileReport`. */
export interface CompileReport {
  today: CompileResult;
  week: CompileResult;
  longterm: CompileResult;
  facts: CompileResult;
  assembled: boolean;
  elapsed_ms: number;
}

/** Frontend mirror of the backend `CompiledSection`. */
export interface CompiledSection {
  /** Raw markdown contents; empty string when the section is missing. */
  content: string;
  /** ISO-8601 file mtime, or null when the section has never been compiled. */
  last_compiled_at: string | null;
  /** Character count of `content` (Unicode scalars, not bytes). */
  chars: number;
}

/** Frontend mirror of the backend `CompiledMemoryDto`. */
export interface CompiledMemoryDto {
  memory_md: string;
  today: CompiledSection;
  week: CompiledSection;
  longterm: CompiledSection;
  facts: CompiledSection;
}

/**
 * Trigger the full Phase 8B compile pipeline (today → week → longterm →
 * facts → assemble) for `scope`.  Each stage honours its fingerprint
 * cache so repeated calls without changes are cheap.
 */
export async function memoryCompileNow(
  scope: "current" | "all" | "project" | "global",
): Promise<CompileReport> {
  return invoke<CompileReport>("memory_compile_now", { scope });
}

/**
 * Snapshot every compiled `*.md` for `scope` plus the assembled
 * `memory.md`.  Read-only — never invokes the LLM.
 */
export async function memoryCompiledRead(
  scope: "current" | "all" | "project" | "global",
): Promise<CompiledMemoryDto> {
  return invoke<CompiledMemoryDto>("memory_compiled_read", { scope });
}

/**
 * Drop the compiled cache for `scope`: truncates the four section
 * files plus `memory.md`, then unlinks every fingerprint sidecar so
 * the next [`memoryCompileNow`] is guaranteed to re-run the LLM.
 */
export async function memoryCompiledClear(
  scope: "current" | "all" | "project" | "global",
): Promise<void> {
  await invoke<void>("memory_compiled_clear", { scope });
}

// ─── Phase 8B.11 / T-UI-3 — session-summary timeline ──────────────────

/**
 * Frontend mirror of the backend `SessionSummaryDto`
 * (`src-tauri/src/commands/memory.rs`).  One row per session of the
 * rolling-summary table, surfaced to MemoryNarrativeViewer.
 */
export interface SessionSummaryDto {
  session_id: string;
  project_id: string | null;
  summary: string;
  /** RFC-3339 / ISO-8601 timestamp of first save. */
  created_at: string;
  /** RFC-3339 / ISO-8601 timestamp of latest save. */
  updated_at: string;
  message_count: number;
  /** `'rolling'` = scheduled summary update; `'compact'` = T-B5 context-window compaction. */
  source: "rolling" | "compact";
}

/**
 * List session summaries in `[now - sinceDays, now]` ordered newest-first.
 *
 * Used by MemoryNarrativeViewer (Phase 8B.11 / T-UI-3) to render the
 * per-session rolling-summary timeline grouped by date.  `limit`
 * defaults to 100 backend-side and is hard-capped at 500.  `sinceDays`
 * defaults to 90.
 */
export async function memorySummariesList(
  scope: "current" | "all" | "project" | "global",
  limit?: number,
  sinceDays?: number,
): Promise<SessionSummaryDto[]> {
  return invoke<SessionSummaryDto[]>("memory_summaries_list", {
    scope,
    limit,
    sinceDays,
  });
}

// ─── TTS (Text-to-Speech) Commands ───────────────────────────────────────────

/** TTS generation parameters mirroring the Rust `GenerationParams` struct. */
export interface TtsGenerationParams {
  max_new_frames: number;
  voice_clone_max_text_tokens: number;
  tts_max_batch_size: number;
  codec_max_batch_size: number;
  do_sample: boolean;
  text_temperature: number;
  text_top_p: number;
  text_top_k: number;
  audio_temperature: number;
  audio_top_p: number;
  audio_top_k: number;
  audio_repetition_penalty: number;
  seed: number | null;
  enable_robust_normalization: boolean;
}

// ── User-tunable TTS settings (Settings UI entry point) ───────────────────

/** Quality preset for the audio sampler.  Maps to a tuned triple of
 *  audio_temperature / top_p / top_k server-side. */
export type TtsQualityPreset = "natural" | "balanced" | "precise";

/** Persisted TTS settings (`~/.if2ai/tts.toml`).  Mirrors the Rust
 *  `TtsSettings` struct field-for-field. */
export interface TtsSettings {
  /** Frontend playback speed multiplier (0.5-2.0). 1.0 = original. */
  playback_rate: number;
  /** Sampler quality preset. */
  quality: TtsQualityPreset;
  /** Hard cap on generated audio frames per chunk (64-1500). */
  max_new_frames: number;
  /** Audio repetition penalty (>= 1.0). */
  audio_repetition_penalty: number;
  /** Optional fixed RNG seed; null = random. */
  seed: number | null;
  /** Whether MOSS-TTS-Nano's robust text normaliser is enabled. */
  enable_robust_normalization: boolean;
}

/** Defaults matching the Rust `TtsSettings::default()` impl. */
export const TTS_DEFAULT_SETTINGS: TtsSettings = {
  playback_rate: 1.0,
  quality: "natural",
  max_new_frames: 375,
  audio_repetition_penalty: 1.2,
  seed: null,
  enable_robust_normalization: true,
};

/** Read persisted TTS settings.  Always succeeds (falls back to defaults). */
export async function getTtsSettings(): Promise<TtsSettings> {
  return invoke<TtsSettings>("get_tts_settings");
}

/** Persist TTS settings to `~/.if2ai/tts.toml`.  Out-of-range values
 *  are clamped server-side. */
export async function setTtsSettings(settings: TtsSettings): Promise<void> {
  return invoke<void>("set_tts_settings", { settings });
}

/** Translate a `TtsSettings` payload into the
 *  [`TtsGenerationParams`] shape `tts_synthesize` / `tts_stream_start`
 *  expects.  Uses the `apply_to_generation_params` semantics from the
 *  Rust `TtsSettings::apply_to_generation_params` so backend and
 *  frontend always produce identical params for the same settings. */
export function ttsParamsFromSettings(s: TtsSettings): TtsGenerationParams {
  const samplerByPreset: Record<
    TtsQualityPreset,
    { t: number; p: number; k: number }
  > = {
    natural: { t: 0.9, p: 0.95, k: 30 },
    balanced: { t: 0.8, p: 0.95, k: 25 },
    precise: { t: 0.6, p: 0.85, k: 15 },
  };
  const sampler = samplerByPreset[s.quality];
  return {
    ...TTS_DEFAULT_PARAMS,
    max_new_frames: Math.min(Math.max(s.max_new_frames, 64), 1500),
    audio_temperature: sampler.t,
    audio_top_p: sampler.p,
    audio_top_k: sampler.k,
    audio_repetition_penalty: Math.max(s.audio_repetition_penalty, 1.0),
    seed: s.seed,
    enable_robust_normalization: s.enable_robust_normalization,
  };
}

// ── TTS Profiles (named voice + settings recipes) ────────────────────────

/** Lightweight text post-processing flags carried by a TTS profile. */
export interface TtsTextPostprocess {
  /** Replace 。/. with ，/, to soften the cadence (温柔系 profile). */
  soften_punctuation: boolean;
  /** Append "…" to every sentence so prosody trails off (沉浸朗读). */
  add_trailing_dots: boolean;
}

/** A single named TTS recipe.  Mirrors the Rust `TtsProfile` struct
 *  (which uses `#[serde(flatten)]` to inline `TtsSettings` fields). */
export interface TtsProfile {
  id: string;
  name: string;
  description: string;
  voice_id: string;
  // ── flattened TtsSettings ──
  playback_rate: number;
  quality: TtsQualityPreset;
  max_new_frames: number;
  audio_repetition_penalty: number;
  seed: number | null;
  enable_robust_normalization: boolean;
  // ── profile-only fields ──
  postprocess: TtsTextPostprocess;
  is_builtin: boolean;
}

/** On-disk container returned by `list_tts_profiles`. */
export interface TtsProfileBook {
  default_profile_id: string;
  profiles: TtsProfile[];
}

/** Read the entire profile book.  Backend seeds 6 builtins on first call. */
export async function listTtsProfiles(): Promise<TtsProfileBook> {
  return invoke<TtsProfileBook>("list_tts_profiles");
}

/** Insert or update a single profile.  Returns the resulting profile. */
export async function saveTtsProfile(profile: TtsProfile): Promise<TtsProfile> {
  return invoke<TtsProfile>("save_tts_profile", { profile });
}

/** Delete a user-created profile (builtins are protected server-side). */
export async function deleteTtsProfile(id: string): Promise<void> {
  return invoke<void>("delete_tts_profile", { id });
}

/** Mark `id` as the active default profile. */
export async function setDefaultTtsProfile(id: string): Promise<void> {
  return invoke<void>("set_default_tts_profile", { id });
}

/** Project a `TtsProfile` onto a `TtsSettings` shape (for re-using
 *  `ttsParamsFromSettings` to derive generation params). */
export function ttsSettingsFromProfile(p: TtsProfile): TtsSettings {
  return {
    playback_rate: p.playback_rate,
    quality: p.quality,
    max_new_frames: p.max_new_frames,
    audio_repetition_penalty: p.audio_repetition_penalty,
    seed: p.seed,
    enable_robust_normalization: p.enable_robust_normalization,
  };
}

/** Frontend mirror of the Rust `apply_postprocess` — keep in sync if
 *  you tweak the Rust impl.  Used so chat-side preview matches what
 *  the backend will actually synthesize. */
export function applyTtsPostprocess(p: TtsProfile, text: string): string {
  let out = text;
  if (p.postprocess.soften_punctuation) {
    out = out.replace(/。/g, "，").replace(/\./g, ",");
  }
  if (p.postprocess.add_trailing_dots) {
    const trimmed = out.replace(/\s+$/, "");
    if (!trimmed.endsWith("…") && !trimmed.endsWith("...")) {
      out = `${trimmed}…`;
    }
  }
  return out;
}

/** Default TTS generation parameters matching the MOSS-TTS-Nano Python reference defaults. */
export const TTS_DEFAULT_PARAMS: TtsGenerationParams = {
  max_new_frames: 375,
  voice_clone_max_text_tokens: 75,
  tts_max_batch_size: 0,
  codec_max_batch_size: 0,
  do_sample: true,
  text_temperature: 1.0,
  text_top_p: 1.0,
  text_top_k: 50,
  audio_temperature: 0.8,
  audio_top_p: 0.95,
  audio_top_k: 25,
  audio_repetition_penalty: 1.2,
  seed: null,
  enable_robust_normalization: true,
};

/** TTS health check response. */
/** Phase TTS-D.1：Provider 生命周期状态。 */
export type TtsProviderState =
  | { kind: "notLoaded" }
  | { kind: "loading" }
  | { kind: "loaded"; elapsedSeconds: number }
  | { kind: "failed"; error: string }
  | { kind: "evicted"; elapsedSeconds: number };

export interface TtsHealthResponse {
  status: string;
  warmup_state: string;
  warmup_progress: number;
  message: string;
  /** Phase TTS-D.1：Provider 生命周期状态机。 */
  provider_state: TtsProviderState;
}

/** TTS warmup status response. */
export interface TtsWarmupStatusResponse {
  state: string;
  progress: number;
  message: string;
  error: string | null;
}

/** Buffered synthesis response — WAV audio as base64. */
export interface TtsSynthesisResponse {
  audio_base64: string;
  sample_rate: number;
  duration_seconds: number;
  voice: string;
  text_chunks: string[];
}

/** Streaming job start response. */
export interface TtsStreamStartResponse {
  stream_id: string;
  sample_rate: number;
  channels: number;
}

/** Demo audio response. */
export interface TtsDemoAudioResponse {
  audio_base64: string;
  content_type: string;
}

/** Check TTS system health. */
export async function ttsHealth(): Promise<TtsHealthResponse> {
  return invoke<TtsHealthResponse>("tts_health");
}

/** Get current warmup status. */
export async function ttsWarmupStatus(): Promise<TtsWarmupStatusResponse> {
  return invoke<TtsWarmupStatusResponse>("tts_warmup_status");
}

/** Trigger TTS warmup (runs in background). */
export async function ttsStartWarmup(): Promise<void> {
  return invoke<void>("tts_start_warmup");
}

/** Buffered synthesis — returns complete WAV as base64. */
export async function ttsSynthesize(
  text: string,
  demoId: string | null,
  promptAudioPath: string | null,
  params: TtsGenerationParams,
  voiceId?: string | null,
): Promise<TtsSynthesisResponse> {
  return invoke<TtsSynthesisResponse>("tts_synthesize", {
    text,
    demoId,
    voiceId: voiceId ?? null,
    promptAudioPath,
    params,
  });
}

/** Start streaming synthesis — returns stream_id for tracking. */
export async function ttsStreamStart(
  text: string,
  demoId: string | null,
  promptAudioPath: string | null,
  params: TtsGenerationParams,
  voiceId?: string | null,
): Promise<TtsStreamStartResponse> {
  return invoke<TtsStreamStartResponse>("tts_stream_start", {
    text,
    demoId,
    voiceId: voiceId ?? null,
    promptAudioPath,
    params,
  });
}

/** Poll streaming job status. */
export async function ttsStreamStatus(
  streamId: string,
): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("tts_stream_status", { streamId });
}

/** Get final streaming job result. */
export async function ttsStreamResult(
  streamId: string,
): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("tts_stream_result", { streamId });
}

/** Close/cancel a streaming job. */
export async function ttsStreamClose(
  streamId: string,
): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("tts_stream_close", { streamId });
}

/** Get demo audio by ID as base64. */
export async function ttsDemoAudio(
  demoId: string,
): Promise<TtsDemoAudioResponse> {
  return invoke<TtsDemoAudioResponse>("tts_demo_audio", { demoId });
}

/** List available voice names. */
export async function ttsListVoices(): Promise<string[]> {
  return invoke<string[]>("tts_list_voices");
}

/** Split text into chunks for voice clone preview. */
export async function ttsSplitText(
  text: string,
  maxTokens: number,
): Promise<string[]> {
  return invoke<string[]>("tts_split_text", { text, maxTokens });
}

// ── TTS-D / P0：Voice asset registry + Agent voice picker ────────────────

export interface TtsVoiceAsset {
  id: string;
  display_name: string;
  /** "builtin" | "bundled" | "user" */
  kind: string;
  language: string | null;
  description: string | null;
  is_previewable: boolean;
}

/** 列出所有可用 voice assets（builtin manifest 18 + bundled mp3/wav + user uploaded）。 */
export async function ttsListVoiceAssets(): Promise<TtsVoiceAsset[]> {
  return invoke<TtsVoiceAsset[]>("tts_list_voice_assets");
}

/** 获取某个 voice 的原始音频文件 base64（前端 <audio> 试听原始 prompt 声音）。 */
export async function ttsVoiceAudio(
  voiceId: string,
): Promise<TtsDemoAudioResponse> {
  return invoke<TtsDemoAudioResponse>("tts_voice_audio", { voiceId });
}

/** 用某个 voice 合成预览文本（默认"你好，我是 X。"），返回 base64 WAV。 */
export async function ttsPreviewVoice(
  voiceId: string,
  sampleText?: string,
): Promise<TtsSynthesisResponse> {
  return invoke<TtsSynthesisResponse>("tts_preview_voice", {
    voiceId,
    sampleText: sampleText ?? null,
  });
}

// ── Phase TTS-E.1：User-uploaded custom voices ──────────────────────────

export interface TtsUploadVoiceResponse {
  asset: TtsVoiceAsset;
  saved_path: string;
}

/** 上传一段自定义声纹文件（wav/mp3/flac/ogg/m4a，≤30MB）。 */
export async function ttsUploadUserVoice(
  fileName: string,
  fileBytes: Uint8Array,
  displayName?: string,
): Promise<TtsUploadVoiceResponse> {
  // base64 编码（浏览器原生 btoa 只能搞 ASCII，二进制要走 binary string）
  let binary = "";
  const len = fileBytes.byteLength;
  const chunkSize = 0x8000;
  for (let i = 0; i < len; i += chunkSize) {
    const sub = fileBytes.subarray(i, Math.min(i + chunkSize, len));
    binary += String.fromCharCode.apply(null, Array.from(sub));
  }
  const base64 = btoa(binary);
  return invoke<TtsUploadVoiceResponse>("tts_upload_user_voice", {
    fileName,
    fileBytesBase64: base64,
    displayName: displayName ?? null,
  });
}

/** 删除用户上传的自定义声纹（仅 user kind 可删）。 */
export async function ttsDeleteUserVoice(voiceId: string): Promise<void> {
  return invoke<void>("tts_delete_user_voice", { voiceId });
}

/** 重命名用户上传的声纹（display_name only，voice_id 不变）。 */
export async function ttsRenameUserVoice(
  voiceId: string,
  newDisplayName: string,
): Promise<void> {
  return invoke<void>("tts_rename_user_voice", { voiceId, newDisplayName });
}

/** Phase TTS-E / P2：后台预合成 voice preview WAV（fire-and-forget）。 */
export async function ttsWarmVoicePreview(voiceId: string): Promise<string> {
  return invoke<string>("tts_warm_voice_preview", { voiceId });
}

/** Phase TTS-E / P2：获取 voice 的预览 WAV（优先缓存，fallback 原声）。 */
export async function ttsCachedVoicePreview(
  voiceId: string,
): Promise<TtsDemoAudioResponse> {
  return invoke<TtsDemoAudioResponse>("tts_cached_voice_preview", { voiceId });
}

// ── STT (OpenFlow / SenseVoice) ───────────────────────────────────────────
//
// Apr 2026: 精简为单 backend = OpenFlow。`whisper` / `groq` provider 字段保留
// 仅为兼容旧 settings 文件，所有调用都会被后端归一化为 openflow。

export interface SttModelStatusResponse {
  /** SenseVoice (OpenFlow) 模型是否已就绪 */
  openflow_ready: boolean;
  /** SenseVoice 模型目录 */
  openflow_model_dir: string;
}

export interface OpenFlowDownloadProgress {
  file: string;
  downloaded: number;
  total: number | null;
  /** 0-100；total=null 时为 -1 */
  percent: number;
}

export interface DownloadOpenflowRequest {
  /** 'quantized' | 'fp16'，默认 quantized */
  preset?: string;
  force?: boolean;
}

export interface SttTranscribeRequest {
  audio_bytes_base64: string;
  language: string | null;
  sample_rate: number | null;
  /** 已废弃：保留字段以兼容旧调用，后端忽略 */
  provider_override?: string | null;
}

export interface SttTranscribeResponse {
  text: string;
  language: string;
  elapsed_seconds: number;
  /** 始终为 "openflow" */
  provider: string;
}

export interface SttSettingsDto {
  /** 始终为 "openflow"（保留字段用于兼容） */
  provider: string;
}

export interface SaveSttSettingsRequest {
  /** 已废弃：后端忽略此字段 */
  provider?: string;
}

export async function sttModelStatus(): Promise<SttModelStatusResponse> {
  return invoke<SttModelStatusResponse>("stt_model_status");
}

/**
 * 下载 SenseVoice (OpenFlow) 模型。
 *
 * 调用前请先订阅 `stt:openflow-download-progress` 事件以接收实时进度。
 * 量化版约 230MB，FP16 版约 450MB。
 */
export async function sttDownloadOpenflowModel(
  request: DownloadOpenflowRequest = {},
): Promise<string> {
  return invoke<string>("stt_download_openflow_model", { request });
}

export async function sttTranscribe(
  request: SttTranscribeRequest,
): Promise<SttTranscribeResponse> {
  return invoke<SttTranscribeResponse>("stt_transcribe", { request });
}

export async function sttGetSettings(): Promise<SttSettingsDto> {
  return invoke<SttSettingsDto>("stt_get_settings");
}

export async function sttSaveSettings(
  request: SaveSttSettingsRequest,
): Promise<SttSettingsDto> {
  return invoke<SttSettingsDto>("stt_save_settings", { request });
}

// ── TTS Model Download ─────────────────────────────────────────────────────

export interface TtsModelFileInfo {
  name: string;
  size: number;
  present: boolean;
}

export interface TtsModelStatusResponse {
  ready: boolean;
  tts_files: TtsModelFileInfo[];
  tokenizer_files: TtsModelFileInfo[];
  total_bytes: number;
  missing_bytes: number;
  cache_dir: string;
}

export interface TtsDownloadStatusResponse {
  is_downloading: boolean;
  percent: number;
  downloaded_bytes: number;
  total_bytes: number;
  current_file: string;
  error: string | null;
}

/** Check TTS model file status. */
export async function ttsModelStatus(): Promise<TtsModelStatusResponse> {
  return invoke<TtsModelStatusResponse>("tts_model_status");
}

/** Start downloading missing TTS model files. */
export async function ttsModelDownloadStart(): Promise<void> {
  return invoke<void>("tts_model_download_start");
}

/** Get current download progress. */
export async function ttsModelDownloadStatus(): Promise<TtsDownloadStatusResponse> {
  return invoke<TtsDownloadStatusResponse>("tts_model_download_status");
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase M5 closeout — Strategy diagnostics IPC bindings
//
// Wire-stable shapes mirroring the Rust types in
// `src-tauri/src/modules/learning/strategy_registry.rs` and friends.
// Kept as plain TypeScript records (no runtime validation) — the
// page just renders what the backend hands back.
// ─────────────────────────────────────────────────────────────────────────────

export type LearningRolloutState =
  | "draft"
  | "candidate"
  | "compared"
  | "recommended"
  | "promotion_ready"
  | "promotion_blocked"
  | "promoted_candidate"
  | "active"
  | "rolled_back"
  | "rejected"
  | "deprecated";

export interface LearningStrategyIndexEntry {
  registryVersion: string;
  strategyId: string;
  label: string;
  sourceKind: string;
  rolloutState: LearningRolloutState;
  createdAt: string;
  updatedAt: string;
  hasCompareRef: boolean;
  hasRecommendationRef: boolean;
  sizeBytes: number;
  path: string;
}

export interface LearningCompareRef {
  baselineRunId: string;
  candidateRunId: string;
  compareVersion: string;
  recordedAt: string;
}

export interface LearningRecommendationRef {
  gateVersion: string;
  policyId: string;
  decision: string;
  reasonCodes: string[];
  summary: string;
  recordedAt: string;
}

export interface LearningSuiteEvaluationRef {
  suiteId: string;
  corpusName: string;
  corpusVersion: string;
  suiteReportVersion: string;
  suiteGrade: string;
  taskCount: number;
  regressionCount: number;
  recordedAt: string;
}

export interface LearningActivationAudit {
  activatedAt: string;
  activatedBy: string;
  sourcePolicyId?: string | null;
  sourceGateVersion?: string | null;
  note?: string | null;
}

export interface LearningRollbackAudit {
  rolledBackAt: string;
  initiatedBy: string;
  reason: string;
  target?:
    | { kind: "candidate"; strategyId: string }
    | { kind: "baseline_policy"; policyVersion: string }
    | { kind: "other"; description: string }
    | null;
}

export interface LearningSupersedeRecord {
  supersededBy: string;
  supersededAt: string;
}

export type LearningStrategyDefinition =
  | { kind: "noop" }
  | { kind: "prompt_overlay"; text: string }
  | { kind: "discourage_tool"; toolName: string };

export interface LearningCandidateStrategy {
  registryVersion: string;
  identity: {
    strategyId: string;
    label: string;
    policyVersion?: string | null;
    definitionRef?: string | null;
  };
  source:
    | { kind: "reflection"; noteId: string }
    | { kind: "manual" }
    | { kind: "curated_rule"; ruleId?: string | null }
    | { kind: "other"; summary: string };
  rolloutState: LearningRolloutState;
  createdAt: string;
  updatedAt: string;
  basedOnReflectionNote?: string | null;
  compareTarget?: {
    baselineRunId?: string | null;
    candidateRunId?: string | null;
  } | null;
  lastCompareRef?: LearningCompareRef | null;
  lastRecommendationRef?: LearningRecommendationRef | null;
  lastSuiteEvaluationRef?: LearningSuiteEvaluationRef | null;
  lastSuiteRecommendationRef?: LearningRecommendationRef | null;
  activationAudit?: LearningActivationAudit | null;
  rollbackAudit?: LearningRollbackAudit | null;
  definition: LearningStrategyDefinition;
  supersededBy?: LearningSupersedeRecord | null;
  compareHistory?: LearningCompareRef[];
  recommendationHistory?: LearningRecommendationRef[];
  suiteEvaluationHistory?: LearningSuiteEvaluationRef[];
  suiteRecommendationHistory?: LearningRecommendationRef[];
  activationHistory?: LearningActivationAudit[];
  rollbackHistory?: LearningRollbackAudit[];
  notes?: string | null;
}

export interface LearningActiveStrategyEffect {
  strategyId: string;
  label: string;
  kind: string;
  promptOverlay: string;
}

export interface LearningActiveStrategyOverlay {
  overlayVersion: string;
  effects: LearningActiveStrategyEffect[];
}

export async function learningListCandidates(): Promise<
  LearningStrategyIndexEntry[]
> {
  return invoke<LearningStrategyIndexEntry[]>("learning_list_candidates");
}

export async function learningGetCandidate(
  strategyId: string,
): Promise<LearningCandidateStrategy | null> {
  return invoke<LearningCandidateStrategy | null>("learning_get_candidate", {
    strategyId,
  });
}

export async function learningGetActiveStrategies(): Promise<
  LearningCandidateStrategy[]
> {
  return invoke<LearningCandidateStrategy[]>("learning_get_active_strategies");
}

export async function learningResolveActiveOverlay(): Promise<LearningActiveStrategyOverlay> {
  return invoke<LearningActiveStrategyOverlay>(
    "learning_resolve_active_overlay",
  );
}

// ─── DayDream IPC (Memory Consolidation) ─────────────────────────────────────

/**
 * Serde externally-tagged enum from Rust:
 * - `"Idle"` / `"Disabled"` for unit variants
 * - `{ "Running": { "started_at": "..." } }` for struct variants
 */
export type DayDreamState =
  | "Idle"
  | "Disabled"
  | { Running: { started_at: string } }
  | { Completed: { finished_at: string } };

/** Consolidation aggressiveness — Rust enum unit variants serialize as strings. */
export type ConsolidationStrategy = "Conservative" | "Balanced" | "Aggressive";

export interface DayDreamConfig {
  enabled: boolean;
  idle_trigger_minutes: number;
  session_end_trigger: boolean;
  max_entries_per_cycle: number;
  llm_budget_tokens: number;
  strategy: ConsolidationStrategy;
}

export interface PruneReport {
  scanned: number;
  pruned: number;
  reasons: string[];
}

export interface MergeReport {
  clusters_found: number;
  merged: number;
  entries_consumed: number;
}

export interface RefreshReport {
  candidates: number;
  refreshed: number;
  unchanged: number;
}

export interface DayDreamReport {
  cycle_id: string;
  started_at: string;
  finished_at: string;
  duration_ms: number;
  prune: PruneReport;
  merge: MergeReport;
  refresh: RefreshReport;
  strategy: ConsolidationStrategy;
  errors: string[];
}

// ── Helper to normalize DayDreamState for display ─────────────────────────────

export type DayDreamStateKind = "Idle" | "Running" | "Completed" | "Disabled";

export function parseDayDreamState(raw: DayDreamState): {
  kind: DayDreamStateKind;
  startedAt?: string;
  finishedAt?: string;
} {
  if (raw === "Idle") return { kind: "Idle" };
  if (raw === "Disabled") return { kind: "Disabled" };
  if (typeof raw === "object" && "Running" in raw) {
    return { kind: "Running", startedAt: raw.Running.started_at };
  }
  if (typeof raw === "object" && "Completed" in raw) {
    return { kind: "Completed", finishedAt: raw.Completed.finished_at };
  }
  return { kind: "Idle" };
}

/** 获取 DayDream 当前运行状态。 */
export async function daydreamStatus(): Promise<DayDreamState> {
  return invoke<DayDreamState>("daydream_status");
}

/** 手动触发一次 DayDream 巩固周期。 */
export async function daydreamTrigger(): Promise<DayDreamReport> {
  return invoke<DayDreamReport>("daydream_trigger");
}

/** 获取 DayDream 配置。 */
export async function daydreamConfigGet(): Promise<DayDreamConfig> {
  return invoke<DayDreamConfig>("daydream_config_get");
}

/** 更新 DayDream 配置。 */
export async function daydreamConfigSet(config: DayDreamConfig): Promise<void> {
  await invoke<void>("daydream_config_set", { config });
}

/** 获取 DayDream 历史报告（最近 50 条）。 */
export async function daydreamHistory(): Promise<DayDreamReport[]> {
  return invoke<DayDreamReport[]>("daydream_history");
}

// ─── Memory Graph IPC ───────────────────────────────────────────────────────────────

/** A typed link between two memory entries. */
export interface MemoryLinkDto {
  source_key: string;
  target_key: string;
  link_type: string;
  created_at: string;
}

/** Minimal entry DTO carried inside graph nodes. */
export interface GraphEntryDto {
  key: string;
  content: string;
  category: string;
  importance: number;
  trust_score: number;
  created_at: string;
  cognitive_layer: number;
}

/** A node discovered during BFS graph traversal. */
export interface GraphNodeDto {
  key: string;
  depth: number;
  entry?: GraphEntryDto;
  links: MemoryLinkDto[];
}

/** Structured neighborhood of a single memory node. */
export interface GraphNeighborhoodDto {
  center: GraphEntryDto;
  incoming: [MemoryLinkDto, GraphEntryDto][];
  outgoing: [MemoryLinkDto, GraphEntryDto][];
  by_link_type: Record<string, string[]>;
}

/** BFS graph traversal from a seed key. */
export async function memoryGraphTraverse(
  seedKey: string,
  depth?: number,
  linkTypes?: string[],
): Promise<GraphNodeDto[]> {
  return invoke<GraphNodeDto[]>("memory_graph_traverse", {
    seedKey,
    depth: depth ?? null,
    linkTypes: linkTypes ?? null,
  });
}

/** Get the structured neighborhood of a memory node. */
export async function memoryGraphNeighborhood(
  key: string,
  radius?: number,
): Promise<GraphNeighborhoodDto> {
  return invoke<GraphNeighborhoodDto>("memory_graph_neighborhood", {
    key,
    radius: radius ?? null,
  });
}

/** Discover new links between memories in a scope. */
export async function memoryGraphDiscover(args?: {
  scopeKind?: string;
  sessionId?: string;
  projectId?: string;
}): Promise<MemoryLinkDto[]> {
  return invoke<MemoryLinkDto[]>("memory_graph_discover", {
    scopeKind: args?.scopeKind ?? null,
    sessionId: args?.sessionId ?? null,
    projectId: args?.projectId ?? null,
  });
}

/** Full graph DTO containing all memory nodes and their links. */
export interface FullGraphDto {
  nodes: GraphEntryDto[];
  links: MemoryLinkDto[];
}

/** Load the full memory graph — all nodes and all links. */
export async function memoryGraphFull(): Promise<FullGraphDto> {
  return invoke<FullGraphDto>("memory_graph_full");
}

// ─── Evolution IPC (Agent Self-Improvement) ──────────────────────────────────

/** A single tool call recorded within a turn. */
export interface ToolCallRecord {
  tool_name: string;
  args_summary: string;
  success: boolean;
  duration_ms: number;
  error_message: string | null;
}

/** A single turn within an agent trajectory. */
export interface TurnRecord {
  turn_id: number;
  timestamp: string;
  user_input_summary: string;
  agent_action: string; // "Reply" | "ToolUse" | "Reasoning" | "Error"
  tool_calls: ToolCallRecord[];
  success: boolean;
  self_assessment: string | null;
}

/** Full trajectory DTO from backend. */
export interface TrajectoryDto {
  trajectory_id: string;
  session_id: string;
  task_description: string;
  turns: TurnRecord[];
  outcome: string; // JSON serialized TaskOutcome
  started_at: string;
  finished_at: string | null;
  duration_secs: number;
  tool_usage: Record<string, number>;
  error_count: number;
}

/** Insight DTO — an extracted insight from self-reflection. */
export interface InsightDto {
  id: string;
  category: string; // "HeuristicRule" | "AntiPattern" | "BestPractice" | "UserPreference" | "ToolUsagePattern"
  content: string;
  confidence: number;
  applicable_contexts: string[];
  source_trajectory_ids: string[];
  created_at: string;
}

/** DayDream reflection report summary. */
export interface DayDreamReflectionReport {
  trajectories_analyzed: number;
  insights_extracted: number;
  procedures_created: number;
  procedures_updated: number;
  patterns_found: number;
}

/** Insight category string union for display. */
export type InsightCategory =
  | "HeuristicRule"
  | "AntiPattern"
  | "BestPractice"
  | "UserPreference"
  | "ToolUsagePattern";
