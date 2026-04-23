# If2Ai vNext Session Runtime Design

> 版本：`1.0.0`
> 状态：`proposed`
> 前置依赖：[spec.md](./spec.md)、[vNext Blueprint](../design-docs/if2ai-vnext-session-runtime-blueprint.md)
> 实现约定：Rust 后端 + TypeScript 前端，模块边界遵循 `crate::modules::*`，编码规则引用 [CLAUDE.md](../../CLAUDE.md)
> 最后更新：`2026-04-23`

---

## 1. 目标架构总览

### 1.1 三层分离架构图

```text
┌─────────────────────────────────────────────────────────────────┐
│ Frontend                                                        │
│   AppShell / SessionShell                                       │
│   Projection Store (唯一 UI 真相)                                │
│   History Pager                                                 │
│   Action Facade (start / stop / approve / resume / retry)       │
└────────────────────────────┬────────────────────────────────────┘
                             │ Tauri IPC (Command / Event / Projection API)
┌────────────────────────────▼────────────────────────────────────┐
│ Gateway / Host Boundary                                         │
│   Session API / Stream Subscription / History Replay            │
│   Permission / Control API                                      │
└────────────────────────────┬────────────────────────────────────┘
                             │ typed application services
┌────────────────────────────▼────────────────────────────────────┐
│ Session Supervisor                                              │
│   Run Supervisor / Permission Pending Registry                  │
│   Retry / Resume Coordinator / Tool Attempt Tracker             │
│   Stream Fanout / Reconnect Recovery                            │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│ Runtime Core                                                    │
│   Prompt / Memory / Provider / Tool execution                   │
│   TurnService / ToolExecutionBroker                             │
│   Event emission (canonical runtime events)                     │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│ Persistence                                                     │
│   session_meta / run_log / projection checkpoints               │
│   permission_pending / tool_attempt ledger                      │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 层间数据流图

**事件流**（运行时事实写入）：
```
runtime core  →  event_log.append()  →  Tauri emit  →  projection bridge  →  reducer  →  store
```

**命令流**（用户动作下发）：
```
UI  →  Action Facade  →  Tauri invoke  →  application service  →  runtime core
```

**读取流**（冷启动 / 断线恢复）：
```
UI  →  Projection API  →  checkpoint + incremental replay  →  store
```

### 1.3 当前架构 vs 目标架构差异映射

| 层 | 已有 | 需新建 / 改造 |
|----|------|-------------|
| Frontend Projection | runtime-projection store, chat-run-projection, history-replay | supervisor-projection, attempt-projection, resume-projection |
| Gateway | api/conversations, api/streaming, api/sessions | Projection API (get_session_projection, get_supervisor_snapshot) |
| Supervisor | **缺失** — 生命周期散落在 TurnService/stream_task/session_manager | SessionSupervisor bounded context |
| Runtime Core | TurnService, stream_task, ToolExecutionBroker, stream_emitter | stream_task 分解为 5 模块 |
| Persistence | event_log, history, pending_permission | SessionMeta, ProjectionCheckpoint, SupervisorSnapshot, ToolAttempt Ledger |

---

## 2. 后端模块设计

### 2.1 Runtime Core 模块划分

```
src-tauri/src/modules/runtime/
├── event_log.rs           (已有 — DONE)    append-only run log
├── history.rs             (已有 — DONE)    history replay & paging
├── pending_permission.rs  (已有 — DONE)    permission recovery
├── stream_emitter.rs      (已有)           event emission boundary
├── supervisor.rs          (新建)           SessionSupervisor
├── attempt_ledger.rs      (新建)           ToolAttempt tracking
├── run_report.rs          (新建)           CanonicalRunReport generator
├── contracts/             (已有 — 扩展)    event types & correlation
│   ├── common.rs          (已有)           envelope, correlation
│   ├── activation.rs      (已有)
│   ├── execution_mode.rs  (已有)
│   ├── memory.rs          (已有)
│   └── prompt.rs          (已有)
└── session.rs             (已有 — 兼容)
```

### 2.2 SessionSupervisor 设计

**状态机**：

```text
                    start_run
   idle ──────────────────────► running
     ▲                              │
     │              run_completed   ├──► blocked (permission pending)
     │                     │        │
     │                     ▼        ├──► recoverable_failed
     │               completed      │        │
     │                              │        │ resume_run
     │                              │        ▼
     └──────────────────────────────┴── running
                                    │
                                    │ close_session
                                    ▼
                                 closed
