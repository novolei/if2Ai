# ADR-011 Agent Loop Harness Framework — 运行时观测与控制框架

## 状态

**日期**: 2026-04-13
**状态**: 草案

## 概述

**目标**: 为 if2Ai 的 AI Agent Loop 构建一个内置的运行时观测与控制框架，使生产 APP 的 Agent 行为可测、可控、可优化、可复现。

**动机**: 当前的 Agent Loop (`start_agent_stream`) 只能通过前端 SSE 事件观察，且无法从外部控制或重放。需要一个独立的内部遥测系统，区别于开发时测试的 Python harness。

---

## 背景

### 现有架构

```
Frontend (React)
     ↓ SSE events
Tauri App (Rust)
     ↓ start_agent_stream
Agent Loop ──→ Tool Execution ──→ Session Manager
                      ↓
               HookRunner (PreToolUse/PostToolUse)
```

### 问题

1. **观测性不足**: 前端 SSE 事件是唯一观测途径，无法在后台记录
2. **可控性缺失**: 无法从外部注入命令或中断 Agent 行为
3. **复现困难**: 会话无法重放，调试靠日志
4. **性能黑盒**: Token 使用、工具耗时、决策点无结构化数据

### 解决方案

构建一个**内置于 Rust binary**的运行时框架，通过独立的内部事件总线收集遥测数据，并通过 Tauri IPC 向外部（Python CLI/其他进程）暴露控制接口。

```
External Harness (Python CLI)
         ↓ IPC (Tauri Commands)
┌─────────────────────────────────────┐
│  Tauri App (Rust Binary)            │
│                                     │
│  ┌───────────────────────────────┐ │
│  │   Agent Loop Harness Core      │ │
│  │   (Internal Event Bus)          │ │
│  └───────────────────────────────┘ │
│         ↓ emit                      │
│  ┌───────────────────────────────┐ │
│  │ Telemetry Collectors          │ │
│  │ - TokenUsage                  │ │
│  │ - ToolMetrics                 │ │
│  │ - DecisionPoints              │ │
│  └───────────────────────────────┘ │
│         ↓ store                    │
│  ┌───────────────────────────────┐ │
│  │ Session Recorder             │ │
│  │ (Local SQLite)                │ │
│  └───────────────────────────────┘ │
└─────────────────────────────────────┘
```

---

## 设计决策

### DECISION-11-01: 事件总线架构

**选项**:
- A) 直接在 `start_agent_stream` 中埋点，耦合到业务逻辑
- B) 使用 `tracing` crate 扩展，发送结构化日志
- C) 独立的内部 `EventBus` 单例，Actor 风格

**决定**: C — 独立的 `EventBus` 单例

**理由**:
- 解耦：业务逻辑通过 trait 接口发送事件，不直接依赖消费者
- 可测试：可以用 mock event collector 验证事件
- 可组合：多个 collector 可以订阅同一事件类型
- 性能：异步 channel，非阻塞

### DECISION-11-02: 遥测存储

**选项**:
- A) 仅内存存储，重启丢失
- B) SQLite 本地存储
- C) 文件 append-only log

**决定**: B — SQLite 本地存储

**理由**:
- 持久化支持会话重放
- 结构化查询支持分析
- 与 Phase 6B 的 SQLite 复用同库

### DECISION-11-03: 外部控制协议

**选项**:
- A) WebSocket 长连接
- B) Tauri IPC 命令轮询
- C) STDIO Unix Domain Socket

**决定**: B — Tauri IPC 命令

**理由**:
- 已在使用 Tauri，无需引入新协议
- 前端和外部 harness 都可以调用
- 天然支持跨语言（Python JS 都可）

---

## 核心模块

### 1. EventBus (事件总线)

```rust
// src-tauri/src/modules/harness/event_bus.rs

/// 事件总线单例
pub struct EventBus {
    subscribers: RwLock<Vec<Box<dyn EventSubscriber>>>,
    channel: mpsc::UnboundedChannel<AgentEvent>,
}

impl EventBus {
    pub fn global() -> &'static EventBus;
    pub fn subscribe(&self, subscriber: Box<dyn EventSubscriber>);
    pub fn emit(&self, event: AgentEvent);
}

pub trait EventSubscriber: Send + Sync {
    fn on_event(&self, event: &AgentEvent);
}

/// Agent 生命周期事件
#[derive(Debug, Clone)]
pub enum AgentEvent {
    TurnStart { stream_id: String, session_id: String },
    TurnEnd { stream_id: String, outcome: TurnOutcome },
    LlmStart { stream_id: String, request_id: String },
    LlmEnd { stream_id: String, request_id: String, tokens: TokenUsage },
    ToolStart { stream_id: String, tool_name: String, tool_args: String },
    ToolEnd { stream_id: String, tool_name: String, duration_ms: u64, success: bool },
    DecisionPoint { stream_id: String, point: DecisionType, context: serde_json::Value },
    Error { stream_id: String, error: String },
}
```

