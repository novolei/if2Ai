//! Tauri IPC 封装层
//!
//! 所有前端与 Tauri 后端的交互都通过这个模块，
//! App.tsx 不直接调用 @tauri-apps/api。

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

/**
 * 流式 Token 事件载荷
 */
export interface StreamTokenPayload {
  stream_id: string;
  text?: string;
  thinking?: string;
  event_type: 'text_delta' | 'thinking_delta' | 'thinking_start' | 'stream_complete' | 'stream_error';
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
  created_at: string;
  pinned: boolean;
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

/**
 * 运行一次 Agent 对话轮次
 *
 * @param sessionId - 会话 ID
 * @param userMessage - 用户消息
 * @returns Agent 响应
 */
export async function runAgentTurn(
  sessionId: string,
  userMessage: string
): Promise<AgentTurnResponse> {
  return await invoke<AgentTurnResponse>('run_agent_turn', {
    sessionId,
    userMessage,
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
  userMessage: string
): Promise<string> {
  return await invoke<string>('start_agent_stream', {
    sessionId,
    userMessage,
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
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  try {
    const result = await invoke<string>('execute_tool', {
      name,
      args: JSON.stringify(args),
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
