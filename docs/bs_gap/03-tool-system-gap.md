# 三、工具注册 / 分发 / 执行偏差

> 本章节对比 CLAW-CLI 和 If2Ai 桌面端在工具系统上的实现差异。

---

## 目录

| 节 | 内容 |
|----|------|
| [3.1](#31-工具注册架构对比) | GlobalToolRegistry vs ToolRegistry 架构对比 |
| [3.2](#32-工具定义传给-llm-的差异) | tools: Some vs tools: None 的致命差异 |
| [3.3](#33-工具分发-dispatch-对比) | dispatch 机制和 ToolHandler 签名对比 |
| [3.4](#34-工具执行后结果回传对比) | tool_result 回传 session 的流程对比 |
| [3.5](#35-工具列表详细对比) | 26 个工具的逐一对比表 |
| [3.6](#36-工具-security-对比) | bash/file_read/write_file 安全机制对比 |
| [3.7](#37-toolset-分类if2ai-特有) | ToolSet 分类系统（Phase 4 特有） |

---

## 3.1 工具注册架构对比

### CLAW-CLI Baseline

```
GlobalToolRegistry (tools/src/lib.rs)
  ├─ mvp_tool_specs()        — 硬编码 17 个内置工具规格
  ├─ with_plugin_tools()      — 聚合插件工具（去重检查）
  └─ execute_tool() 分派函数   — match name → 调用 runtime 函数

tools/src/lib.rs 内部：
  "bash"        → runtime::bash::execute_bash()
  "read_file"   → runtime::file_ops::read_file()
  "write_file"  → runtime::file_ops::write_file()
  "glob_search" → runtime::file_ops::glob_search()
  "grep_search" → runtime::file_ops::grep_search()
  ...
```

### If2Ai 桌面端

```
ToolRegistry (modules/tools/registry.rs)
  ├─ DashMap<String, ToolEntry>  — 运行时注册
  ├─ names_to_toolsets: DashMap   — 名称到工具集映射
  ├─ context: SharedToolContext   — 持有 workdir + permission_mode
  └─ dispatch(name, args)         — 调用 handler（带 timeout）

register_builtin_tools() (modules/tools/mod.rs)
  → builtin/bash.rs              → ToolHandler
  → builtin/file_read.rs          → ToolHandler
  → builtin/json_parse.rs         → ToolHandler
  → builtin/file_write.rs         → ToolHandler
  ...
```

---

## 3.2 工具定义传给 LLM 的差异

### CLAW-CLI

```rust
// runtime/conversation.rs — Baseline
let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: self.session.messages.clone(),
    tools: Some(definitions),  // ✅ 从 tool_registry.get_definitions() 获取
};
```

### If2Ai（非流式路径）

```rust
// modules/runtime/conversation.rs:246 — 🔴 问题
tools: None,  // ❌ 硬编码 None
```

### If2Ai（流式路径）

```rust
// src-tauri/src/commands/agent.rs:614-619 — ✅ 正确
tools: if tool_defs.is_empty() {
    None
} else {
    Some(tool_defs)  // ✅ 工具定义确实传给 LLM
},
```

但流式路径传了工具定义却**不处理工具调用结果**（见第二章）。

---

## 3.3 工具分发 (dispatch) 对比

### CLAW-CLI

```rust
// GlobalToolRegistry.execute_tool()
pub fn execute_tool(name: &str, input: &Value) -> Result<String, String> {
    match name {
        "bash" => run_bash(from_value(input)?),
        "read_file" => run_read_file(from_value(input)?),
        ...
    }
}
```

### If2Ai

```rust
// ToolRegistry.dispatch() — modules/tools/registry.rs
pub async fn dispatch(&self, name: &str, args: &Value) -> Result<String, ToolError> {
    let entry = self.get(name).ok_or_else(|| ToolError::NotFound(name.to_string()))?;
    let ctx = self.context.lock().map_err(|e| ToolError::Handler(...))?;
    (entry.handler)(args, &ctx).await  // 调用 handler
}
```

**If2Ai 的额外包装**：
- `ToolHandler` 签名：`Fn(Value, SharedToolContext) -> Pin<Box<dyn Future>>`
- 多了 `ToolContext`（workdir + permission_mode）参数
- 多了 `async` 和 timeout 包装

---

## 3.4 工具执行后结果回传对比

### CLAW-CLI Baseline

```
tool_executor.execute(tool_name, input)
    │
    ├─ Ok(output) → 生成 tool_result 入 session.messages
    │
    └─ Err(error) → 生成 tool_result(is_error=true) 入 session.messages

下一轮 LLM 调用时：
    messages = session.messages.clone()  // 包含 tool_result
    → LLM 看到：Assistant 说"我来帮你读取文件"，然后接收到 tool_result: "文件内容是..."
```

### If2Ai 桌面端

**非流式路径（`run_agent_turn`）**：
```
tool_executor.execute() → 结果入 Runtime Session
    → save_session() → 结果持久化
    → RunAgentTurnResponse { message, thinking, tool_calls? }
```
返回的 `tool_calls` 字段在代码中存在，但**前端未处理**。

**流式路径（`start_agent_stream`）**：
```
LLM 返回 tool_use 事件 → InputJsonDelta 被忽略
    → 直接保存 assistant_text → save_session()
```
**工具结果完全丢失**，没有回传给 LLM。

---

## 3.5 工具列表详细对比

| 工具 | CLAW-CLI | If2Ai | 状态 |
|------|---------|-------|------|
| `bash` | ✅ | ✅ | 已实现，workdir 限制 |
| `read_file` | ✅ | ✅ | 已实现，workdir allowlist |
| `write_file` | ✅ | ✅ | 已实现，workdir allowlist |
| `edit_file` | ✅ | ⚠️ | stub（未实现） |
| `glob_search` | ✅ | ✅ | 已实现 |
| `grep_search` | ✅ | ✅（重命名为 content_search） | 已实现 |
| `json_parse` | ✅ | ✅ | 已实现 |
| `WebFetch` | ✅ | ✅（web_fetch） | 已实现 |
| `WebSearch` | ✅ | ✅（web_search） | 已实现 |
| `TodoWrite` | ✅ | ❌ | 未实现 |
| `Skill` | ✅ | ❌ | 未实现 |
| `Agent` | ✅ | ❌ | 未实现（Agent 嵌套） |
| `ToolSearch` | ✅ | ❌ | 未实现 |
| `NotebookEdit` | ✅ | ❌ | 未实现 |
| `Sleep` | ✅ | ❌ | 未实现 |
| `SendUserMessage`/`Brief` | ✅ | ❌ | 未实现 |
| `Config` | ✅ | ❌ | 未实现 |
| `StructuredOutput` | ✅ | ❌ | 未实现 |
| `REPL` | ✅ | ❌ | 未实现 |
| `PowerShell` | ✅ | ❌ | 未实现 |
| `memory_store/recall/forget/purge/export` | ❌ | ✅ | Phase 4 新增 |
| `cron_add/list/remove/run/runs` | ❌ | ✅ | Phase 4 新增 |
| `http_request` | ❌ | ✅ | Phase 4 新增 |

**评价**：Phase 4 新增了 Memory 和 Cron 工具，这些是 CLAW-CLI 没有的。但 CLAW-CLI 中的一些高级工具（Agent 嵌套、StructuredOutput 等）未实现。

---

## 3.6 工具 security 对比

| 工具 | CLAW-CLI 安全措施 | If2Ai 安全措施 |
|------|------------------|----------------|
| `bash` | 危险命令黑名单 + timeout | 危险命令黑名单 + timeout + `cd $workdir` 包装 |
| `read_file` | 无 workdir 限制 | workdir allowlist + 敏感路径黑名单（`/etc/passwd`等） |
| `write_file` | 无 workdir 限制 | workdir allowlist + 1MB 大小限制 |
| `glob_search` | 无 workdir 限制 | ✅ 无限制 |
| `http_request` | 无 | timeout + 大小限制 |

**评价**：If2Ai 在 `bash` 和 `file_read` 上比 CLAW-CLI 更安全（增加了 workdir allowlist）。

---

## 3.7 ToolSet 分类（If2Ai 特有）

```rust
// modules/tools/toolset.rs
TOOLSETS = {
    files: [read_file, file_write, file_edit, glob_search, content_search],
    terminal: [bash],
    utility: [json_parse, calculator],
    web: [web_fetch, web_search, http_request],
    memory: [memory_store, memory_recall, memory_forget, memory_purge, memory_export],
    scheduler: [cron_add, cron_list, cron_remove, cron_run, cron_runs],
    minimal: [read_file, bash],
    development: [files, terminal],
}
```

CLAW-CLI **没有 ToolSet 概念**。这是 Phase 4 的额外设计，更适合 UI 场景的工具按需加载。
