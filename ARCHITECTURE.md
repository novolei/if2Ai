# If2Ai 架构全景

> 最后更新: 2026-05-05（基于完整 codebase 重新扫描；CLAUDE.md 同步至 Superpowers SKILLS 流程）
>
> 本文档描述当前 codebase 的真实架构、设计哲学、可视化数据流、vNext 目标态、以及与 [`docs/IMPROVEMENTS-2026-05-05.md`](docs/IMPROVEMENTS-2026-05-05.md) 配套的 Gap / 二次真相清单。
>
> **Pack / CHARTER / REGISTRY 已正式弃用**。开发流程统一走 **Superpowers SKILLS**（见 [`CLAUDE.md`](CLAUDE.md) §Development Workflow）。架构联合审计仍以本文与 [`.qoder/specs/if2ai-agent-evolution-report.md`](.qoder/specs/if2ai-agent-evolution-report.md) 双文档为准。

---

## 1. Staff 摘要

If2Ai 是一个 **React + TypeScript + Tauri 2 + Rust/Tokio** 的桌面 Agent 应用。当前架构处于 vNext 收敛末期：

- **运行时事实源已收敛到 `runtime_event` 单通道**：PR C-1/C-2/C-3（2026-05-01）+ PR D-1（2026-05-02）已退役 `agent-token` / `permission-request` / `memory_event` 三个旧通道；所有运行时事件经 [`runtime_event::dispatch`](src-tauri/src/modules/runtime/runtime_event.rs) 同时广播到 UI 与 append-only run log。
- **Session 生命周期 owner 已建立**：[`runtime/supervisor.rs`](src-tauri/src/modules/runtime/supervisor.rs)（MIG-020）持有 `SupervisorSnapshot`，状态机覆盖 `Idle → Running → Blocked → RecoverableFailed/Completed/Closed`。
- **History 优先 run-log JSONL**：MIG-018 完成后 [`history.rs`](src-tauri/src/modules/runtime/history.rs) 仅当 `session_has_run_log_jsonl_files` 为 false 时回落 `session.json`。
- **前端 projection 已是 chat 真值源**：[`runtime-projection-bridge`](src/runtime-projection/runtime-projection-bridge.ts) 单 channel 订阅 + envelope router；`conversation-slice` 中 transcript 写入路径已加 `@deprecated` 标记（MIG-017）。

**当前最大风险不再是缺能力，而是三处局部漂移**：
1. 前端 god-component（`chat-ui.tsx` 5,043 行 + `App.tsx` 3,310 行 + `lib/tauri.ts` 3,414 行）阻碍 projection-first 的最后一公里。
2. 后端两处巨型文件（`work_loop.rs` 3,226 行 + `stream_finalize.rs` 1,503 行）承载过多编排细节。
3. Harness 仍以独立 EventBus 生成 `HarnessRunReport`（MIG-023 partial），与 canonical run log 形成长期并行真相。

详见 [`docs/IMPROVEMENTS-2026-05-05.md`](docs/IMPROVEMENTS-2026-05-05.md)。

---

## 2. 设计理念（Design Philosophy）

If2Ai 的架构由七条北极星原则统辖。每条原则都直接对应代码层面的强约束。

### 2.1 七条核心原则

| # | 原则 | 含义 | 落地位置 |
|---|------|------|----------|
| **P1** | **Single Source of Truth** | 同一事实只能有一个写入点；多读视图必须可派生。 | run event log 写、projection 读；session.json 仅为迁移 fallback。 |
| **P2** | **Append-only Event Log** | Runtime 事实以不可变事件形式落盘，时间线本身可被 replay。 | [`runtime/event_log.rs`](src-tauri/src/modules/runtime/event_log.rs) 写 JSONL；[`history.rs`](src-tauri/src/modules/runtime/history.rs) 分页 replay。 |
| **P3** | **Projection-first UI** | 前端 UI 不订阅 raw event，只读稳定的 projection 快照；天然支持 reload / time-travel / multi-window。 | [`runtime-projection/`](src/runtime-projection/) — bridge / translator / reducer / store。 |
| **P4** | **Layered Boundaries, No Cross-cuts** | 严格分层依赖，向下单向。Commands 不互相 import；runtime 不依赖 application；harness 不向 turn loop 反向写入。 | [`CLAUDE.md`](CLAUDE.md) §Architecture Boundaries 强约束。 |
| **P5** | **Typed Facade over Raw IPC** | 前端不直连 `@tauri-apps/api`；统一走 `src/api/*` 与 `src/transport/contracts.ts`。Wire schema 是契约，不是实现细节。 | [`src/api/`](src/api/) 与 [`src/transport/contracts.ts`](src/transport/contracts.ts)。 |
| **P6** | **Explicit Lifecycle Owner** | Session / run / permission 的生命周期必须有显式 owner（不是散落 flag）。 | [`runtime/supervisor.rs`](src-tauri/src/modules/runtime/supervisor.rs) (MIG-020) + [`pending_permission.rs`](src-tauri/src/modules/runtime/pending_permission.rs) (MIG-019)。 |
| **P7** | **Observability ≠ Truth** | Harness / telemetry 是观测视图，不创造新事实；最终须从 canonical event log 派生（MIG-023 收敛中）。 | [`harness/`](src-tauri/src/modules/harness/) 与 §6.1 过渡契约。 |

### 2.2 推论：架构守则

从七原则推出的硬约束，已编码到 [`CLAUDE.md`](CLAUDE.md) 的 Hard Rules：

- 不再新增直接消费 raw Tauri event 的 UI surface（违反 P3）。
- 不再新增并行 Tauri 通道，所有 emit 走 `runtime_event::dispatch`（违反 P1/P5）。
- 不再扩展 `session.json` 承载 transcript / projection / resume（违反 P1/P2）。
- 不再让 harness、projection、session manager 各自生成独立 run truth（违反 P1/P7）。
- 所有 runtime event 必须能关联 `session_id` / `run_id`，必要时 `turn_id` / `stream_id` / `team_id` / `member_id` / `delegation_id`（违反 P6）。

### 2.3 创新性差异化方向

不仅仅是补齐基础架构，下列能力作为 If2Ai 的产品差异化：

- **Flight Recorder Runtime** — 每个 session/team run 都是可回放黑盒，用于 debug / review / resume / training。
- **Projection-first Desktop UX** — UI 读稳定 projection，原生支持 reload / reconnect / time-travel / multi-window。
- **Explainable Tool Attempt Ledger** — [`attempt_ledger.rs`](src-tauri/src/modules/runtime/attempt_ledger.rs) 8 状态机记录每次工具尝试、失败、重试、人工批准（MIG-022）。
- **Policy-as-Data Agent Teams** — Team 协作策略可配置、可审计、可复用（见 §10）。
- **Memory Promotion Gate** — 从 run evidence 到 long-term/team memory 有显式证据与审批边界。
- **Reviewer-native Workflow** — planner/executor/reviewer 是一等协作角色，review 是 runtime graph 节点而非事后检查。
- **Session Integrity Score** — 基于 event log 完整性、missing event、permission dangling、tool retry exhaustion、provider failure 生成会话完整性指标。

---

## 3. 系统全景（System Topology）

### 3.1 完整分层