```

**核心字段**：

```rust
pub struct SupervisorSnapshot {
    pub session_id: String,
    pub status: SupervisorStatus,           // idle / running / blocked / recoverable_failed / completed / closed
    pub active_run_id: Option<String>,
    pub active_run_status: Option<RunStatus>,  // streaming / completed / failed / cancelled
    pub pending_permission_count: usize,
    pub last_error_kind: Option<String>,
    pub recoverable: bool,
    pub retry_budget_remaining: u32,
    pub disconnect_grace_until: Option<String>,  // RFC3339
    pub last_updated_at: String,            // RFC3339
}
```

**快照持久化**：`{app_data_dir}/runtime/supervisor/{session_id}.json`

**与 TurnService 的关系**：
- TurnService 通知 supervisor（`supervisor.on_run_started()`, `supervisor.on_run_completed()` 等）
- Supervisor 不反向驱动 TurnService（单向通知）
- Supervisor 拥有生命周期状态的权威快照

### 2.3 ToolAttempt Ledger 设计

**attempt_id / attempt_no 生成策略**：
- `attempt_id` = UUID v4
- `attempt_no` = 同一 `tool_call_id` 下的单调递增序号，从 1 开始
- 每次 `retrying → running` 迁移时 `attempt_no` 递增

**状态机**：

```text
queued ──► authorizing ──► running ──┐
                                     │
                          ┌──────────┘
                          │
                          ▼
            completed / failed / cancelled / blocked
                          │
                          │ (if retryable)
                          ▼
                      retrying ──► running (attempt_no++)
```

**写入时机**：stream_task emit tool event 时同步写入 attempt_ledger + event_log

**查询接口**：
- `by_run_id(run_id) -> Vec<ToolAttempt>`
- `by_tool_call_id(tool_call_id) -> Vec<ToolAttempt>` （按 attempt_no 排序）
- `by_session_id(session_id) -> Vec<ToolAttempt>`

**与 event_log 的关系**：attempt_ledger 是结构化索引，不是独立事实源。底层事实来自 event_log 的 `tool_call_*` 事件。

### 2.4 Resume Contract 设计

**ResumeRecoverability 结构**：

```rust
pub struct ResumeRecoverability {
    pub resume_available: bool,
    pub resume_cursor: Option<String>,
    pub resume_reason: Option<ResumeReason>,
    pub safe_to_retry_mutations: bool,
    pub failed_attempt_ids: Vec<String>,
    pub last_settled_tool_call_id: Option<String>,
}

