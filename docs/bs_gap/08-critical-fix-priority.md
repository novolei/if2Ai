# 八、修复优先级与实施路线图（完整版 — UI/UX 审查修订版）

> 本章节汇总来自 00-index、01-architecture-overview、02-agent-loop-gap、03-tool-system-gap、04-permissions-gap、05-session-gap、06-frontend-gap、07-mcp-plugin-gap、09-skills-commands-gap 所有 gap 报告，按优先级排序，给出经代码验证的修复方案。
>
> **修订记录**：2026-04-12 — 高级 UI/UX 设计师审查，修正 F7（渐进式工具调用展示）、F8（合并 tool_call_update 事件）、F10（双重模型选择器冲突）、F11（合并 SlashCommand 交互层）、新增 UI-1~UI-7 新发现 gap。

---

## 8.1 优先级总览

> **Phase/slice 映射**（2026-04-12 回补）：每个 Fix 项已标记对应的 Phase 和 slice ID。
> 详见 `docs/exec-plans/active/phase-5{a,b,c,d,e}-*.yaml`

| 优先级 | 编号 | 问题 | 严重度 | 来源报告 | 影响 | Phase/slice |
|--------|------|------|--------|---------|------|-------------|
| **P0** | F1 | 非流式路径 `tools: None`，LLM 收不到工具定义 | 🔴 阻塞 | 02-agent-loop-gap | Agent 无法调用工具 | **Phase 5A / 5a.1** |
| **P0** | F2 | 流式路径忽略 `InputJsonDelta` / `ContentBlockStart(ToolUse)` | 🔴 阻塞 | 02-agent-loop-gap | 流式工具循环断裂 | **Phase 5B / 5b.2 + 5b.3** |
| **P1** | F3 | PermissionPolicy 硬编码 `DangerFullAccess` | 🟠 严重 | 04-permissions-gap | 安全机制形同虚设 | **Phase 5C / 5c.3** |
| **P1** | F4 | Prompter 参数传 `None`，无法触发用户确认 | 🟠 严重 | 04-permissions-gap | 交互式权限确认缺失 | **Phase 5C / 5c.3** |
| **P1** | F5 | SlashCommand 系统代码存在但未暴露 | 🟠 严重 | 09-skills-commands-gap | 28 个 CLI 命令无法使用 | **Phase 5C / 5c.5** |
| **P1** | F6 | Skill 工具完全缺失（注册 + 发现 + 执行） | 🟠 严重 | 09-skills-commands-gap | 无法使用自定义技能 | **Phase 5C / 5c.4** |
| **P2** | F7 | 前端工具调用无可见性（渐进式展示） | 🟡 中 | 06-frontend-gap | 用户无法理解 Agent 行为 | **Phase 5D / 5d.3** |
| **P2** | F8 | `StreamTokenPayload` 缺少工具事件类型 | 🟡 中 | 06-frontend-gap | SSE 无法传输工具事件 | **Phase 5B / 5b.1** |
| **P2** | F9 | Context Compaction 未接入 Agent 路径 | 🟡 中 | 05-session-gap | 长对话 token 溢出 | **Phase 5D / 5d.5** |
| **P2** | F10 | 双重模型选择器冲突（全局 + 局部断开） | 🟡 中 | 06-frontend-gap | 模型选择无效且混淆 | **Phase 5D / 5d.6** |
| **P2** | F11 | SlashCommand 前端交互层（`/` 解析 + Tab 补全 + 下拉建议） | 🟡 中 | 09-skills-commands-gap | `/` 输入直接发给 LLM | **Phase 5D / 5d.7** |
| **P2** | F12 | `ToolSearch` 工具未注册 | 🟡 中 | 03-tool-system-gap, 09-skills-commands-gap | Agent 无法搜索工具 | **Phase 5E / 5e.1** |
| **P2** | F13 | `Agent` 工具未注册（嵌套 Agent 调用） | 🟡 中 | 03-tool-system-gap, 09-skills-commands-gap | 无法多 Agent 协作 | **Phase 5E / 5e.5** |
| **P2** | F14 | `SkillSearch` 工具未注册 | 🟡 中 | 09-skills-commands-gap | 无法搜索 Skill | **Phase 5C / 5c.7** |
| **P2** | F15 | Hook 系统未接入 `start_agent_stream` | 🟡 中 | 07-mcp-plugin-gap | Pre/PostToolUse Hook 不触发（流式路径） | **Phase 5B / 5b.3（合并到 F2）** |
| **P2** | F16 | 流式场景窗口关闭导致 session 丢失 | 🟡 中 | 05-session-gap | 后台任务未完成时数据丢失 | **Phase 5E / 5e.5** |
| **P2** | F17 | `/skills` `/agents` 命令未实现为 Tauri command | 🟡 中 | 09-skills-commands-gap | 前端无法列出 Skill/Agent | **Phase 5C / 5c.5** |
| **P3** | F18 | SKILL.md 格式规范未定义 | 🟢 低 | 09-skills-commands-gap | 无统一 Skill 文件格式 | **Phase 5C / 5c.4（隐含）** |
| **P3** | F19 | Session 与 Project 强绑定，无法跨项目/临时会话 | 🟢 低 | 05-session-gap | 架构灵活性受限 | **未排期**（已支持 `project_id.is_empty()`） |
| **P3** | F20 | MCP 协议完全缺失 | 🟢 低 | 07-mcp-plugin-gap | 外部工具集成能力 | **未排期** |
| **P3** | F21 | 插件系统完全缺失 | 🟢 低 | 07-mcp-plugin-gap | 第三方扩展能力 | **未排期** |
| **P3** | F22 | Sandbox 沙箱缺失 | 🟢 低 | 04-permissions-gap | 高级安全隔离 | **未排期** |
| **P3** | F23 | OAuth 登录 UI 缺失 | 🟢 低 | 07-mcp-plugin-gap | 仅支持 Bearer Token | **未排期** |
| **P3** | F24 | 独立 SlashCommand Handler 实现（10+ 命令） | 🟢 低 | 09-skills-commands-gap | `/branch` `/commit` `/diff` 等 | **未排期** |
| **P3** | F25 | 会话状态显示不完整（token/turn/model/permission） | 🟢 低 | 06-frontend-gap | 缺乏 `/status` 信息面板 | **Phase 5E / 5e.3（N9 SessionStatus）** |
| **P3** | UI-1 | 权限模式选择器仅为占位符 | 🟡 中 | 新发现 | 无法切换权限模式 | **Phase 5C / 5c.3** |
| **P3** | UI-2 | 分支选择器占位符无功能 | 🟡 中 | 新发现 | 无法切换 git 分支 | **Phase 5C / 5c.5（隐含）** |
| **P3** | UI-3 | ChatWorkspace header 按钮无功能 | 🟡 中 | 新发现 | Play/terminal/submit 按钮无响应 | **未排期** |
| **P3** | UI-4 | Header 硬编码 diff 统计数字 | 🟢 低 | 新发现 | 显示虚假数据 | **Phase 5E / 5e.3** |
| **P3** | UI-5 | `Message.role` 缺少 `tool` 角色 | 🟡 中 | 新发现 | 工具结果无法正确渲染 | **Phase 5B / 5b.4** |
| **P3** | UI-6 | Session 恢复丢失 tool_call 上下文 | 🟡 中 | 新发现 | 历史对话工具链断裂 | **Phase 5E / 5e.5** |
| **P3** | UI-7 | 无流式中断能力 | 🟢 低 | 新发现 | 无法取消正在进行的 Agent 响应 | **Phase 5E / 5e.2** |
| **P2** | UI-8 | `TodoWrite` 工具未注册到 Phase 4 ToolRegistry + 前端无 Todo 面板 | 🟡 中 | 新发现 | Agent 无法管理任务列表，用户看不到任务进度 | **Phase 5D / 5d.4** |
| **P1** | N1 | SystemPromptBuilder 完全未使用，硬编码 prompt 替代 | 🟠 严重 | 10-comprehensive-audit | Agent 无工作目录/git/instruction 上下文 | **Phase 5C / 5c.1** |
| **P0** | N5 | `start_agent_stream` 的 `system_prompt: None` — 无系统提示 | 🔴 阻塞 | 10-comprehensive-audit | 流式模式 LLM 无行为约束 | **Phase 5A / 5a.3** |
| **P1** | N3 | 两套工具系统共存且互不通信 | 🟠 严重 | 10-comprehensive-audit | 工具注册混乱 | **Phase 5C / 5c.2** |

---

## 8.2 P0 修复：Agent 工具调用

### N5：`start_agent_stream` 无系统提示（升级为 P0）

**来源**：10-comprehensive-audit §10.3 N5