```text
┌──────────────────────────────────────────────────────────────────────────┐
│  Frontend  (React + TypeScript, Vite :9527)                              │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  App.tsx  (boot sequencer + projection wiring)                   │    │
│  │     │                                                            │    │
│  │     └─▶ AppShell ─▶ ContentRouter ─▶ {Chat, Memory, Settings,    │    │
│  │                                       Skills, Jiaochang,         │    │
│  │                                       Browser, …}                │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│                       │                       │                          │
│                       ▼                       ▼                          │
│  ┌──────────────────────────┐    ┌────────────────────────────────┐     │
│  │  src/api/* (typed facade) │    │  src/runtime-projection/*       │     │
│  │  conversations, streaming │    │  bridge → translator → reducer  │     │
│  │  sessions, memory, models │    │  → RuntimeProjectionSnapshot    │     │
│  └──────────────────────────┘    └────────────────────────────────┘     │
│                       │                       ▲                          │
│                       ▼                       │                          │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  src/transport/contracts.ts  (canonical wire envelopes)           │    │
│  │  src/lib/tauri.ts            (transport seam — to be thinned)     │    │
│  └──────────────────────────────────────────────────────────────────┘    │
└────────────────────────────────────┬─────────────────────────────────────┘
                                     │  Tauri IPC: invoke + listen('runtime_event')
                                     ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  Backend  (Rust, Tokio, binary: if2ai-backend)                           │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  commands/     (IPC adapters; thin; AppState injection only)     │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│                                  │                                       │
│                                  ▼                                       │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  modules/application/                                             │    │
│  │     turn_service/  (TurnService — canonical chat orchestrator)    │    │
│  │     prompt_coordinator, memory_coordinator,                       │    │
│  │     request_intelligence, provider_service,                       │    │
│  │     permission_service, tool_executor, …                          │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│                                  │                                       │
│                                  ▼                                       │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  modules/control_plane/  (policy boundary)                       │    │
│  │     SessionContextResolver, prepare_step_execution,               │    │
│  │     boundary_resolver, tool_execution_broker, audit, …            │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│                                  │                                       │
│                                  ▼                                       │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  modules/runtime/  (vNext core — truth source)                   │    │
│  │     event_log.rs           ← append-only JSONL writer             │    │
│  │     history.rs             ← paged replay (run-log preferred)     │    │
│  │     supervisor.rs          ← MIG-020 lifecycle snapshot owner     │    │
│  │     runtime_event.rs       ← canonical dispatch helper            │    │
│  │     stream_emitter.rs      ← envelope-only emit (post-D-1)        │    │
│  │     conversation.rs        ← agentic loop                          │    │
│  │     run_delegate.rs        ← async↔sync streaming bridge          │    │
│  │     attempt_ledger.rs      ← MIG-022 tool attempt state machine   │    │
│  │     pending_permission.rs  ← MIG-019 permission persistence       │    │
│  │     contracts/*            ← wire schemas (mirrored to TS)        │    │
│  │     budget, compact, mcp, resume, working_checkpoint, self_repair │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│                                  │                                       │
│                                  ▼                                       │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  Domain Modules (bounded contexts)                                │    │
│  │     memory  identity  projects  session  tools  provider          │    │
│  │     learning  harness  security  browser  smart_browser           │    │
│  │     scheduler  skills  desktop_host  observability  updater       │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│                                  │                                       │
│                                  ▼                                       │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │  Persistence (~/.if2ai/)                                          │    │
│  │     SQLite (memory + sessions + ledger)                           │    │
│  │     LanceDB (vector embeddings)                                   │    │
│  │     runtime/run-log/<session>/<run>.jsonl                         │    │
│  │     runtime/supervisor/<session>.json                              │    │
│  │     runtime/pending_permission/                                    │    │
│  │     session.json (legacy fallback only)                           │    │
│  └──────────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────────┘
```

### 3.2 模块计数（2026-05-05 实测）

| 区域 | 模块/文件数（核心） | 关键巨型文件（LOC）|
|------|---------------------|--------------------|
| `src-tauri/src/modules/runtime/` | 71 文件 ~13K LOC | `conversation.rs` 1471 / `compact.rs` 965 / `budget.rs` 864 / `history.rs` 710 / `event_log.rs` 653 / `run_delegate.rs` 644 / `attempt_ledger.rs` 579 / `stream_emitter.rs` 548 / `supervisor.rs` 449 |
| `src-tauri/src/modules/application/turn_service/` | 18 文件 | `work_loop.rs` 3226 / `stream_finalize.rs` 1503 / `stream_iteration.rs` 1078 / `stream_task.rs` 996 / `run.rs` 957 / `stream_tool_execution.rs` 871 / `stream.rs` 854 |
| `src/runtime-projection/` | 16 文件 ~4.8K LOC | `types.ts` 828 / `runtime-event-reducer.ts` 531 / `runtime-event-translator.ts` 437 / `runtime-projection-bridge.ts` 390 |
| `src/api/` | 16 文件 ~2.3K LOC | `memory.ts` 647 |
| `src/transport/` | 6 文件 ~1.5K LOC | `contracts.ts` 935 |
| Frontend god-files | — | `chat-ui.tsx` **5043** / `lib/tauri.ts` **3414** / `App.tsx` **3310** |

---

## 4. 后端架构（Backend Detail）

### 4.1 Bootstrap 与 Command Boundary

| 路径 | 职责 |
|------|------|
| [`src-tauri/src/main.rs`](src-tauri/src/main.rs) | Tauri 应用 entry，委托 `bootstrap::*`。 |
| [`src-tauri/src/bootstrap/mod.rs`](src-tauri/src/bootstrap/mod.rs) | 路径解析、初始化编排。 |
| [`src-tauri/src/bootstrap/app.rs`](src-tauri/src/bootstrap/app.rs) | AppState 构建、SQLite/LanceDB 装配。 |
| [`src-tauri/src/bootstrap/memory.rs`](src-tauri/src/bootstrap/memory.rs) | 记忆子系统启动（providers / stores）。 |
| [`src-tauri/src/bootstrap/migration.rs`](src-tauri/src/bootstrap/migration.rs) | MEM-MOD-PATH-FIX：`~/Library/Application Support/.if2ai/` → `~/.if2ai/` 一次性、幂等迁移。 |
| [`src-tauri/src/commands/mod.rs`](src-tauri/src/commands/mod.rs) | AppState 聚合 + 所有 service / manager。**仍偏大，待瘦身**。 |
| [`src-tauri/src/commands/command_surface.rs`](src-tauri/src/commands/command_surface.rs) | 宏化命令注册组。 |
| [`src-tauri/src/commands/agent/`](src-tauri/src/commands/agent/) / [`session.rs`](src-tauri/src/commands/session.rs) / [`memory/`](src-tauri/src/commands/memory/) / … | 25+ 命令模块；每个适配器 5–15 LOC。 |

**约束**：commands 互不 import；所有依赖经 AppState 注入。

### 4.2 Application Layer — TurnService

[`src-tauri/src/modules/application/turn_service/`](src-tauri/src/modules/application/turn_service/) 是 chat 执行脊柱。