pub enum ResumeReason {
    ProviderTimeout,
    NetworkError,
    ProviderRateLimit,
    PermissionTimeout,
    UserCancelled,
    ToolFailure,
    ContextOverflow,
}
```

**safe_to_retry_mutations 判定规则**：

| 工具类型 | 判定 | 依据 |
|---------|------|------|
| 只读工具（read_file, list_directory, search） | 安全 | 无副作用 |
| 已知幂等工具（idempotent write, checksum verified） | 安全 | 工具声明 idempotent |
| 有副作用且无幂等键（bash, write_file） | 不安全 | 可能重复执行 |
| 部分成功（partial_success） | 需人工确认 | 部分 tool 已执行 |

**前端展示**：resume CTA + 原因说明（typed） + 风险标签（safe / unsafe / needs_confirmation）

### 2.5 stream_task.rs 分解设计（GAP-005）

当前 `stream_task.rs` (1249 LOC) 承担：model loop / tool loop / event emission / permission wait / finalization

**分解方案**：

```text
stream_task.rs (≤200 LOC)       — 顶层入口、状态协调、错误路由
├── stream_orchestrator.rs       — 顶层循环、状态机驱动、cancel 传播
├── model_loop.rs                — provider 交互、token 累积、resume cursor 管理
├── tool_loop.rs                 — 工具调度、attempt tracking、结果收集
├── permission_wait.rs           — 权限等待、超时、重试逻辑
└── stream_finalize.rs (已有)    — 结束事件、持久化收尾
```

**行为不变性保证**：分解前后，相同输入产生的 event 顺序必须等价。验收方式：对比分解前后的 event log 快照。

### 2.6 Command Boundary Thinning 设计（GAP-004）

**当前问题**：`commands/mod.rs` 的 `AppState` 聚合了大量 service / manager，command adapter 跨层直接访问。

**拆出 service registry 方案**：

```rust
// 新增：src-tauri/src/modules/application/service_registry.rs
pub struct ServiceRegistry {
    pub turn_service: Arc<TurnService>,
    pub session_supervisor: Arc<SessionSupervisor>,
    pub permission_service: Arc<PermissionService>,
    pub provider_service: Arc<ProviderService>,
    pub memory_coordinator: Arc<MemoryCoordinator>,
    // ... 不再直接暴露 manager
}
```

**command adapter 薄化**：

```rust
// 之前：command 直接编排多个 service
// 之后：command 只做参数校验 + service 调用 + error mapping
pub async fn start_agent_stream(state: State<'_, ServiceRegistry>, req: StreamRequest) -> Result<String> {
    validate_session_id(&req.session_id)?;
    let run_id = state.turn_service.stream_turn(req.into()).await
        .map_err(|e| map_service_error(e))?;
    Ok(run_id)
}
```

---

## 3. 前端模块设计

### 3.1 runtime-projection 层扩展

**现有文件**：
```
src/runtime-projection/
├── types.ts                    (620 行) — 核心类型定义
├── runtime-projection-store.ts (134 行) — 全局存储
├── runtime-event-queue.ts      — 微任务队列
├── runtime-event-translator.ts — payload 规范化
├── runtime-event-reducer.ts    (409 行) — 纯 reducer
├── runtime-projection-bridge.ts — Tauri 事件接线
├── chat-run-projection.ts      (266 行) — 聊天消息投影
├── history-replay.ts           — 历史重放
├── use-runtime-projection.ts   — React hooks
└── pending-permission-recovery.ts — 权限恢复
```

**新增模块**：

```typescript
// supervisor-projection.ts
interface SupervisorProjection {
  sessionId: string;
  status: "idle" | "running" | "blocked" | "recoverable_failed" | "completed" | "closed";
  activeRunId: string | null;
  pendingPermissionCount: number;
  recoverable: boolean;
  retryBudgetRemaining: number;
  lastErrorKind: string | null;
  capturedAt: number;
}

// attempt-projection.ts
interface AttemptProjection {
  attemptId: string;
  toolCallId: string;
  toolName: string;
  attemptNo: number;
  status: "queued" | "authorizing" | "running" | "retrying" | "completed" | "failed" | "cancelled" | "blocked";
  policyDecision?: "allow" | "deny" | "prompt";
  failureKind?: string;
  retryReason?: string;
  startedAt?: number;
  endedAt?: number;
  firstSeenAt: number;
}

// resume-projection.ts
interface ResumeProjection {
  runId: string;
  resumeAvailable: boolean;
  resumeCursor?: string;
  resumeReason?: string;
  safeToRetryMutations: boolean;
  failedAttemptIds: string[];
  riskLabel: "safe" | "unsafe" | "needs_confirmation";
}
```

**types.ts 扩展**：
- `RuntimeProjectionSnapshot` 新增 `supervisor: SupervisorProjection | null`
- `CanonicalRuntimeEvent` 新增 `supervisor_state_changed` / `attempt_started` / `attempt_settled` / `attempt_retried` / `resume_contract_resolved` / `session_opened` / `session_closed` / `session_compacted`
- `RunProjection` 新增 `attempts: Record<string, AttemptProjection>` 索引

### 3.2 MIG-017 Chat Truth Cutover 设计（核心）

**当前双真相精确路径**：

```text
Path A (Conversation):
  App.tsx listen("agent-token")
    → conversation-slice.appendMessage(sessionId, message)
    → ChatWorkspace reads useConversationStore()