### 2. TelemetryCollector (遥测收集器)

```rust
// src-tauri/src/modules/harness/telemetry.rs

pub struct TelemetryCollector {
    event_bus: EventBus,
    token_stats: RwLock<TokenStats>,
    tool_stats: RwLock<HashMap<String, ToolStats>>,
    decision_stats: RwLock<HashMap<String, u32>>,
}

impl TelemetryCollector {
    pub fn new(event_bus: &EventBus) -> Self;
    pub fn get_token_stats(&self) -> TokenStats;
    pub fn get_tool_stats(&self) -> HashMap<String, ToolStats>;
    pub fn get_decision_stats(&self) -> HashMap<String, u32>;
}

pub struct TokenStats {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read: u64,
    pub total_cache_creation: u64,
    pub estimated_cost_usd: f64,
}

pub struct ToolStats {
    pub name: String,
    pub call_count: u64,
    pub total_duration_ms: u64,
    pub success_count: u64,
    pub failure_count: u64,
}
```

### 3. SessionRecorder (会话录制器)

```rust
// src-tauri/src/modules/harness/recorder.rs

pub struct SessionRecorder {
    db_path: PathBuf,
    event_bus: EventBus,
}

impl SessionRecorder {
    pub fn new(db_path: PathBuf, event_bus: &EventBus) -> Self;
    pub fn start_recording(&self, session_id: &str) -> Result<RecordingId>;
    pub fn stop_recording(&self, recording_id: RecordingId);
    pub fn get_recording(&self, recording_id: RecordingId) -> Result<RecordedSession>;
    pub fn list_recordings(&self, session_id: &str) -> Result<Vec<RecordingMeta>>;
}

#[derive(Debug, Clone)]
pub struct RecordedSession {
    pub id: RecordingId,
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub events: Vec<AgentEvent>,
    pub final_state: SessionSnapshot,
}
```

### 4. HarnessControl (控制接口)

```rust
// src-tauri/src/modules/harness/control.rs

pub struct HarnessControl {
    event_bus: EventBus,
    telemetry: Arc<TelemetryCollector>,
    recorder: Arc<SessionRecorder>,
}

/// Tauri IPC 命令
#[tauri::command]
impl HarnessControl {
    /// 获取当前会话的实时遥测数据
    pub fn get_telemetry(session_id: String) -> TelemetrySnapshot;

    /// 开始录制会话
    pub fn start_recording(session_id: String) -> Result<RecordingId>;

    /// 停止录制并获取录制数据
    pub fn stop_recording(recording_id: RecordingId) -> Result<RecordedSession>;

    /// 注入暂停点（仅在 debug 模式）
    #[cfg(debug_assertions)]
    pub fn inject_pause_point(session_id: String, reason: String) -> Result<()>;

    /// 获取 Agent 决策历史
    pub fn get_decision_history(session_id: String) -> Vec<DecisionRecord>;
}
```

### 5. AgentLoopIntegration (集成到 start_agent_stream)

在 `start_agent_stream` 的关键位置添加事件发射:

```rust
// 在 start_agent_stream 函数内

// 1. Turn 开始
event_bus.emit(AgentEvent::TurnStart {
    stream_id: stream_id.clone(),
    session_id: session_id.clone(),
});

// 2. LLM 调用前
event_bus.emit(AgentEvent::LlmStart {
    stream_id: stream_id.clone(),
    request_id: request_id.clone(),
});

// 3. 工具执行前
event_bus.emit(AgentEvent::ToolStart {
    stream_id: stream_id.clone(),
    tool_name: tool_name.to_string(),
    tool_args: serde_json::to_string(&args).unwrap_or_default(),
});

// 4. 决策点
event_bus.emit(AgentEvent::DecisionPoint {
    stream_id: stream_id.clone(),
    point: DecisionType::ToolSelection { candidate_count },
    context: json!({ "session_length": messages.len() }),
});

// 5. Turn 结束
event_bus.emit(AgentEvent::TurnEnd {
    stream_id,
    outcome,
});
```