| 文件 | LOC | 职责 |
|------|-----|------|
| `mod.rs` | 781 | `TurnService` 结构、`TurnServiceDeps`、公共方法。 |
| `run.rs` | 957 | 非流式 turn 生命周期 (`run_turn`)。 |
| `stream.rs` | 854 | 流式 turn 入口 (`stream_turn`)。 |
| `stream_task.rs` | 996 | 主循环 orchestrator。 |
| `stream_finalize.rs` | **1503** | 后处理（guardrails / outcomes / dispatch / persistence 收尾）。**god-file 警示**。 |
| `stream_iteration.rs` | 1078 | 单次迭代 preflight + tool execution。 |
| `stream_tool_execution.rs` | 871 | 工具批量执行。 |
| `stream_event_loop.rs` | 675 | SSE 事件内循环。 |
| `stream_delegate.rs` | 661 | 流式 callback↔同步 consumer 桥。 |
| `stream_preflight.rs` | 307 | 迭代前置检查。 |
| `stream_loop_state.rs` | 188 | 循环状态机。 |
| `stream_task_run_log.rs` | 138 | run log helper。 |
| `prompt_cache.rs` | 169 | 提示缓存策略。 |
| `preflight_hooks.rs` | 250 | 启动钩子。 |
| `todo_ledger.rs` | 263 | TODO 工具 ledger。 |
| `work_loop.rs` | **3226** | Work-loop 路由 + skill resolution。**最大 god-file**。 |
| `tests.rs` | 344 | 单元测试。 |

**`TurnServiceDeps` 注入项**：`SessionManager`、`ProjectManager`、`ToolRegistry`、`MemoryProvider`、`PinnedStore`、`ActiveRetrievalManager`、`HarnessState`（可选）、`LearningModule`（可选）、`MemoryTicker`、`TrajectoryManager`、`ContextBudget`、`RollingSummarizer`、`UtilityLlm`、`LearnedTraitsStore`。

**关键设计**：
- TurnService 不 import `crate::commands::*`（分层墙）。
- 事件发射统一走 [`runtime_event::dispatch()`](src-tauri/src/modules/runtime/runtime_event.rs)。
- 三处持久化钩子：supervisor snapshot（生命周期态）、run event log（durable facts）、session.json（legacy 兼容）。

### 4.3 Application Layer — 编排服务

| 模块 | LOC | 决策边界 |
|------|-----|----------|
| [`prompt_coordinator.rs`](src-tauri/src/modules/application/prompt_coordinator.rs) | 649 | user input + system + memory → prompt + assembly decision |
| [`memory_coordinator.rs`](src-tauri/src/modules/application/memory_coordinator.rs) | 401 | 记忆候选准备 → quality gate → injection |
| [`memory_injection_service.rs`](src-tauri/src/modules/application/memory_injection_service.rs) | 353 | system-prompt 记忆块构建（按 context budget） |
| [`request_intelligence_service.rs`](src-tauri/src/modules/application/request_intelligence_service.rs) | 327 | 请求 → `ExecutionModeDecision`（complexity / risk / route） |
| [`provider_service.rs`](src-tauri/src/modules/application/provider_service.rs) | 271 | 解析 provider + model（config + override） |
| [`permission_service.rs`](src-tauri/src/modules/application/permission_service.rs) | 446 | 权限策略 + 提示处理 |
| [`tool_executor.rs`](src-tauri/src/modules/application/tool_executor.rs) | 190 | 注册表驱动的工具调用 wrapper |
| [`memory_candidate_extractor.rs`](src-tauri/src/modules/application/memory_candidate_extractor.rs) | 706 | 从 assistant 输出 + 用户输入抽取候选 |
| `memory_*`（quality_gate / conflict_resolution / 等） | ~1800 | 写路径管线 |

### 4.4 Control Plane

[`src-tauri/src/modules/control_plane/`](src-tauri/src/modules/control_plane/) 是 session runtime 的策略与预检层：

| 模块 | 用途 |
|------|------|
| `session_context.rs` | `SessionContextResolver`：从 session + project + identity 解析执行上下文。 |
| `session_bridge.rs` | AppSession ↔ RuntimeSession 转换。 |
| `ingress_classifier.rs` | 入站请求分类（activation / 权限）。 |
| `prepare_step_execution.rs` | 工具执行前 policy / permission / sandbox 准备。 |
| `boundary_resolver.rs` | 工具/权限边界判定。 |
| `tool_execution_broker.rs` | 工具执行治理入口。 |
| `audit.rs` | 审计事件发射。 |

**设计目标**：commands 保持薄 → 策略与预检全部沉到 control_plane → runtime 不感知策略细节。

### 4.5 Runtime Core（vNext 真值源）

[`src-tauri/src/modules/runtime/`](src-tauri/src/modules/runtime/) 71 文件 ~13K LOC。

#### A. 事件与监督

| 文件 | LOC | 角色 |
|------|-----|------|
| [`runtime_event.rs`](src-tauri/src/modules/runtime/runtime_event.rs) | 68 | **canonical dispatch helper** — `dispatch(handle, event_type, family, correlation, payload, logger)` 同时广播到 Tauri channel `"runtime_event"` + 写 run log。 |
| [`evolution_emitter.rs`](src-tauri/src/modules/runtime/evolution_emitter.rs) | — | 底层 emit 实现。 |
| [`stream_emitter.rs`](src-tauri/src/modules/runtime/stream_emitter.rs) | 548 | typed factories；PR D-1（2026-05-02）退役 `agent-token` Tauri channel。 |
| [`supervisor.rs`](src-tauri/src/modules/runtime/supervisor.rs) | 449 | **MIG-020 生命周期 owner**：`SupervisorSnapshot` + 5 lifecycle hooks (`start_run` / `block_permission` / `unblock_permission` / `run_completed` / `run_failed_*` / `close_session`)；持久化于 `runtime/supervisor/<session>.json`。 |
| [`event_log.rs`](src-tauri/src/modules/runtime/event_log.rs) | 653 | append-only `RunLogEntry` JSONL（best-effort 非阻塞）。 |
| [`history.rs`](src-tauri/src/modules/runtime/history.rs) | 710 | 分页 history + replay；`session_has_run_log_jsonl_files` 决定是否 fallback `session.json`。 |

#### B. Conversation 与 Agent Loop

| 文件 | LOC | 职责 |
|------|-----|------|
| [`conversation.rs`](src-tauri/src/modules/runtime/conversation.rs) | 1471 | 同步 turn executor；agentic loop + LLM call + tool execution。 |
| [`run_delegate.rs`](src-tauri/src/modules/runtime/run_delegate.rs) | 644 | 流式 adapter：`ApiClient::stream()` async callback → `conversation.rs` 同步消费者。 |
| [`agent_loop/`](src-tauri/src/modules/runtime/agent_loop/) | — | agentic loop 配置 + 迭代 tracker。 |
| [`attempt_ledger.rs`](src-tauri/src/modules/runtime/attempt_ledger.rs) | 579 | **MIG-022 8-state ledger**：每次工具尝试 / 失败 / 重试 / 批准的 canonical fact。 |
| [`session.rs`](src-tauri/src/modules/runtime/session.rs) | 580 | `ContentBlock` / `MessageRole` / `ConversationMessage` 序列化。 |
| [`pending_permission.rs`](src-tauri/src/modules/runtime/pending_permission.rs) | 143 | **MIG-019 权限持久化**：跨 reload 恢复。 |

#### C. 上下文与预算