Path B (Projection):
  runtime-projection-bridge listen("agent-token")
    → runtime-event-translator.translateAgentTokenPayload()
    → runtime-event-queue enqueue
    → runtime-event-reducer.reduceRuntimeEventBatch()
    → runtimeProjectionStore.dispatch()
    → ChatWorkspace reads useRuntimeProjection() + projectConversationMessagesFromRuns()
```

ChatWorkspace 当前渲染：`Conversation + RunProjection overlay` — 即先拿 conversation-slice 的消息列表，再用 `projectConversationMessagesFromRuns()` 把 RunProjection 覆盖上去。

**目标单一真相路径**：

```text
唯一路径 (Projection-only):
  runtime-projection-bridge listen("agent-token")
    → translator → queue → reducer → store
  ChatWorkspace 只从 runtimeProjectionStore 读消息
    → projectConversationMessagesFromRuns([], runs, sessionId)
    → 所有 assistant/tool 消息从 projection 派生
```

**4 步迁移步骤**：

**Step 1: Bridge 成为唯一事件入口**
- `App.tsx` 中 raw `agent-token` listener 的 `appendMessage` 调用标记 `@deprecated`
- Raw listener 仍存在，但只用于 dispatch 到 projection bridge
- 不新增任何直接消费 raw listener 的组件

**Step 2: ChatWorkspace 逐步切换到 projection selector**
- `activeMessages` 的计算从 `conversation-slice + RunProjection overlay` 改为 `projection-only`
- 用户消息（role=user）从 conversation-slice 读取（非 runtime truth）
- assistant/tool 消息从 `projectConversationMessagesFromRuns([], runs, sessionId)` 派生
- 移除 ChatWorkspace 对 `conversation-slice` 的 assistant/tool 消息依赖

**Step 3: conversation-slice 收缩为 UI 临时态存储**
- 保留：`sessionLoading` / `streamAbortHandles` / `sessionTitleStates` / `sessionTodos`
- 迁移到 projection store：所有 assistant/tool 消息内容
- `conversations` 字段仅保留 `user` 消息 + 元数据

**Step 4: Raw listener 完全降级**
- `App.tsx` 中 `agent-token` listener 逻辑移到 `runtime-projection-bridge`
- `App.tsx` 不再直接 import `conversation-slice.appendMessage`
- Conversation 初始化走 Projection API（冷启动）或 bridge 实时流

**projectConversationMessagesFromRuns 改造**：
- 当前签名：`(messages: Message[], runs: Record<string, RunProjection>, sessionId?) => Message[]`
- 目标签名：`(userMessages: UserMessage[], runs: Record<string, RunProjection>, sessionId?) => Message[]`
- 不再接受含 assistant/tool 的混合消息列表
- 返回值 = userMessages + 从 runs 派生的 assistant/tool 消息，按时间排序

### 3.3 Projection Checkpoint 与冷启动设计

**冷启动流程**：

```text
1. App opens session
2. → load SessionMeta (session list / title / project binding)
3. → get_session_projection(session_id)
     → load ProjectionCheckpoint (如果存在)
     → 从 last_applied_seq 之后 replay event log
