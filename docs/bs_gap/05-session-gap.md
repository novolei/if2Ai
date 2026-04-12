# 五、会话管理偏差

> 本章节对比会话管理在 CLAW-CLI Baseline 和 If2Ai 桌面端的实现差异。

---

## 5.1 Session 数据结构对比

### CLAW-CLI Baseline

```rust
// runtime/session.rs
pub struct Session {
    pub version: u32,
    pub messages: Vec<ConversationMessage>,
}

pub enum MessageRole { System, User, Assistant, Tool }

pub enum ContentBlock {
    Text { text: String },
    ToolUse { id, name, input },           // 工具调用请求
    ToolResult { tool_use_id, tool_name, output, is_error },  // 工具执行结果
}

pub struct ConversationMessage {
    pub role: MessageRole,
    pub blocks: Vec<ContentBlock>,
    pub thinking: Option<String>,  // Claude 思考内容
    pub usage: Option<TokenUsage>,
}
```

### If2Ai 桌面端

```rust
// modules/runtime/session.rs
pub struct Session {
    pub id: String,              // UUID v4
    pub project_id: String,      // 空或项目 ID
    pub title: String,
    pub messages: Vec<ConversationMessage>,  // ✅ 相同
    pub created_at: String,      // RFC3339
    pub updated_at: String,
    pub token_count: u64,
    pub pinned: bool,
}

pub enum MessageRole { System, User, Assistant, Tool }  // ✅ 相同

pub enum ContentBlock {
    Text { text: String },                             // ✅ 相同
    ToolUse { id, name, input },                       // ✅ 相同
    ToolResult { tool_use_id, tool_name, output },    // ⚠️ 缺少 is_error
}
```

**偏差**：`ToolResult` 缺少 `is_error` 布尔字段。

---

## 5.2 Session 持久化对比

### CLAW-CLI Baseline

```
~/.claw/sessions/<session_id>.json
```

Session 由 `CliToolExecutor` 或 `PluginManager` 直接写入磁盘。

### If2Ai 桌面端

**双路径存储**：

| 场景 | 路径 |
|------|------|
| legacy（无项目） | `~/.if2ai/sessions/<id>.json` |
| 有项目 | `~/.if2ai/projects/<project_id>/sessions/<id>.json` |

```rust
// modules/session/manager.rs
if session.project_id.is_empty() {
    sessions_dir.join(format!("{}.json", session.id))
} else {
    projects_base_dir
        .join(&session.project_id)
        .join("sessions")
        .join(format!("{}.json", session.id))
}
```

**兼容性**：自动检测项目路径，向后兼容 legacy session。

---

## 5.3 消息追加对比

### CLAW-CLI Baseline

每次 tool_result 后，直接 `session.messages.push(tool_result)`，下一轮 LLM 调用时全量发送：

```rust
// runtime/conversation.rs
session.messages.push(tool_result);
// 下一轮 loop
let messages = session.messages.clone(); // 包含 tool_result
```

### If2Ai 桌面端

**非流式路径（`run_agent_turn`）**：
```rust
// conversation.rs — 相同逻辑
session.messages.push(tool_result);
// 继续 loop
```

**流式路径（`start_agent_stream`）**：
```rust
// agent.rs:755 — 只保存 user message 和 assistant text
updated_app_session.messages.push(user_msg);
updated_app_session.messages.push(assistant_msg);  // 仅文本，无 tool_result
```

**问题**：流式路径在 tool_use 被忽略后，根本没有机会追加 tool_result。

---

## 5.4 Context Compaction（上下文压缩）对比

### CLAW-CLI Baseline

```rust
// runtime/compact.rs
pub struct CompactionConfig {
    pub preserve_recent_messages: usize,  // 默认 4
    pub max_estimated_tokens: usize,       // 默认 10,000
}

pub fn should_compact(session: &Session, config: CompactionConfig) -> bool {
    compactable_count > preserve_recent_messages
        && estimated_tokens >= max_estimated_tokens
}

pub fn compact_session(session: &mut Session, config: CompactionConfig) {
    // 1. 提取已有摘要
    // 2. 保留最近 4 条消息
    // 3. summarize_messages() → XML 摘要
    // 4. 用 System 消息替换被压缩的消息
}
```

在 `run_turn()` 内部，当 token 预算超限时触发。

### If2Ai 桌面端

**存在但未接入**：
```rust
// modules/runtime/compact.rs — 同样实现
pub struct CompactionConfig { ... }
pub fn should_compact(...) -> bool { ... }
pub fn compact_session(...) { ... }
```

**问题**：`run_agent_turn` 和 `start_agent_stream` 都没有调用 `should_compact()` 或 `compact_session()`。

长对话会持续累积消息，最终可能超出 LLM 的 context window。

---

## 5.5 Session 与 Project 关联对比

### CLAW-CLI Baseline

Session 与 Project 完全独立管理：
- Session 存储在 `~/.claw/sessions/`
- Project 信息通过 `--project` 参数指定
- 无固定绑定关系

### If2Ai 桌面端

**强绑定关系**：
```rust
pub struct Session {
    pub project_id: String,  // 非空则属于某项目
}
```

Session 在创建时就绑定了 `project_id`：
```rust
SessionManager::create_session_for_project(project_id, title)
    // Session 的 project_id = project_id
```

**优点**：Session 与 Project 一目了然，删除 Project 时可级联删除 Session。
**代价**：无法方便地实现"跨项目会话"或"临时会话"。

---

## 5.6 会话持久化时机对比

### CLAW-CLI Baseline

```rust
// main.rs:run_repl — 每次 submit 后保存
cli.run_turn(&input)?;
cli.persist_session()?;  // 立即持久化
```

### If2Ai 桌面端

**非流式**：`run_agent_turn` 返回前调用 `save_session()`
```rust
// agent.rs:389-407
match result {
    Ok(summary) => {
        // save_session() 在返回前调用
    }
}
```

**流式**：后台任务结束时调用
```rust
// agent.rs:758
if let Err(e) = session_manager.save_session(&updated_app_session).await { ... }
```

**问题**：流式场景下，如果用户关闭应用（浏览器），后台任务可能还没完成，session 可能丢失。

---

## 5.7 小结

| 问题 | 位置 | 严重度 |
|------|------|--------|
| `ToolResult` 缺少 `is_error` 字段 | `session.rs` | 🟢 低 |
| 流式路径不保存 tool_result | `agent.rs:755` | 🔴 阻塞 |
| Context Compaction 未接入 | `agent.rs` | 🟡 中 |
| 流式场景窗口关闭可能导致 session 丢失 | `agent.rs` | 🟡 中 |
