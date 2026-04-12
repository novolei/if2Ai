# 十、全流程细粒度审计（Comprehensive Node-by-Node Audit）

> 审计日期：2026-04-12
> 范围：CLAW-CLI 全部 crates (claw-cli, commands, runtime, tools, api, plugins) vs If2Ai 桌面端全部前后端代码
> 方法：逐行扫描 + 1:1 流程节点对比 + 二次审查现有 bs_gap 文档

---

## 10.1 执行摘要

在完成对 `rust/` 目录下全部 6 个 crates 和 If2Ai 前后端全部代码的逐行扫描后，本审计报告对现有 bs_gap 报告进行了二次审查，确认了原有发现的正确性，同时补充了 **12 项新的细粒度发现**。

### 原有发现确认（9 份 bs_gap 文档正确性评估）

| 文档 | 正确性 | 补充说明 |
|------|--------|---------|
| 00-index.md | ✅ 完全正确 | 执行摘要准确反映了核心问题 |
| 01-architecture-overview.md | ✅ 完全正确 | 模块映射表精准 |
| 02-agent-loop-gap.md | ✅ 完全正确 | tools:None 和 InputJsonDelta 忽略均已代码验证 |
| 03-tool-system-gap.md | ✅ 正确 | 但需要补充"两套工具系统互不通信"的具体路径 |
| 04-permissions-gap.md | ✅ 完全正确 | 硬编码 DangerFullAccess 和 Prompter=None 已验证 |
| 05-session-gap.md | ✅ 正确 | is_error 字段已确认存在（08 已记录移除） |
| 06-frontend-gap.md | ✅ 正确 | 但遗漏了更多前端空按钮和无效 UI 细节 |
| 07-mcp-plugin-gap.md | ✅ 正确 | Hook 在 run_turn 路径已接入（08 已修正） |
| 08-critical-fix-priority.md | ✅ 正确 | 经过 UI/UX 审查修订，修复方案详尽 |
| 09-skills-commands-gap.md | ✅ 正确 | SlashCommand 28 命令和 Skill 工具差距分析完整 |

### 新发现（本次审计追加）

| 编号 | 发现 | 严重度 | 来源 |
|------|------|--------|------|
| N1 | SystemPromptBuilder 完全未使用，硬编码中文 prompt 替代 | 🟠 严重 | 代码对比 |
| N2 | RealApiClient 的 `call_api()` 内部已获取工具定义但架构不干净 | 🟡 中（已知但需量化） | 代码对比 |
| N3 | 两套工具系统共存且互不通信（ToolRegistry vs GlobalToolRegistry） | 🟠 严重 | 架构分析 |
| N4 | `commands/mod.rs` 未导入 `commands/lib.rs` 的 SlashCommand 内容 | 🟡 中 | 代码验证 |
| N5 | `start_agent_stream` 的 `system: None` — 无系统提示 | 🟠 严重 | agent.rs:572 |
| N6 | `run_agent_turn` 的 error 消息使用硬编码中文字符串 | 🟢 低 | agent.rs:346-348 |
| N7 | `GlobalNavbar.tsx` 中 Automation/Skills 图标区完全无功能 | 🟡 中 | 前端代码 |
| N8 | `WorkbenchBackdrop.tsx` 纯装饰组件，无实际功能 | 🟢 低 | 前端代码 |
| N9 | `SessionStatus.tsx` 已定义但从未被任何组件 import | 🟡 中 | grep 验证 |
| N10 | 前端存在两个 ChatUI 入口（ChatWorkspace + App.tsx 直接引用） | 🟡 中 | 前端代码 |
| N11 | `lib.rs` 中的 `execute_todo_write` 使用 `CLAW_TODO_STORE` 环境变量 | 🟡 中 | 代码验证 |
| N12 | `RealApiClient::call_api()` 的 `tools: None` 分支永远不会触发 | 🟢 低（已知） | agent.rs:234 |

