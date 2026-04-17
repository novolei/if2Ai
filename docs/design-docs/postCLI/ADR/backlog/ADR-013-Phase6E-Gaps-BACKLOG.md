# ADR-013 Phase 6E Harness Framework — 详细实施 backlog

## 概述

**目标**: 从零实现 Phase 6E Agent Loop Harness 框架，为 AI Agent Loop 构建内置的运行时观测与控制框架。
**ADR**: [ADR-013](./ADR-013-Phase-Remediation-Design.md), [ADR-011](./ADR-011-Agent-Loop-Harness-BACKLOG.md)
**优先级**: P0 (核心基础设施 — Phase 6E 整个模块不存在)
**预估工时**: 5-6 天
**基于审计**: [phase-6b-6bw-6e-gap-audit-report.md](../../../../generated/phase-6b-6bw-6e-gap-audit-report.md) v2

**现状**: `src-tauri/src/modules/harness/` 目录不存在，需要创建。Phase 6E 在 exec-plan 中声明 7 slices 全部 `status: pending`。

---

## 子任务清单

### TASK-013-E1: EventBus 核心

**目标**: 实现事件总线单例和事件类型

#### 具体任务

- [ ] 创建 `src-tauri/src/modules/harness/mod.rs`
- [ ] 定义 `AgentEvent` 枚举
- [ ] 实现 `EventBus` 单例 (`global()`)
- [ ] 实现 `EventSubscriber` trait
- [ ] 实现 `subscribe()` 和 `emit()` 方法
- [ ] 使用 `mpsc::unbounded_channel` 非阻塞发送
- [ ] 编写单元测试

#### 实现代码

File: `src-tauri/src/modules/harness/mod.rs`

```rust
pub mod event_bus;
pub mod telemetry;

pub use event_bus::{AgentEvent, EventBus, EventSubscriber, TurnOutcome, DecisionType};
pub use telemetry::TelemetryCollector;
```

File: `src-tauri/src/modules/harness/event_bus.rs`

```rust
use std::sync::Arc;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use parking_lot::Mutex;
use serde::{Serialize, Deserialize};

/// Outcome of an agent turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnOutcome {
    Completed,
    Error(String),
    Cancelled,
}

/// Types of decision points in the agent loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionType {
    ToolSelection { tool_name: String, confidence: f32 },
    PermissionRequest { tool_name: String },
    CompactionTrigger,
}

/// Events emitted at key points in the agent lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    TurnStart { stream_id: String, session_id: String },
    TurnEnd { stream_id: String, outcome: TurnOutcome },
    LlmStart { stream_id: String, request_id: String },
    LlmEnd { stream_id: String, request_id: String, tokens: u64 },
    ToolStart { stream_id: String, tool_name: String, tool_args: String },
    ToolEnd { stream_id: String, tool_name: String, duration_ms: u64, success: bool },
    DecisionPoint { stream_id: String, point: DecisionType, context: serde_json::Value },
    Error { stream_id: String, error: String },
}

pub trait EventSubscriber: Send + Sync {
    fn on_event(&self, event: &AgentEvent);
}

/// Thread-safe event bus singleton
///
/// Uses mpsc::unbounded_channel for non-blocking emit.
/// A dedicated background thread drains the channel and dispatches to subscribers.
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<Box<dyn EventSubscriber>>>>,
    sender: mpsc::UnboundedSender<AgentEvent>,
}

static GLOBAL_BUS: OnceLock<EventBus> = OnceLock::new();

impl EventBus {
    /// Get the global event bus singleton. Creates it on first call.
    pub fn global() -> &'static EventBus {
        GLOBAL_BUS.get_or_init(EventBus::new)
    }

    /// Create a new EventBus with a background dispatch thread.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let subscribers = Arc::new(Mutex::new(Vec::new()));

        // Spawn background thread to drain channel
        let subs = Arc::clone(&subscribers);
        thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                let subs = subs.lock();
                for subscriber in subs.iter() {
                    subscriber.on_event(&event);
                }
            }
        });

        Self { subscribers, sender }
    }

    /// Subscribe to all events. Returns immediately.
    pub fn subscribe(&self, subscriber: Box<dyn EventSubscriber>) {
        self.subscribers.lock().push(subscriber);
    }

    /// Emit an event. Non-blocking — event is queued for background dispatch.
    pub fn emit(&self, event: AgentEvent) {
        // Ignore send errors (only happens if receiver is dropped, which shouldn't happen)
        let _ = self.sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn event_bus_delivers_to_subscriber() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        bus.subscribe(Box::new(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }));

        bus.emit(AgentEvent::TurnStart {
            stream_id: "1".into(),
            session_id: "1".into(),
        });
        bus.emit(AgentEvent::TurnEnd {
            stream_id: "1".into(),
            outcome: TurnOutcome::Completed,
        });

        // Wait for async processing
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn global_returns_singleton() {
        let a = EventBus::global();
        let b = EventBus::global();
        assert!(std::ptr::eq(a, b));
    }
}
```

