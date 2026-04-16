//! Tauri IPC 封装层
//!
//! 所有前端与 Tauri 后端的交互都通过这个模块，
//! App.tsx 不直接调用 @tauri-apps/api。

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// Re-export invoke for App.tsx stopAgentStream
export { invoke };

/**
 * 流式 Token 事件载荷
 */
export interface StreamTokenPayload {
  stream_id: string;
  text?: string;
  thinking?: string;
  event_type:
    | 'text_delta'
    | 'thinking_delta'
    | 'thinking_start'
    | 'tool_call_update'
    | 'final_text_override'
    | 'stream_complete'
    | 'stream_error';
  // tool_call_update 专用字段
  tool_call_id?: string;
  tool_name?: string;
  tool_status?: 'queued' | 'running' | 'completed' | 'error';
  tool_args?: Record<string, unknown>;
  tool_result?: string;
  tool_duration_ms?: number;
  effective_workdir?: string;
  policy_decision?: 'allow' | 'deny' | 'prompt';
  evidence_id?: string;
  request_id?: string;
  task_outcome?: 'completed' | 'partial_success' | 'failed';
  degraded_reason?: string;
  resume_available?: boolean;
  resume_cursor?: string;
}

/**
 * 权限请求事件载荷（后端 permission-request 事件）
 */
export interface PermissionRequestPayload {
  session_id: string;
  tool_name: string;
  permission_mode: string;
  current_mode: string;
  message: string;
}

/**
 * Agent 权限模式（映射后端 PermissionMode）
 */
export type PermissionMode = 'readOnly' | 'workspaceWrite' | 'dangerFullAccess'

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
  created_at: string;
  updated_at: string;
  pinned: boolean;
  message_count: number;
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
  kind: 'folder' | 'file';
  modified_ms?: number | null;
}