---

## 10.2 逐流程节点 1:1 对比

### 流程 1：应用启动

| 节点 | CLAW-CLI | If2Ai | 状态 |
|------|---------|-------|------|
| 入口 | `claw-cli/src/main.rs` → `CliApp::new()` | `src-tauri/src/main.rs` → Tauri Builder | ✅ 已实现 |
| 配置加载 | `settings.json` + `.claw/` | `~/.claude/settings.json` | ✅ 已实现 |
| Session 恢复 | `~/.claw/sessions/<id>.json` | `~/.if2ai/...` 双路径 | ✅ 已实现 |
| 工具注册 | `GlobalToolRegistry` + `mvp_tool_specs()` | `ToolRegistry` + `register_builtin_tools()` | ⚠️ 两套系统 |
| 插件加载 | `discover_plugins()` | ❌ 缺失 | N/A |
| MCP 服务器 | `McpServerManager::discover_tools()` | ❌ 缺失 | N/A |
| Hook 注册 | `HookRunner::from_config()` | `HookRunner` 存在但未在 stream 路径使用 | ⚠️ 部分 |
| Skill 发现 | `discover_skill_roots()` | ❌ 缺失 | ❌ |
| SlashCommand 初始化 | `SlashCommand::parse` 就绪 | 代码存在但未暴露 | ❌ |

### 流程 2：用户输入 → 分发

| 节点 | CLAW-CLI | If2Ai | 状态 |
|------|---------|-------|------|
| 输入捕获 | `LineEditor.read_line()` | `<textarea>` `handleKeyDown` | ✅ |
| `/` 前缀检测 | `SlashCommand::parse(trimmed)` | ❌ 无检测 | ❌ |
| Tab 补全 | `complete_slash_command()` | ❌ 无补全 | ❌ |
| 空输入处理 | 跳过 | 跳过 | ✅ |
| 普通消息分发 | `run_turn(input)` | `startAgentStream()` 或 `runAgentTurn()` | ✅ |
| SlashCommand 分发 | `handle_slash_command()` → 各 handler | ❌ 无分发 | ❌ |

### 流程 3：Agent Loop (ConversationRuntime::run_turn)

| 节点 | CLAW-CLI | If2Ai run_agent_turn | If2Ai start_agent_stream | 状态 |
|------|---------|---------------------|-------------------------|------|
| user message 入 session | ✅ | ✅ | ✅ |
| max_iterations 检查 | ✅ | ✅ | ❌ |
| token_budget 检查 | ✅ | ✅ | ❌ |
| **tools: Some(definitions)** | ✅ | ❌ `tools: None` | ✅ `tools: Some` | 🔴 |
| api_client.stream() | ✅ | ✅ | ✅ |
| 提取 pending_tool_uses | ✅ | ✅ | ❌ 不提取 | 🔴 |
| permission.authorize() | ✅ | ✅ (硬编码 Allow) | ❌ 无检查 | 🟠 |
| PreToolUse Hook | ✅ | ✅ | ❌ | 🟡 |
| tool_executor.execute() | ✅ | ✅ | ❌ | 🔴 |
| PostToolUse Hook | ✅ | ✅ | ❌ | 🟡 |
| tool_result 入 session | ✅ | ✅ | ❌ | 🔴 |
| loop 继续 | ✅ | ✅ | ❌ | 🔴 |
| compaction 检查 | ✅ | ❌ | ❌ | 🟡 |
| save_session | ✅ | ✅ | ✅ (但仅文本) | 🟡 |

### 流程 4：工具注册与分发