#### 验收标准

- [ ] EventBus 是 thread-safe 单例（`OnceLock`）
- [ ] emit 不阻塞主线程（unbounded channel）
- [ ] 事件不丢失
- [ ] 单元测试通过

#### 测试标准

```rust
#[test]
fn event_bus_delivers_to_subscriber() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    bus.subscribe(Box::new(move |_| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    }));

    bus.emit(AgentEvent::TurnStart { stream_id: "1".into(), session_id: "1".into() });
    bus.emit(AgentEvent::TurnEnd { stream_id: "1".into(), outcome: TurnOutcome::Completed });

    // 等待异步处理
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}
```

---

### TASK-013-E2: TelemetryCollector

**目标**: 实现 Token 使用和工具耗时的结构化统计

#### 具体任务

- [ ] 创建 `src-tauri/src/modules/harness/telemetry.rs`
- [ ] 定义 `TokenStats` 结构体
- [ ] 定义 `ToolStats` 结构体
- [ ] 实现 `TelemetryCollector::new(event_bus: &EventBus)`
- [ ] 实现 `get_token_stats()` 和 `get_tool_stats()`
- [ ] 订阅 LlmStart/LlmEnd, ToolStart/ToolEnd 事件
- [ ] 使用 `RwLock` 保证并发安全
- [ ] 编写测试

#### 实现代码

File: `src-tauri/src/modules/harness/telemetry.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::modules::harness::event_bus::{AgentEvent, EventBus, EventSubscriber, TurnOutcome};

/// Aggregated token usage statistics
#[derive(Debug, Clone, Default)]
pub struct TokenStats {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read: u64,
    pub total_cache_creation: u64,
    pub estimated_cost_usd: f64,
    pub llm_call_count: u64,
}

/// Aggregated tool call statistics
#[derive(Debug, Clone)]
pub struct ToolStats {
    pub name: String,
    pub call_count: u64,
    pub total_duration_ms: u64,
    pub success_count: u64,
    pub failure_count: u64,
}

/// Subscribes to agent events and aggregates telemetry data
pub struct TelemetryCollector {
    token_stats: Arc<RwLock<TokenStats>>,
    tool_stats: Arc<RwLock<HashMap<String, ToolStats>>>,
}

impl TelemetryCollector {
    pub fn new(event_bus: &EventBus) -> Self {
        let collector = Self {
            token_stats: Arc::new(RwLock::new(TokenStats::default())),
            tool_stats: Arc::new(RwLock::new(HashMap::new())),
        };

        // Subscribe to events
        let token_stats = Arc::clone(&collector.token_stats);
        let tool_stats = Arc::clone(&collector.tool_stats);

        event_bus.subscribe(Box::new(TelemetrySubscriber {
            token_stats,
            tool_stats,
        }));

        collector
    }

    pub fn get_token_stats(&self) -> TokenStats {
        self.token_stats.read().clone()
    }

    pub fn get_tool_stats(&self) -> HashMap<String, ToolStats> {
        self.tool_stats.read().clone()
    }
}

struct TelemetrySubscriber {
    token_stats: Arc<RwLock<TokenStats>>,
    tool_stats: Arc<RwLock<HashMap<String, ToolStats>>>,
}

impl EventSubscriber for TelemetrySubscriber {
    fn on_event(&self, event: &AgentEvent) {
        match event {
            AgentEvent::LlmEnd { tokens, .. } => {
                let mut stats = self.token_stats.write();
                stats.total_output_tokens += tokens;
                stats.llm_call_count += 1;
            }
            AgentEvent::ToolEnd { tool_name, duration_ms, success, .. } => {
                let mut stats = self.tool_stats.write();
                let entry = stats.entry(tool_name.clone()).or_insert(ToolStats {
                    name: tool_name.clone(),
                    call_count: 0,
                    total_duration_ms: 0,
                    success_count: 0,
                    failure_count: 0,
                });
                entry.call_count += 1;
                entry.total_duration_ms += duration_ms;
                if *success {
                    entry.success_count += 1;
                } else {
                    entry.failure_count += 1;
                }
            }
            _ => {}
        }
    }
}
```