| 文件 | LOC | 用途 |
|------|-----|------|
| [`budget.rs`](src-tauri/src/modules/runtime/budget.rs) | 864 | token 预算分配（system / history / memory / output reserve）。 |
| [`compact.rs`](src-tauri/src/modules/runtime/compact.rs) | 965 | session 压缩（`should_compact()` / `compact_session()`）。 |
| [`context_compression/`](src-tauri/src/modules/runtime/context_compression/) | — | 长上下文消息消化（`digester.rs` / `mini_index.rs`）。 |

#### D. Contracts（wire schemas）

[`src-tauri/src/modules/runtime/contracts/`](src-tauri/src/modules/runtime/contracts/) — 与 `src/transport/contracts.ts` 镜像：

| 文件 | 拥有的类型 |
|------|------------|
| `common.rs` | `RuntimeEventEnvelope`、`CorrelationIds`、`RuntimeEventType` |
| `agent_loop.rs` | `SkillResolutionPlan`、`WorkLoopDecision`、`FinalRunReport` |
| `execution_mode.rs` | `ExecutionModeDecision`、`ComplexityLevel`、`RiskLevel` |
| `memory.rs` | `MemoryWriteDecision`、`MemoryItemProjection`、`MemoryScope` |
| `prompt.rs` | `PromptDiagnosticsSummary`、`PromptDiagnosticsActivatedEntry` |
| `activation.rs` | `ActivationSnapshot`、`ActivationStatus` |

#### E. 配置与基础设施

| 文件 | 用途 |
|------|------|
| [`config/`](src-tauri/src/modules/runtime/config/) | runtime 配置 loader（MCP、memory、schema） |
| `mcp*.rs` | MCP server lifecycle + stdio RPC + health checks |
| `permissions.rs` | 权限模式枚举 (plan / acceptEdits / bypassPermissions / dangerFullAccess) |
| `oauth.rs` | OAuth token 管理 + 刷新 |
| `prompt/`、`prompt_tools_guide.rs` | prompt 装配 + skills 索引 |
| `hooks.rs`、`lifecycle_hooks.rs` | 周期钩子注册 |
| `bash.rs`、`file_ops.rs` | 系统 IO 抽象 |
| `snapshot.rs`、`working_checkpoint.rs`、`resume_cursor.rs` | 恢复/回放 |
| `self_repair.rs`、`cost_guard` | 韧性 |
| `usage.rs` | token / cost 计量 |

### 4.6 Domain Modules

| 模块 | 关键能力 |
|------|----------|
| [`memory/`](src-tauri/src/modules/memory/) | 分层：providers (SQLite / Vector / Hybrid)、`pinned/`、`summary/`、`retrieval/`、`quality/` (Phase 8A)、`security/`、`cognitive/`、`learning_traits.rs`、`promotion.rs`、`job_runner.rs`（信号量限流）、`ticker.rs`、`inject.rs`、`compiler.rs`、`llm.rs`（`UtilityLlm` trait）、`embedding.rs`。 |
| [`identity/`](src-tauri/src/modules/identity/) | Soul / Persona / ResolvedIdentity；session-scoped 身份包。 |
| [`session/`](src-tauri/src/modules/session/) | 产品 session 元数据；session.json 兼容；undo。 |
| [`projects/`](src-tauri/src/modules/projects/) | workspace / project 边界。 |
| [`tools/`](src-tauri/src/modules/tools/) | ~50 模块；tool registry + builtin (bash / file / http / memory / search / cron / …)。 |
| [`provider/`](src-tauri/src/modules/provider/) | 11 模块；known models + capability + resilience decorator。 |
| [`learning/`](src-tauri/src/modules/learning/) | `trajectory.rs`（ShareGPT JSONL）、`reflection.rs`、`self_model.rs`、`trust_tracker.rs`、`promotion_gate.rs`、`failure_taxonomy.rs`、`strategy_registry.rs`。 |
| [`harness/`](src-tauri/src/modules/harness/) | **独立 EventBus**（`tokio::broadcast` cap 256）；订阅者 `TelemetryCollector` / `SessionRecorder` / `TraceAggregator`；`HarnessRunReport` + `report_persistence`。**MIG-023 partial**：仍是与 canonical run log 并行的治理侧真相。 |
| [`smart_browser/`](src-tauri/src/modules/smart_browser/) + [`browser/`](src-tauri/src/modules/browser/) | 统一本地 CDP（chromiumoxide）+ browser-use MCP + 未来云端。 |
| [`security/`](src-tauri/src/modules/security/) | redaction / safety policy / ThreatScanner / secret detection。 |
| [`scheduler/`](src-tauri/src/modules/scheduler/)、[`skills/`](src-tauri/src/modules/skills/)、[`desktop_host/`](src-tauri/src/modules/desktop_host/)、[`observability/`](src-tauri/src/modules/observability/)、[`onboarding/`](src-tauri/src/modules/onboarding/)、[`jiaochang_audio/`](src-tauri/src/modules/jiaochang_audio/)、[`updater/`](src-tauri/src/modules/updater/) | 专项子系统。 |

---

## 5. 前端架构（Frontend Detail）

### 5.1 Shell 与路由

| 文件 | LOC | 职责 |
|------|-----|------|
| [`src/main.tsx`](src/main.tsx) | 82 | 窗口路由（settings / browser-viewer / main）；Tauri runtime 检测；mount `<App />` + providers。 |
| [`src/App.tsx`](src/App.tsx) | **3310** | 全局 boot、session、stream、permission、memory overlay 编排中心；wire `runtimeProjectionStore`；用 `@/api/*` facade（不再 raw invoke）。**god-component 警示**。 |
| [`src/modules/app-shell/AppShell.tsx`](src/modules/app-shell/AppShell.tsx) | 116 | BootShell + MainShell + ContentRouter 组合；纯渲染，零业务。 |
| [`src/modules/app-shell/ContentRouter.tsx`](src/modules/app-shell/ContentRouter.tsx) | 53 | `AppSection` ('chat' \| 'skills' \| 'automation' \| 'memory' \| 'jiaochang') → 模块组件。**未为 `teams` 预留**（见 §10）。 |

### 5.2 API Facade（`src/api/*`，~2.3K LOC）

| 文件 | LOC | 角色 |
|------|-----|------|
| [`client.ts`](src/api/client.ts) | 67 | 薄 transport seam（`call` / `subscribe`）；可注入测试。 |
| [`conversations.ts`](src/api/conversations.ts) | 126 | **`startChatTurn()`** — 返回 `ChatStreamHandle`，封装 streamId / subscribe / stop / respondPermission。 |
| [`streaming.ts`](src/api/streaming.ts) | 150 | `listenToStream()` — 订阅 `runtime_event` 并按 `correlation.streamId` 过滤（PR D-1）。 |
| [`sessions.ts`](src/api/sessions.ts) | 250 | session CRUD、undo/redo、history、identity 更新。 |
| [`memory.ts`](src/api/memory.ts) | 647 | 记忆 facade + `useMemoryEntries` / `useMemoryPromotionCandidates` hooks + invalidation 订阅。 |
| `models.ts` / `projects.ts` / `identity.ts` / `updater.ts` / `onboarding.ts` / `window.ts` / `slash.ts` | ~390 | 各域薄 facade。 |

**强约束**：所有 UI 必须通过 `src/api/*`，禁止 raw `invoke`（除少量遗留点；见 IMPROVEMENTS）。

### 5.3 Transport Layer（`src/transport/*`，~1.5K LOC）