| 节点 | CLAW-CLI | If2Ai | 状态 |
|------|---------|-------|------|
| 工具定义 | `mvp_tool_specs()` 17+ 工具 | `register_builtin_tools()` 19 工具 | ✅ |
| 工具注册 | 静态 `match` 分发 | DashMap 动态注册 | ⚠️ 架构不同 |
| 工具执行 | `execute_tool()` match → runtime 函数 | `ToolRegistry.dispatch()` → handler | ⚠️ 架构不同 |
| 权限注册 | 每工具 `required_permission` | `ToolContext.permission_mode` 全局 | ❌ 粒度不同 |
| 两套系统共存 | N/A | ToolRegistry + GlobalToolRegistry | 🟠 N3 |
| ToolSet 分类 | 无 | ✅ ToolSetRegistry | ✅ 新增 |

### 流程 5：API 请求/响应

| 节点 | CLAW-CLI | If2Ai | 状态 |
|------|---------|-------|------|
| ApiRequest 构建 | `tools: Some(defs)` | `tools: None` (路径 A) / `Some` (路径 B) | ⚠️ |
| SSE 解析 | `SseParser` + Content-Length | 同 | ✅ |
| StreamEvent 变体 | 7 种 | 同 | ✅ |
| ContentBlockDelta | 4 种 | 同 | ✅ |
| ToolDefinition 转换 | 内部转换 | `agent.rs:594-608` 手动转换 | ✅ |
| 模型选择 | 参数传入 | 从 settings.json 读取 | ⚠️ 前端无效 |
| AuthSource | Bearer/API Key/OAuth | Bearer/API Key | ❌ OAuth 缺失 |

### 流程 6：Session 持久化

| 节点 | CLAW-CLI | If2Ai | 状态 |
|------|---------|-------|------|
| 存储路径 | `~/.claw/sessions/` | `~/.if2ai/` 双路径 | ✅ |
| 数据格式 | JSON (version + messages) | JSON (id + project_id + title + ...) | ✅ 扩展 |
| 保存时机 | 每次 turn 后 | run_turn 返回前 / stream 结束 | ⚠️ stream 延迟 |
| ContentBlock | Text/ToolUse/ToolResult(is_error) | 同 | ✅ |
| Compaction | 内置触发 | 代码存在但未调用 | ❌ |
| 窗口关闭保护 | N/A (CLI 无窗口) | ❌ 无定期保存 | 🟡 |

---

## 10.3 新发现详细说明

### N1：SystemPromptBuilder 完全未使用

**位置**：`src-tauri/src/modules/runtime/prompt.rs`

**代码验证**：
- `SystemPromptBuilder` 存在于 `modules/runtime/prompt.rs`，包含 `discover_instruction_files()`、`git_status()`、`git_diff()` 集成
- 但 `run_agent_turn`（agent.rs:361-365）硬编码了三行英文 prompt：
  ```rust
  let system_prompt = vec![
      "You are If2Ai, a helpful AI assistant.".to_string(),
      "You have access to various tools to help the user.".to_string(),
      "Always be helpful, harmless, and honest.".to_string(),
  ];
  ```
- Baseline 的 `SystemPromptBuilder` 会聚合：instruction 文件 + git 状态 + git diff + 工作目录信息

**影响**：Agent 没有工作目录上下文、没有 git 状态、没有自定义 instruction 文件支持。

**修复**：`run_agent_turn` 应使用 `SystemPromptBuilder` 而非硬编码字符串。

---

### N2：RealApiClient 的 `call_api()` 内部已获取工具定义

**位置**：`src-tauri/src/commands/agent.rs:234`

**代码验证**：
```rust
// agent.rs:234
let tool_defs = self.tool_registry.get_definitions(None);
```
- `call_api()` 内部已经从 `tool_registry` 获取了工具定义
- `stream()` 调用 `call_api()`，所以工具定义最终被注入了
- **但 `ConversationRuntime` 构造的 `ApiRequest.tools: None` 被 `call_api()` 忽略了**

**评价**：功能上 `tools: None` 被掩盖了（F1 的实际影响比报告看起来小），但架构上仍然不健康。修复 F1 仍然正确。

---

### N3：两套工具系统共存且互不通信