#### 验收标准

- [ ] Token 统计与 LLM 响应中的 usage 一致
- [ ] 工具耗时统计准确
- [ ] 并发访问安全（RwLock）

---

### TASK-013-E3: AgentLoopIntegration

**目标**: 在 `start_agent_stream` 关键路径埋点

#### 具体任务

- [ ] 修改 `src-tauri/src/commands/agent.rs`
- [ ] 在以下位置插入事件发射：
    - Turn 开始时: `TurnStart`
    - LLM 调用前: `LlmStart`
    - LLM 调用后: `LlmEnd`
    - 工具执行前: `ToolStart`
    - 工具执行后: `ToolEnd`
    - 决策点 (工具选择): `DecisionPoint`
    - 错误时: `Error`
    - Turn 结束时: `TurnEnd`
- [ ] 确保不影响现有 SSE 事件流
- [ ] 验证编译通过

#### 实施计划

File: `src-tauri/src/commands/agent.rs` — 在 `start_agent_stream()` 中插入事件

```rust
use crate::modules::harness::event_bus::{AgentEvent, EventBus, TurnOutcome, DecisionType};

// 1. Turn start
EventBus::global().emit(AgentEvent::TurnStart {
    stream_id: stream_id.clone(),
    session_id: session_id.clone(),
});

// 2. LLM call before
EventBus::global().emit(AgentEvent::LlmStart {
    stream_id: stream_id.clone(),
    request_id: request_id.clone(),
});

// ... LLM call ...

// 3. LLM call after
EventBus::global().emit(AgentEvent::LlmEnd {
    stream_id: stream_id.clone(),
    request_id: request_id.clone(),
    tokens: output_tokens,
});

// 4. Tool call before
EventBus::global().emit(AgentEvent::ToolStart {
    stream_id: stream_id.clone(),
    tool_name: tool_name.clone(),
    tool_args: input.clone(),
});

// ... tool execution ...

// 5. Tool call after
EventBus::global().emit(AgentEvent::ToolEnd {
    stream_id: stream_id.clone(),
    tool_name: tool_name.clone(),
    duration_ms: duration.as_millis() as u64,
    success: !is_error,
});

// 6. Tool selection decision point
EventBus::global().emit(AgentEvent::DecisionPoint {
    stream_id: stream_id.clone(),
    point: DecisionType::ToolSelection {
        tool_name: tool_name.clone(),
        confidence: 1.0,
    },
    context: serde_json::json!({}),
});

// 7. Error path
EventBus::global().emit(AgentEvent::Error {
    stream_id: stream_id.clone(),
    error: error_message.clone(),
});

// 8. Turn end
EventBus::global().emit(AgentEvent::TurnEnd {
    stream_id: stream_id.clone(),
    outcome: TurnOutcome::Completed,
});
```

#### 验收标准

- [ ] 编译无错误
- [ ] 所有 8 个关键路径都有事件
- [ ] 不破坏现有 SSE 事件流

---

### TASK-013-E4: SessionRecorder

**目标**: 实现会话录制和重放

#### 具体任务

