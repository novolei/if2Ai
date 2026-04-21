# 智能体模块（Agent）

> If2Ai 的对话运行时——ConversationRuntime 驱动的 Agent 循环、权限控制与流式输出

## 📚 文档目录

| 文件 | 说明 |
|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 面向用户的使用指南 |
| [02-implementation.md](./02-implementation.md) | 面向开发者的实现原理 |
| [03-prompt-builder.md](./03-prompt-builder.md) | Prompt 构建深度解析 |

## 📍 核心概念速查表

| 概念 | 说明 |
|------|------|
| **ConversationRuntime** | 核心对话运行时，管理 Agent 循环（~11,500 行核心代码） |
| **TurnService** | 轮次服务，组合提供商解析 + 记忆注入 + 提示词规划 |
| **PromptPlanner** | 提示词规划器，构建系统提示词块序列 |
| **PermissionPolicy** | 权限策略，5 种模式（ReadOnly/WorkspaceWrite/DangerFullAccess/Prompt/Allow） |
| **PermissionPrompter** | 权限提示器，交互式审批工具调用 |
| **TurnHook** | 轮次钩子 trait，用于记忆子系统与 Agent 循环的集成 |
| **WorkingMemory** | 工作记忆，滑动窗口过滤发送给 LLM 的消息 |
| **UsageTracker** | 用量追踪器，累计 token 使用量 |
| **ContextBudget** | 上下文预算，限制单轮 token 消耗 |
| **Session** | 会话管理，消息持久化与恢复 |
| **StreamEmitter** | 流式发射器，Tauri event → 前端运行时投影 |
| **ToolRegistryExecutor** | 工具执行桥接，async ToolRegistry → sync ToolExecutor trait |
| **MemoryInjectionService** | 记忆注入服务，Pinned → Compiled → Rules → Retrieved |

## 🏗️ 源码位置

| 组件 | 路径 |
|------|------|
| 对话运行时 | `src-tauri/src/modules/runtime/conversation.rs` |
| 轮次服务 | `src-tauri/src/modules/application/turn_service.rs` |
| 提示词规划器 | `src-tauri/src/modules/application/prompt_planner/mod.rs` |
| 权限管理 | `src-tauri/src/modules/runtime/permissions.rs` |
| 会话管理 | `src-tauri/src/modules/runtime/session.rs` |
| 流式发射器 | `src-tauri/src/modules/runtime/stream_emitter.rs` |
| 工具执行器 | `src-tauri/src/modules/application/tool_executor.rs` |
| Agent 命令 | `src-tauri/src/commands/agent.rs` |
| 记忆注入 | `src-tauri/src/modules/application/memory_injection_service.rs` |
| 工作记忆 | `src-tauri/src/modules/memory/working_memory.rs` |
| 请求智能 | `src-tauri/src/modules/application/request_intelligence.rs` |
| MCP 集成 | `src-tauri/src/modules/channel/` |

## 🔗 相关资源

- [Agent Loop 设计文档](../../design-docs/agent-loop.md)
- [Agent Orchestrator](../../design-docs/agent-orchestrator.md)
- [错误处理](../../design-docs/error-handling.md)
- [提示词构建](../../design-docs/prompt-builder.md)
- [提供商解析](../../design-docs/provider-resolution.md)