**系统 A（Phase 4 ToolRegistry）**：
- `modules/tools/registry.rs` → `ToolRegistry`
- `modules/tools/mod.rs` → `register_builtin_tools()` 注册 19 个工具
- `commands/agent.rs` → `ToolRegistryExecutor` 包装
- 前端通过 `start_agent_stream` → `tool_registry.get_definitions()` 获取工具定义

**系统 B（Baseline GlobalToolRegistry 副本）**：
- `modules/tools/lib.rs` → `GlobalToolRegistry`
- 包含 `mvp_tool_specs()` 和 `execute_tool()` match 分发
- 包含 TodoWrite、Skill、Agent 等工具定义
- **完全未被使用！** 没有任何代码调用 `GlobalToolRegistry`

**问题**：如果未来有人按照 `lib.rs` 的 `execute_todo_write()` 注册 TodoWrite，它会注册到 GlobalToolRegistry，但 LLM 收到的工具定义来自 ToolRegistry。两个 registry 互不知道对方。

**修复**：统一为单一 ToolRegistry。将 `lib.rs` 中有用的工具（TodoWrite 等）适配为 ToolHandler 模式注册到 ToolRegistry。

---

### N4：`commands/mod.rs` 未导入 `commands/lib.rs`

**位置**：`src-tauri/src/commands/mod.rs`

**代码验证**：
```rust
// commands/mod.rs
pub mod agent;
pub mod project;
pub mod session;
pub mod window;
// ❌ 没有 pub mod commands; 或 pub mod lib;
```

`modules/commands/lib.rs` 包含完整的 SlashCommand 系统，但：
- 没有被 `commands/mod.rs` 导入
- 没有被任何 Tauri command 引用
- 编译后存在于二进制中（死代码）

---

### N5：`start_agent_stream` 的 `system: None`

**位置**：`src-tauri/src/commands/agent.rs:572`

**代码验证**：
```rust
let api_request = ApiRequest {
    system_prompt: None,  // 🔴 无系统提示！
    messages: request_messages,
    tools: if tool_defs.is_empty() { None } else { Some(tool_defs) },
    stream: true,
};
```

- 路径 A（`run_agent_turn`）有系统提示（虽然是硬编码的三行）
- 路径 B（`start_agent_stream`）**完全没有系统提示**
- 这意味着流式模式下 LLM 没有任何行为约束、工具使用指南或角色定义

**影响**：流式模式下的 Agent 行为质量显著低于非流式模式。

**修复**：`start_agent_stream` 应使用与 `run_agent_turn` 相同的 `SystemPromptBuilder`。

---

### N6：硬编码中文错误消息

**位置**：`src-tauri/src/commands/agent.rs:346-348`

```rust
return Err(format!(
    "无法连接 AI 服务: {}. 请检查 ~/.claude/settings.json 配置是否正确。",
    e
));
```

**评价**：不影响功能，但不符合国际化标准。Baseline 使用英文错误消息。

---

### N7：GlobalNavbar Automation/Skills 图标区无功能

**位置**：`src/modules/app-shell/components/GlobalNavbar.tsx`

**代码验证**：
- icon rail 中有 Chat/Skills/Automation 三个区段
- Skills 和 Automation 区段的图标点击后没有任何响应
- 没有对应的面板或路由

---

### N8：WorkbenchBackdrop.tsx 纯装饰

**位置**：`src/components/WorkbenchBackdrop.tsx`

**代码验证**：只是一个带背景色的 div，没有连接任何状态或事件。

---

### N9：SessionStatus.tsx 从未被 import

**验证**：`grep -r "SessionStatus" src/` 只匹配到 `SessionStatus.tsx` 自身。

**影响**：死代码。如果有计划使用 session 状态展示，需要实际接入。

---

### N10：两个 ChatUI 入口

**位置**：`src/App.tsx` 直接引用 `ChatUI`，`ChatWorkspace.tsx` 也引用 `ChatUI`