- [ ] 创建 `src-tauri/src/modules/harness/recorder.rs`
- [ ] 定义 `RecordingId`, `RecordedSession`, `RecordingMeta`
- [ ] 实现 SQLite 表 schema
- [ ] 实现 `start_recording(session_id) -> RecordingId`
- [ ] 实现 `stop_recording(recording_id)`
- [ ] 实现 `get_recording(recording_id) -> RecordedSession`
- [ ] 实现 `list_recordings(session_id) -> Vec<RecordingMeta>`
- [ ] 订阅所有 AgentEvent 并持久化
- [ ] 编写测试

#### SQLite Schema

```sql
CREATE TABLE IF NOT EXISTS harness_recordings (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    final_state TEXT
);

CREATE TABLE IF NOT EXISTS harness_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recording_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY (recording_id) REFERENCES harness_recordings(id)
);
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/harness/recorder.rs` | **创建** — SessionRecorder |
| `src-tauri/src/modules/harness/mod.rs` | 导出 recorder 模块 |

#### 验收标准

- [ ] 可录制完整会话
- [ ] 可查询录制列表和详情
- [ ] SQLite 存储正确
- [ ] 录制ID全局唯一

---

### TASK-013-E5: HarnessControl (IPC 命令)

**目标**: 向外部暴露 Tauri IPC 命令

#### 具体任务

- [ ] 创建 `src-tauri/src/commands/harness.rs`
- [ ] 定义 `TelemetrySnapshot` 结构体
- [ ] 定义 `DecisionRecord` 结构体
- [ ] 实现 5 个 Tauri 命令
- [ ] 在 `src-tauri/src/commands/mod.rs` 导出命令
- [ ] 在 `main.rs` 注册到 invoke_handler

#### 实现代码

File: `src-tauri/src/commands/harness.rs`

```rust
use serde::Serialize;
use tauri::State;
use crate::commands::AppState;

#[derive(Serialize)]
pub struct TelemetrySnapshot {
    pub session_id: String,
    pub token_stats: TokenStatsDto,
    pub tool_stats: Vec<ToolStatsDto>,
}

#[derive(Serialize)]
pub struct DecisionRecord {
    pub timestamp: String,
    pub decision_type: String,
    pub context: serde_json::Value,
}

#[tauri::command]
pub fn harness_get_telemetry(
    session_id: String,
    state: State<AppState>,
) -> Result<TelemetrySnapshot, String> {
    // Return telemetry from TelemetryCollector
    todo!("implement")
}

#[tauri::command]
pub fn harness_start_recording(
    session_id: String,
    state: State<AppState>,
) -> Result<String, String> {
    // Start a new harness recording
    todo!("implement")
}

#[tauri::command]
pub fn harness_stop_recording(
    recording_id: String,
    state: State<AppState>,
) -> Result<RecordedSessionDto, String> {
    // Stop recording and return session data
    todo!("implement")
}

#[tauri::command]
pub fn harness_get_decision_history(
    session_id: String,
    state: State<AppState>,
) -> Result<Vec<DecisionRecord>, String> {
    // Return decision history for session
    todo!("implement")
}

#[cfg(debug_assertions)]
#[tauri::command]
pub fn harness_inject_pause_point(
    session_id: String,
    reason: String,
    state: State<AppState>,
) -> Result<(), String> {
    // Inject a debug pause point
    todo!("implement")
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/commands/harness.rs` | **创建** — IPC 命令 |
| `src-tauri/src/commands/mod.rs` | **修改** — 导出 harness 命令 |
| `src-tauri/src/main.rs` | **修改** — 注册到 invoke_handler |

#### 验收标准

- [ ] IPC 命令可被 Rust harness-cli 调用
- [ ] 返回值可被解析
- [ ] `harness_inject_pause_point` 仅 debug build 有效

---

### TASK-013-E6: harness-cli (External CLI Binary)

**目标**: 实现独立的 Rust CLI 二进制

#### 具体任务

- [ ] 创建 `harness-cli/` 目录作为 workspace member
- [ ] 创建 `harness-cli/Cargo.toml`，依赖 clap
- [ ] 实现 CLI 命令
- [ ] 调用 Tauri IPC 获取数据
- [ ] 格式化输出 (JSON 或文本)