4. → 得到最新 projection snapshot
5. → subscribe_session_events(session_id, after_seq = checkpoint.last_applied_seq)
6. → if pending permission → restore prompt from runtimeProjectionStore.approvals
7. → if recoverable run → show resume CTA from ResumeProjection
```

**Checkpoint 持久化策略**：
- 每次 `run_completed` / `run_cancelled` / `stream_error` 终结事件时保存
- 或者每 N 个 event（建议 N=100）保存
- 保存是 best-effort，失败不阻断主链

**断线恢复**：
- 前端记录当前 `lastAppliedSeq`
- 重连后 `subscribe_session_events(session_id, after_seq = lastAppliedSeq)`
- 增量 events 通过 reducer 合并到现有 snapshot

### 3.4 conversation-slice 退役设计

**当前职责审计**：

| 字段 | 归属 | 目标 |
|------|------|------|
| `conversations` | runtime truth (messages) | 迁移到 projection store |
| `sessionLoading` | UI 临时态 | 保留在 conversation-slice 或迁到 sessionStore |
| `streamAbortHandles` | UI 临时态 | 保留在 chat-store |
| `sessionTitleStates` | UI 临时态 | 保留在 sessionStore |
| `sessionTodos` | UI 临时态 | 保留在 sessionStore |

**最终状态**：conversation-slice 退役，仅保留 `sessionLoading` / `titleStates` / `todos` 在其他 store。

### 3.5 Memory UI Read Model Unification 设计（GAP-006）

**当前问题**：Memory UI 存在多读面，同一 memory 事实可从多个路径获取：
- 消息 `memoryContext`（从 stream_complete 获取）
- projection `memory.recentEvents`（从 bridge 获取）
- `MemoryBrowser` / `MemoryDebugTab`（直接 IPC 调用）
- `MemoryRollingProjection.lastRecallItems`（从 stream_complete 获取）

**统一方案**：

1. **统一 read model**：所有 memory UI 组件通过 `src/api/memory.ts` facade 访问
2. **MemoryProjection 扩展**：
   ```typescript
   // 统一的 memory read model
   interface MemoryReadModel {
     // 实时状态（从 projection store 派生）
     recentEvents: MemoryEventPayload[];
     lastRecallItems: MemoryContextItem[];
     writeDecisions: MemoryWriteDecisionProjection[];
     lastAfterTurn: MemoryAfterTurnProjection | null;
     invalidationVersion: number;
     // 持久化查询（从 API facade 获取）
     entries: Entry[];
     searchResults: SearchResult[];
   }
   ```
3. **消息 memoryContext 改从 projection 派生**：`runtimeProjectionStore.memory.lastRecallItems` 成为唯一来源
4. **MemoryBrowser / MemoryDebugTab 走 facade**：不再直接 IPC，统一通过 `getMemoryEntries` / `searchMemory` 等 facade 方法

**迁移步骤**：
1. 确认 `src/api/memory.ts` facade 已覆盖所有查询需求
2. MemoryBrowser / MemoryDebugTab 改用 facade
3. 消息 memoryContext 改从 projection store 读取
4. 移除直接 IPC 调用路径

---

## 4. 前后端交互协议设计

### 4.1 现有协议梳理

| 事件/命令 | 当前实现 | 类型 |
|----------|---------|------|
| `agent-token` | Tauri event, `StreamTokenPayload` | 事件流 |
| `permission-request` | Tauri event | 事件流 |
| `memory_event` | Tauri event | 事件流 |
| `memory_after_turn` | Tauri event | 事件流 |
| `start_agent_stream` | Tauri invoke | 命令 |
| `stop_agent_stream` | Tauri invoke | 命令 |
| `respond_permission` | Tauri invoke | 命令 |
| `get_session` | Tauri invoke | 查询 |
| `get_session_history_page` | Tauri invoke | 查询 |

### 4.2 向三类 API 演进设计

**Command API 演进**：

| 现有 | 目标 | 变更 |
|------|------|------|
| `start_agent_stream(session_id, ...)` | `start_run(session_id, input, mode)` | 返回 run_id 而非 stream_id |
| `stop_agent_stream(stream_id)` | `stop_run(session_id, run_id)` | 参数从 stream_id 改为 run_id |
| `respond_permission(session_id, decision)` | `resolve_permission(pending_id, decision, scope)` | 新增 pending_id + scope |
| — | `resume_run(session_id, resume_cursor)` | **新增** |
| — | `retry_run(session_id, failed_run_id, strategy)` | **新增** |

**Event Subscription API 演进**：

| 现有 | 目标 |
|------|------|
| `agent-token` broad subscription | `subscribe_session_events(session_id, after_seq?)` |
| `permission-request` subscription | 通过 session_events 过滤 |
| `memory_event` subscription | 通过 session_events 过滤 |
| — | `subscribe_run_events(run_id, after_seq?)` **新增**（细粒度 run 级订阅） |

**Projection API 演进**：

| 现有 | 目标 |
|------|------|
| `get_session` (完整 session 含 messages) | `get_session_projection(session_id)` (projection snapshot) |
| `get_session_history_page` | `get_history_page(session_id, before_seq?, limit)` |
| — | `get_pending_permissions(session_id)` **新增** |
| — | `get_run_summary(run_id)` **新增** |
| — | `get_supervisor_snapshot(session_id)` **新增** |

### 4.3 Contract 统一设计（GAP-002）

**当前三体问题**：

```text
RuntimeEventEnvelope (Rust contracts/common.rs)
  → schema_version, event_type, payload_family, emitted_at, correlation, payload
  → 不含 seq、event_id

