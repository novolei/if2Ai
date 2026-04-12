# If2Ai Baseline 调研报告索引

> 调研日期：2026-04-12
> 调研范围：`/Users/ryanliu/Documents/IfAI/if2Ai/rust/crates/`（claw-cli、tools、runtime、api、plugins）

## 报告清单

| 文件 | 内容 |
|------|------|
| [claw-cli-baseline.md](claw-cli-baseline.md) | CLAW-CLI 框架设计报告 — CLI 入口、REPL 循环、LiveCli、Agent Loop 完整流程、配置加载 |
| [tools-baseline.md](tools-baseline.md) | 工具系统设计报告 — 权限系统、沙箱隔离、文件操作、MCP 协议、API Provider 抽象、上下文压缩 |
| [gap-analysis-phase4.md](gap-analysis-phase4.md) | Phase 4 与 Baseline 的 Gap 分析 — 10 个关键 Gap 及修复优先级建议 |

## 快速摘要

### Baseline 架构（原有 CLAW-CLI）

```
claw-cli (main.rs REPL)
  ├─ ConversationRuntime (runtime/conversation.rs)
  │    ├─ DefaultRuntimeClient → ClawApiClient (Anthropic API)
  │    ├─ CliToolExecutor → GlobalToolRegistry → tools crate
  │    ├─ PermissionPolicy.authorize() → 5级权限检查
  │    ├─ HookRunner (Pre/Post ToolUse)
  │    └─ 工具执行后 tool_result 入 session.messages → LOOP
  ├─ SystemPromptBuilder (动态分界线机制)
  ├─ ConfigLoader (多层配置文件 deep merge)
  ├─ McpServerManager (Stdio MCP 服务器生命周期)
  ├─ SandboxConfig (Linux namespace unshare 沙箱)
  ├─ CompactionEngine (上下文压缩)
  └─ PluginManager (动态插件加载)
```

### Phase 4 当前实现

```
Tauri Commands (src-tauri/)
  ├─ run_agent_turn → ConversationRuntime (复用)
  │    ├─ MockApiClient（⚠️ 非真实 LLM）
  │    ├─ ToolRegistryExecutor bridge
  │    └─ ToolRegistry (19个工具已注册)
  ├─ execute_tool (Tauri command)
  │    └─ ToolRegistry.dispatch() → 各工具 handler
  ├─ 工具：bash, file_read, json_parse, file_write,
  │        glob_search, content_search, file_edit(stub),
  │        web_fetch, web_search, http_request,
  │        memory_*(5), cron_*(5)
  ├─ ToolContext (workdir + permission_mode)
  ├─ ToolSet 分类（files/web/memory/cron）
  └─ MemoryProvider + Scheduler trait
```

### 关键 Gap

| 优先级 | Gap |
|--------|-----|
| 🔴 高 | LLM Provider 为 Mock，Agent 无法真实工作 |
| 🔴 高 | 权限系统在 dispatch() 中未生效 |
| 🟡 中 | 沙箱隔离缺失（bash 仅靠 cd 限制） |
| 🟡 中 | 工具结果循环未完全打通 |
| 🟡 中 | 上下文压缩触发逻辑不精确 |
| 🟢 低 | MCP 协议缺失 |
| 🟢 低 | 插件系统缺失 |

## 文件路径速查

### Baseline 关键文件
- `rust/crates/claw-cli/src/main.rs` — CLI 入口、LiveCli、REPL、build_runtime
- `rust/crates/runtime/src/conversation.rs` — ConversationRuntime、Agent Loop 状态机
- `rust/crates/runtime/src/permissions.rs` — PermissionPolicy、PermissionMode
- `rust/crates/runtime/src/sandbox.rs` — SandboxConfig、Linux namespace
- `rust/crates/runtime/src/bash.rs` — execute_bash
- `rust/crates/runtime/src/file_ops.rs` — 文件操作工具
- `rust/crates/runtime/src/mcp_stdio.rs` — McpServerManager
- `rust/crates/tools/src/lib.rs` — GlobalToolRegistry、mvp_tool_specs
- `rust/crates/api/src/client.rs` — ProviderClient 路由
- `rust/crates/api/src/providers/claw_provider.rs` — Anthropic API
- `rust/crates/api/src/providers/openai_compat.rs` — OpenAI/xAI 兼容
- `rust/crates/runtime/src/compact.rs` — 上下文压缩
- `rust/crates/runtime/src/prompt.rs` — SystemPromptBuilder

### Phase 4 实现文件
- `src-tauri/src/modules/tools/mod.rs` — register_builtin_tools
- `src-tauri/src/modules/tools/context.rs` — ToolContext / SharedToolContext
- `src-tauri/src/modules/tools/registry.rs` — ToolRegistry
- `src-tauri/src/modules/tools/toolset.rs` — ToolSet / ToolSetRegistry / TOOLSETS
- `src-tauri/src/modules/tools/builtin/*.rs` — 19 个工具实现
- `src-tauri/src/modules/commands/tools.rs` — execute_tool Tauri command
- `src-tauri/src/modules/runtime/mod.rs` — ConversationRuntime 复用
- `src-tauri/src/modules/memory/mod.rs` — MemoryProvider
- `src-tauri/src/modules/scheduler/mod.rs` — Scheduler