| 文件 | LOC | 角色 |
|------|-----|------|
| [`contracts.ts`](src/transport/contracts.ts) | 935 | **canonical wire schema** — `RuntimeEventEnvelope<T>`、`CorrelationIds`、`RuntimeEventType`、`StreamTokenPayload`、`PermissionRequestPayload`、`ActivationStatusKind`、`ExecutionMode` 等。 |
| `runtime-event-translator.ts` | 95 | wire payload → `CanonicalRuntimeEvent`。 |
| `runtime-event-reducer.ts` | 125 | reducer。 |
| `runtime-event-payloads.ts` | 123 | 后端 payload 形状定义。 |
| `gateway.ts` | 113 | future sidecar / local gateway seam（stub）。 |

### 5.4 Runtime Projection（`src/runtime-projection/*`，~4.8K LOC）

**这是前端的 chat 真值源**。整体管线：

```text
Backend runtime_event (envelope)
   │
   ▼
[runtime-projection-bridge.ts]      ← 单 channel listen('runtime_event') + family router
   │
   ▼
[envelope-router.ts]                ← 按 event_type 分发到 family handlers
   │
   ▼
[runtime-event-translator.ts]       ← wire payload → CanonicalRuntimeEvent
   │
   ▼
[runtime-event-queue.ts]            ← micro-task batching (默认 microtask；test 用 sync)
   │
   ▼
[runtime-event-reducer.ts]          ← 同步不可变 reducer
   │
   ▼
RuntimeProjectionSnapshot           ← runs / approvals / memory / activation / executionMode
   │
   ▼
[useRuntimeProjectionSelector]      ← React hook (useSyncExternalStore)
   │
   ▼
[chat-run-projection.ts]            ← runs[] → Conversation.messages[]
   │
   ▼
React components (Chat UI / Memory / Settings / …)
```

| 文件 | LOC | 角色 |
|------|-----|------|
| [`runtime-projection-bridge.ts`](src/runtime-projection/runtime-projection-bridge.ts) | 390 | 单一 wiring 入口；活化/执行模式 fetch seams。 |
| [`runtime-projection-store.ts`](src/runtime-projection/runtime-projection-store.ts) | 143 | 进程级 snapshot holder；`useSyncExternalStore` 兼容。 |
| [`runtime-event-translator.ts`](src/runtime-projection/runtime-event-translator.ts) | 437 | 8+ translator（AgentToken / Permission / Memory / Activation / ExecutionMode / ToolAttempt / Supervisor / BrowserStatus）。 |
| [`runtime-event-reducer.ts`](src/runtime-projection/runtime-event-reducer.ts) | 531 | 同步不可变 reducer。 |
| [`chat-run-projection.ts`](src/runtime-projection/chat-run-projection.ts) | 306 | runs → messages 派生。 |
| [`types.ts`](src/runtime-projection/types.ts) | 828 | canonical event alphabet + snapshot schema。 |
| [`history-replay.ts`](src/runtime-projection/history-replay.ts) | 245 | 从 history page 恢复 snapshot（T-020）。 |
| [`envelope-router.ts`](src/runtime-projection/envelope-router.ts) | 33 | family-based dispatch。 |
| [`runtime-event-queue.ts`](src/runtime-projection/runtime-event-queue.ts) | 123 | micro-task batching。 |
| [`use-runtime-projection.ts`](src/runtime-projection/use-runtime-projection.ts) | 50 | React hook。 |
| [`pending-permission-recovery.ts`](src/runtime-projection/pending-permission-recovery.ts) | 26 | 权限轮询恢复（T-012）。 |
| [`use-execution-mode-preview.ts`](src/runtime-projection/use-execution-mode-preview.ts) | 86 | 执行模式预览。 |
| [`browser-events.ts`](src/runtime-projection/browser-events.ts) | 62 | BrowserStatusEvent translator。 |

### 5.5 状态层

四类 store，边界清晰：

| Store | 路径 | 职责 |
|-------|------|------|
| `bootstrapStore` | [`src/state/bootstrap-store.ts`](src/state/bootstrap-store.ts) (158) | boot phase + project list（**boot 真相**）。 |
| `sessionStore` | [`src/stores/session-store.ts`](src/stores/session-store.ts) (119) | active session cursor only。 |
| `conversation-slice` | [`src/stores/conversation-slice.ts`](src/stores/conversation-slice.ts) (307) | 每会话 UI 状态（loading / todos / title / abort handles）；transcript 写入路径已 `@deprecated`。 |
| `runtimeProjectionStore` | [`src/runtime-projection/runtime-projection-store.ts`](src/runtime-projection/runtime-projection-store.ts) (143) | **transcript canonical truth**（messages / runs / tools / thinking）。 |
| `browser-slice` | [`src/stores/browser-slice.ts`](src/stores/browser-slice.ts) (133) | 浏览器运行态。 |

### 5.6 Chat UI

| 文件 | LOC | 状况 |
|------|-----|------|
| [`src/modules/chat/components/ChatWorkspace.tsx`](src/modules/chat/components/ChatWorkspace.tsx) | 576 | 主 chat surface；侧栏 + header + ChatUI；通过 `@/api/*` facade，零 raw IPC。 |
| [`src/modules/chat/components/HomeScreen.tsx`](src/modules/chat/components/HomeScreen.tsx) | 513 | 空态 / project picker。 |
| [`src/modules/chat/components/SidebarTop.tsx`](src/modules/chat/components/SidebarTop.tsx) | 502 | 标题编辑 + icon picker。 |
| [`src/components/ui/chat-ui.tsx`](src/components/ui/chat-ui.tsx) | **5043** | **god-component**：消息渲染 + 输入 + slash 补全 + 各种 picker + 工具调用 + artifacts + thinking + 文件浏览器 + 浏览器卡 + …。**仍持有 raw `invoke('get_models')` + `listen('if2ai://models-changed')`**（违反 P5）。 |
| [`src/lib/tauri.ts`](src/lib/tauri.ts) | **3414** | 遗留 DTO + transport bridge；新代码禁止从此 import DTO。 |

---

## 6. 核心数据流（Sequence Diagrams）

### 6.1 Chat Turn — 完整流程