**代码验证**：
- `App.tsx` 中有 `ChatUI` 的直接使用
- `ChatWorkspace.tsx` 也渲染 `ChatUI`
- 导致可能存在两套独立的聊天界面状态

---

### N11：`execute_todo_write` 使用 `CLAW_TODO_STORE` 环境变量

**位置**：`src-tauri/src/modules/tools/lib.rs`

**代码验证**：
```rust
fn todo_store_path() -> PathBuf {
    let env_var = std::env::var("CLAW_TODO_STORE").unwrap_or_default();
    // 应该改为 IF2AI_TODO_STORE 或 ~/.if2ai/
}
```

**影响**：如果 TodoWrite 被启用，它会寻找 CLAW 特定的环境变量和路径，而非 If2Ai 的路径。

---

### N12：`call_api()` 的 `tools: None` 分支永远不会触发

**位置**：`agent.rs:232-236`

**代码验证**：
```rust
// call_api() 内部
let tool_defs = self.tool_registry.get_definitions(None);
// ↑ 总是从 registry 获取，忽略 request.tools
```

`request.tools` 字段被完全忽略。无论调用方传 `Some(...)` 还是 `None`，最终都是 registry 的定义。这确认了 N2 的分析。

---

## 10.4 bs_gap 文档二次审查结论

### 需要修正的内容

| 文档 | 位置 | 修正 |
|------|------|------|
| 03-tool-system-gap.md | §3.5 | 补充 N3（两套工具系统共存）说明 |
| 06-frontend-gap.md | §6.1 | 补充 N7, N8, N9, N10 发现 |
| 08-critical-fix-priority.md | §8.1 | 补充 N1-N12 到优先级表 |
| 01-architecture-overview.md | §1.2 | 补充 SystemPromptBuilder 未使用说明 |

### 已确认正确的内容

所有 00-09 文档的核心发现、严重度评估和修复方案均经过代码验证确认为正确。特别是 08 文档经过 UI/UX 审查后的修订版本，修复方案详尽且可行。

---

## 10.5 关键路径风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 用户发送敏感指令（如 `rm -rf /`）时 bash 无权限阻止 | 高 | 严重 | P1 修复 F3/F4 |
| 长对话导致 token 溢出崩溃 | 中 | 严重 | P2 修复 F9 |
| 窗口关闭导致 session 数据丢失 | 中 | 中 | P2 修复 F16 |
| 两套工具系统导致工具注册混乱 | 低 | 中 | Phase 0 统一 |
| 流式模式无系统提示导致低质量回复 | 高 | 中 | P0 修复 F2 附带修复 |

---

## 10.6 审计结论

**If2Ai 桌面端当前状态总结**：

1. **Agent 核心框架**：✅ 基础结构已迁移（ConversationRuntime, Session, PermissionPolicy, ToolRegistry）
2. **工具调用**：🔴 两条路径均有致命缺陷（tools:None / tool_use 忽略），但 N2 发现路径 A 被 RealApiClient 掩盖
3. **权限系统**：🟠 形同虚设（硬编码 + 无 prompter）
4. **SlashCommand**：❌ 完整代码存在但完全死代码
5. **Skill 系统**：❌ 完全缺失
6. **前端 UI**：🟡 基础聊天可用，但工具可见性、slash command、model/permission 选择等核心交互完全缺失
7. **MCP/Plugin**：❌ 完全缺失（Phase 5+）
8. **SystemPrompt**：🟠 硬编码替代，丢失 git/instruction 上下文

**最关键的修复顺序不变**（08-critical-fix-priority.md 的 Phase 0-3 方案仍然最优）：
1. 先修工具调用（F1+F2），这是 Agent 能不能用工具的根本问题
2. 再修安全和 SlashCommand/Skill（F3-F6）
3. 然后完善 UX（F7-F17, UI-1~UI-8）
4. 最后长期增强（F18-F25）
