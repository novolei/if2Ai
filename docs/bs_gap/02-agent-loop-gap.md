# 二、Agent Loop 核心流程偏差

> 本章节详细分析 CLAW-CLI 的 Agent Loop 与 If2Ai 桌面端实现之间的核心流程偏差。

---

## 2.1 CLAW-CLI Baseline Agent Loop

**源码位置**：`rust/crates/runtime/src/conversation.rs` — `ConversationRuntime::run_turn()`

### 完整 Loop 流程

```
User Message
    │
    ▼
session.messages.push(user_message)
    │
    ▼ LOOP (max_iterations 检查)
    │
    ├─ [Step 1] 构建 ApiRequest
    │     system_prompt: self.system_prompt.clone()
    │     messages: self.session.messages.clone()   ← 全量历史
    │     tools: Some(definitions)  或  tools: None  ← 由调用方决定
    │
    ├─ [Step 2] api_client.stream(request) → Vec<AssistantEvent>
    │     ├─ TextDelta       → 累积文本
    │     ├─ ToolUse         → 收集 pending_tool_uses
    │     ├─ Thinking        → 累积思考内容
    │     ├─ Usage          → 记录 token 使用
    │     └─ MessageStop    → 结束
    │
    ├─ [Step 3] build_assistant_message() → ConversationMessage
    │
    ├─ [Step 4] session.messages.push(assistant_message)
    │
    ├─ [Step 5] 提取 pending_tool_uses
    │     if pending_tool_uses.is_empty():
    │           └─ break（turn 结束）
    │
    └─ [Step 6] FOR EACH tool_use in pending_tool_uses:
            │
            ├─ permission_policy.authorize(tool_name, input, prompter)
            │     ├─ Allow  → 继续
            │     ├─ Deny   → 生成 error tool_result，跳过执行
            │     └─ Prompt → 用户确认后再决定
            │
            ├─ PreToolUse Hook 运行
            │
            ├─ tool_executor.execute(tool_name, input)
            │     └─ match name → 调用实际工具函数
            │
            ├─ PostToolUse Hook 运行
            │
            └─ tool_result 入 session.messages
            └─ LOOP 继续（下一 iteration 调用 LLM）
    │
    ▼
TurnSummary { assistant_messages, tool_results, iterations, usage }
```

**关键特性**：
- 每个 tool_result 后都会**立即继续下一轮 LLM 调用**（tool-in-loop）
- 工具调用结果会进入 `session.messages`，被下一轮 LLM 看到
- 这是 Anthropic 官方推荐的 Agent 架构

---

## 2.2 If2Ai 桌面端 Agent 路径对比

### 路径 A：`run_agent_turn`（非流式）

**源码位置**：`src-tauri/src/commands/agent.rs` — `run_agent_turn()`

```rust
// agent.rs:368-374
let mut runtime = ConversationRuntime::new(
    runtime_session,
    api_client,         // RealApiClient
    tool_executor,      // ToolRegistryExecutor
    permission_policy,  // PermissionPolicy::new(PermissionMode::DangerFullAccess)
    system_prompt,
);
let result = runtime.run_turn(user_message.clone(), None);
```

**问题：工具定义从未传给 LLM**

```rust
// conversation.rs:243-247 — ConversationRuntime::run_turn()
let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: self.session.messages.clone(),
    tools: None,  // 🔴 硬编码 None！LLM 永远看不到工具列表
};
```

**后果**：即使 LLM 实际上有工具调用能力，但从未收到工具定义，所以它无法知道该调用哪个工具。

### 路径 B：`start_agent_stream`（流式）

**源码位置**：`src-tauri/src/commands/agent.rs:508-769`

```
用户输入 → start_agent_stream()
    │
    ├─ [Step 1] 恢复 session，加载历史消息
    │
    ├─ [Step 2] 构建 API 请求
    │     system: None
    │     messages: 历史消息 + 用户消息
    │     tools: Some(definitions)  ← ✅ 工具定义确实传给了 LLM
    │     stream: true
    │
    ├─ [Step 3] 后台任务：claw_client.stream_message(api_request)
    │
    ├─ [Step 4] 流式事件处理（loop）
    │     ├─ TextDelta       → window.emit("agent-token", text_delta)
    │     ├─ ThinkingDelta   → window.emit("agent-token", thinking_delta)
    │     ├─ InputJsonDelta  → 🟠 完全忽略（工具参数增量）
    │     └─ MessageStop     → 保存会话，退出
    │
    └─ [Step 5] 保存会话（仅文本），返回 stream_id
```

**问题：tool_use 事件被完全忽略**