```text
User input
  │
  ▼
[Frontend] src/api/conversations.ts::startChatTurn(text, opts)
  │   ├─ generates streamId
  │   └─ returns ChatStreamHandle { streamId, subscribe, stop, respondPermission }
  ▼
Tauri invoke: start_agent_stream / run_agent_turn
  │
  ▼
[Backend] commands/agent/{stream,run}.rs        (thin adapter)
  │
  ▼
TurnService::stream_turn() / run_turn()
  │   ├─ SessionContextResolver.resolve()       → execution context
  │   ├─ PermissionService.prepare()            → PermissionPolicy
  │   ├─ ProviderService.resolve()              → ProviderHandle
  │   ├─ MemoryCoordinator.prepare()            → memory candidates
  │   ├─ PromptCoordinator.assemble()           → system + user prompt
  │   ├─ supervisor.start_run(session, run_id)  → SupervisorSnapshot
  │   └─ RunEventLogger.open(run_id)            → JSONL writer
  ▼
stream_task::run_loop() {
   loop {
     stream_iteration::run_one_iteration() {
       prepare_step_execution()                 → policy snapshot
       provider.stream(messages)                → SSE tokens
       runtime_event::dispatch(Conversation, …) ──┐
                                                  │ ┌─▶ Tauri emit("runtime_event", envelope)
                                                  │ └─▶ event_log.append_with_correlation()
       if tool_use:
         stream_tool_execution::execute_batch() {
           attempt_ledger.record(Pending)
           PermissionService.maybe_prompt() {
             pending_permission.persist(req)
             runtime_event::dispatch(Permission, prompt_opened, …)
             // wait for respond_permission IPC
           }
           tool_executor.invoke(tool, args)
           attempt_ledger.record(Succeeded | Failed | Retried)
           runtime_event::dispatch(Tool, attempt_resolved, …)
         }
     }
   }
}
  │
  ▼
stream_finalize::finalize() {
  flush_tokens()
  build_final_message()
  runtime_event::dispatch(Conversation, run_completed, …)
  trajectory.record(ShareGPT)                   (LearningModule)
  memory_after_turn → runtime_event(Memory, after_turn, …)
  supervisor.run_completed(session, run_id)
  RunEventLogger.close()
  session.persist(snapshot)                     (legacy session.json fallback)
}
  │
  │  ── via Tauri channel "runtime_event" ──
  ▼
[Frontend] runtime-projection-bridge.ts
  └─ envelope-router.ts → translator → reducer → RuntimeProjectionSnapshot
       │
       ▼
useRuntimeProjectionSelector(s => projectConversationMessagesFromRuns(s.runs))
       │
       ▼
ChatUI re-renders with new messages / tool attempts / thinking blocks
```

### 6.2 History Replay — Reload 场景

```text
UI mount / session switch
  │
  ▼
src/api/sessions.ts::getSessionHistoryPage(sessionId, cursor?)
  │
  ▼
commands/session.rs::get_session_history_page()
  │
  ▼
runtime/history.rs::page() {
  if session_has_run_log_jsonl_files(base, session_id) {
    read runtime/run-log/<session>/*.jsonl
    return SessionHistoryPageResponse { entries, next_cursor }
  } else {
    // legacy fallback — session.json snapshot
    return SessionHistoryReplay { messages_from_session_json }
  }
}
  │
  ▼
[Frontend] history-replay.ts::replayRunLogEntriesToMessages()
  └─ feeds into runtimeProjectionStore (initial snapshot)
       │
       ▼
ChatUI renders historical transcript
```

**关键**：一旦该 session 目录存在任意 `.jsonl`，就**不再用** `session.json` 覆盖首屏（[`history.rs`](src-tauri/src/modules/runtime/history.rs) `session_has_run_log_jsonl_files`）。

### 6.3 Permission Recovery — 跨 Reload

```text
Tool execution in turn
  │
  ▼
PermissionService.maybe_prompt(tool_invocation)
  │   └─ generates request_id + payload
  ▼
pending_permission.persist({ session_id, run_id, request, ts })
  │
  ▼
runtime_event::dispatch(Permission, prompt_opened, payload, logger)
  │   ├─▶ Tauri emit("runtime_event", envelope)
  │   └─▶ event_log.append("permission_requested", …)
  │
  │  ── If user reloads here ──
  ▼
[After reload] AppBoot
  │
  ▼
src/runtime-projection/pending-permission-recovery.ts::refreshPendingPermission()
  │
  ▼
commands/permission.rs::list_pending_permissions(session_id)
  │
  ▼
pending_permission.read_all(session_id) → Vec<PendingPermission>
  │
  ▼
runtimeProjectionStore.dispatch(PermissionPromptOpened) for each
  │
  ▼
Permission UI renders prompts
  │
  │  ── User responds ──
  ▼
src/api/conversations.ts::respondPermission(streamId, requestId, decision)
  │
  ▼
commands/permission.rs::respond_permission()
  │
  ▼
permission_service.resolve(request_id, decision)
  │   ├─ pending_permission.delete(request_id)
  │   ├─ runtime_event::dispatch(Permission, prompt_resolved, …)
  │   └─ unblocks turn waiting on permission
  ▼
turn loop continues
```

### 6.4 Tool Attempt Ledger（MIG-022）

```text
attempt_ledger 8 状态机：

   Pending ──▶ Started ──▶ Succeeded
      │           │
      │           ├──▶ Failed ──▶ Retried ──▶ (回到 Started)
      │           │       │
      │           │       └──▶ AbandonedExhausted
      │           │
      │           └──▶ AwaitingApproval ──▶ Approved ──▶ Started
      │                       │
      │                       └──▶ Denied ──▶ AbandonedDenied
      │
      └──▶ Cancelled

每次状态转移：
  - 写 attempt_ledger（SQLite 持久化）
  - 发 runtime_event(Tool, attempt_resolved, payload)
  - 累计 turn 级 cost / time / retry budget
```

### 6.5 Memory Lifecycle

```text
Turn end
  │
  ▼
MemoryCandidateExtractor.extract(user_input, assistant_output, tool_results)
  │   └─ MemoryCandidate { content, scope, evidence, confidence }
  ▼
MemoryQualityGate.evaluate(candidate)
  │   ├─ ThreatScanner.scan()              (security)
  │   ├─ ConflictResolution.check()        (vs existing)
  │   └─ → MemoryWriteDecision { Accept | Reject | DeferReview }
  ▼
if Accept:
  PromotionEngine.promote(candidate, scope ∈ {session, project, team, global})
  MemoryProvider.write()                   (SQLite + LanceDB hybrid)
  │
  ▼
runtime_event::dispatch(Memory, after_turn, payload, logger)
  │
  ▼
[Frontend] memory family handler in projection bridge
  │
  ▼
runtimeProjectionStore.memory.recent.push(event)
useMemoryEntries() invalidates → MemoryBrowser re-fetches via @/api/memory
```

---

## 7. Facts 与事实源

| 事实 | 当前来源 | 目标来源 | 状态 |
|------|----------|----------|------|
| Session list / title / project binding | session manager + `session.json` metadata | `SessionMeta` / session registry | ⚠️ session.json 仍混合多职责 |
| Transcript / run events | run event log（主） + session.json（fallback） | run event log + projection replay | ✅ MIG-018 完成；fallback gated |
| Active run status | `runtime_event` envelope → projection | runtime projection from event log/envelope | ✅ MIG-017 完成（reducer 真相） |
| Permission pending | `pending_permission` record + event log + projection | 同左 | ✅ MIG-019 完成 |
| Tool attempts | `attempt_ledger` + event log | 同左 | ✅ MIG-022 完成 |
| Memory evidence | message memoryContext + memory events + `@/api/memory` | memory domain projection + evidence API | ⚠️ MemoryBrowser 已切；其他 surface 仍混 |
| Harness report | harness EventBus + `HarnessRunReport` + `report_persistence` | canonical run report derived from event log | ⚠️ MIG-023 partial（独立真相） |
| Activation / license | runtime fetch seams + projection trace | activation domain projection | ⚠️ projection 已读，policy contract 模糊 |

**Staff 原则**：**写事实只进 canonical log；读事实只读 projection；兼容存储只能作为迁移 fallback。**

### 7.1 Run-log Envelope 收敛细节