**位置**：[agent.rs:572](src-tauri/src/commands/agent.rs#L572)

**现状**：
```rust
let api_request = ApiRequest {
    system_prompt: None,  // 🔴 无系统提示！
    messages: request_messages,
    tools: if tool_defs.is_empty() { None } else { Some(tool_defs) },
    stream: true,
};
```

**影响**：流式模式下 LLM 没有任何角色定义、行为约束或工具使用指南。与路径 A（`run_agent_turn`）的硬编码三行 prompt 相比，流式模式的 Agent 回复质量会显著更低。

**修复方案**：
```rust
// 与 run_agent_turn 使用相同的 SystemPromptBuilder
let system_prompt = build_system_prompt(&cwd, &session);
let api_request = ApiRequest {
    system_prompt: Some(system_prompt),
    ...
};
```

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — start_agent_stream 使用 SystemPromptBuilder
- `src-tauri/src/modules/runtime/prompt.rs` — SystemPromptBuilder

---

### F1：非流式路径 `tools: None`

**来源**：02-agent-loop-gap, 03-tool-system-gap §3.2

**位置**：[conversation.rs:246](src-tauri/src/modules/runtime/conversation.rs#L246)

```rust
// 现状
let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: self.session.messages.clone(),
    tools: None,  // 🔴 硬编码 None
};
```

**代码验证**：
- `ConversationRuntime<C, T>` 的 `ToolExecutor` trait ([conversation.rs:40-42](src-tauri/src/modules/runtime/conversation.rs#L40-L42)) 只有 `execute()` 方法，没有 `get_definitions()`
- `RealApiClient` ([agent.rs:107-125](src-tauri/src/commands/agent.rs#L107-L125)) 持有 `tool_registry: Arc<ToolRegistry>`，其 `call_api()` 方法 ([agent.rs:184-264](src-tauri/src/commands/agent.rs#L184-L264)) **已经在内部从 registry 获取了工具定义**（line 234 `self.tool_registry.get_definitions(None)`），但这些定义是在 `RealApiClient` 层面，不在 `ConversationRuntime` 层面
- `ConversationRuntime` 只知道 `ApiClient::stream()` 接口，它构造 `ApiRequest` 时无法访问工具定义

**修复方案（推荐）：让 `RealApiClient` 的 `stream()` 自动注入工具定义**

当前 `RealApiClient::stream()` 调用 `self.call_api(request)`，而 `call_api()` 内部已经有 `self.tool_registry.get_definitions(None)`。问题是 `stream()` 接收的是 `ApiRequest`，`call_api()` 忽略 `request.tools` 字段，直接使用 registry 的定义。所以 `tools: None` 传入后，`call_api()` 仍然从 registry 取了工具定义——**但这是在 `RealApiClient` 内部的转换中完成的**。

**结论**：`RealApiClient` 的 `stream()` 实际上已经正确地将工具定义注入到了最终 API 请求中（通过 `call_api` 内部的 `get_definitions()`）。`request.tools: None` 在 `RealApiClient` 中被覆盖了。

**但是**，这个设计有严重问题：
1. `ConversationRuntime` 的 `tools: None` 误导代码读者——看起来没有工具
2. 如果将来换用其他 `ApiClient` 实现（不持有 `tool_registry`），工具会静默丢失
3. 违反单一职责——`ApiClient` 不应该决定发什么工具定义

**正确修复**：让 `ToolExecutor` trait 暴露 `get_definitions()`，由 `ConversationRuntime` 获取后填入 `ApiRequest.tools`。

```rust
// modules/runtime/conversation.rs — 修改 ToolExecutor trait
pub trait ToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError>;
    fn get_definitions(&self) -> Vec<ToolDefinition>;  // 新增
}

// ToolRegistryExecutor 实现（agent.rs）
impl ToolExecutor for ToolRegistryExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ...> { ... }
    fn get_definitions(&self) -> Vec<ToolDefinition> {
        // 从 tool_registry 获取并转换为 ToolDefinition 格式
        let definitions = self.tool_registry.get_definitions(None);
        definitions.into_iter().filter_map(|def| {
            // 转换逻辑（同 agent.rs:594-608）
            ...
        }).collect()
    }
}

// conversation.rs:243-247 — 修改为
let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: self.session.messages.clone(),
    tools: Some(self.tool_executor.get_definitions()),  // ✅
};
```

**合理性评估**：✅ 正确。这是最符合架构职责分离的方案。`ConversationRuntime` 通过 `ToolExecutor` trait 获取工具定义，不依赖于具体实现。

**涉及文件**：
- `src-tauri/src/modules/runtime/conversation.rs` — 扩展 `ToolExecutor` trait，修改 `run_turn` 中 `tools` 字段
- `src-tauri/src/commands/agent.rs` — `ToolRegistryExecutor` 实现 `get_definitions()`
- `src-tauri/src/modules/api/mod.rs` — 可能需要引入 `ToolDefinition` 到 conversation 可见范围

---

### F2：流式路径忽略 tool_use 事件

**来源**：02-agent-loop-gap §2.2, 03-tool-system-gap §3.4

**位置**：[agent.rs:686](src-tauri/src/commands/agent.rs#L686), [agent.rs:699-713](src-tauri/src/commands/agent.rs#L699-L713)

```rust
// 现状 — InputJsonDelta 被空处理
crate::modules::api::ContentBlockDelta::InputJsonDelta { .. } => {}  // line 686

// ContentBlockStart 只处理 Thinking，不处理 ToolUse
ApiStreamEvent::ContentBlockStart(start_event) => {
    if matches!(start_event.content_block, OutputContentBlock::Thinking { .. }) {
        // thinking_start 事件
    }
    // ↑ ToolUse 块完全没有处理！
}
```

**代码验证**：
- `start_agent_stream` ([agent.rs:508-769](src-tauri/src/commands/agent.rs#L508-L769)) 是一个 `tokio::spawn` 的后台任务
- 工具定义确实传给了 LLM（line 593-619: `tools: Some(tool_defs)`）
- 但 `InputJsonDelta`（line 686）和 `ContentBlockStart` 中的 `ToolUse`（line 699-712）都被忽略了
- 流式结束后只保存了 user_msg + assistant_msg（文本）（line 733-756），没有 tool_result
- 没有工具循环——没有权限检查、没有工具执行、没有 tool_result 回传

**对比 Baseline**：`rust/crates/runtime/src/conversation.rs` 的 `run_turn()` 有完整 loop：
1. 提取 `pending_tool_uses`
2. `authorize()` → `execute()` → `tool_result` 入 `session.messages`
3. 继续下一轮 LLM 调用

**修复方案**：重写 `start_agent_stream`，实现完整工具循环。这是一个较大的重构。

```
架构决策：两条路径是否统一？
方案 A（推荐短期）: 在 start_agent_stream 中实现 tool_use 处理循环
  - 在后台任务中累积 InputJsonDelta 到 tool_arguments 缓存
  - ContentBlockStop 时提取完整 tool_use
  - 执行 permission + tool_execution
  - 继续发送下一轮 API 请求
  - 工具结果通过 SSE 推送给前端

方案 B（推荐长期）: 废弃 run_agent_turn + start_agent_stream 双路径
  - 让 start_agent_stream 直接调用 ConversationRuntime::run_turn()
  - 通过回调机制将 tool_use/tool_result 事件推送给前端
  - 单一真相源，减少维护成本
```

**合理性评估**：✅ 正确。Baseline 的 `run_turn()` 已经验证了工具循环的正确性（有完整的测试用例 [conversation.rs:565-614](src-tauri/src/modules/runtime/conversation.rs#L565-L614)）。流式路径必须实现同等逻辑。

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — 重写 start_agent_stream 工具循环
- `src/lib/tauri.ts` — 新增工具事件类型
- `src/App.tsx` — 处理工具调用事件
- `src/components/ui/chat-ui.tsx` — 渲染工具调用结果

---

## 8.3 P1 修复：安全性与核心系统连接

### N1：SystemPromptBuilder 完全未使用（P1）

**来源**：10-comprehensive-audit §10.3 N1

**位置**：[modules/runtime/prompt.rs](src-tauri/src/modules/runtime/prompt.rs) 存在但未被调用

**现状**：
- `SystemPromptBuilder` 已存在于 `modules/runtime/prompt.rs`，包含 `discover_instruction_files()`、git 状态/diff 集成
- `run_agent_turn`（agent.rs:361-365）硬编码三行英文 prompt
- `start_agent_stream`（agent.rs:572）甚至完全没有 system_prompt（N5）

**Baseline 的 SystemPromptBuilder 会聚合**：
1. 自定义 instruction 文件（`.claw/INSTRUCTION.md` 等）
2. git status 输出
3. git diff 输出
4. 工作目录信息

**修复方案**：
```rust
// agent.rs — run_agent_turn 中替换硬编码 prompt
let builder = SystemPromptBuilder::new(&cwd);
let system_prompt = builder.build()?;

// agent.rs — start_agent_stream 中使用相同的 builder
let builder = SystemPromptBuilder::new(&cwd);
let system_prompt = builder.build()?;
```

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — 替换两处硬编码 prompt
- `src-tauri/src/modules/runtime/prompt.rs` — 已存在，无需修改

---

### N3：两套工具系统共存（P1）— 方案 A：全量迁移

**来源**：10-comprehensive-audit §10.3 N3

**现状**：
- **系统 A**：`ToolRegistry`（modules/tools/registry.rs）— Phase 4 实现，注册了 19 个工具，async Handler 模式，支持 per-tool timeout/max_result_size/toolset/SharedToolContext
- **系统 B**：`GlobalToolRegistry`（modules/tools/lib.rs）— CLAW-CLI baseline 副本，包含 18 个工具定义（sync match 分支模式），有完整的类型验证、权限声明、插件机制
- 两套系统**互不通信**，注册到 B 的 11 个独有工具对主系统完全不可见

**工具覆盖分析**：

| 分类 | 工具数 | 工具名 |
|------|--------|--------|
| 两者重叠 | 7 | bash, read_file, write_file, edit_file, glob_search, web_fetch/WebFetch, web_search/WebSearch |
| 仅 GlobalToolRegistry | 11 | grep_search, TodoWrite, Skill, Agent, ToolSearch, Sleep, SendUserMessage, Config, NotebookEdit, StructuredOutput, PowerShell |
| 仅 ToolRegistry (Phase 4) | 13 | cron_add/list/remove/run/runs, content_search, file_edit(disabled stub), http_request, memory_store/recall/forget/purge/export |
| **合计** | **31** | 7 去重 + 11 独有 + 13 独有 |

**架构决策（2026-04-12 确定）**：采用**方案 A — 全量迁移**，废弃 GlobalToolRegistry。

**决策依据**：
1. `start_agent_stream` 工具循环（Phase 5B）是 async tokio 任务，sync 的 GlobalToolRegistry 需要 `spawn_blocking` 包装，增加死锁风险
2. `SharedToolContext`（workdir + permission_mode）是 F3/F4 权限系统修复的基础，只有 ToolRegistry 的 Handler 模式支持
3. 单一真相源消除维护混淆，per-tool timeout/max_result_size 比全局超时更精细
4. Phase 4 的 13 个工具开发投入完全保留

**迁移策略 — 三阶段执行**：

```
阶段 1（Phase 5C / 5c.2）：废弃 + 迁移验证
  ├─ lib.rs 标记 #[deprecated]
  ├─ 迁移 TodoWrite（路径验证：sync → async ToolHandler 适配）
  ├─ 确认 ToolRegistry 注册/执行链路通畅
  └─ 输出迁移 checklist（供后续阶段复用）

阶段 2（Phase 5C / 5c.4 + 5c.7）：核心工具迁移
  ├─ 5c.4: 迁移 Skill（复用 discover_skill_roots + parse_skill_frontmatter）
  ├─ 5c.7: 迁移 ToolSearch（复用 registry.get_definitions）
  └─ 验证：工具发现 + 搜索链路完整

阶段 3（Phase 5E / 5e.5 + 5e.6）：剩余工具 + 重叠替换
  ├─ 5e.5: 迁移 Agent + grep_search + Sleep
  ├─ 5e.6: 迁移 SendUserMessage + Config + NotebookEdit + StructuredOutput + PowerShell
  ├─ 替换 7 个重叠工具（以 GlobalToolRegistry 实现为基准覆盖 Phase 4 版本）
  └─ 确认 lib.rs 无任何引用后删除
```

**迁移 checklist（每个工具必须满足）**：
1. 从 lib.rs 的 `execute_tool()` match 分支提取输入 struct + run 函数
2. 适配为 `ToolEntry { handler: Arc::new(|args, ctx| async move { ... }) }` 模式
3. 使用 `IF2AI_*` 环境变量替代 `CLAW_*` 环境变量
4. 设置合理的 `toolset`、`max_result_size`、`timeout_secs`
5. 在 `register_builtin_tools()` 中注册
6. 确认 Phase 4 中不存在同名工具（或明确替换关系）

**涉及文件**：
- `src-tauri/src/modules/tools/lib.rs` — 标记废弃，最终删除
- `src-tauri/src/modules/tools/mod.rs` — `register_builtin_tools()` 注册全部迁移工具
- `src-tauri/src/modules/tools/builtin/todo_write.rs` — 新建（阶段 1 路径验证）
- `src-tauri/src/modules/tools/builtin/skill.rs` — 新建（阶段 2）
- `src-tauri/src/modules/tools/builtin/skill_search.rs` — 新建（阶段 2）
- `src-tauri/src/modules/tools/builtin/agent.rs` — 新建（阶段 3）
- `src-tauri/src/modules/tools/builtin/tool_search.rs` — 新建（阶段 3）
- `src-tauri/src/modules/tools/builtin/grep_search.rs` — 新建（阶段 3）
- `src-tauri/src/modules/tools/builtin/sleep.rs` — 新建（阶段 3）
- `src-tauri/src/modules/tools/builtin/send_user_message.rs` — 新建（阶段 3）
- `src-tauri/src/modules/tools/builtin/config.rs` — 新建（阶段 3）
- `src-tauri/src/modules/tools/builtin/notebook_edit.rs` — 新建（阶段 3）
- `src-tauri/src/modules/tools/builtin/structured_output.rs` — 新建（阶段 3）
- `src-tauri/src/modules/tools/builtin/powershell.rs` — 新建（阶段 3）

**重叠工具替换策略**：7 个重叠工具（bash, read_file, write_file, edit_file, glob_search, web_fetch, web_search）以 GlobalToolRegistry 的 baseline 实现为第一优先级。如果 Phase 4 的实现与 baseline 有行为差异，以 baseline 为准进行替换。替换工作在 Phase 5E 5e.6 中执行。

---

### F3：PermissionPolicy 硬编码 `DangerFullAccess`

**来源**：04-permissions-gap §4.1

**位置**：[agent.rs:358](src-tauri/src/commands/agent.rs#L358)

```rust
// 现状
let permission_policy = PermissionPolicy::new(PermissionMode::DangerFullAccess);
```

**代码验证**：
- `PermissionPolicy::new()` 只接受一个 `PermissionMode`，无工具级需求映射
- Baseline（`rust/crates/runtime/src/permissions.rs`）中的 `PermissionPolicy` 有 `tool_requirements: BTreeMap<String, PermissionMode>`，每个工具独立注册权限需求
- If2Ai 的 `PermissionPolicy` 代码存在于 [modules/runtime/permissions.rs](src-tauri/src/modules/runtime/permissions.rs)，结构与 baseline 一致
- `start_agent_stream` 完全没有创建 `PermissionPolicy`——工具执行时根本不做权限检查

**修复方案**：

```rust
// 1. 从配置或参数读取 PermissionMode
let mode = // 从 session 配置 / 前端参数获取
let permission_policy = PermissionPolicy::new(mode);

// 2. 注册工具级权限需求（参考 baseline）
// bash → DangerFullAccess, read_file → ReadOnly, write_file → WorkspaceWrite
```

**合理性评估**：✅ 正确。`PermissionPolicy` 的授权算法已经在 [modules/runtime/permissions.rs](src-tauri/src/modules/runtime/permissions.rs) 中实现，只需从正确来源获取 `mode` 并注册工具需求。

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — 从参数读取 `permission_mode`
- `src-tauri/src/modules/tools/mod.rs` — 注册工具级权限需求
- `src/App.tsx` — 传递 `permission_mode` 参数

---

### F4：Prompter 参数传 `None`

**来源**：04-permissions-gap §4.2

**位置**：[agent.rs:382](src-tauri/src/commands/agent.rs#L382)

```rust
// 现状
let result = runtime.run_turn(user_message.clone(), None);
//                                                 ↑ None
```

**代码验证**：
- `ConversationRuntime::run_turn()` ([conversation.rs:214-333](src-tauri/src/commands/agent.rs#L214-L333)) 的 `prompter: Option<&mut dyn PermissionPrompter>` 参数
- 当 `prompter` 是 `None` 时，`authorize()` 调用传入 `None`（line 276）
- Baseline 中 `CliPermissionPrompter` 在终端等待 `y/n` 输入
- Tauri 桌面端需要通过 `window.emit()` 向前端发送确认事件，等待用户响应

**修复方案**：

```rust
// 实现 TauriPermissionPrompter
struct TauriPermissionPrompter {
    window: WebviewWindow,
}

impl PermissionPrompter for TauriPermissionPrompter {
    fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision {
        // 1. emit 确认事件给前端
        // 2. 等待前端返回用户决定
        // 3. 返回 Allow / Deny
    }
}
```

**合理性评估**：✅ 正确。`PermissionPrompter` trait 已存在于 baseline，Tauri 需要通过 IPC 机制实现异步确认。注意 `run_turn` 是同步方法（非 async），prompter 需要是阻塞等待或使用 `block_in_place`。

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — 实现 TauriPermissionPrompter
- `src/App.tsx` — 处理权限确认事件
- `src-tauri/src/modules/runtime/permissions.rs` — PermissionPrompter trait

---

### F5：SlashCommand 系统代码存在但未暴露

**来源**：09-skills-commands-gap §9.2, §9.5

**位置**：[modules/commands/lib.rs](src-tauri/src/modules/commands/lib.rs)

**代码验证**：
- `rust/crates/commands/src/lib.rs`（Baseline）包含 28 个 SlashCommand + SlashCommandSpec + parse + suggest + handle_slash_command + handle_branch_slash_command + handle_worktree_slash_command + handle_commit_slash_command + handle_commit_push_pr_slash_command + handle_agents_slash_command + handle_skills_slash_command
- `src-tauri/src/modules/commands/lib.rs` 是**完整副本**
- `handle_slash_command()` 在 Baseline 中只处理 `/compact` 和 `/help`，其他命令返回 `None`（line 1740-1788）
- 但其他命令有独立的 handler 函数（`handle_branch_slash_command`, `handle_commit_slash_command` 等）
- 当前无任何 Tauri command 暴露这些函数
- `commands/mod.rs` 未导入 `lib.rs` 的内容

**修复方案**：

```rust
// src-tauri/src/commands/slash.rs — 新建
use crate::modules::commands::{SlashCommand, slash_command_specs, suggest_slash_commands, handle_slash_command};

#[tauri::command]
pub fn parse_slash_command(input: String) -> Option<SlashCommandParseResult> {
    SlashCommand::parse(&input).map(|cmd| SlashCommandParseResult {
        name: format!("{:?}", cmd),  // 或直接枚举转字符串
    })
}

#[tauri::command]
pub fn list_slash_commands() -> Vec<SlashCommandSpecDto> {
    slash_command_specs().iter().map(|s| s.into()).collect()
}

#[tauri::command]
pub fn suggest_slash_commands_endpoint(input: String, limit: usize) -> Vec<String> {
    suggest_slash_commands(&input, limit)
}

#[tauri::command]
pub fn execute_slash_command(input: String, session_id: String, cwd: String) -> Result<SlashCommandResult, String> {
    // 解析后调用对应 handler
}
```

**合理性评估**：✅ 正确。`SlashCommand::parse()` 和 `suggest_slash_commands()` 都是纯函数，可以直接通过 Tauri command 暴露。`handle_slash_command()` 需要 session 和 compaction config 参数，需要从 session manager 获取。

**注意**：`SlashCommandSpec` 包含 `&'static str` 引用，不能直接通过 serde 序列化，需要创建 DTO 结构体。

**涉及文件**：
- `src-tauri/src/commands/slash.rs` — 新建
- `src-tauri/src/commands/mod.rs` — 导入
- `src/components/ui/chat-ui.tsx` — `/` 前缀检测

---

### F6：Skill 工具完全缺失（注册 + 发现 + 执行）

**来源**：09-skills-commands-gap §9.3, §9.5

**代码验证**：
- Baseline 的 `Skill` 工具定义在 `rust/crates/tools/src/lib.rs` 的 `mvp_tool_specs()` 中
- Skill 执行：读取 `SKILL.md` 文件，返回内容作为 LLM 上下文
- Skill 发现：`discover_skill_roots()` ([commands/lib.rs:1304-1379](rust/crates/commands/src/lib.rs#L1304-L1379)) — 8+ 路径搜索（项目级 `.codex/skills/`, `.claw/skills/`, `.codex/commands/`, `.claw/commands/` + 用户级 `$CODEX_HOME/skills/`, `~/.codex/skills/`, `~/.claw/skills/`）
- SKILL.md 格式：YAML frontmatter (`name`, `description`) + 指令内容（line 1550-1578 `parse_skill_frontmatter()`）
- If2Ai 的 `register_builtin_tools()` ([modules/tools/mod.rs](src-tauri/src/modules/tools/mod.rs)) 中没有 Skill
- `SkillsSettingsPage.tsx` 是硬编码占位符

**修复方案**：

```rust
// src-tauri/src/modules/tools/builtin/skill.rs — 新建
pub async fn run_skill(input: SkillInput, ctx: SharedToolContext) -> Result<String, ToolError> {
    let skill_path = resolve_skill_path(&input.skill, &ctx.workdir)?;
    let content = tokio::fs::read_to_string(&skill_path).await
        .map_err(|e| ToolError::NotFound(format!("Skill file not found: {}", e)))?;
    Ok(content)
}

fn resolve_skill_path(skill: &str, workdir: &Path) -> Result<PathBuf, String> {
    // 参考 baseline discover_skill_roots() 实现
    let roots = discover_skill_roots(workdir);
    for root in roots {
        let candidate = root.path.join(skill).join("SKILL.md");
        if candidate.is_file() { return Ok(candidate); }
        // 也支持 legacy commands dir
        let legacy = root.path.join(format!("{skill}.md"));
        if legacy.is_file() { return Ok(legacy); }
    }
    Err(format!("Skill '{skill}' not found in any search path"))
}

// modules/tools/mod.rs — 注册
registry.register(skill_tool_entry())?;
```

**合理性评估**：✅ 正确。`discover_skill_roots()` 和 `parse_skill_frontmatter()` 的 Baseline 实现是纯文件系统操作，可以移植。注意 SKILL.md 格式规范需要先定义。

**⚠️ 注意**：`discover_skill_roots()` 在 Baseline 中使用了 `env::var("CODEX_HOME")`，If2Ai 应改为 `IF2AI_HOME` 或 `~/.if2ai/skills/`。

**涉及文件**：
- `src-tauri/src/modules/tools/builtin/skill.rs` — 新建
- `src-tauri/src/modules/tools/mod.rs` — 注册
- `src/modules/settings/pages/SkillsSettingsPage.tsx` — 接入真实数据

---

## 8.4 P2 修复：UI 功能与中级 Gap

### F7：前端工具调用无可见性（渐进式展示）

**来源**：06-frontend-gap §6.4

**现状**：`ChatUI` 无任何工具调用展示组件。用户只能看到纯文本回复，完全不知道 Agent 在背后调用了哪些工具、传了什么参数、得到了什么结果。

**设计原则：渐进式披露（Progressive Disclosure）**

桌面 AI Agent 的核心 UX 目标是让用户**理解 Agent 的思考与行动过程**，而不是用冗长的 JSON 淹没用户。采用三层渐进式展示：

```
Layer 1 — 紧凑状态行（默认展示）
┌─────────────────────────────────────────────┐
│ ✓ read_file    main.tsx                      │  ← 成功：绿色 ✓ + 工具名 + 摘要
│ ◐ bash         cargo build                   │  ← 运行中：黄色 ◐ 旋转指示器
│ ○ glob_search  waiting...                     │  ← 排队中：灰色 ○
└─────────────────────────────────────────────┘

Layer 2 — 展开参数（点击状态行）
┌─────────────────────────────────────────────┐
│ ▼ ✓ read_file    main.tsx                   │
│   ┌─ Arguments ─────────────────────────┐   │
│   │ path: "src/main.tsx"                │   │
│   └─────────────────────────────────────┘   │
│   ┌─ Result (1.2s, 342 lines) ─────────┐   │
│   │ import { ... } from 'react';        │   │
│   │ ...                                 │   │
│   └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘

Layer 3 — 完整 JSON（调试模式）
  在展开参数后显示 "View raw JSON" 链接
```

**修复方案**：

```typescript
// src/components/ui/chat-ui.tsx — 新增组件

// 工具调用状态类型
type ToolCallStatus = "queued" | "running" | "completed" | "error";

// 单个工具调用的紧凑展示
function ToolCallItem({ toolCall }: { toolCall: ToolCallData }) {
  const [expanded, setExpanded] = useState(false);
  const statusIcon = {
    queued: "○",
    running: "◐",
    completed: "✓",
    error: "✗",
  }[toolCall.status];

  const statusColor = {
    queued: "text-muted-foreground",
    running: "text-yellow-500 animate-pulse",
    completed: "text-green-500",
    error: "text-red-500",
  }[toolCall.status];

  return (
    <div className="my-1 rounded-md border bg-muted/30">
      {/* Layer 1: 紧凑状态行 — 始终可见 */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-3 py-1.5 text-sm"
      >
        <span className={cn("font-mono", statusColor)}>{statusIcon}</span>
        <span className="font-medium">{toolCall.name}</span>
        <span className="text-muted-foreground truncate">
          {toolCall.summary}
        </span>
        {toolCall.duration && (
          <span className="ml-auto text-xs text-muted-foreground">
            {toolCall.duration}
          </span>
        )}
      </button>

      {/* Layer 2: 展开参数与结果 */}
      {expanded && (
        <div className="border-t px-3 py-2 text-xs font-mono">
          <details>
            <summary className="cursor-pointer text-muted-foreground">
              Arguments
            </summary>
            <pre className="mt-1 overflow-x-auto rounded bg-muted p-2">
              {JSON.stringify(toolCall.args, null, 2)}
            </pre>
          </details>
          <details>
            <summary className="mt-2 cursor-pointer text-muted-foreground">
              Result
            </summary>
            <pre
              className={cn(
                "mt-1 overflow-x-auto rounded bg-muted p-2",
                toolCall.status === "error" && "border border-red-300 bg-red-50"
              )}
            >
              {toolCall.result}
            </pre>
          </details>
        </div>
      )}
    </div>
  );
}

// 工具调用序列（多个工具连续调用时）
function ToolCallSequence({ calls }: { calls: ToolCallData[] }) {
  return (
    <div className="space-y-1">
      {calls.map((call) => (
        <ToolCallItem key={call.id} toolCall={call} />
      ))}
    </div>
  );
}
```

**消息流中的集成**：

```typescript
// ChatMessage 组件中，在 assistant 文本后插入工具调用序列
function ChatMessage({ message }: { message: Message }) {
  return (
    <div className="message">
      <Markdown content={message.content} />
      {message.toolCalls && message.toolCalls.length > 0 && (
        <ToolCallSequence calls={message.toolCalls} />
      )}
    </div>
  );
}
```

**合理性评估**：✅ 改进后的方案比原始 `<details>` 方案更适合桌面 Agent 场景。紧凑状态行让用户一目了然看到 Agent 做了什么，展开式详情避免信息过载。与 Baseline CLI 纯文本输出相比，桌面端的视觉优势得到发挥。

**涉及文件**：
- `src/components/ui/chat-ui.tsx` — 新增 ToolCallItem, ToolCallSequence 组件
- `src/App.tsx` — 处理工具调用事件并聚合到 message.toolCalls
- `src/lib/tauri.ts` — StreamTokenPayload 需包含 tool_call 事件（依赖 F8）

---

### F8：`StreamTokenPayload` 缺少工具事件类型

**来源**：06-frontend-gap §6.6

**位置**：[lib/tauri.ts](src/lib/tauri.ts)

**现状**：
```typescript
event_type:
  | 'text_delta'
  | 'thinking_delta'
  | 'thinking_start'
  | 'stream_complete'
  | 'stream_error'
// 缺少工具相关类型
```

**问题分析**：原始方案设计了 `tool_call_start` + `tool_call_end` 两个独立事件。经过审查，这会导致前端需要自己做事件关联（matching start with end），增加状态管理复杂度。更好的设计是合并为单一 `tool_call_update` 事件，用 `status` 字段区分生命周期阶段。

**修复方案 — 前端事件类型**：
```typescript
// src/lib/tauri.ts — StreamTokenPayload
type StreamTokenPayload = {
  stream_id: string;
  event_type:
    | 'text_delta'
    | 'thinking_delta'
    | 'thinking_start'
    | 'tool_call_update'   // 合并 start + end + result
    | 'stream_complete'
    | 'stream_error';
  // text/thinking 字段（不变）
  text?: string;
  thinking?: string;
  // tool_call_update 专用字段
  tool_call_id?: string;    // 关联同一工具调用的多个事件
  tool_name?: string;       // 工具名（如 "bash", "read_file"）
  tool_status?: 'queued' | 'running' | 'completed' | 'error';
  tool_args?: Record<string, unknown>;  // 工具参数
  tool_result?: string;     // 工具输出（completed/error 时存在）
  tool_duration_ms?: number;  // 执行耗时
}
```

**对应后端 `agent.rs` 的 `StreamTokenPayload`**：
```rust
// src-tauri/src/commands/agent.rs
#[derive(Serialize)]
struct StreamTokenPayload {
    stream_id: String,
    text: Option<String>,
    thinking: Option<String>,
    // tool_call_update 字段
    tool_call_id: Option<String>,
    tool_name: Option<String>,
    tool_status: Option<String>,       // "queued" | "running" | "completed" | "error"
    tool_args: Option<serde_json::Value>,
    tool_result: Option<String>,
    tool_duration_ms: Option<u64>,
    event_type: String,
}
```

**事件流示例**：
```
1. LLM 开始调用工具 → emit tool_call_update { status: "queued", tool_call_id: "abc", tool_name: "bash" }
2. 工具开始执行     → emit tool_call_update { status: "running", tool_call_id: "abc" }
3. 工具执行完成     → emit tool_call_update { status: "completed", tool_call_id: "abc", tool_result: "...", tool_duration_ms: 1234 }
4. LLM 继续文本响应 → emit text_delta { text: "我找到了以下文件..." }
```

**合理性评估**：✅ 改进后的单一事件类型比原始的 4 种独立事件类型更简洁。前端只需监听 `tool_call_update`，根据 `tool_status` 更新 UI 状态。`tool_call_id` 确保同一工具调用的多个事件能正确关联。

**涉及文件**：
- `src/lib/tauri.ts` — 扩展 StreamTokenPayload 类型
- `src-tauri/src/commands/agent.rs` — 扩展后端 StreamTokenPayload 结构体，在工具循环中 emit 事件
- `src/components/ui/chat-ui.tsx` — ToolCallItem 根据 tool_status 更新展示

---

### F9：Context Compaction 未接入

**来源**：05-session-gap §5.4

**位置**：[agent.rs:382-440](src-tauri/src/commands/agent.rs#L382-L440)

**代码验证**：
- `compact_session()` 和 `should_compact()` 存在于 [modules/runtime/compact.rs](src-tauri/src/modules/runtime/compact.rs)，与 Baseline 实现一致
- 有完整的测试用例 ([compact.rs:506-709](src-tauri/src/modules/runtime/compact.rs#L506-L709))
- `run_agent_turn` 在返回前没有调用 `should_compact()` 或 `compact_session()`
- `ConversationRuntime` 有 `compact()` 方法 ([conversation.rs:336-338](src-tauri/src/modules/runtime/conversation.rs#L336-L338))，但调用方（`run_agent_turn`）没有使用

**修复方案**：

```rust
// run_agent_turn 返回前添加
if should_compact(&updated_runtime_session, CompactionConfig::default()) {
    let compact_result = compact_session(&updated_runtime_session, CompactionConfig::default());
    // 使用 compacted session 保存
}
```

**合理性评估**：✅ 正确。Compaction 代码已存在且经过测试验证，只需在合适的时机调用。

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — run_agent_turn 返回前调用 compact
- `start_agent_stream` 的后台任务结束时也需要调用

---

### F10：双重模型选择器冲突

**来源**：06-frontend-gap §6.3

**位置**：[ChatUI.tsx:61](src/components/ui/chat-ui.tsx), [ChatWorkspace.tsx:48](src/modules/chat/components/ChatWorkspace.tsx#L48)

**代码验证**：
- `ChatUI` 有 `selectedModel` 状态（line 61），默认 `gpt-5.4-mini`，通过 ComposerDock 中的 `<Select>` 组件展示
- `ChatWorkspace` 的 header 中硬编码 `const modelLabel = 'GPT-5.4-Mini'`（line 48），**与 ChatUI 的 selectedModel 完全断开**
- `sendMessage()` 调用 `startAgentStream(sessionId, userMsg.content)` 时**不传 model 参数**
- 后端 `start_agent_stream` 从 `~/.claude/settings.json` 读取 model（[agent.rs:546](src-tauri/src/commands/agent.rs#L546)）
- 用户看到两个模型选择 UI（ComposerDock 的 Select + header 的 modelLabel），但它们互不感知

**问题本质**：这是经典的 "lift state up" 问题。`selectedModel` 应该是 App 级全局状态（与 permissionMode、cwd 同级），而非 ChatUI 的局部状态。

**修复方案**：

```typescript
// 1. src/App.tsx — 提升 selectedModel 为全局状态
const [selectedModel, setSelectedModel] = useState("gpt-5.4-mini");

// sendMessage 时传递 model
const sendMessage = async (content: string) => {
  await startAgentStream(sessionId, content, { model: selectedModel });
};

// 2. src/modules/chat/components/ChatWorkspace.tsx — header 接收全局 model
function ChatWorkspace({ selectedModel, onModelChange }: Props) {
  // 替换硬编码 modelLabel
  const modelLabel = formatModelName(selectedModel);
  // 点击 header model 时触发全局变更
  return <ModelDropdown value={selectedModel} onChange={onModelChange} />;
}

// 3. src/components/ui/chat-ui.tsx — ComposerDock 接收全局 model
function ChatUI({ selectedModel, onModelChange }: Props) {
  // 不再维护本地 selectedModel 状态
  return <Select value={selectedModel} onValueChange={onModelChange}>...</Select>;
}
```

**关键决策**：header 中的模型按钮和 ComposerDock 中的模型选择器**必须指向同一个全局状态**。两个 UI 入口可以共存（header 适合快速切换，ComposerDock 适合发送前确认），但它们必须同步。

**合理性评估**：✅ 正确。这是 React 状态管理的标准解决方案。改动小，只需提升状态 + 传递 props。注意如果 ChatWorkspace 和 ChatUI 不在同一组件树下（如通过不同的 route 渲染），则需要通过 Context 而非 props 传递。

**涉及文件**：
- `src/App.tsx` — 提升 selectedModel 为全局状态，传递给子组件
- `src/components/ui/chat-ui.tsx` — 移除本地 selectedModel 状态，接收 props
- `src/modules/chat/components/ChatWorkspace.tsx` — 替换硬编码 modelLabel
- `src-tauri/src/commands/agent.rs` — startAgentStream/runAgentTurn 接收 model 参数

---

### F11：SlashCommand 前端交互层（`/` 解析 + Tab 补全 + 下拉建议）

**来源**：09-skills-commands-gap Gap C2, C3（原 F11 + F12 合并）

**位置**：[ChatUI.tsx](src/components/ui/chat-ui.tsx) — `handleKeyDown`

**代码验证**：
- 前端 textarea 的 `handleKeyDown` 不检查 `/` 前缀
- 所有输入直接通过 `startAgentStream` 或 `runAgentTurn` 发给 LLM
- Baseline 的 `suggest_slash_commands()` ([commands/lib.rs:576-613](rust/crates/commands/src/lib.rs#L576-L613)) 使用 Levenshtein 距离模糊匹配
- `SlashCommand::parse()` 已存在于 `modules/commands/lib.rs`

**修复方案**：三层交互

```typescript
// src/components/ui/chat-ui.tsx — 新增 SlashCommandOverlay 组件

function ChatUI({ ... }) {
  const [slashOverlay, setSlashOverlay] = useState<{
    visible: boolean;
    selectedIndex: number;
    suggestions: string[];
    rawInput: string;
  } | null>(null);

  const handleInput = (value: string) => {
    // 1. 实时检测 / 前缀（50ms debounce）
    if (value.startsWith('/')) {
      const cmdPart = value.split(/\s+/)[0]; // "/he" from "/help foo"
      invoke('suggest_slash_commands_endpoint', { input: cmdPart, limit: 8 })
        .then(suggestions => {
          setSlashOverlay({
            visible: suggestions.length > 0,
            selectedIndex: 0,
            suggestions,
            rawInput: cmdPart,
          });
        });
    } else {
      setSlashOverlay(null);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (slashOverlay?.visible) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSlashOverlay(prev => ({
          ...prev!,
          selectedIndex: (prev!.selectedIndex + 1) % prev!.suggestions.length,
        }));
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSlashOverlay(prev => ({
          ...prev!,
          selectedIndex: (prev!.selectedIndex - 1 + prev!.suggestions.length) % prev!.suggestions.length,
        }));
        return;
      }
      if (e.key === 'Tab' || e.key === 'Enter') {
        // Tab/Enter 补全选中项
        e.preventDefault();
        const selected = slashOverlay.suggestions[slashOverlay.selectedIndex];
        setInput(selected);
        setSlashOverlay(null);
        return;
      }
      if (e.key === 'Escape') {
        setSlashOverlay(null);
        return;
      }
    }

    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (input.startsWith('/')) {
        // 2. 执行 slash command
        invoke('execute_slash_command', { input, sessionId: activeSessionId })
          .then((result) => {
            // 将结果作为系统消息插入聊天
            addSystemMessage(result);
          });
        return;
      }
      if (input.trim() && !isLoading) onSubmit();
    }
  };

  return (
    <div className="relative">
      <textarea ... onInput={handleInput} onKeyDown={handleKeyDown} />
      {/* 3. 下拉建议层 */}
      {slashOverlay?.visible && (
        <SlashCommandSuggestions
          items={slashOverlay.suggestions}
          selectedIndex={slashOverlay.selectedIndex}
          onSelect={(cmd) => {
            setInput(cmd);
            setSlashOverlay(null);
          }}
        />
      )}
    </div>
  );
}
```

**用户体验要求**：
- **实时检测**：输入 `/` 即触发建议，50ms debounce 避免频繁调用
- **Tab 补全**：Tab 键补全当前选中项（与 CLI 体验一致）
- **键盘导航**：↑↓ 箭头切换，Enter 确认，Escape 关闭
- **自动关闭**：输入不包含 `/` 前缀时自动关闭建议层

**合理性评估**：✅ 合并后的方案比原始分离的 F11+F12 更完整。用户输入 `/` 的瞬间就看到建议，不需要按 Tab 才触发。这更接近现代 IDE 的 IntelliSense 体验，而非传统 CLI 的 Tab 补全。需与 F5（后端 Tauri command 暴露）配合。

**涉及文件**：
- `src/components/ui/chat-ui.tsx` — 新增 slashOverlay 状态、handleInput debounce、SlashCommandSuggestions 组件
- `src-tauri/src/commands/slash.rs` — `suggest_slash_commands_endpoint` Tauri command（F5 的一部分）
- `src/lib/tauri.ts` — 新增 Tauri command 类型声明

---

### F12：`ToolSearch` 工具未注册

**来源**：03-tool-system-gap §3.5, 09-skills-commands-gap §9.5

**代码验证**：
- Baseline 的 `ToolSearch` 工具存在于 `rust/crates/tools/src/lib.rs`
- If2Ai 的 `register_builtin_tools()` 中没有

**修复方案**：在 `modules/tools/mod.rs` 的 `register_builtin_tools()` 中注册。

**合理性评估**：✅ 正确。工具实现参考 Baseline，注册到 ToolRegistry 即可。

**涉及文件**：
- `src-tauri/src/modules/tools/builtin/tool_search.rs` — 新建
- `src-tauri/src/modules/tools/mod.rs` — 注册

---

### F13：`Agent` 工具未注册

**来源**：03-tool-system-gap §3.5

**代码验证**：
- Baseline 的 `Agent` 工具允许嵌套调用其他 Agent
- If2Ai 未实现

**修复方案**：注册 `Agent` 工具，内部调用 `start_agent_stream` 或 `run_agent_turn`。

**合理性评估**：⚠️ 需要注意递归深度控制和 session 隔离。建议 P3 阶段再实现。

**优先级调整**：从 P2 降为 P3

**涉及文件**：
- `src-tauri/src/modules/tools/builtin/agent.rs` — 新建

---

### F14：`SkillSearch` 工具未注册

**来源**：09-skills-commands-gap

**修复方案**：类似 `ToolSearch`，搜索可用 Skill。

**合理性评估**：✅ 正确。依赖 `discover_skill_roots()` 实现（F6 的一部分）。

**优先级调整**：与 F6 绑定，在 F6 完成后实现

**涉及文件**：
- `src-tauri/src/modules/tools/builtin/skill_search.rs` — 新建

---

### F16：流式场景窗口关闭导致 session 丢失

**来源**：05-session-gap §5.6

**代码验证**：
- `start_agent_stream` 在 `tokio::spawn` 后台任务中运行（[agent.rs:632](src-tauri/src/commands/agent.rs#L632)）
- session 仅在后台任务结束时保存（line 758）
- 如果用户关闭应用，后台任务可能被取消

**修复方案**：
1. 后台任务定期（每 N 个 token）保存 session
2. 或使用 `tokio::task::JoinHandle` 跟踪任务，在窗口关闭时等待任务完成

**合理性评估**：✅ 正确。Tauri 有 `app_handle.on_window_event()` 可以捕获窗口关闭事件。

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — 定期保存 session

---

### F17：`/skills` `/agents` 命令未实现为 Tauri command

**来源**：09-skills-commands-gap

**代码验证**：
- Baseline 的 `handle_skills_slash_command()` ([commands/lib.rs:813-823](rust/crates/commands/src/lib.rs#L813-L823)) 和 `handle_agents_slash_command()` ([commands/lib.rs:801-811](rust/crates/commands/src/lib.rs#L801-L811)) 是完整的处理函数
- If2Ai 副本存在于 `modules/commands/lib.rs`
- 但未通过 Tauri command 暴露

**修复方案**：作为 F5 的一部分暴露。

**合理性评估**：✅ 正确。`handle_skills_slash_command` 和 `handle_agents_slash_command` 需要 `cwd: &Path` 参数，可以从 session 的工作目录获取。

**涉及文件**：
- `src-tauri/src/commands/slash.rs` — 新增 skills/agents endpoint

---

## 8.5 P3 修复：长期增强 + 新发现 UI Gap

### F18：SKILL.md 格式规范未定义

**来源**：09-skills-commands-gap Gap C16

**Baseline 格式**（[commands/lib.rs:1550-1578](rust/crates/commands/src/lib.rs#L1550-L1578)）：
```markdown
---
name: my-skill
description: 这是一个自定义技能
---

# 技能指令

这里可以包含详细的技能描述和执行指令...
```

**修复方案**：在 `docs/design-docs/` 中定义 SKILL.md 格式规范，然后在前端 SkillsSettingsPage 中验证。

**合理性评估**：✅ 正确。Baseline 已经定义了格式，If2Ai 只需文档化。

---

### F19：Session 与 Project 强绑定

**来源**：05-session-gap §5.5

**代码验证**：
- If2Ai 的 `AppSession` 有 `project_id: String` 字段
- Baseline 的 Session 没有 project_id

**修复方案**：允许 `project_id` 为空字符串表示"全局会话"。

**合理性评估**：✅ 正确。当前已经支持（`project_id.is_empty()` 检查存在于 session manager），但前端 UI 可能没有提供"无项目"模式。

---

### F20：MCP 协议完全缺失

**来源**：07-mcp-plugin-gap §7.1

**代码验证**：Baseline 的 `McpServerManager` ([rust/crates/runtime/src/mcp_stdio.rs](rust/crates/runtime/src/mcp_stdio.rs)) 管理 Stdio 传输的 MCP 服务器。

**修复方案**：Phase 5+ 实现。

---

### F21：插件系统完全缺失

**来源**：07-mcp-plugin-gap §7.2

**修复方案**：Phase 5+ 实现。

---

### F22：Sandbox 沙箱缺失

**来源**：04-permissions-gap §4.4

**代码验证**：Baseline 的 `SandboxConfig` 使用 `unshare` 命令进行 Linux namespace 隔离。

**修复方案**：Phase 5+ 实现。在 P1 权限修复后再考虑。

---

### F23：OAuth 登录 UI 缺失

**来源**：07-mcp-plugin-gap §7.4

**修复方案**：Phase 5+ 实现。

---

### F24：独立 SlashCommand Handler 实现

**来源**：09-skills-commands-gap Gap C4-C12

**Baseline 已有的 handler**：
| 命令 | Handler | 复杂度 |
|------|---------|--------|
| `/help` | `render_slash_command_help()` | 低（纯字符串渲染） |
| `/compact` | `compact_session()` | 低（已有实现） |
| `/status` | 需要收集 session/token/turn 信息 | 中 |
| `/model` | 切换模型配置 | 中 |
| `/permissions` | 切换权限模式 | 中 |
| `/branch` | `handle_branch_slash_command()` | 中（git 操作） |
| `/worktree` | `handle_worktree_slash_command()` | 中（git 操作） |
| `/commit` | `handle_commit_slash_command()` | 中（git 操作） |
| `/commit-push-pr` | `handle_commit_push_pr_slash_command()` | 高（需要 gh CLI） |
| `/pr` | 需要 LLM 生成 PR 描述 | 高 |
| `/issue` | 需要 LLM 生成 Issue 描述 | 高 |
| `/diff` | `git diff` 输出 | 低 |
| `/export` | 导出对话到文件 | 低 |
| `/session` | 列出/切换会话 | 中 |
| `/plugins` | `handle_plugins_slash_command()` | 高（需要插件系统） |
| `/agents` | `handle_agents_slash_command()` | 中（需要 Agent 发现） |
| `/skills` | `handle_skills_slash_command()` | 中（需要 Skill 发现） |

**修复方案**：优先实现 `/help`, `/compact`, `/diff`, `/export`（低复杂度），然后是 `/model`, `/permissions`, `/status`（中复杂度）。Git 相关命令和需要 LLM 的命令留给 P3。

---

### F25：会话状态显示不完整

**来源**：06-frontend-gap §6.7

**修复方案**：扩展 `SessionStatus` 组件，从 session 元数据中读取 token_count、turn 数等信息。详见 UI-8（SessionInfo 面板重新设计）。

---

### 新发现 UI Gap（代码审查发现，不在 bs_gap 原始报告中）

以下 7 项 UI/UX 差距是在逐行审查前端源码时发现，未被 06-frontend-gap 或 09-skills-commands-gap 报告覆盖。

#### UI-1：权限模式选择器仅为占位符

**发现位置**：`GlobalNavbar.tsx` 或 Settings 页面

**现状**：权限模式切换 UI 存在但为占位符，点击不触发任何后端调用。用户无法从 `ReadOnly` 切换到 `WorkspaceWrite` 或 `Prompt` 模式。

**修复方案**：将权限模式选择器连接到后端的 `PermissionMode` 状态，切换时更新 `ToolContext` 中的 `permission_mode` 字段。

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — 接收 `permission_mode` 参数
- `src/App.tsx` — 全局 permissionMode 状态
- Settings 页面 — 真实后端连接

---

#### UI-2：分支选择器占位符无功能

**发现位置**：`GlobalNavbar.tsx` 或 `ChatWorkspace.tsx` header

**现状**：分支选择器 UI 元素存在但不触发任何 git 操作。与 Baseline 的 `/branch` 和 `/worktree` 命令对应。

**修复方案**：先实现 F5（SlashCommand 暴露），然后通过 `/branch` 命令或专用 Tauri command 提供 git 分支列表和切换能力。

**涉及文件**：
- `src-tauri/src/commands/slash.rs` — `/branch` 命令
- 前端分支选择器组件 — 连接真实数据

---

#### UI-3：ChatWorkspace header 按钮无功能

**发现位置**：[ChatWorkspace.tsx:48](src/modules/chat/components/ChatWorkspace.tsx#L48)

**现状**：header 中有 Play/terminal/submit 三个按钮，但 onClick 全部为空实现或无效回调。这些按钮给用户"可操作"的错觉但点击无响应。

**修复方案**：
- **Play 按钮**：连接到"重新执行当前对话"或"重放"功能
- **Terminal 按钮**：打开内嵌终端面板（如果有 bash 工具输出）
- **Submit 按钮**：与 ComposerDock 的发送按钮统一，避免重复入口

**建议**：如果短期内不实现这些功能，应**先移除或 disable 这些按钮**，避免虚假交互比没有更糟糕。

---

#### UI-4：Header 硬编码 diff 统计数字

**发现位置**：[ChatWorkspace.tsx](src/modules/chat/components/ChatWorkspace.tsx)

**现状**：header 显示硬编码的 `+4,523 / -3,223` diff 统计，是迁移时遗留的示例数据。

**修复方案**：
- 短期：移除硬编码数字，显示占位文本如 "—"
- 长期：通过 `git diff --stat` Tauri command 获取真实数据

---

#### UI-5：`Message.role` 缺少 `tool` 角色

**发现位置**：[chat-ui.tsx](src/components/ui/chat-ui.tsx) — Message 接口

**现状**：
```typescript
interface Message {
  role: "user" | "assistant";  // 缺少 "tool"
  content: string;
}
```

后端的 `ContentBlock::ToolResult` 在 session 中存储为独立消息，但前端 Message 类型不支持 `role: "tool"`，导致工具结果消息无法正确渲染。

**修复方案**：
```typescript
interface Message {
  role: "user" | "assistant" | "tool";
  content: string;
  toolCallId?: string;      // 关联到对应的 tool_use
  toolName?: string;        // 工具名
  isError?: boolean;        // 标记工具执行错误
}
```

**涉及文件**：
- `src/components/ui/chat-ui.tsx` — 扩展 Message 接口
- `src/App.tsx` — 处理 tool_result 事件时创建 role: "tool" 消息

---

#### UI-6：Session 恢复丢失 tool_call 上下文

**发现位置**：Session 加载逻辑

**现状**：当用户重新打开历史会话时，session 文件中可能包含 `ContentBlock::ToolResult` 消息，但前端在恢复会话时只渲染 `user` 和 `assistant` 角色的消息，工具调用上下文完全丢失。用户看到的是不完整的对话历史。

**修复方案**：会话恢复时遍历所有 `ContentBlock` 类型，将 `ToolResult` 转换为前端 `role: "tool"` 消息展示。

---

#### UI-7：无流式中断能力

**发现位置**：[ChatUI.tsx](src/components/ui/chat-ui.tsx), [App.tsx](src/App.tsx)

**现状**：当 `startAgentStream` 正在运行时，用户无法取消正在进行的 Agent 响应。只能等待 LLM 完成所有轮次（包括工具调用）。对于错误的 prompt 或长时间运行的工具调用，用户无法介入。

**修复方案**：
```typescript
// App.tsx — 维护 stream abort handle
let abortController: AbortController | null = null;

const stopAgent = () => {
  abortController?.abort();
  abortController = null;
};

// ChatUI 中显示 Stop 按钮
{isStreaming && <Button onClick={stopAgent}>Stop</Button>}
```

后端需要支持中断信号：`tokio::select!` 监听取消通道。

**涉及文件**：
- `src/App.tsx` — 维护 abortController
- `src-tauri/src/commands/agent.rs` — 支持取消信号
- `src/components/ui/chat-ui.tsx` — 新增 Stop 按钮

---

#### UI-8：`TodoWrite` 工具未注册到 Phase 4 ToolRegistry + 前端无 Todo 面板

**发现位置**：后端工具注册 + 前端 UI

**现状分析**：

1. **后端 `TodoWrite` 工具代码存在但未注册**
   - `src-tauri/src/modules/tools/lib.rs`（Baseline 副本）包含完整的 `TodoWrite` ToolSpec 定义、`execute_todo_write()` 执行逻辑、`todo_store_path()` 持久化路径（`.claw-todos.json` → 应改为 `.if2ai-todos.json`）
   - 但该工具**未注册到 Phase 4 的 `ToolRegistry`**（[modules/tools/mod.rs](src-tauri/src/modules/tools/mod.rs) 的 `register_builtin_tools()` 中没有 TodoWrite）
   - Phase 4 的 ToolRegistry 使用 `ToolHandler` 模式（`builtin/bash.rs`, `builtin/file_read.rs` 等），而 `lib.rs` 的 `TodoWrite` 走的是 `GlobalToolRegistry` 的 `execute_tool()` match 分支，**两条工具系统互不兼容**

2. **前端完全缺失 Todo 展示**
   - `ChatUI` 的 `Message` 接口没有 todo 相关字段
   - 无任何 Todo 面板或 Todo 列表组件
   - 当 LLM 调用 TodoWrite 工具时，工具结果会作为普通 tool_result 返回，但前端只会显示 JSON 文本

**Baseline 的工作机制**：
```
LLM 分析用户需求 → 调用 TodoWrite 工具
    │
    ├─ 输入: { todos: [
    │     { content: "读取文件", activeForm: "读取目标文件", status: "pending" },
    │     { content: "创建文件", activeForm: "创建新文件", status: "pending" },
    │     { content: "验证结果", activeForm: "验证输出", status: "pending" },
    │   ]}
    │
    ├─ 保存到 .claw-todos.json
    ├─ 返回: { oldTodos: [], newTodos: [...], verificationNudgeNeeded: null }
    │
    └─ LLM 开始逐项执行，每完成一项就调用 TodoWrite 更新 status:
         status: "completed" → status: "in_progress" → 下一项
```

**这是一个关键的 Agent 行为可视化工具**。没有它，用户只能看到 Agent 在执行，却不知道：
- Agent 计划做哪些事
- 当前正在做什么
- 还剩多少未完成

**UI/UX 设计方案 — 对齐 Codex/Qwen 桌面 Agent**：

```
┌─────────────────────────────────────────────────────────┐
│  ChatWorkspace                                          │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Todo Panel (Composer 上方，可折叠)               │   │
│  │                                                 │   │
│  │ Tasks (3/5 completed)                    [▾]    │   │
│  │ ─────────────────────────────────────────────── │   │
│  │ ✓ 读取目标文件                          (1.2s)   │   │
│  │ ✓ 创建新文件                            (3.4s)   │   │
│  │ ◐ 重构模块结构                      running...   │   │
│  │ ○ 验证测试结果                                   │   │
│  │ ○ 提交变更                                       │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Composer (输入框)                                │   │
│  │ ┌───────────────────────────────────────────┐   │   │
│  │ │ 告诉我你的需求...                      [→] │   │   │
│  │ └───────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Chat Transcript (消息历史)                       │   │
│  │                                                 │   │
│  │ User: 请帮我重构这个模块                         │   │
│  │                                                 │   │
│  │ Assistant: 好的，我来完成这个任务。              │   │
│  │ ✓ read_file  module.ts  (1.2s)                  │   │
│  │ ✓ file_write  module.new.ts  (3.4s)             │   │
│  │ ◐ bash  npm test  running...                    │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**数据流设计**：

```
LLM 调用 TodoWrite 工具
    │
    ├─ 后端: execute_todo_write() 保存到 .if2ai-todos.json
    ├─ 后端: tool_result 返回 JSON:
    │     { oldTodos: [...], newTodos: [...], verificationNudgeNeeded: null }
    │
    └─ 前端 SSE: tool_call_update 事件
         tool_name: "TodoWrite"
         tool_result: "{ ... }"
         │
         └─ App.tsx 解析 newTodos 字段
              └─ 更新全局 todos 状态
                   └─ TodoPanel 实时渲染
```

**修复方案**：

```typescript
// src/components/ui/TodoPanel.tsx — 新建
interface TodoItem {
  content: string;
  activeForm: string;
  status: "pending" | "in_progress" | "completed";
}

function TodoPanel({ todos }: { todos: TodoItem[] }) {
  if (todos.length === 0) return null;

  const completedCount = todos.filter(t => t.status === "completed").length;
  const isCollapsed = todos.every(t => t.status === "completed");

  return (
    <div className="border-b bg-muted/20 px-4 py-2">
      {/* Header: 进度摘要 */}
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span className="font-medium text-foreground">
          Tasks ({completedCount}/{todos.length} completed)
        </span>
        {isCollapsed && <span>All tasks completed ✓</span>}
      </div>

      {/* Todo List */}
      {!isCollapsed && (
        <div className="mt-1 space-y-0.5">
          {todos.map((todo, i) => (
            <TodoItemRow key={i} todo={todo} />
          ))}
        </div>
      )}
    </div>
  );
}

function TodoItemRow({ todo }: { todo: TodoItem }) {
  const icon = {
    pending: "○",
    in_progress: "◐",
    completed: "✓",
  }[todo.status];

  const color = {
    pending: "text-muted-foreground",
    in_progress: "text-blue-500",
    completed: "text-green-500",
  }[todo.status];

  const animation = todo.status === "in_progress" ? "animate-pulse" : "";

  return (
    <div className={cn("flex items-center gap-2 text-sm", color, animation)}>
      <span className="w-4 text-center font-mono">{icon}</span>
      <span className="flex-1 truncate">{todo.activeForm}</span>
    </div>
  );
}
```

**集成到 ChatUI**：

```typescript
// src/components/ui/chat-ui.tsx — 在 Composer 上方插入 TodoPanel
interface ChatUIProps {
  // ... existing props
  todos?: TodoItem[];  // 新增
}

export function ChatUI({ todos, ... }: ChatUIProps) {
  return (
    <div className="flex flex-col h-full">
      {/* Transcript */}
      <ChatTranscript messages={messages} />

      {/* Todo Panel — 在 Composer 上方 */}
      {todos && todos.length > 0 && (
        <TodoPanel todos={todos} />
      )}

      {/* Composer */}
      <ComposerDock ... />
    </div>
  );
}
```

**后端适配**：

```rust
// src-tauri/src/modules/tools/builtin/todo_write.rs — 新建
pub fn todo_write_tool_entry() -> ToolEntry {
    ToolEntry {
        name: "TodoWrite".to_string(),
        description: "Update the structured task list for the current session.".to_string(),
        input_schema: json!({ ... }),  // 同 baseline
        handler: Arc::new(|args, _ctx| {
            Box::pin(async move {
                let input: TodoWriteInput = serde_json::from_value(args)
                    .map_err(|e| format!("Invalid TodoWrite input: {}", e))?;
                let result = execute_todo_write(input)?;
                serde_json::to_string_pretty(&result)
                    .map_err(|e| e.to_string())
            })
        }),
        required_permission: PermissionMode::WorkspaceWrite,
    }
}

// modules/tools/mod.rs — 注册
if let Err(e) = registry.register(builtin::todo_write::todo_write_tool_entry()) {
    eprintln!("Failed to register todo_write tool: {}", e);
}

// .if2ai-todos.json 路径: 从 CLAW_TODO_STORE 改为 IF2AI_TODO_STORE 或 ~/.if2ai/
```

**SSE 事件关联**：

当 `TodoWrite` 工具被调用时，F8 的 `tool_call_update` 事件会携带 `tool_name: "TodoWrite"` 和 `tool_result: "{ \"newTodos\": [...] }"`。前端需要：

1. 在 `App.tsx` 的 stream event handler 中检测 `tool_name === "TodoWrite"`
2. 解析 `tool_result` 中的 `newTodos` 数组
3. 更新全局 `todos` 状态
4. 传给 `ChatUI` 的 `todos` prop

```typescript
// App.tsx — stream event handler
const [todos, setTodos] = useState<TodoItem[]>([]);

if (payload.event_type === 'tool_call_update' && payload.tool_name === 'TodoWrite') {
  try {
    const result = JSON.parse(payload.tool_result || '{}');
    if (result.newTodos) {
      setTodos(result.newTodos);
    }
  } catch (e) {
    console.error('Failed to parse TodoWrite result:', e);
  }
}

// 所有任务完成后清空
if (payload.event_type === 'stream_complete') {
  setTodos([]);
}
```

**合理性评估**：✅ 这是桌面 Agent 与 CLI 的核心差异化优势。CLI 只能看到 JSON 输出，桌面端可以实时渲染进度条和状态图标。与 Codex CLI 的 terminal todo display 和 Qwen 的 task list UI 对齐，是用户感知 Agent "智能程度"的最直接窗口。建议 Phase 2 优先实现。

**涉及文件**：
- `src-tauri/src/modules/tools/builtin/todo_write.rs` — 新建（Phase 4 ToolHandler 适配）
- `src-tauri/src/modules/tools/mod.rs` — 注册 TodoWrite
- `src-tauri/src/modules/tools/lib.rs` — 修改 `todo_store_path()` 使用 `IF2AI_TODO_STORE` / `.if2ai-todos.json`
- `src/components/ui/TodoPanel.tsx` — 新建
- `src/components/ui/chat-ui.tsx` — 集成 TodoPanel 到 Composer 上方
- `src/App.tsx` — 全局 todos 状态 + SSE 事件解析
- `src/lib/tauri.ts` — StreamTokenPayload 支持 TodoWrite 结果

---

### N4：`commands/mod.rs` 未导入 `lib.rs` 的 SlashCommand 内容

**来源**：10-comprehensive-audit §10.3 N4

**位置**：[src-tauri/src/commands/mod.rs](src-tauri/src/commands/mod.rs)

**现状**：
```rust
// commands/mod.rs
pub mod agent;
pub mod project;
pub mod session;
pub mod window;
// ❌ 缺少: pub mod commands_lib; 或 re-export
```

`modules/commands/lib.rs` 包含完整的 SlashCommand 系统（28 命令、parse、suggest、handle），但：
- 不在 `commands/mod.rs` 的模块树中
- 编译后被包含（main.rs 可能通过 modules 引入），但没有任何 Tauri command 引用它
- 是**死代码**

**修复方案（后端）**：

```rust
// src-tauri/src/modules/commands/mod.rs — 新建或修改
pub mod lib;  // 或重命名为 mod.rs

// 在 mod.rs 中 re-export 需要的公开项
pub use lib::{SlashCommand, SlashCommandSpec, suggest_slash_commands, handle_slash_command};
```

或直接让 `commands/mod.rs` 包含 lib.rs 作为模块：
```rust
// src-tauri/src/commands/mod.rs
pub mod agent;
pub mod project;
pub mod session;
pub mod window;
pub mod slash;  // 新增：见 F5
```

**修复方案（前端）**：不需要前端变更。这是纯后端模块连接问题，F5 的 `commands/slash.rs` 会解决暴露问题。

**优先级**：与 F5（SlashCommand 暴露）绑定，F5 实现时自动解决。

**涉及文件**：
- `src-tauri/src/modules/commands/mod.rs` — 确保 lib.rs 被正确导入
- `src-tauri/src/commands/slash.rs` — F5 新建文件，引用 commands lib

---

### N6：硬编码中文错误消息

**来源**：10-comprehensive-audit §10.3 N6

**位置**：[agent.rs:346-348](src-tauri/src/commands/agent.rs#L346-L348)

**现状**：
```rust
return Err(format!(
    "无法连接 AI 服务: {}. 请检查 ~/.claude/settings.json 配置是否正确。",
    e
));
```

**影响**：不影响功能。Baseline 使用英文错误消息。国际化场景下应统一为英文或 i18n。

**修复方案（后端）**：
```rust
return Err(format!(
    "Failed to connect to AI service: {}. Please check ~/.claude/settings.json configuration.",
    e
));
```

**修复方案（前端）**：前端 `lib/tauri.ts` 中将后端返回的错误消息映射为用户友好的本地化提示：
```typescript
// lib/tauri.ts — 错误消息映射
function formatErrorMessage(error: string): string {
  if (error.includes("Failed to connect")) {
    return "无法连接 AI 服务，请检查 ~/.claude/settings.json 配置";
  }
  return error;
}
```

**优先级**：P3（低优先级，可在其他修复附带处理）

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — 改为英文
- `src/lib/tauri.ts` — 可选：前端本地化映射

---

### N7：GlobalNavbar Automation/Skills 图标区无功能

**来源**：10-comprehensive-audit §10.3 N7

**位置**：[src/modules/app-shell/components/GlobalNavbar.tsx](src/modules/app-shell/components/GlobalNavbar.tsx)

**现状**：icon rail 有 Chat/Skills/Automation 三个区段，Skills 和 Automation 的图标点击后无任何响应。

**修复方案（前端）**：

短期（P2）：**隐藏或 disable 无功能图标**
```tsx
// GlobalNavbar.tsx
// 暂时注释掉 Skills 和 Automation 区段，或添加 disabled 样式
{/* TODO: 连接 F6 Skill 系统后启用 */}
{/* <SkillsSection /> */}
{/* TODO: 连接自动化系统后启用 */}
{/* <AutomationSection /> */}
```

长期（P3）：
- **Skills 图标**：连接到 `/skills` SlashCommand 或 SkillsSettingsPage
- **Automation 图标**：连接到 cron 调度器 UI（Phase 4 已有 cron 工具）

```tsx
// 长期方案
<Icon onClick={() => invoke('execute_slash_command', { input: '/skills', sessionId })}>
  Skills
</Icon>
```

**涉及文件**：
- `src/modules/app-shell/components/GlobalNavbar.tsx` — 暂时隐藏无功能图标
- `src/components/settings/SkillsSettingsPage.tsx` — F6 连接真实数据后启用

---

### N8：WorkbenchBackdrop.tsx 纯装饰组件

**来源**：10-comprehensive-audit §10.3 N8

**位置**：[src/components/WorkbenchBackdrop.tsx](src/components/WorkbenchBackdrop.tsx)

**现状**：只是一个带背景色的 div，不连接任何状态、事件或 props。

**修复方案（前端）**：

**直接移除**。装饰性背景应在父组件（如 ChatWorkspace）中通过 CSS 实现，而非独立组件文件。

```bash
# 删除文件
rm src/components/WorkbenchBackdrop.tsx

# 清理所有 import
# grep -r "WorkbenchBackdrop" src/ → 移除引用
```

如果确实需要背景效果，内联到 ChatWorkspace 的 CSS 中：
```tsx
// ChatWorkspace.tsx
<div className="relative">
  <div className="absolute inset-0 bg-gradient-to-b from-muted/10 to-transparent" />
  {/* 内容 */}
</div>
```

**优先级**：P3（清理任务，不影响功能）

**涉及文件**：
- `src/components/WorkbenchBackdrop.tsx` — 删除
- `src/modules/chat/components/ChatWorkspace.tsx` — 移除 import

---

### N9：SessionStatus.tsx 已定义但从未被 import

**来源**：10-comprehensive-audit §10.3 N9

**位置**：[src/components/SessionStatus.tsx](src/components/SessionStatus.tsx)

**现状**：组件已定义（Idle/Running/Error/Working 状态展示），但 grep 验证无任何文件 import 它。

**修复方案（前端）**：

短期（P2）：**接入到 App.tsx 或 ChatUI**
```tsx
// App.tsx
import { SessionStatus } from './components/SessionStatus';

// 在 ChatUI 上方或 header 中展示
<SessionStatus status={agentStatus} tokenCount={tokenCount} turnCount={turnCount} />
```

需要扩展 SessionStatus 组件的 props 以支持更多状态信息：
```tsx
interface SessionStatusProps {
  status: 'idle' | 'running' | 'error' | 'working';
  tokenCount?: number;
  turnCount?: number;
  modelName?: string;
  permissionMode?: string;
}
```

**修复方案（后端）**：需要提供 session 元数据接口：
```rust
// commands/session.rs — 新增
#[tauri::command]
pub fn get_session_status(session_id: String) -> Result<SessionStatus, String> {
    // 返回 token_count, turn_count, model, permission_mode 等
}
```

**优先级**：P3（与 F25 会话状态面板合并实现）

**涉及文件**：
- `src/components/SessionStatus.tsx` — 扩展 props
- `src/App.tsx` — import 并传递状态
- `src-tauri/src/commands/session.rs` — 新增 get_session_status

---

### N10：两个 ChatUI 入口导致状态分裂

**来源**：10-comprehensive-audit §10.3 N10

**位置**：[src/App.tsx](src/App.tsx) 和 [src/modules/chat/components/ChatWorkspace.tsx](src/modules/chat/components/ChatWorkspace.tsx)

**现状**：
- `App.tsx` 直接引用 `ChatUI` 并管理消息状态
- `ChatWorkspace.tsx` 也引用 `ChatUI` 并传入自己的 props
- 两套独立的 ChatUI 实例，各自维护独立的消息/状态

**修复方案（前端）**：

**统一到单一入口**。选择 App.tsx 作为唯一真相源，ChatWorkspace 仅作为布局容器：

```tsx
// App.tsx — 唯一管理 chat 状态
const [messages, setMessages] = useState<Message[]>([]);
const [isLoading, setIsLoading] = useState(false);

// 传递给 ChatWorkspace
<ChatWorkspace
  messages={messages}
  onSendMessage={handleSendMessage}
  isLoading={isLoading}
  selectedModel={selectedModel}
  onModelChange={setSelectedModel}
/>

// ChatWorkspace.tsx — 纯布局，不管理 chat 状态
function ChatWorkspace({ messages, onSendMessage, isLoading, ... }: Props) {
  return (
    <div className="flex flex-col h-full">
      <Header ... />
      <ChatUI
        messages={messages}
        onSubmit={onSendMessage}
        isLoading={isLoading}
        ...
      />
    </div>
  );
}
```

**具体步骤**：
1. 从 ChatWorkspace 移除所有 chat 状态管理逻辑
2. 所有 props 从 App.tsx 透传
3. ChatWorkspace 变为纯展示组件

**优先级**：P2（在 F2 工具循环修复前统一状态管理，避免后续重构成本增加）

**涉及文件**：
- `src/App.tsx` — 提升为唯一状态管理
- `src/modules/chat/components/ChatWorkspace.tsx` — 改为纯展示组件
- `src/components/ui/chat-ui.tsx` — 确保 props 接口统一

---

### N11：`execute_todo_write` 使用 `CLAW_TODO_STORE` 环境变量

**来源**：10-comprehensive-audit §10.3 N11

**位置**：[src-tauri/src/modules/tools/lib.rs](src-tauri/src/modules/tools/lib.rs)

**现状**：
```rust
fn todo_store_path() -> PathBuf {
    let env_var = std::env::var("CLAW_TODO_STORE").unwrap_or_default();
    if env_var.is_empty() {
        std::env::current_dir().unwrap_or_default().join(".claw-todos.json")
        // ↑ 应改为 .if2ai-todos.json 或 ~/.if2ai/todos.json
    } else {
        PathBuf::from(env_var)
    }
}
```

**修复方案（后端）**：
```rust
fn todo_store_path() -> PathBuf {
    let env_var = std::env::var("IF2AI_TODO_STORE").unwrap_or_default();
    if env_var.is_empty() {
        // 优先使用项目目录，fallback 到用户目录
        std::env::current_dir()
            .unwrap_or_default()
            .join(".if2ai-todos.json")
    } else {
        PathBuf::from(env_var)
    }
}
```

同时适配为 ToolHandler 模式以注册到 Phase 4 的 ToolRegistry：

```rust
// src-tauri/src/modules/tools/builtin/todo_write.rs — 新建
use super::super::registry::{ToolEntry, ToolHandler, ToolError};
use crate::modules::tools::context::SharedToolContext;

pub fn todo_write_tool_entry() -> ToolEntry {
    ToolEntry {
        name: "TodoWrite".to_string(),
        toolset: "utility".to_string(),
        description: "Update the structured task list for the current session.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string", "description": "Task description" },
                            "activeForm": { "type": "string", "description": "Present-tense description of current action" },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["todos"]
        }),
        max_result_size: Some(10240),
        timeout_secs: Some(30),
        disabled: false,
        handler: Arc::new(|args, _ctx| {
            Box::pin(async move {
                let input: TodoWriteInput = serde_json::from_value(args)
                    .map_err(|e| ToolError::Handler(format!("Invalid input: {}", e)))?;
                let result = execute_todo_write(input)
                    .map_err(ToolError::Handler)?;
                serde_json::to_string_pretty(&result)
                    .map_err(|e| ToolError::Handler(e.to_string()))
            })
        }),
    }
}
```

**修复方案（前端）**：无额外变更。F8 的 `tool_call_update` 事件已覆盖 TodoWrite 结果传输。UI-8 的 TodoPanel 负责渲染。

**优先级**：P2（与 UI-8 TodoWrite 工具注册合并实现）

**涉及文件**：
- `src-tauri/src/modules/tools/lib.rs` — 修改环境变量和默认路径名
- `src-tauri/src/modules/tools/builtin/todo_write.rs` — 新建（ToolHandler 适配）
- `src-tauri/src/modules/tools/mod.rs` — 注册

---

### N12：`call_api()` 的 `tools: None` 分支永远不会触发

**来源**：10-comprehensive-audit §10.3 N12

**位置**：[agent.rs:232-236](src-tauri/src/commands/agent.rs#L232-L236)

**现状**：
```rust
// call_api() 内部
let tool_defs = self.tool_registry.get_definitions(None);
// ↑ 总是从 registry 获取，完全忽略 request.tools 参数
```

**影响**：功能上无 bug（工具定义最终会被注入），但：
1. `ApiRequest.tools` 字段被完全忽略，API 契约不一致
2. 如果将来换用其他 ApiClient 实现，工具会静默丢失
3. 代码读者被 `tools: None` 误导

**修复方案（后端）**：

在 F1 修复时一并处理。正确做法是让 `call_api()` 优先使用 `request.tools`，如果为 None 则 fallback 到 registry：

```rust
// RealApiClient::call_api()
let tool_defs = request.tools.clone().unwrap_or_else(|| {
    self.tool_registry.get_definitions(None)
});
```

这样无论调用方传不传工具定义，行为都是正确的。

**修复方案（前端）**：无需变更。

**优先级**：P0（与 F1 绑定，同一修复）

**涉及文件**：
- `src-tauri/src/commands/agent.rs` — 修改 call_api() 的 tool_defs 获取逻辑
- `src-tauri/src/modules/runtime/conversation.rs` — F1 修复后传 tools: Some(...)

---

## 8.6 Gap 移除/合并记录

| 原编号 | 操作 | 原因 | Phase/slice |
|--------|------|------|-------------|
| Hook 系统未接入 (run_turn 路径) | **已移除** | `ConversationRuntime::run_turn()` 中 Hook 已经正确接入（[conversation.rs:281, 298-300](src-tauri/src/modules/runtime/conversation.rs#L281-L300)） | — |
| Hook 系统未接入 (stream 路径) | **合并到 F2** | 流式工具循环重构时需同时接入 Hook，不单独列为 Issue | **Phase 5B / 5b.3** |
| `ToolResult.is_error` 缺失 | **已移除** | 字段已存在于 [session.rs:37](src-tauri/src/modules/runtime/session.rs#L37) | — |
| `Agent` 工具未注册 | **降为 P3** (F13) | 需要递归深度控制和 session 隔离，复杂度高 | **Phase 5E / 5e.5** |
| `SkillSearch` 工具未注册 | **合并到 F6** (F14) | 依赖 Skill 发现系统实现 | **Phase 5C / 5c.7** |
| F11 + F12 (SlashCommand 解析 + Tab 补全) | **合并为 F11** | 统一为 "SlashCommand 前端交互层"，包含 `/` 检测 + Tab 补全 + 下拉建议 | **Phase 5D / 5d.7** |
| F19 (`is_error` 字段) | **已移除** | Gap 不存在，字段已存在 | — |
| 08 中 Issue 10/11 重复 Compaction | **已修复** | 统一为 F9 | **Phase 5D / 5d.5** |
| 08 中 8.3/8.3b/8.4/8.5 编号混乱 | **已修复** | 统一按 F1-F25 + UI-1~UI-8 编号 | — |
| N2 (RealApiClient 内部获取工具定义) | **合并到 F1** | 非独立 gap，F1 修复时一并处理 | **Phase 5A / 5a.1** |
| N12 (`call_api()` 忽略 `request.tools`) | **合并到 F1** | 与 F1/N2 同源，同一修复 | **Phase 5A / 5a.2** |

## 8.7 代码验证总结

| 评估结果 | 条目数 | 说明 |
|---------|--------|------|
| ✅ 验证通过，修复方案已落地 Phase 5A-5E | 37 | F1-F17, F25, UI-1~UI-8, N1, N3-N12 |
| ⏸️ 未排期（P3 长期） | 7 | F18-F24 中部分项（MCP, 插件, Sandbox, OAuth, 独立 Handler） |
| ❌ Gap 不存在（已移除） | 3 | Hook 系统 (run_turn) 已接入、is_error 字段已存在、F19 重复 |

### Phase 5 各 Phase 覆盖统计

| Phase | 标题 | Slices | 覆盖 Fix 项 |
|-------|------|--------|-------------|
| 5A | Agent 核心修复 | 4 (5a.1-5a.4) | F1, N5, N12 |
| 5B | 流式工具循环重写 | 5 (5b.1-5b.5) | F2, F8, F15, UI-5 |
| 5C | 安全与系统连接 | 7 (5c.1-5c.7) | N1, N3, F3, F4, F5, F6, F14, F17, N4, UI-1 |
| 5D | UX 完善 | 8 (5d.1-5d.8) | N10, N7, F7, F9, F10, F11, UI-8, N11 |
| 5E | 长期增强 + 缺失补回 | 5 (5e.1-5e.5) | F12, F13, F16, UI-7, UI-4, UI-6, N6, N8, N9, F25 |

### N1-N12 追加条目汇总

| 编号 | 条目 | 优先级 | 修复方案 | Phase/slice |
|------|------|--------|---------|-------------|
| N5 | `start_agent_stream` 无系统提示 | **P0** | agent.rs 使用 SystemPromptBuilder（前后端均涉及） | **Phase 5A / 5a.3** |
| N1 | SystemPromptBuilder 完全未使用 | **P1** | 替换两处硬编码 prompt（纯后端） | **Phase 5C / 5c.1** |
| N3 | 两套工具系统共存 | **P1** | 统一为单一 ToolRegistry，适配 ToolHandler（纯后端） | **Phase 5C / 5c.2** |
| N4 | commands/mod.rs 未导入 lib.rs | P2（与 F5 绑定） | 模块连接（纯后端） | **Phase 5C / 5c.5** |
| N10 | 两个 ChatUI 入口状态分裂 | P2 | 统一到 App.tsx 为唯一真相源（纯前端） | **Phase 5D / 5d.1** |
| N11 | TodoWrite 使用 CLAW_TODO_STORE | P2（与 UI-8 绑定） | 改为 IF2AI_TODO_STORE + ToolHandler 适配（纯后端） | **Phase 5D / 5d.4** |
| N7 | GlobalNavbar 图标无功能 | P2 | 暂时隐藏，F6 后启用（纯前端） | **Phase 5D / 5d.2** |
| N9 | SessionStatus 从未被 import | P3（与 F25 绑定） | 接入 session 元数据（前后端） | **Phase 5E / 5e.3** |
| N6 | 硬编码中文错误消息 | P3 | 改为英文 + 前端本地化（前后端） | **Phase 5E / 5e.3** |
| N8 | WorkbenchBackdrop 纯装饰 | P3 | 直接移除（纯前端） | **Phase 5E / 5e.3** |
| N2 | RealApiClient 内部获取工具定义 | 合并到 F1 | — | **Phase 5A / 5a.1（隐含）** |
| N12 | call_api() 忽略 request.tools | 合并到 F1 | — | **Phase 5A / 5a.2** |

---

## 8.8 推荐实施顺序

```
Phase 0（P0 紧急修复）:
  ├─ F1: 修复 run_agent_turn 的 tools: None
  │     → 扩展 ToolExecutor trait 增加 get_definitions()
  │     → ToolRegistryExecutor 实现 get_definitions()
  │     → conversation.rs 修改 tools: Some(executor.get_definitions())
  │     → N12: call_api() 优先使用 request.tools（fallback 到 registry）
  │
  ├─ N5: start_agent_stream 添加 system_prompt
  │     → 使用 SystemPromptBuilder 替代 system_prompt: None
  │
  └─ F2: 重写 start_agent_stream 工具循环
        → 实现 InputJsonDelta 累积 + ContentBlockStop 提取
        → 工具执行 + tool_result 回传
        → SSE 新增工具事件类型（F8）
        → ChatUI 工具调用展示 — 渐进式披露（F7）
        → Hook 系统接入（F15 合并到 F2）
        → UI-5: Message.role 扩展 "tool"（前置依赖）

Phase 1（P1 安全 + SlashCommand/Skill 连接）:
  ├─ N1: 接入 SystemPromptBuilder
  │     → 替换 run_agent_turn 的硬编码三行 prompt
  │
  ├─ N3: 统一工具注册系统
  │     → 将 lib.rs 的 GlobalToolRegistry 废弃
  │     → 所有工具适配 ToolHandler 模式注册到 ToolRegistry
  │
  ├─ F5: SlashCommand 系统暴露到前端
  │     → 新建 commands/slash.rs
  │     → N4: 确保 commands/mod.rs 正确导入 lib.rs
  │     → parse/list/suggest/execute Tauri commands
  │
  ├─ F6: Skill 工具注册 + 发现
  │     → 新建 builtin/skill.rs
  │     → discover_skill_roots() + resolve_skill_path()
  │     → register_builtin_tools() 注册
  │     → SkillsSettingsPage 接入真实数据
  │
  ├─ F3: 权限系统从参数读取
  │     → run_agent_stream / run_agent_turn 接收 permission_mode
  │     → UI-1: 权限模式选择器连接后端
  │
  └─ F4: TauriPermissionPrompter 实现

Phase 2（P2 UX 完善）:
  ├─ F9: Context Compaction 接入
  ├─ F10: 模型选择 UI 生效 — 双重选择器统一
  ├─ F11: SlashCommand 前端交互层 — / 解析 + Tab 补全 + 下拉建议
  ├─ F12: ToolSearch 工具注册
  ├─ F14: SkillSearch 工具注册（F6 完成后）
  ├─ F16: 流式 session 定期保存
  ├─ F17: /skills /agents Tauri commands
  ├─ N10: 统一 ChatUI 入口 — App.tsx 为唯一状态管理
  ├─ UI-2: 分支选择器连接后端
  ├─ UI-3: ChatWorkspace header 按钮 — 移除或 disable 非功能按钮
  ├─ UI-4: 移除硬编码 diff 统计
  ├─ UI-6: Session 恢复保留 tool_call 上下文
  ├─ UI-7: 流式中断能力（Stop 按钮）
  ├─ UI-8: TodoWrite 工具注册 + TodoPanel 实时展示
  │     → 新建 builtin/todo_write.rs（Phase 4 ToolHandler 适配）
  │     → N11: 修改 todo_store_path 使用 IF2AI_TODO_STORE
  │     → register_builtin_tools() 注册
  │     → 新建 TodoPanel.tsx（Composer 上方）
  │     → App.tsx SSE 事件解析 → 更新 todos 状态
  │
  └─ N7: 隐藏 GlobalNavbar 无功能图标（或 F6 后启用）

Phase 3（P3 长期增强）:
  ├─ F13: Agent 工具注册
  ├─ F18: SKILL.md 格式文档
  ├─ F19: Session 与 Project 解绑（全局会话）
  ├─ F20: MCP 协议支持
  ├─ F21: 插件系统
  ├─ F22: Sandbox 沙箱
  ├─ F23: OAuth 登录
  ├─ F24: 低复杂度 SlashCommand Handler（/help, /compact, /diff, /export）
  ├─ F25: 会话状态面板完善（SessionStatus 扩展）
  │     → N9: 接入 SessionStatus 组件
  ├─ N6: 错误消息国际化（后端英文 + 前端本地化）
  └─ N8: 移除 WorkbenchBackdrop 装饰组件
```

---

## 8.9 快速验证清单

### 场景 1：非流式工具调用

```
用户输入："列出当前目录的 Rust 文件"
预期：
  1. run_agent_turn 的 ApiRequest 中 tools: Some([...]) 包含 glob_search 定义
  2. LLM 返回 tool_use: glob_search(pattern: "**/*.rs")
  3. 工具执行后返回结果
  4. LLM 根据工具结果回复
```

### 场景 2：流式工具调用

```
用户输入："帮我创建一个 hello.rs 文件"
预期：
  1. 流式响应开始，看到 Thinking 内容
  2. 看到工具调用紧凑行：◐ file_write  hello.rs  （running 状态）
  3. 工具完成后变为：✓ file_write  hello.rs  （2.3s）
  4. 点击工具行可展开查看参数和结果
  5. 流式响应继续（Agent 根据工具结果回复）
```

### 场景 3：SlashCommand

```
用户输入："/he"
预期：
  1. 50ms 后出现下拉建议：/help, /export, /permissions, ...
  2. ↓ 箭头高亮 /help
  3. Tab 键补全为 "/help"
  4. Enter 执行，返回帮助文本并显示在聊天窗口
```

### 场景 4：Skill 加载

```
用户在工作目录创建 .if2ai/skills/my-skill/SKILL.md
用户在 ChatUI 中请求使用 my-skill
预期：
  1. LLM 调用 Skill 工具
  2. resolve_skill_path 找到 .if2ai/skills/my-skill/SKILL.md
  3. 返回文件内容作为 LLM 上下文
```

### 场景 5：TodoWrite 任务列表

```
用户输入："请帮我重构这个模块，分为以下步骤：读取、修改、测试、提交"
预期：
  1. LLM 调用 TodoWrite 工具创建任务列表
  2. Composer 上方出现 TodoPanel，显示 4 个 pending 任务
  3. Agent 开始执行，第一个任务变为 ◐ in_progress（蓝色脉冲动画）
  4. 完成后变为 ✓ completed（绿色），下一个任务变为 ◐ in_progress
  5. 所有任务完成后，TodoPanel 折叠为 "All tasks completed ✓"
  6. 新会话开始时 TodoPanel 自动清空
```