当 LLM 决定调用工具时，流式响应会发送：
1. `ContentBlockStart` → 标记 tool_use 块开始
2. `InputJsonDelta` → 工具参数的 JSON 增量（被忽略）
3. `ContentBlockStop` → 工具块结束

但 `start_agent_stream` 的事件处理中：

```rust
// agent.rs:686
crate::modules::api::ContentBlockDelta::InputJsonDelta { .. } => {}
// ↑ 空处理，工具参数被丢弃

// agent.rs:699-712
ApiStreamEvent::ContentBlockStart(start_event) => {
    if matches!(start_event.content_block, OutputContentBlock::Thinking { .. }) {
        // thinking 块有处理
    }
    // ↑ ToolUse 块完全没有处理！
}
```

**后果**：即使 LLM 生成了工具调用请求，桌面端也完全忽略了它，只把文本部分保存为 assistant message。工具从未被执行。

---

## 2.3 两条路径的完整对比

| 维度 | CLAW-CLI Baseline | run_agent_turn（路径 A） | start_agent_stream（路径 B） |
|------|-------------------|--------------------------|------------------------------|
| 调用路径 | `run_turn()` 唯一路径 | `run_turn()` | 独立实现 |
| 工具定义传给 LLM | ✅ `tools: Some(defs)` | ❌ `tools: None` | ✅ `tools: Some(defs)` |
| tool_use 事件处理 | ✅ 完整 | ✅ 完整（在 run_turn 内） | ❌ 完全忽略 |
| 工具执行 | ✅ 每次 tool_result 后继续 loop | ✅ 完整（在 run_turn 内） | ❌ 不执行 |
| tool_result 回传给 LLM | ✅ 下一轮自动发送 | ✅ 下一轮自动发送 | ❌ 不回传 |
| 流式 SSE 事件 | ❌ 不支持（CLI 场景不需要） | ❌ 不流式（返回完整结果） | ✅ text/thinking delta 流式推送 |
| 思考内容 | ✅ ThinkingDelta | ✅ ThinkingDelta | ✅ ThinkingDelta |
| 权限检查 | ✅ `authorize()` 真实调用 | ⚠️ 硬编码 Allow | ⚠️ 无权限检查 |
| 前端 UI | 无（CLI） | 返回完整文本 | ✅ SSE 实时推送 |

---

## 2.4 前端流式处理（无工具支持）

**源码位置**：`src/App.tsx` — `sendMessage()`

```typescript
const streamId = await startAgentStream(activeSessionId, userMsg.content)
const unlisten = await listenToStream(streamId, (payload: StreamTokenPayload) => {
    if (payload.event_type === 'text_delta') {
        // 累加文本到 assistant message
    } else if (payload.event_type === 'thinking_delta') {
        // 累加思考内容
    } else if (payload.event_type === 'thinking_start') {
        // 标记开始思考
    } else if (payload.event_type === 'stream_complete') {
        // 标记流结束
    }
    // ↑ 没有 'tool_call' / 'tool_result' 等事件类型！
})
```

**问题**：前端没有设计任何工具调用相关的事件类型或 UI 展示机制。

---

## 2.5 核心修复方案

### 修复 A：路径 A — 传入工具定义

```rust
// conversation.rs:243-247 — 修改为：
let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: self.session.messages.clone(),
    tools: Some(/* 从 tool_executor 获取工具定义 */),  // ✅
};
```

但这里有个问题：`ConversationRuntime<C, T>` 的泛型约束中没有暴露 `get_definitions()` 方法。需要通过 `tool_executor` trait 或额外字段获取。

### 修复 B：路径 B — 实现完整工具循环

`start_agent_stream` 需要重写，参考 `ConversationRuntime::run_turn()` 的逻辑：

1. 在流式接收 `InputJsonDelta` 时**累积工具参数**
2. 在 `ContentBlockStop` 时**提取完整的 tool_use**
3. 对每个 tool_use 执行**权限检查 → 工具执行 → tool_result**
4. 将 tool_result 作为新消息**追加到 session**
5. **继续发送下一轮 LLM 请求**（循环）
6. 在 SSE 上推送 `tool_call_start` / `tool_result` 事件供前端渲染

---

## 2.6 小结

| 问题 | 位置 | 严重度 |
|------|------|--------|
| `run_agent_turn` 的 `tools: None` | `conversation.rs:246` | 🔴 阻塞 |
| `start_agent_stream` 忽略 tool_use 事件 | `agent.rs:686` | 🔴 阻塞 |
| 两条路径能力不一致 | 架构设计 | 🟠 严重 |
| 前端无工具事件类型 | `App.tsx` | 🟡 中 |
| 流式路径无 tool_result 回传 | `agent.rs` | 🟡 中 |