- [`RunEventLogger::append_with_correlation`](src-tauri/src/modules/runtime/event_log.rs) 把 `StreamTokenPayload.correlation`（含 Teams 预留字段 `team_id` / `member_id` / `delegation_id`）合并到 `RunLogEntry` 顶层列。
- 非流式 turn（`run.rs`）对同一 `run_id` 使用固定 `CorrelationIds`，与流式路径对齐。
- Tauri `emit("runtime_event", envelope)` 携带完整 `RuntimeEventEnvelope`（前端见 [`src/transport/contracts.ts`](src/transport/contracts.ts)）；当调用方传入 `RunEventLogger` 时，使用 `append_sync_from_envelope` 使 durable 行与广播 envelope 同源。

### 7.2 通道演进时间线

| 日期 | PR | 内容 |
|------|-----|------|
| 2026-04-30 | — | `feat(runtime)` 统一 run-log envelope；扩展 correlation for Agents Teams（commit `595c48f`）|
| 2026-04-30 | — | `feat(runtime)` history fallback 收窄到 event-log 缺席（MIG-018，commit `e460d73`）|
| 2026-05-01 | C-1 | `agent-token` channel 收敛到 `runtime_event`（commit `c83dd4e`）|
| 2026-05-01 | C-2 | `permission-request` 收敛到 `runtime_event`（commit `697ffac`）|
| 2026-05-01 | C-3 | `memory_event` / `memory_after_turn` 收敛到 `runtime_event`（commits `ba17bb4`, `8ec5c08`, `f68caed`）|
| 2026-05-02 | D-1 | `agent-token` channel 正式退役；`listenToStream` 改读 `runtime_event` 按 `streamId` 过滤（commit `1256e2e`）|
| 2026-05-02 | D-2 | App.tsx 删除 699 行死代码（commit `5002d21`）|
| 2026-05-05 | FIX-14/17 | 退役通道常量 + dead listeners 清理；chat-store 不再 re-export deprecated transcript actions |

---

## 8. vNext 架构目标与进展

vNext 目标态：

```text
Session
  = durable event log               ← 写入唯一事实源
  + runtime supervisor              ← 显式生命周期 owner
  + frontend projection             ← UI 唯一读模型
```

### 8.1 收敛进度（MIG-* 编号为本文档命名目标，非 Pack）

| ID | 主题 | 状态 | 证据 |
|----|------|------|------|
| MIG-016 | run event log 成为 canonical fact source | ✅ done | `event_log.rs`、`history.rs` |
| MIG-017 | chat runtime UI 切到 projection truth | ⚠️ 95% | 真值已切换；`conversation-slice` 仍承载 UI 状态（合理）；`chat-ui.tsx` 仍 prop-drilled |
| MIG-018 | session history 通过 event log replay + paging | ✅ done | `session_has_run_log_jsonl_files()` + 测试 `history_prefers_event_log_over_session_json()` |
| MIG-019 | permission recovery cross-reload | ✅ done | `pending_permission.rs` + `pending-permission-recovery.ts` |
| MIG-020 | runtime supervisor 显式 owning lifecycle | ⚠️ partial | snapshot owner 已建；TurnService / stream_task / permission / 前端 store 仍各自管理子步骤 |
| MIG-021 | resume / recovery contract | ✅ done | `ResumeReason` / `ResumeRecoverability` |
| MIG-022 | tool attempt ledger | ✅ done | `attempt_ledger.rs` 8-state + Tauri command + TS types |
| MIG-023 | run report / harness report 从 canonical log 派生 | ❌ partial | harness 仍独立 EventBus（治理侧旁路真相）|

### 8.2 架构守则（已编入 [`CLAUDE.md`](CLAUDE.md) Hard Rules）

- 不再新增直接消费 raw Tauri event 的 UI surface。
- 不再把 transcript 作为 `session.json` 的长期事实源。
- 不再让 harness、projection、session manager 各自生成独立 run truth。
- 所有新 runtime event 必须能关联 `session_id`、`run_id`、必要时 `turn_id`、`stream_id`、`team_id`、`member_id`、`delegation_id`。
- 新能力与 vNext 收敛应遵循 [`docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`](docs/design-docs/if2ai-vnext-session-runtime-blueprint.md)。

---

## 9. Gap / 二次真相 / 改善点

完整改善清单见 **[`docs/IMPROVEMENTS-2026-05-05.md`](docs/IMPROVEMENTS-2026-05-05.md)**。本节仅列出 P0/P1 摘要。

### P0 阻塞性
- **`work_loop.rs` 3,226 行 god-file** — 必须拆分（skill resolution / routing / decision）。
- **`chat-ui.tsx` 5,043 行 god-component** — 阻碍 projection-first 的最后一公里；仍含 raw `invoke('get_models')` 违反 P5。
- **`stream_finalize.rs` 1,503 行** — 后处理过载（guardrail / outcome / learning / persistence 收尾混在一起）。
- **`setActiveModel` 数据丢失（F1F-5）** — 后端 `model_set_active` 不接收 `auth_variant`，多认证 variant provider（如 Moonshot-CN）保存时静默丢字段。

### P1 重要
- **MIG-023 Harness 二次真相** — `HarnessRunReport` 仍由独立 EventBus 拼装；需从 canonical event log 派生。
- **`session.json` 多职责** — 仍混合 metadata / transcript / identity / skills / memory toggle / project binding；与 `SessionMeta` 目标态冲突。
- **`commands/mod.rs` 过大 AppState** — 命令边界不够薄。
- **`lib/tauri.ts` 3,414 行遗留 DTO** — 新 UI 容易绕过 `src/api/*` facade。
- **App.tsx 3,310 行编排中心** — MIG-014/15 未完成。
- **`conversation.rs` + `run_delegate.rs` 紧耦合** — 边界含糊（runtime vs streaming adapter）。
- **`chat-ui.tsx` prop-drilled messages** — 应改为 `useRuntimeProjectionSelector(s => projectConversationMessagesFromRuns(s.runs))`。

### P2 优化
- 4 处遗留 raw `invoke('model_list_available')`（chat-ui:302、HomeScreen:75、ProvidersSettingsPage:285、ModelSettingsPage:358）应迁到 `src/api/models.ts`。
- Memory UI 多读面（`memoryContext` 内嵌 + projection events + 直 IPC pages）需收敛到「仅 projection + 单一 evidence API」。
- Projection checkpoint / seq 增量 replay 还没成为 session 打开主路径。
- Provider resilience / cost guard / self repair 模块骨架已有，与 run event log / projection contract 还需统一可观测闭环。
- Activation / license / execution mode 在 projection 已有读模型痕迹，产品级 runtime policy contract 仍模糊。

---

## 10. Agents Teams 设计建议（前置条件 §11.3）

### 10.1 产品定义

> Team 是一个持久化的多 Agent 协作图。绑定 project/session/runtime facts，由 team supervisor 调度多个 agent member，以 planner / executor / reviewer / researcher 等 role 协作完成一个或多个 run。

核心能力：
- 多 agent member 与 role 编排。
- planner / executor / reviewer / critic / researcher 等角色分工。
- 串行、并行、handoff、review gate、quorum 等协作模式。
- 共享 team memory 与成员私有 memory 的边界。
- team-level tool permission、budget、provider、sandbox policy。
- 全量事件可回放，前端能展示每个 agent 的动作、失败、重试、审核与交接。

### 10.2 后端新增 bounded context

新增 `src-tauri/src/modules/team/`，**不要塞进** `session` / `project` / `identity`。

