//! Tauri IPC 封装层
//!
//! 所有前端与 Tauri 后端的交互都通过这个模块，
//! App.tsx 不直接调用 @tauri-apps/api。

import { invoke } from '@tauri-apps/api/core';

/**
 * 助手消息响应
 */
export interface AgentTurnResponse {
  /** 生成的文本消息 */
  message: string;
  /** 会话 ID */
  session_id: string;
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
