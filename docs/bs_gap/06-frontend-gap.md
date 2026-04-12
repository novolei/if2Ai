# 六、前端 UI 功能缺失

> 本章节对比 CLAW-CLI 的 CLI 交互能力与 If2Ai 桌面端 React 前端的实现差异。

---

## 6.1 CLI 功能 vs UI 功能对照

| CLI 功能 | Baseline (claw) | If2Ai 前端 | 状态 |
|---------|---------------|-----------|------|
| REPL 交互式对话 | ✅ | ✅ ChatUI | 已实现 |
| 流式文本响应 | N/A（CLI 非流式） | ✅ SSE | 已实现 |
| 思考内容展示 | ✅ | ✅ ThinkingBlock | 已实现 |
| Markdown 渲染 | ✅ | ✅ react-markdown | 已实现 |
| 代码高亮 | ✅ | ✅ rehype-highlight | 已实现 |
| Slash 命令 `/help` | ✅ | ❌ | **未实现** |
| Slash 命令 `/status` | ✅ | ❌ | **未实现** |
| Slash 命令 `/compact` | ✅ | ❌ | **未实现** |
| 权限模式选择 | ✅ `--permission-mode` | ❌ | **未实现** |
| 模型选择 | ✅ `--model` | ⚠️ 有 UI 但不起作用 | **半实现** |
| 工具调用结果展示 | ✅ 终端打印 | ❌ | **未实现** |
| 会话列表/切换 | ✅ | ✅ ProjectRail | 已实现 |
| 项目管理 | ❌ | ✅ | 已实现 |
| 会话持久化 | ✅ | ✅ | 已实现 |

---

## 6.2 Slash 命令系统

### CLAW-CLI Baseline

```rust
// main.rs — SlashCommand 定义
enum SlashCommand {
    Help,    // /help
    Status,  // /status
    Compact, // /compact
    Unknown(String),
}

fn SlashCommand::parse(trimmed: &str) -> Option<SlashCommand> {
    match trimmed {
        "/help"    => Some(Help),
        "/status"  => Some(Status),
        "/compact" => Some(Compact),
        _          => None,
    }
}
```

**功能**：
- `/help` — 打印帮助信息
- `/status` — 打印当前 session 状态（token 使用、turn 数等）
- `/compact` — 手动触发上下文压缩

### If2Ai 桌面端

**完全缺失**。前端 `ChatUI` 中有 `selectedModel` / `selectedStrength` 等状态，但：
- 模型选择 UI 存在，但**选择的模型从未传给后端**
- 无任何 slash 命令解析
- 无手动 compact 功能
- 无 session status 显示

---

## 6.3 模型选择 UI

### If2Ai 前端

```typescript
// ChatUI.tsx:61
const [selectedModel, setSelectedModel] = useState('gpt-5.4-mini')  // 默认值
```

但这个 `selectedModel` **从未被使用**：
- `sendMessage()` 调用 `startAgentStream(sessionId, userMsg.content)`
- 没有任何参数传递 `model`
- 后端 `start_agent_stream` 从 `~/.claude/settings.json` 读取模型配置

### CLAW-CLI Baseline

```rust
// main.rs — 模型通过参数或 config 传入
CliAction::Repl { model, allowed_tools, permission_mode } =>
    run_repl(model, allowed_tools, permission_mode)?
```

**差距**：If2Ai 前端暴露了模型选择 UI，但它是**无效的**——模型选择不影响任何行为。

---

## 6.4 工具调用结果展示

### CLAW-CLI Baseline

当 Agent 调用工具时，CLI 在终端打印：
```
[bash] Executing: ls -la
[/bash] Output: total 48
drwxr-xr-x  4 ryanliu  staff   128 Apr 12 20:00 .
...
```

### If2Ai 桌面端

**前端完全没有工具调用的 UI**：
- `StreamTokenPayload` 没有 `tool_call` / `tool_result` 事件类型
- `ChatUI` 没有渲染工具调用的组件
- 用户完全看不到 Agent 调用了哪些工具

**这意味着**：
1. 用户不知道 Agent 在干什么（缺乏透明度）
2. 无法调试工具调用问题
3. 工具执行失败时用户也无法看到错误信息

---

## 6.5 思考内容展示对比

### CLAW-CLI Baseline

思考内容通过 SSE 的 `ThinkingDelta` 事件传输，在终端用特殊样式显示（通常是灰色斜体）。

### If2Ai 桌面端

```typescript
// App.tsx — thinking 处理
if (payload.event_type === 'thinking_delta' && payload.thinking) {
    accumulatedThinking += payload.thinking
    // → 显示在 ThinkingBlock 中（可折叠）
}
if (payload.event_type === 'thinking_start') {
    // 标记开始思考
}
```

✅ **已正确实现**。ChatUI 有 `ThinkingBlock` 组件，支持折叠。

---

## 6.6 前端事件类型对比

### tauri.ts 定义的事件类型

```typescript
// lib/tauri.ts
type StreamTokenPayload = {
    stream_id: string
    text?: string           // text_delta
    thinking?: string       // thinking_delta
    event_type:
        | 'text_delta'
        | 'thinking_delta'
        | 'thinking_start'
        | 'stream_complete'
        | 'stream_error'    // ← 没有 tool_call / tool_result
}
```

**缺少的事件类型**：
- `tool_call_start` — 工具调用开始
- `tool_call_progress` — 工具执行中（长时间运行的工具）
- `tool_result` — 工具执行结果
- `tool_error` — 工具执行失败

### AgentTurnResponse（run_agent_turn 返回）

```typescript
// lib/tauri.ts
interface AgentTurnResponse {
    message: string
    session_id: string
    thinking?: string
    tool_calls?: ToolCall[]  // ← 存在但前端未使用
    tokens?: TokenUsage
}

interface ToolCall {
    id: string
    name: string
    arguments: Record<string, unknown>
}
```

`tool_calls` 字段在响应中**存在**，但前端完全没有使用它。

---

## 6.7 会话状态显示

### CLAW-CLI Baseline

```rust
// /status 命令输出
Turn: 12 / 100
Tokens used: 45,231 (input: 32,100 | output: 13,131)
Model: claude-opus-4-6
Permission: workspace-write
Last compact: 3 turns ago
```

### If2Ai 桌面端

```typescript
// SessionStatus.tsx — 存在但简单
// 仅显示 Idle / Running / Error / Working 状态
```

**缺失**：
- Token 使用量统计
- Turn 计数
- 压缩历史
- 当前模型显示
- 权限模式显示

---

## 6.8 小结

| 问题 | 位置 | 严重度 |
|------|------|--------|
| Slash 命令完全缺失 | `ChatUI` | 🟡 中 |
| 模型选择 UI 无效 | `ChatUI` / `App.tsx` | 🟡 中 |
| 工具调用结果无 UI | `ChatUI` | 🟡 中（用户透明度） |
| StreamTokenPayload 缺少工具事件类型 | `tauri.ts` | 🟡 中 |
| tool_calls 在响应中但前端未使用 | `App.tsx` | 🟡 中 |
| 会话状态显示不完整 | `SessionStatus.tsx` | 🟢 低 |