---

## 外部 Harness CLI (Rust Binary)

外部 Harness 用 Rust 开发，编译为独立的 `harness-cli` 二进制:

**架构选择**: External CLI 作为 Tauri IPC 命令实现，格式化为 CLI 输出

```
┌──────────────────────────────────────────────────────────────┐
│  harness-cli (Rust Binary)                                   │
│  - CLI 界面 (clap)                                           │
│  - 调用 tauri::command 获取数据                              │
│  - 格式化输出 (JSON/文本)                                    │
└──────────────────────────────────────────────────────────────┘
                            ↓ IPC
┌──────────────────────────────────────────────────────────────┐
│  Tauri App (if2ai-backend)                                   │
│  modules/harness/                                             │
│  - EventBus                                                  │
│  - TelemetryCollector                                        │
│  - SessionRecorder (SQLite)                                  │
└──────────────────────────────────────────────────────────────┘
```

**文件结构**:
```
src-tauri/
├── src/
│   ├── modules/harness/          # 核心模块
│   │   ├── mod.rs
│   │   ├── event_bus.rs
│   │   ├── telemetry.rs
│   │   ├── recorder.rs
│   │   └── types.rs             # AgentEvent, TelemetrySnapshot
│   └── commands/
│       └── harness.rs            # Tauri IPC 命令
└── bin/
    └── main.rs                  # 主 App 入口

harness-cli/                     # 独立 crate (workspace member)
├── Cargo.toml
└── src/
    └── main.rs                  # CLI 入口，调用 Tauri IPC
```

**构建**:
```bash
# 主 App
cargo build --release

# External CLI (单独构建)
cd harness-cli && cargo build --release
```

**CLI 命令**:
```bash
# 获取会话遥测
./harness-cli get-telemetry <session_id>

# 开始录制
./harness-cli start-recording <session_id>

# 停止录制并获取数据
./harness-cli stop-recording <recording_id>

# 重放录制的会话 (文本模式)
./harness-cli replay <recording_id> --speed 2.0

# 实时仪表盘 (TUI ratatui)
./harness-cli dashboard
```

**优势**:
- 复用 `modules/harness/` 中的类型定义
- 通过 Tauri IPC 调用（命令模式，非 invoke）
- 编译为单一二进制，无 Python/Node 依赖
- 用 `clap` 实现 CLI，`ratatui` 实现 TUI 仪表盘
- 与主 App 共用 SQLite，数据一致性

---

## 与 Phase 6B Memory 的关系

| Phase 6B 组件 | Harness 依赖 | 说明 |
|---------------|-------------|------|
| SQLite P0 | SessionRecorder | 复用 SQLite 连接 |
| Token Budget | TelemetryCollector | 收集 token 使用统计 |
| Memory Provider | 无关 | 正交 |

---

## 实现优先级

| 优先级 | 模块 | 工时 | 依赖 |
|--------|------|------|------|
| P0 | EventBus Core | 1 天 | 无 |
| P0 | TelemetryCollector | 1 天 | EventBus |
| P0 | AgentLoopIntegration | 1 天 | EventBus |
| P1 | SessionRecorder | 2 天 | SQLite (Phase 6B) |
| P1 | HarnessControl (IPC) | 1 天 | EventBus |
| P1 | harness-cli Binary | 1 天 | HarnessControl |
| P2 | Replay Player | 2 天 | SessionRecorder |

---

## 验收标准

- [ ] EventBus 单例正确工作，事件不丢失
- [ ] `start_agent_stream` 所有关键路径都有事件发射
- [ ] TelemetryCollector 正确统计 token 使用和工具耗时
- [ ] SessionRecorder 可录制完整会话并重放
- [ ] Tauri IPC 命令可被 Rust harness-cli 调用
- [ ] harness-cli CLI 命令正确格式化输出
- [ ] debug 模式可注入 pause point

---

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 性能开销 | 高 | EventBus 用 unbounded channel，不阻塞主循环 |
| 存储膨胀 | 中 | SessionRecorder 按需开启，默认不录制 |
| 耦合风险 | 低 | EventBus trait 接口，解耦业务逻辑 |

---

## 相关文档

- [Phase 6B Memory Control Plane](../phase-6b-memory-control-plane.yaml)
- [ADR-004 Token Budget](./ADR-004-Token-Budget-Allocation.md)
- [llm-stream-reliability-control-plane-v1](../llm-stream-reliability-control-plane-v1.md)