StreamTokenPayload (Rust runtime/stream_emitter.rs)
  → stream_id, text, thinking, event_type, tool_call_id, ...
  → snake_case wire format, 不含 correlation

RunLogEntry (Rust runtime/event_log.rs)
  → event_id, session_id, run_id, seq, event_type, occurred_at, payload, causation_id, ...
  → append-only 存储，含完整审计字段
```

**统一方案**：

```text
StreamTokenPayload → 无损转换为 → RuntimeEventEnvelope
    translator: stream_id → correlation.stream_id
                event_type → payload_family
                补充 correlation.session_id / run_id

RuntimeEventEnvelope → 构造 → RunLogEntry
    补充: event_id (UUID), seq (monotonic), occurred_at (emitted_at)
    保留: 全部 correlation / causation_id / payload
```

**run_id 统一**：
- `stream_id` 降级为 transport compatibility alias
- 新代码优先使用 `run_id`
- 前端 `StreamRunBoundEvent` 的 `runId` 来源于 `stream_id`，M2.4+ 改为 `correlation.runId`

### 4.4 Contract Drift Guardrails 设计（GAP-008）

**Rust event kind vs TS translator 支持集对比**：

```typescript
// 测试：Rust 端新增的 event_type 必须在 TS translator 中有映射
test("all rust event types have ts translator mappings", () => {
  const rustEventTypes = loadRustEventTypes(); // 从 schema fixture 读取
  const tsMappings = Object.keys(translatorMap);
  const unmapped = rustEventTypes.filter(t => !tsMappings.includes(t));
  expect(unmapped).toEqual([]);
});
```

**Schema snapshot 策略**：
- 每次修改 contracts/ 后运行 `cargo test` 生成 `contracts-schema-snapshot.json`
- 前端测试加载此 snapshot 与 TS 类型定义对比
- CI 中 `pack verify` 包含此检查

---

## 5. 状态机设计

### 5.1 Session Lifecycle 状态机

```
states: created → active → (running | blocked | recoverable_failed) → closed

transitions:
  open_session:       created → active
  start_run:          active → running
  run_complete:        running → active
  run_fail_recoverable: running → recoverable_failed
  permission_pending:  running → blocked
  permission_resolved: blocked → running
  resume_run:         recoverable_failed → running
  close_session:      active → closed
```

Supervisor 拥有此状态机的权威快照。

### 5.2 Run Lifecycle 状态机

```
states: started → streaming → (completing | failing | cancelling) → terminal

terminal: completed | failed | cancelled

transitions:
  run_started:    → started/streaming
  text_delta:     → streaming (status stays)
  tool_call:      → streaming (status stays)
  stream_complete: → completing → completed | failed
  stream_error:    → failing → failed
  run_cancelled:   → cancelling → cancelled
```

与 event log 映射：每个 transition 对应一个 event_type。

### 5.3 Tool Attempt 状态机

```
states: queued → authorizing → running → (retrying → running)* → settled
settled: completed | failed | cancelled | blocked

transitions:
  tool_call_queued:    → queued
  permission_needed:   → authorizing
  permission_granted:  → running
  tool_executing:      → running
  tool_success:        → completed (settled)
  tool_error:          → failed (settled) 或 retrying
  tool_timeout:        → retrying (if budget) 或 failed (settled)
  tool_policy_blocked: → blocked (settled)
  user_cancelled:      → cancelled (settled)

attempt_no: 每次 retrying → running 时递增
```

### 5.4 Permission 状态机

```
states: requested → (resolved | expired)

resolved: allowed | denied

