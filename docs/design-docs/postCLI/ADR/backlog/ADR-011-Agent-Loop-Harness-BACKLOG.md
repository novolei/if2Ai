# ADR-011 Agent Loop Harness Framework — 详细实施 backlog

## 概述

**目标**: 为 if2Ai 的 AI Agent Loop 构建内置的运行时观测与控制框架
**ADR**: [ADR-011](./ADR-011-Agent-Loop-Harness-Framework.md)
**优先级**: P0 (核心基础设施)
**预估工时**: 5-6 天
**现状**: `src-tauri/src/modules/harness/` 目录不存在，需要创建

---

## 子任务清单

### TASK-011-01: EventBus 核心

**目标**: 实现事件总线单例和事件类型

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/harness/mod.rs`
- [ ] 定义 `AgentEvent` 枚举:
  ```rust
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
- [ ] 实现 `EventBus` 单例 (`global()`)
- [ ] 实现 `EventSubscriber` trait
- [ ] 实现 `subscribe()` 和 `emit()` 方法
- [ ] 使用 `mpsc::unbounded_channel` 非阻塞发送
- [ ] 编写单元测试

**验收标准**:
- [ ] EventBus 是 thread-safe 单例
- [ ] emit 不阻塞主线程
- [ ] 事件不丢失

**测试标准**:
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
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}
```

---

### TASK-011-02: TelemetryCollector

**目标**: 实现 Token 使用和工具耗时的结构化统计

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/harness/telemetry.rs`
- [ ] 定义 `TokenStats` 结构体:
  ```rust
  pub struct TokenStats {
      pub total_input_tokens: u64,
      pub total_output_tokens: u64,
      pub total_cache_read: u64,
      pub total_cache_creation: u64,
      pub estimated_cost_usd: f64,
  }
  ```
- [ ] 定义 `ToolStats` 结构体:
  ```rust
  pub struct ToolStats {
      pub name: String,
      pub call_count: u64,
      pub total_duration_ms: u64,
      pub success_count: u64,
      pub failure_count: u64,
  }
  ```
- [ ] 实现 `TelemetryCollector::new(event_bus: &EventBus)`
- [ ] 实现 `get_token_stats()` 和 `get_tool_stats()`
- [ ] 订阅 LlmStart/LlmEnd, ToolStart/ToolEnd 事件
- [ ] 使用 `RwLock` 保证并发安全
- [ ] 编写测试

**验收标准**:
- [ ] Token 统计与 LLM 响应中的 usage 一致
- [ ] 工具耗时统计准确
- [ ] 并发访问安全

---

### TASK-011-03: AgentLoopIntegration

**目标**: 在 start_agent_stream 关键路径埋点

**具体任务**:
- [ ] 修改 `src-tauri/src/commands/agent.rs`
- [ ] 在函数签名中添加 `event_bus: &EventBus` 参数
- [ ] 在以下位置插入事件发射:
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

**验收标准**:
- [ ] 编译无错误
- [ ] 所有关键路径都有事件
- [ ] 不破坏现有功能

---

### TASK-011-04: SessionRecorder

**目标**: 实现会话录制和重放

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/harness/recorder.rs`
- [ ] 定义 `RecordingId`, `RecordedSession`, `RecordingMeta`
- [ ] 实现 SQLite 表 schema:
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
- [ ] 实现 `start_recording(session_id) -> RecordingId`
- [ ] 实现 `stop_recording(recording_id)`
- [ ] 实现 `get_recording(recording_id) -> RecordedSession`
- [ ] 实现 `list_recordings(session_id) -> Vec<RecordingMeta>`
- [ ] 订阅所有 AgentEvent 并持久化
- [ ] 编写测试

**验收标准**:
- [ ] 可录制完整会话
- [ ] 可重放录制内容
- [ ] SQLite 存储正确

---

### TASK-011-05: HarnessControl (IPC 命令)

**目标**: 向外部暴露 Tauri IPC 命令

**具体任务**:
- [ ] 创建 `src-tauri/src/commands/harness.rs`
- [ ] 定义 `TelemetrySnapshot` 结构体
- [ ] 定义 `DecisionRecord` 结构体
- [ ] 实现 Tauri 命令:
  ```rust
  #[tauri::command]
  pub fn harness_get_telemetry(session_id: String) -> TelemetrySnapshot

  #[tauri::command]
  pub fn harness_start_recording(session_id: String) -> Result<RecordingId, String>

  #[tauri::command]
  pub fn harness_stop_recording(recording_id: String) -> Result<RecordedSession, String>

  #[tauri::command]
  pub fn harness_get_decision_history(session_id: String) -> Vec<DecisionRecord>

  #[cfg(debug_assertions)]
  #[tauri::command]
  pub fn harness_inject_pause_point(session_id: String, reason: String) -> Result<(), String>
  ```
- [ ] 在 `src-tauri/src/commands/mod.rs` 导出命令
- [ ] 编写测试

**验收标准**:
- [ ] IPC 命令可被 Rust harness-cli 调用
- [ ] 返回值可被解析
- [ ] debug pause point 仅 debug build 有效

---

### TASK-011-06: harness-cli (External CLI Binary)

**目标**: 实现独立的 Rust CLI 二进制

**具体任务**:
- [ ] 创建 `harness-cli/` 目录作为 workspace member
- [ ] 创建 `harness-cli/Cargo.toml`，依赖 clap 和 tauri
- [ ] 实现 CLI 命令:
  ```rust
  #[derive(Parser)]
  enum Commands {
      GetTelemetry { session_id: String },
      StartRecording { session_id: String },
      StopRecording { recording_id: String },
      Replay { recording_id: String, #[arg(default = "1.0")] speed: f32 },
      Dashboard,
  }
  ```
- [ ] 调用 Tauri IPC 获取数据
- [ ] 格式化输出 (JSON 或 文本)
- [ ] 可选: 用 ratatui 实现 TUI dashboard

**验收标准**:
- [ ] CLI 可独立编译
- [ ] 命令参数解析正确
- [ ] 输出格式化正确

---

### TASK-011-07: Integration Tests

**目标**: 验证完整的遥测-录制-重放流程

**具体任务**:
- [ ] 创建 `src-tauri/test/harness_integration_test.rs`
- [ ] 编写集成测试:
  - `test_event_bus_no_drop` — 验证事件不丢失
  - `test_telemetry_stats_correct` — 验证统计正确
  - `test_recorder_records_full_session` — 验证录制完整
  - `test_control_ipc_commands` — 验证 IPC 命令
- [ ] 使用 mock event_bus 测试
- [ ] 使用 mock session 测试

**验收标准**:
- [ ] 所有集成测试通过
- [ ] 不依赖外部服务
- [ ] 可在 CI 中运行

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-011-01 | EventBus 是所有模块的基础 |
| P0 | TASK-011-02 | 遥测数据是核心价值 |
| P0 | TASK-011-03 | 埋点是必须集成 |
| P1 | TASK-011-04 | 录制功能支持复现 |
| P1 | TASK-011-05 | IPC 是外部接口 |
| P1 | TASK-011-06 | External CLI 是用户界面 |
| P2 | TASK-011-07 | 集成测试保证质量 |

---

## 依赖关系

```
TASK-011-01 EventBus ──┬── TASK-011-02 TelemetryCollector
                       ├── TASK-011-03 AgentLoopIntegration
                       └── TASK-011-04 SessionRecorder
                                              │
TASK-011-05 HarnessControl ←─────────────────┘
                │
TASK-011-06 harness-cli ←────────────────────┘
                │
TASK-011-07 IntegrationTests ←───────────────┘
```

---

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