#### 实施计划

File: `harness-cli/Cargo.toml`

```toml
[package]
name = "harness-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "harness-cli"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
```

File: `harness-cli/src/main.rs`

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "harness-cli", about = "If2Ai Agent Loop Harness CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get telemetry for a session
    GetTelemetry {
        #[arg(long)]
        session_id: String,
    },
    /// Start recording a session
    StartRecording {
        #[arg(long)]
        session_id: String,
    },
    /// Stop recording and return session data
    StopRecording {
        #[arg(long)]
        recording_id: String,
    },
    /// Replay a recorded session
    Replay {
        #[arg(long)]
        recording_id: String,
        #[arg(long, default_value = "1.0")]
        speed: f32,
    },
    /// Show live dashboard
    Dashboard,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::GetTelemetry { session_id } => {
            // Fetch telemetry via IPC
            println!("Fetching telemetry for session: {}", session_id);
        }
        Commands::StartRecording { session_id } => {
            println!("Starting recording for session: {}", session_id);
        }
        Commands::StopRecording { recording_id } => {
            println!("Stopping recording: {}", recording_id);
        }
        Commands::Replay { recording_id, speed } => {
            println!("Replaying recording: {} at {}x speed", recording_id, speed);
        }
        Commands::Dashboard => {
            println!("Dashboard (TUI mode)");
        }
    }
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `harness-cli/Cargo.toml` | **创建** — CLI 包定义 |
| `harness-cli/src/main.rs` | **创建** — CLI 入口 |
| `Cargo.toml` (workspace) | **修改** — 添加 harness-cli member |

#### 验收标准

- [ ] CLI 可独立编译
- [ ] 命令参数解析正确
- [ ] 输出格式化正确

---

### TASK-013-E7: Integration Tests

**目标**: 验证完整的遥测-录制-重放流程

#### 具体任务

- [ ] 创建 `src-tauri/tests/harness_integration_test.rs`
- [ ] 编写集成测试:
    - `test_event_bus_no_drop` — 验证事件不丢失
    - `test_telemetry_stats_correct` — 验证统计正确
    - `test_recorder_records_full_session` — 验证录制完整
    - `test_control_ipc_commands` — 验证 IPC 命令
- [ ] 使用 mock event_bus 测试
- [ ] 使用 mock session 测试

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/tests/harness_integration_test.rs` | **创建** — harness 集成测试 |

#### 验收标准

- [ ] 所有集成测试通过
- [ ] 不依赖外部服务
- [ ] 可在 CI 中运行

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-013-E1 | EventBus 是所有模块的基础 |
| P0 | TASK-013-E2 | 遥测数据是核心价值 |
| P0 | TASK-013-E3 | 埋点是必须集成 |
| P1 | TASK-013-E4 | 录制功能支持复现 |
| P1 | TASK-013-E5 | IPC 是外部接口 |
| P1 | TASK-013-E6 | External CLI 是用户界面 |
| P2 | TASK-013-E7 | 集成测试保证质量 |

## 依赖关系

```
TASK-013-E1 EventBus ──┬── TASK-013-E2 TelemetryCollector
                       ├── TASK-013-E3 AgentLoopIntegration
                       └── TASK-013-E4 SessionRecorder
                                              │
TASK-013-E5 HarnessControl ←─────────────────┘
                │
TASK-013-E6 harness-cli ←────────────────────┘
                │
TASK-013-E7 IntegrationTests ←───────────────┘
```

## 验收总览

- [ ] EventBus 单例 thread-safe
- [ ] emit 不阻塞主线程
- [ ] Token 统计准确
- [ ] 工具耗时统计准确
- [ ] start_agent_stream 所有关键路径有事件
- [ ] SessionRecorder 可录制完整会话
- [ ] Tauri IPC 命令可被 harness-cli 调用
- [ ] harness-cli CLI 正确格式化输出
- [ ] 集成测试通过
- [ ] cargo fmt + clippy + test 全部通过