transitions:
  permission_requested: → requested
  permission_resolved:  → allowed | denied (resolved)
  permission_expired:   → expired (if expires_at passed)
```

与 event_log 映射：`permission_requested` / `permission_resolved` 各一条记录。

---

## 6. 关键代码路径设计

### 6.1 Chat Turn 完整路径（目标态）

```text
1. User input
   → startChatTurn() [src/api/conversations.ts]
   → Tauri invoke: start_run(session_id, input, mode)
   → TurnService::stream_turn()
   → SessionContextResolver + prompt/memory/provider/tool preparation

2. Run initialization
   → run_id created
   → RunEventLogger opened (event_log.append(run_started))
   → Supervisor notified (supervisor.on_run_started())

3. Stream execution
   → stream_task: model loop / tool loop / event emission
   → 每个关键节点同时:
     (a) emit Tauri event (agent-token)
     (b) append event log (event_log.append)
     (c) update supervisor (supervisor.on_status_changed)

4. Frontend projection
   → runtime-projection-bridge: translate → reduce → store
   → Chat UI: read projection store only (projectConversationMessagesFromRuns)
   → 不再同时依赖 conversation-slice

5. Stream finalization
   → stream_finalize: terminal event + session compatibility state
   → event_log.append(run_completed / stream_error)
   → supervisor.on_run_completed()
```

### 6.2 Session 冷启动路径

```text
1. App opens session
   → load SessionMeta
   → get_session_projection(session_id)

2. Projection recovery
   → load ProjectionCheckpoint
   → replay event log from last_applied_seq
   → 得到最新 RuntimeProjectionSnapshot

3. Subscribe incremental
   → subscribe_session_events(session_id, after_seq)
   → incoming events → reducer → store

4. UI renders from projection store
   → if pending permission → restore prompt
   → if recoverable run → show resume CTA
```

### 6.3 Permission Recovery 路径

```text
1. Tool needs approval
   → PermissionService creates pending request
   → pending_permission.rs persists record
   → event_log.append(permission_requested)
   → Tauri event → projection bridge → store.approvals

2. UI renders permission prompt
   → ChatWorkspace reads runtimeProjectionStore.approvals
   → User responds → resolve_permission(pending_id, decision)

3. Resolution
   → pending cleared (pending_permission.rs)
   → event_log.append(permission_resolved)
   → supervisor.on_permission_resolved()
   → runtime continues
```

### 6.4 Resume Recovery 路径

```text
1. Run fails
   → stream_error with resume contract
   → supervisor marks recoverable_failed + typed resume_reason
   → event_log records terminal event with recoverability

2. UI shows resume CTA
   → ResumeProjection provides: reason + risk label + safe_to_retry_mutations

3. User clicks resume
   → resume_run(session_id, resume_cursor)
   → supervisor validates + TurnService resumes from cursor
   → event_log.append(resume_contract_resolved)
   → new run starts from recovery point