| Model | 说明 |
|-------|------|
| `TeamMeta` | team id、name、description、project binding、created/updated |
| `TeamMember` | member id、identity/persona binding、role id、capability tags |
| `AgentRole` | planner / executor / reviewer / researcher / custom 定义 |
| `TeamPolicy` | budget、tool allow/deny、approval mode、provider preference、parallelism |
| `TeamRun` | 一次 team-level run，拥有多个 member run / delegation |
| `Delegation` | parent run → member assignment 任务边 |
| `TeamArtifact` | shared outputs、review notes、handoff summary |
| `TeamProjection` | 前端展示所需的 read model |

服务边界：
- `TeamService` — CRUD、member 管理、project binding。
- `TeamSupervisor` — team run 生命周期、delegation graph、pause/resume/cancel、quorum/review gate。
- `TeamPolicyEngine` — team/member 级 tool permission、budget、provider、memory scope。
- `TeamProjectionService` — 从 runtime event log 派生 team projection。

### 10.3 Runtime Contract 扩展

```rust
pub struct CorrelationIds {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub run_id: Option<String>,
    pub stream_id: Option<String>,
    pub turn_index: Option<u32>,
    pub attempt_id: Option<String>,
    pub team_id: Option<String>,        // 已预留
    pub member_id: Option<String>,      // 已预留
    pub role_id: Option<String>,        // 已预留
    pub parent_run_id: Option<String>,  // 已预留
    pub delegation_id: Option<String>,  // 已预留
}
```

Team 事件**不绕过** run event log。每个 member 的 tool call、permission request、provider retry、failure attempt、review result 都进入 canonical log，并通过 team projection 呈现。

### 10.4 Frontend 设计

- `src/api/teams.ts` — `startTeamRun()` / `stopTeamRun()` / `respondTeamPermission()` / `getTeamProjection()` / `getTeamHistoryPage()`。
- `src/transport/team-contracts.ts` — Team DTO / event contract。
- `src/stores/team-store.ts` — 仅 `activeTeamId` / 选中 member / UI preference；不保存 runtime truth。
- `src/runtime-projection/team-*` — translator / reducer / selectors。
- `src/modules/teams/TeamWorkspace.tsx` — 一级工作区。

UI 结构：
- `TeamRail` — team 列表、active team、run status。
- `AgentRoster` — 成员、角色、在线 / 忙碌 / 等待 permission。
- `DelegationGraph` — 任务分解与 handoff 图。
- `SharedTimeline` — 按 event log replay 的团队时间线。
- `ReviewInbox` — review gate / approval / 失败重试建议。
- `ArtifactPanel` — team 输出物 / handoff summary / run report。

### 10.5 Memory 与 Permission

```text
MemoryScope = session | project | team | global
```

- member 私有 scratchpad 不自动进入 team memory。
- team memory 需要 policy gate 或 explicit promote。
- reviewer 的 review result 默认进入 team event log，但是否进入 long-term memory 由 `TeamPolicy` 控制。
- tool permission 同时支持 team-level policy 与 member-level escalation。

### 10.6 Teams 实施切片（命名参考）

| ID | 内容 |
|----|------|
| `TEAM-001` | Team domain contracts + persistence skeleton |
| `TEAM-002` | Team API facade + projection contract + frontend store skeleton |
| `TEAM-003` | TeamSupervisor MVP（planner → executor → reviewer 串行图）|
| `TEAM-004` | team-aware runtime correlation + event log replay |
| `TEAM-005` | TeamWorkspace UI（roster / timeline / delegation graph）|
| `TEAM-006` | team memory scope + team permission policy |
| `TEAM-007` | tool attempt ledger 与 review gate 贯通 |
| `TEAM-008` | harness/team run report 从 canonical event log 派生 |

---

## 11. 优先级建议

### 11.1 立即优先（阻塞 vNext 闭环）
- 解决 F1F-5 `setActiveModel` `auth_variant` 数据丢失。
- 拆分 `work_loop.rs`（→ skill_resolution / routing / decision sibling）。
- 拆分 `chat-ui.tsx`（→ slash-ui / pickers / virtual-list / file-explorer / browser-card 子模块）。
- 完成 MIG-023：让 harness report 从 canonical event log 派生。

### 11.2 中期优先
- 拆分 `stream_finalize.rs`（guardrail / outcome / learning / persistence）。
- 拆分 `stream_task.rs`（model loop / tool loop / event emission / finalization / permission wait）。
- 瘦身 `commands/mod.rs`，让 AppState 聚合与 command adapter 不再承载业务。
- 厘清 `RuntimeEventEnvelope` / `StreamTokenPayload` / `RunLogEntry` contract 单一来源（可由 codegen 保证）。
- 4 处 raw `invoke('model_list_available')` 迁到 `src/api/models.ts`。
- `lib/tauri.ts` DTO 按 feature 切分到 `src/transport/<feature>.ts`。
- `chat-ui.tsx` 改为直接 `useRuntimeProjectionSelector` 而非 prop-drilled messages。

### 11.3 Agents Teams 前置条件
- run event log 可稳定 replay 单 session ✅
- projection store 是 chat runtime UI 的主要读模型 ⚠️（chat-ui prop-drilled 待修）
- permission recovery 可跨 reload 恢复 ✅
- tool attempt ledger 能记录失败 / 重试 ✅
- correlation id contract 已有 `team_id` / `member_id` / `delegation_id` 预留 ✅

---

## 12. 参考路径

- [`CLAUDE.md`](CLAUDE.md) — Superpowers 流程 + Architecture Boundaries 强约束
- [`AGENTS.md`](AGENTS.md) — agent-facing 规则摘要
- [`docs/IMPROVEMENTS-2026-05-05.md`](docs/IMPROVEMENTS-2026-05-05.md) — 完整改善清单
- [`docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`](docs/design-docs/if2ai-vnext-session-runtime-blueprint.md) — vNext 北极星
- [`.qoder/specs/if2ai-agent-evolution-report.md`](.qoder/specs/if2ai-agent-evolution-report.md) — 联合审计基线
- [`docs/superpowers/plans/`](docs/superpowers/plans/) — `writing-plans` 输出
- [`src/App.tsx`](src/App.tsx)、[`src/api/conversations.ts`](src/api/conversations.ts)、[`src/api/sessions.ts`](src/api/sessions.ts)
- [`src/runtime-projection/`](src/runtime-projection/)、[`src/transport/contracts.ts`](src/transport/contracts.ts)
- [`src-tauri/src/modules/application/turn_service/`](src-tauri/src/modules/application/turn_service/)
- [`src-tauri/src/modules/control_plane/`](src-tauri/src/modules/control_plane/)
- [`src-tauri/src/modules/runtime/event_log.rs`](src-tauri/src/modules/runtime/event_log.rs)
- [`src-tauri/src/modules/runtime/history.rs`](src-tauri/src/modules/runtime/history.rs)
- [`src-tauri/src/modules/runtime/supervisor.rs`](src-tauri/src/modules/runtime/supervisor.rs)
- [`src-tauri/src/modules/runtime/runtime_event.rs`](src-tauri/src/modules/runtime/runtime_event.rs)
- [`src-tauri/src/modules/runtime/pending_permission.rs`](src-tauri/src/modules/runtime/pending_permission.rs)
- [`src-tauri/src/modules/runtime/attempt_ledger.rs`](src-tauri/src/modules/runtime/attempt_ledger.rs)
- [`src-tauri/src/modules/runtime/contracts/`](src-tauri/src/modules/runtime/contracts/)