export interface FilePreviewPayload {
  name: string;
  path: string;
  kind: 'markdown' | 'code' | 'image' | 'pdf' | 'video' | 'html';
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
  permissionMode?: PermissionMode
): Promise<AgentTurnResponse> {
  return await invoke<AgentTurnResponse>('run_agent_turn', {
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
  permissionMode?: PermissionMode
): Promise<string> {
  return await invoke<string>('start_agent_stream', {
    sessionId,
    userMessage,
    permissionMode,
  });
}

/**
 * 监听流式 Token 事件
 *
 * @param streamId - 流 ID，用于过滤事件
 * @param callback - 回调函数，接收 Token 事件
 * @returns 取消监听函数
 */
export async function listenToStream(
  streamId: string,
  callback: (payload: StreamTokenPayload) => void
): Promise<UnlistenFn> {
  return await listen<StreamTokenPayload>('agent-token', (event) => {
    if (event.payload.stream_id === streamId) {
      callback(event.payload);
    }
  });
}

/**
 * 监听后端权限请求事件
 */
export async function listenToPermissionRequests(
  callback: (payload: PermissionRequestPayload) => void
): Promise<UnlistenFn> {
  return await listen<PermissionRequestPayload>('permission-request', (event) => {
    callback(event.payload)
  })
}

/**
 * 响应后端权限请求（allow / deny）
 */
export async function respondPermission(
  sessionId: string,
  decision: 'allow' | 'deny',
  options?: { toolName?: string; scope?: 'once' | 'session' }
): Promise<void> {
  return await invoke<void>('respond_permission', {
    sessionId,
    decision,
    toolName: options?.toolName,
    scope: options?.scope,
  })
}

/**
 * 列出所有会话
 *
 * @returns 会话列表
 */
export async function listSessions(): Promise<SessionMeta[]> {
  return await invoke<SessionMeta[]>('list_sessions');
}

/**
 * 删除指定会话
 *
 * @param id - 要删除的会话 ID
 */
export async function deleteSession(id: string): Promise<void> {
  return await invoke<void>('delete_session', { id });
}

/**
 * 设置会话置顶状态
 *
 * @param id - 会话 ID
 * @param pinned - 是否置顶
 */
export async function setSessionPinned(id: string, pinned: boolean): Promise<void> {
  return await invoke<void>('set_session_pinned', { id, pinned });
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
  workdir: string
): Promise<Project> {
  return await invoke<Project>('create_project', { name, workdir });
}

/**
 * 列出所有项目
 *
 * @returns 项目列表
 */
export async function listProjects(): Promise<ProjectMeta[]> {
  return await invoke<ProjectMeta[]>('list_projects');
}

/**
 * 获取指定项目
 *
 * @param id - 项目 ID
 * @returns 项目信息
 */
export async function getProject(id: string): Promise<Project> {
  return await invoke<Project>('get_project', { id });
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
  newName: string
): Promise<Project> {
  return await invoke<Project>('rename_project', { id, newName });
}

/**
 * 删除指定项目
 *
 * @param id - 要删除的项目 ID
 */
export async function deleteProject(id: string): Promise<void> {
  return await invoke<void>('delete_project', { id });
}

/**
 * 在系统文件管理器中打开指定项目
 *
 * @param id - 项目 ID
 */
export async function openProjectInFinder(id: string): Promise<void> {
  return await invoke<void>('open_project_in_finder', { id });
}

export async function openDirectoryPath(path: string): Promise<void> {
  return await invoke<void>('open_directory_path', { path });
}

export async function listDirectoryPreview(
  path: string,
  limit = 32
): Promise<DirectoryEntryPreview[]> {
  return await invoke<DirectoryEntryPreview[]>('list_directory_preview', { path, limit });
}

export async function readFilePreview(
  path: string,
  maxBytes = 128 * 1024
): Promise<FilePreviewPayload> {
  return await invoke<FilePreviewPayload>('read_file_preview', { path, maxBytes });
}

export async function writeFileContents(
  path: string,
  content: string
): Promise<void> {
  return await invoke<void>('write_file_contents', { path, content });
}

/**
 * 为项目创建永久工作树
 *
 * @param id - 项目 ID
 * @returns 创建后的工作树路径
 */
export async function createPermanentWorktree(id: string): Promise<string> {
  return await invoke<string>('create_permanent_worktree', { id });
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
  title: string
): Promise<SessionMeta> {
  return await invoke<SessionMeta>('create_session', { projectId, title });
}

/**
 * 更新会话标题
 */
export async function renameSession(
  id: string,
  title: string
): Promise<SessionMeta> {
  return await invoke<SessionMeta>('rename_session', { id, title });
}

/**
 * 列出指定项目中的所有会话
 *
 * @param projectId - 项目 ID
 * @returns 会话列表
 */
export async function listProjectSessions(
  projectId: string
): Promise<SessionMeta[]> {
  return await invoke<SessionMeta[]>('list_project_sessions', { projectId });
}

/**
 * 打开设置窗口
 */
export async function openSettingsWindow(): Promise<void> {
  return await invoke<void>('open_settings_window');
}

/**
 * 关闭设置窗口
 */
export async function closeSettingsWindow(): Promise<void> {
  return await invoke<void>('close_settings_window');
}

export interface ChatPrefillPayload {
  prompt: string
}

export async function focusMainWindowAndPrefillPrompt(prompt: string): Promise<void> {
  return await invoke<void>('focus_main_window_and_prefill_prompt', { prompt })
}

export async function listenToChatPrefill(
  callback: (payload: ChatPrefillPayload) => void
): Promise<UnlistenFn> {
  return await listen<ChatPrefillPayload>('if2ai-chat-prefill', (event) => {
    callback(event.payload)
  })
}

/**
 * 获取会话的完整信息（包括消息历史）
 *
 * @param id - 会话 ID
 * @returns 完整的会话信息
 */
export async function getSession(
  id: string
): Promise<Session> {
  return await invoke<Session>('get_session', { id });
}

/**
 * 会话信息（包含消息）
 */
export interface Session {
  id: string
  project_id: string
  title: string
  messages: ConversationMessage[]
  created_at: string
  updated_at: string
  token_count: number
}

/**
 * 对话消息
 */
export interface ConversationMessage {
  role: 'system' | 'user' | 'assistant' | 'tool'
  blocks: ContentBlock[]
  usage?: TokenUsage
  thinking?: string
  task_outcome?: 'completed' | 'partial_success' | 'failed'
  degraded_reason?: string
  resume_available?: boolean
  resume_cursor?: string
  request_id?: string
}

/**
 * 内容块
 */
export interface ContentBlock {
  type: 'text' | 'tool_use' | 'tool_result'
  text?: string
  tool_use_id?: string
  tool_name?: string
  input?: string
  output?: string
  tool_use_block?: {
    id: string
    name: string
    input: Record<string, unknown>
  }
}

/**
 * Token 使用量
 */
export interface TokenUsage {
  input_tokens: number
  output_tokens: number
  cache_creation_input_tokens?: number
  cache_read_input_tokens?: number
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
  sessionId?: string
): Promise<ToolCallResult> {
  try {
    const result = await invoke<string>('execute_tool', {
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
  return invoke<ToolDefinition[]>('list_tools');
}

/**
 * 获取工具定义（OpenAI function calling 格式）
 *
 * @param allowed - 可选的允许工具名称列表
 * @returns 工具定义列表
 */
export async function getToolDefinitions(
  allowed?: string[]
): Promise<object[]> {
  return invoke<object[]>('get_tool_definitions', { allowed });
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
  return invoke<ToolSet[]>('list_toolsets');
}

/**
 * Suggest slash commands based on user input prefix.
 *
 * @param input - The slash command prefix typed by user
 * @param limit - Maximum number of suggestions
 * @returns List of full command strings
 */
export async function suggestSlashCommands(input: string, limit = 8): Promise<string[]> {
  return invoke<string[]>('suggest_slash_commands', { input, limit });
}

/**
 * Execute a slash command for the given session.
 *
 * @param input - The full slash command string
 * @param sessionId - Current session ID
 * @returns Result message from command execution
 */
export async function executeSlashCommand(input: string, sessionId: string): Promise<string> {
  return invoke<string>('execute_slash_command', { input, sessionId });
}

/**
 * Resolve a `/skill-name [instruction]` slash input into a full skill invocation
 * message (containing the SKILL.md content) that can be sent directly to the agent.
 * Returns null if the input does not match any installed skill.
 */
export async function resolveSkillSlash(
  input: string,
  cwd?: string
): Promise<string | null> {
  return invoke<string | null>('resolve_skill_slash', { input, cwd: cwd ?? null })
}

export interface SkillInfo {
  name: string
  description: string
  path: string
  source: 'workspace' | 'user' | 'builtin' | 'remote-quarantine'
  review_status: 'draft' | 'quarantine' | 'review_passed' | 'active' | 'disabled'
  status: 'draft' | 'quarantine' | 'review_passed' | 'active' | 'disabled'
  enabled: boolean
  read_only: boolean
  shadowed_by?: string
}

export async function listSkills(cwd?: string): Promise<SkillInfo[]> {
  return invoke<SkillInfo[]>('list_skills', { cwd })
}

export async function setSkillEnabled(
  skillPath: string,
  enabled: boolean,
  sessionId = '__settings__'
): Promise<string> {
  const action = enabled ? 'enable-path' : 'disable-path'
  return executeSlashCommand(`/skills ${action} ${skillPath}`, sessionId)
}

export async function createSkillDraft(
  skillName: string,
  sessionId = '__settings__'
): Promise<string> {
  return executeSlashCommand(`/skills create ${skillName}`, sessionId)
}

export async function reviewSkillDraft(
  skillPath: string,
  sessionId = '__settings__'
): Promise<string> {
  return executeSlashCommand(`/skills review-path ${skillPath}`, sessionId)
}

export async function approveSkillProposal(
  skillPath: string,
  sessionId = '__settings__'
): Promise<string> {
  return executeSlashCommand(`/skills approve-path ${skillPath}`, sessionId)
}

export async function rollbackSkillProposal(
  skillPath: string,
  sessionId = '__settings__'
): Promise<string> {
  return executeSlashCommand(`/skills rollback-path ${skillPath}`, sessionId)
}

export interface SkillDistributionRequest {
  skillName: string
  url: string
  channel: 'stable' | 'canary'
  checksum: string
  signature: string
}

export interface SkillsMarketAuditItem {
  skill: string
  repo: string
  gen: string
  socketAlerts: string
  snykRisk: string
}

export async function fetchSkillsMarketAudits(): Promise<SkillsMarketAuditItem[]> {
  return invoke<SkillsMarketAuditItem[]>('fetch_skills_market_audits')
}

export interface HubInstallResult {
  success: boolean
  message: string
  skill_name?: string
}

/** One skill result from hub_browse / hub_search. */
export interface HubSkillResult {
  name: string
  description: string
  source: string
  identifier: string
  trust_level: string
  tags: string[]
}

export interface HubCommandResult {
  success: boolean
  message: string
  data?: HubSkillResult[]
}

/** Install a skill from any hub source via the Rust pipeline. */
export async function hubInstall(
  sourceId: string,
  identifier: string,
  skillsDir?: string
): Promise<HubInstallResult> {
  return invoke<HubInstallResult>('hub_install', {
    sourceId,
    identifier,
    skillsDir: skillsDir ?? null,
  })
}

/** Browse skills from all hub sources (empty query returns defaults per source). */
export async function hubBrowse(
  sourceFilter?: string,
  limit?: number
): Promise<HubCommandResult> {
  const result = await invoke<{ success: boolean; message: string; data?: unknown }>(
    'hub_browse',
    { sourceFilter: sourceFilter ?? null, limit: limit ?? 50 }
  )
  return {
    success: result.success,
    message: result.message,
    data: Array.isArray(result.data) ? (result.data as HubSkillResult[]) : [],
  }
}

/** Search skills across all hub sources. */
export async function hubSearch(
  query: string,
  sourceFilter?: string,
  limit?: number
): Promise<HubCommandResult> {
  const result = await invoke<{ success: boolean; message: string; data?: unknown }>(
    'hub_search',
    { query, sourceFilter: sourceFilter ?? null, limit: limit ?? 30 }
  )
  return {
    success: result.success,
    message: result.message,
    data: Array.isArray(result.data) ? (result.data as HubSkillResult[]) : [],
  }
}

async function sha256Hex(text: string): Promise<string> {
  const data = new TextEncoder().encode(text)
  const hash = await crypto.subtle.digest('SHA-256', data)
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

export async function installSkillFromDistribution(
  request: SkillDistributionRequest,
  sessionId = '__settings__'
): Promise<string> {
  let effectiveSessionId = sessionId
  if (effectiveSessionId === '__settings__') {
    const projects = await listProjects()
    if (projects.length === 0) {
      throw new Error('No project available. Please create a project first.')
    }
    const tempSession = await createSession(projects[0].id, 'Skills Market Install')
    effectiveSessionId = tempSession.id
  }
  // Fail-closed: signature and checksum are required for remote installs
  // When empty, the skill will be placed in quarantine for review
  const requiresQuarantine = !request.signature.trim() || !request.checksum.trim();
  if (requiresQuarantine) {
    console.warn('[Skills Market] No signature/checksum provided, will install to quarantine for review')
  }
  const fetched = await executeTool('web_fetch', { url: request.url }, undefined, effectiveSessionId)
  if (!fetched.success || !fetched.output) {
    throw new Error(fetched.error ?? 'download failed')
  }
  const safeName = request.skillName.replace(/[^a-zA-Z0-9_-]/g, '-')
  const basePath = `.if2ai/skills-quarantine/${safeName}`
  const writeSkill = await executeTool('file_write', {
    path: `${basePath}/SKILL.md`,
    content: fetched.output,
  }, undefined, effectiveSessionId)
  if (!writeSkill.success) {
    throw new Error(writeSkill.error ?? 'write SKILL.md failed')
  }
  const actualChecksum = await sha256Hex(fetched.output)
  // Only verify checksum if one was provided
  if (request.checksum.trim() && request.checksum.toLowerCase() !== actualChecksum.toLowerCase()) {
    throw new Error(`checksum mismatch: expected ${request.checksum}, actual ${actualChecksum}`)
  }
  const writeManifest = await executeTool('file_write', {
    path: `${basePath}/skill.json`,
    content: JSON.stringify(
      {
        id: safeName,
        version: '0.1.0',
        apiVersion: 'v1',
        minAppVersion: '0.1.0',
        capabilities: ['custom'],
        distribution: {
          channel: request.channel,
          checksum: actualChecksum,
          signature: request.signature,
        },
        review: {
          status: 'quarantine',
          riskLevel: 'high',
          lastReviewedAt: '',
        },
      },
      null,
      2
    ),
  }, undefined, effectiveSessionId)
  if (!writeManifest.success) {
    throw new Error(writeManifest.error ?? 'write skill.json failed')
  }
  return `downloaded to quarantine: ${basePath}`
}

// ─── Web Search Configuration ──────────────────────────────────────────────

/** A configured web search provider entry returned by the backend. */
export interface WebSearchProviderEntry {
  id: string
  name: string
  /** Redacted key preview shown in the UI, e.g. "tvly-abc…xyz". */
  key_preview: string | null
  base_url: string | null
  enabled: boolean
}

/** Return all configured providers (keys are redacted). */
export async function getWebSearchConfig(): Promise<WebSearchProviderEntry[]> {
  return invoke<WebSearchProviderEntry[]>('get_web_search_config')
}

/** Add or update a provider.  `api_key` and `base_url` are optional. */
export async function upsertWebSearchProvider(
  id: string,
  name: string,
  apiKey?: string,
  baseUrl?: string,
  enabled?: boolean
): Promise<WebSearchProviderEntry[]> {
  return invoke<WebSearchProviderEntry[]>('upsert_web_search_provider', {
    provider: { id, name, api_key: apiKey ?? null, base_url: baseUrl ?? null, enabled: enabled ?? true },
  })
}

/** Remove a provider by id. */
export async function removeWebSearchProvider(id: string): Promise<WebSearchProviderEntry[]> {
  return invoke<WebSearchProviderEntry[]>('remove_web_search_provider', { id })
}

/** Reorder providers by supplying the new ordered list of ids. */
export async function reorderWebSearchProviders(
  orderedIds: string[]
): Promise<WebSearchProviderEntry[]> {
  return invoke<WebSearchProviderEntry[]>('reorder_web_search_providers', {
    orderedIds,
  })
}

/** Validate an API key / base URL for the given provider. Returns a success message or throws. */
export async function validateWebSearchKey(
  providerId: string,
  apiKey?: string,
  baseUrl?: string
): Promise<string> {
  return invoke<string>('validate_web_search_key', {
    providerId,
    apiKey: apiKey ?? null,
    baseUrl: baseUrl ?? null,
  })
}

// ─── Memory Settings Configuration ─────────────────────────────────────────

/** Memory configuration returned by the backend. */
export interface MemoryConfig {
  total_tokens: number
  system_pct: number
  episodic_pct: number
  semantic_pct: number
  working_pct: number
  trajectory_count: number
}

/** Configuration input to persist. */
export interface MemoryConfigInput {
  total_tokens: number
  system_pct: number
  episodic_pct: number
  semantic_pct: number
  working_pct: number
}

/** Get the current memory configuration. */
export async function getMemoryConfig(): Promise<MemoryConfig> {
  return invoke<MemoryConfig>('get_memory_config')
}

/** Save memory configuration. */
export async function setMemoryConfig(
  config: MemoryConfigInput
): Promise<MemoryConfig> {
  return invoke<MemoryConfig>('set_memory_config', { config })
}

/** Export trajectories to a user-selected directory. */
export async function exportTrajectories(): Promise<string> {
  return invoke<string>('export_trajectories')
}

/** Reset onboarding state and return to Step 1. */
export async function configResetOnboarding(): Promise<void> {
  return invoke<void>('config_reset_onboarding')
}