```

---

## 7. 数据持久化设计

### 7.1 Run Event Log 存储

- 格式：JSONL，每行一个 `RunLogEntry`
- 路径：`{app_data_dir}/runtime/run-log/{session_id}/{run_id}.jsonl`
- 写入：`append()` 异步 / `append_sync()` 同步
- 最佳效力：IO 失败不阻断主链，但通过 `tracing::warn!` 可观测
- 敏感信息：写入前自动 redaction

### 7.2 SessionMeta 存储

- 从 session.json 提取 metadata 字段
- session.json 降级为 compatibility 存储
- 新字段写入只进 event log / SessionMeta
- 读取优先 event log 派生，fallback 到 session.json

### 7.3 Projection Checkpoint 存储

- 路径：`{app_data_dir}/runtime/projection-checkpoints/{session_id}/{kind}.json`
- 保存时机：每次 terminal event 或每 N 个 event
- 恢复时优先读 checkpoint + 增量 replay
- 保存是 best-effort

### 7.4 Supervisor Snapshot 存储

- 路径：`{app_data_dir}/runtime/supervisor/{session_id}.json`
- 每次状态迁移时同步更新
- 前端通过 `get_supervisor_snapshot(session_id)` 读取

### 7.5 ToolAttempt Ledger 存储

- 选项 A：SQLite table（推荐，支持复杂查询）
- 选项 B：JSONL 索引（与 event_log 同结构）
- 与 event_log 的关系：attempt_ledger 是结构化视图，不是独立事实源
- 查询通过 `attempt_ledger.rs` 的公开接口，不直接读取存储

### 7.6 PermissionPending 持久化

- 路径：`{app_data_dir}/runtime/pending-permissions/{session_id}.json`（已有，MIG-019 DONE）
- 现有结构 `PendingPermissionRecord` 需扩展：新增 `pending_id`、`run_id`、`tool_call_id`、`expires_at`、`resolution` 字段
- 与 spec §3.4 对齐：扩展后的字段覆盖完整的 PermissionPending 数据模型
- 写入时机：`permission_requested` 事件发射时同步写入
- 清除时机：`permission_resolved` 后清除文件，同时将 resolution 写入 event_log

### 7.7 CanonicalRunReport 生成策略

- 生成方式：**按需实时计算**，从 event log 派生，不持久化独立存储
- 触发时机：`get_run_summary(run_id)` API 调用时生成
- 生成流程：
  1. 读取 run 的所有 event log entries
  2. 遍历 entries，按 section 聚合（Run Summary / Stream Outcome / Tool Summary / Permission Summary / Recoverability / Memory Summary）
  3. Tool Summary 从 attempt_ledger 补充 timeline 信息
  4. Recoverability 从最近 terminal event 的 payload 提取
- 缓存策略：可选，在 ProjectionCheckpoint 中缓存最近 N 个 run 的 report
- 性能目标：单 run report 生成 < 50ms（< 1000 events 的 run）

---

## 8. 迁移路径设计

### 8.1 session.json 拆分迁移（GAP-001）

- **Phase 1**：metadata 字段标记为迁移目标，新增字段只写 SessionMeta
- **Phase 2**：history 读取优先 event log，session.json 作为 fallback
- **Phase 3**：session.json 只保留 compatibility 字段（identity_override / memory_enabled 等），transcript 完全由 event log + projection 提供

Fallback 策略：event log 为空时仍可读 session.json，但必须记录 fallback 事件。

### 8.2 前端 truth 迁移（GAP-003 / MIG-017）

- **Phase 1**：Bridge 成为唯一事件入口，raw listener 的 appendMessage 标记 deprecated
- **Phase 2**：ChatWorkspace 逐步切到 projection selector
- **Phase 3**：conversation-slice 退役 runtime truth 职责
- **Phase 4**：Raw listener 完全降级，App.tsx 不再直接消费 agent-token

### 8.3 Harness truth 迁移（GAP-007 / MIG-023）

- **Phase 1**：canonical run report 可独立生成（从 event log 派生）
- **Phase 2**：grader / replay 改读 canonical run report
- **Phase 3**：旧 trace 路径标记为 fallback only，不新增基于旧 trace 的能力

---

## 9. 关键实施文件索引

| 文件 | 职责 | 迁移影响 |
|------|------|---------|
| `src/runtime-projection/runtime-event-reducer.ts` | 核心投影逻辑 | MIG-017 cutover 主要改造目标 |
| `src/runtime-projection/chat-run-projection.ts` | chat 消息投影 | projectConversationMessagesFromRuns 签名改造 |
| `src/runtime-projection/runtime-projection-bridge.ts` | 事件接线 | 成为唯一事件入口 |
| `src/stores/conversation-slice.ts` | 双真相来源之一 | 退役 runtime truth 职责 |
| `src-tauri/src/modules/application/turn_service/stream_task.rs` | 后端执行 god-file | GAP-005 分解 + attempt ledger 插入点 |
| `src-tauri/src/modules/runtime/contracts/common.rs` | runtime contract 定义 | GAP-002 统一 |
| `src-tauri/src/modules/runtime/event_log.rs` | append-only 事实源 | 所有新持久化模式的参照模板 |
| `src-tauri/src/modules/runtime/stream_emitter.rs` | 事件发射边界 | Contract 统一改造点 |
