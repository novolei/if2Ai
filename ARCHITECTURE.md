# If2Ai 架构全景

> 最后更新: 2026-05-01（arch/evolution 文档真值同步，见 `docs/superpowers/plans/2026-05-01-arch-evolution-truth-sync.md`）
>
> 本文档描述当前 codebase 的真实架构、vNext 目标态、仍存在的 Gap / 二次真相，以及 Agents Teams 的建议设计。
>
> **Pack / CHARTER / REGISTRY 已弃用**（2026-05-01）：`docs/packs/**` 仅历史存档。**开发需求**必须按 **Superpowers 完整流程**（见 **§1.0a** 与 [`CLAUDE.md`](CLAUDE.md)）。架构联合审计仍以 **§1.0** 双文档为准。

## 1. Staff 摘要

If2Ai 当前是一个 **React + TypeScript + Tauri 2 + Rust/Tokio** 的桌面 Agent 应用。真实架构已经从早期的“UI 调命令 + 后端即时流式返回”演进到 vNext 的过渡态：

- 前端已经有 `AppShell` / `ContentRouter` 壳层、`src/api/*` action facade、`runtimeProjectionStore` 运行时投影层。
- 后端已经有 `TurnService`、`control_plane`、`runtime/event_log`、`runtime/history`、`runtime/pending_permission` 等 session runtime 基础设施。
- 目标态是 `Session = durable event log + runtime supervisor + frontend projection`。
- 当前最大风险不是缺少能力，而是 **新旧事实源并存**：`session.json`、run event log、raw stream payload、frontend conversation slice、runtime projection、harness report store 仍有重叠职责。

架构推进的核心方向应当是：把所有运行时事实收敛到 append-only run event log，把 UI 全面切到 projection read model，把 supervisor 变成显式生命周期 owner，并在此基础上再引入 Agents Teams。

### 1.0 架构联合审计基准（superpowers 对齐）

**输入（仅此两份）**

1. 本文件（分层、数据流、事实源、Gap、vNext、Teams 建议、参考路径）。  
2. [`.qoder/specs/if2ai-agent-evolution-report.md`](.qoder/specs/if2ai-agent-evolution-report.md)（Part 0 设计哲学、Part 1 差距与 Part 1.3 实现对照、Part 2 模块设计、Part 6 追溯矩阵等）。

**工作流**（Superpowers 习惯）：范围与成功标准 → 按本文 §2～§6 与 evolution 对应章节逐条对照 `codebase` → 列出 Gap / 二次真相 / 未连线项 → 回写双文档之一或附录。`docs/packs/**` 已弃用，**不作为**任何审计或交付的必读取输入。

### 1.0a Superpowers 开发流程（强制）

所有**开发需求**（功能、协议/行为变更、非平凡重构、Bug、性能与安全相关改动）必须遵循根目录 **[`CLAUDE.md`](CLAUDE.md)** 与 **[`AGENTS.md`](AGENTS.md)** 中的 **Superpowers 完整流程**（含 `using-superpowers`、`brainstorming`、`writing-plans`、计划执行、`systematic-debugging`、`verification-before-completion` 等）。Cursor 由 [`.cursor/rules/superpowers-workflow.mdc`](.cursor/rules/superpowers-workflow.mdc) 提示；**不得以**已弃用的 Pack/CHARTER/REGISTRY 代替。

### 1.1 Agent Evolution 与 vNext 的衔接（2026-05）

[`.qoder/specs/if2ai-agent-evolution-report.md`](.qoder/specs/if2ai-agent-evolution-report.md) **Part 1.3 / Part 2 / Part 6** 描述与 Browser-Harness / GenericAgent 对齐的 **Self-Evolution、自愈、Token 效率** 等模块与前后端矩阵；与本文 **§6 事实源、§7 Gap、§8 vNext** 同一叙事：进化能力是 **观测与沉淀侧** 增量，**不替代**「canonical run log + projection-first chat」主收敛。实现细节以 **仓库代码 + 该 spec 已更新段落** 为准；**§1.0 审计**不依赖 GAP 报告或 truth-loop 计划文件。

### 1.2 MIG-017 技术债速记（projection-first）

下列表面仍可能 **并行推断** runtime 态，需在后续迭代中逐项迁到 **仅读 `runtimeProjectionStore` / history replay**（并保留 raw stream 为 transport-only）：

- `App.tsx` 与 `conversation-slice` 中对 token / approval / memory 的旁路更新（与 §3.3 表一致）。
- 仍直接监听 legacy Tauri 事件、未经过 `runtime_event` translator 的 chat UI 分支（新功能禁止再增加）。
- `src/lib/tauri.ts` 厚 DTO 层：新 UI 只走 `src/api/*`（§3.2 守则）。

## 2. 当前系统分层

```text
┌────────────────────────────────────────────────────────────────────┐
│ Frontend: React + TypeScript                                       │
│ AppShell / ContentRouter / ChatWorkspace / Memory / Settings       │
│ stores + runtime-projection + api facades                          │
└──────────────────────────────┬─────────────────────────────────────┘
                               │ Tauri IPC commands + events
┌──────────────────────────────▼─────────────────────────────────────┐
│ Gateway / IPC Boundary                                             │
│ src/api/*, src/lib/tauri.ts, src-tauri/src/commands/*              │
└──────────────────────────────┬─────────────────────────────────────┘
                               │ typed application services
┌──────────────────────────────▼─────────────────────────────────────┐
│ Application Layer                                                  │
│ TurnService, PermissionService, ProviderService, ToolExecutor,      │
│ MemoryCoordinator, PromptCoordinator                               │
└──────────────┬───────────────────────────────┬─────────────────────┘
               │                               │
┌──────────────▼──────────────┐   ┌────────────▼────────────────────┐
│ Control Plane               │   │ Runtime Core                    │
│ session context resolver,   │   │ event_log, history, contracts,  │
│ tool preflight, audit,      │   │ pending_permission, stream      │
│ policy preparation          │   │ emitter, resume/session state   │
└──────────────┬──────────────┘   └────────────┬────────────────────┘
               │                               │
┌──────────────▼───────────────────────────────▼─────────────────────┐
│ Domain Modules                                                     │
│ session, projects, identity, memory, skills, tools, provider,       │
│ browser, learning, harness, desktop_host, scheduler, security       │
└────────────────────────────────────────────────────────────────────┘
```

## 3. 前端真实架构

### 3.1 Shell 与路由

- `src/App.tsx` 仍是全局 boot、session、stream、permission、memory overlay 的编排中心。
- `src/modules/app-shell/AppShell.tsx` 是顶层应用壳，负责全局布局、侧栏、overlay 容器。
- `src/modules/app-shell/ContentRouter.tsx` 根据 `AppSection` 路由到 chat / memory / settings 等工作区。
- `src/modules/chat/components/ChatWorkspace.tsx` 是当前 chat 主要工作区。

当前 `AppSection` 尚未为 `teams` 预留一级入口；如果要做 Agents Teams，应先把 teams 作为一级 workspace，而不是塞进 chat 页面。

### 3.2 API Facade 与 Transport

- `src/api/client.ts` 提供 `call` / `subscribe` 等薄 transport seam。
- `src/api/conversations.ts` 已将一轮 chat 收口成 `startChatTurn()`，返回 `ChatStreamHandle`，统一封装 start / subscribe / stop / permission response。
- `src/api/sessions.ts` 提供 session list、history page 等会话 API。
- `src/api/streaming.ts` 仍是 stream 和 permission 的主要前端 gateway。
- `src/lib/tauri.ts` 仍保留大量 DTO 和直接 IPC 能力，是需要继续瘦身的兼容层。

架构规则：新功能应优先通过 `src/api/*` 暴露，不让页面组件直接依赖 `@/lib/tauri` 的 raw IPC。

### 3.3 前端状态层

当前前端有四类状态，需要明确边界：

| 层                       | 当前职责                                                 | 目标职责                               |
| ------------------------ | -------------------------------------------------------- | -------------------------------------- |
| `bootstrapStore`         | app 初始化、启动错误、ready 状态                         | 保持启动事实，不承载业务 runtime       |
| `sessionStore`           | `activeSessionId` 等当前指针                             | 仅保存选择指针，不保存 transcript      |
| `conversation-slice`     | 当前 chat UI 展示态、session loading、abort handles      | 过渡层，逐步由 runtime projection 派生 |
| `runtimeProjectionStore` | run、approval、memory、activation、execution mode 的投影 | 成为所有 runtime surface 的唯一读模型  |

当前 chat 展示是 `Conversation + RunProjection` 合成，不是纯 projection。`src/runtime-projection/chat-run-projection.ts` 已经在把 run 投影 overlay 到 message 列表上，但 `conversation-slice` 仍然保存大量 UI 展示事实。

### 3.4 Projection Bridge

- `src/runtime-projection/runtime-projection-bridge.ts` 订阅 `agent-token`、`permission-request`、`memory_event`、`memory_after_turn`。
- `src/runtime-projection/runtime-event-translator.ts` 将 raw event 翻译成 projection event。
- `src/runtime-projection/runtime-event-reducer.ts` 更新 `RuntimeProjectionSnapshot`。
- `src/runtime-projection/history-replay.ts` 已经能把后端 history page replay 成前端消息。

当前 bridge 是关键过渡层。MIG-017 / MIG-018 应继续推动 “所有 runtime UI 只读 projection store”，并减少 App/ChatWorkspace 对 raw stream 的直接 side effect。

## 4. 后端真实架构

### 4.1 Bootstrap 与 Command Boundary

- `src-tauri/src/main.rs` 和 `src-tauri/src/bootstrap/mod.rs` 负责 Tauri 启动、插件、state 注册、命令注册。
- `src-tauri/src/commands/mod.rs` 仍是很大的 AppState 聚合点，持有大量 service / manager。
- `src-tauri/src/commands/agent/mod.rs`、`src-tauri/src/commands/session.rs`、`src-tauri/src/commands/command_surface.rs` 等文件是 IPC 适配层。

Command 层的目标应是薄适配，不应该继续承载业务编排。当前 `commands/mod.rs` 的聚合规模仍偏大，是后续可维护性风险。

### 4.2 Application Layer

核心执行路径已经迁移到 `src-tauri/src/modules/application/turn_service/`：

- `mod.rs` 定义 `TurnService` 和共享依赖。
- `run.rs` 处理非流式 turn。
- `stream.rs` 处理流式 turn 的入口和任务派发。
- `stream_task.rs` 承载大部分流式执行循环、工具调用、事件发送。
- `stream_finalize.rs` 处理最终文本、结束事件、持久化收尾。

`TurnService` 是当前实际的 Agent orchestrator。旧文档中提到的 `src-tauri/src/agent/orchestrator.rs` 不再代表当前主路径。

### 4.3 Control Plane

`src-tauri/src/modules/control_plane/` 是 session runtime 的策略与预检层：

- `SessionContextResolver` 负责从 session / project / cwd 等来源解析执行上下文。
- `prepare_step_execution` 负责工具执行前的 policy / permission / audit 准备。
- `ToolExecutionBroker` 是工具执行治理入口。
- `AuditEmitter` 和相关结构为可审计运行提供基础。

Control Plane 的设计方向应是：执行前做策略判断，运行中发审计事实，运行后让 event log 成为可重放证据。

### 4.4 Runtime Core

`src-tauri/src/modules/runtime/` 正在成为 vNext session runtime 的核心：

- `event_log.rs`：append-only run log，按 session/run 写 JSONL，是未来事实源。
- `history.rs`：从 run log 读取并分页 replay session history。
- `pending_permission.rs`：把 pending permission 持久化，支持恢复。
- `contracts/*`：运行时事件和 correlation contract。
- `stream_emitter` / `conversation` / `session` / `resume_cursor`：仍承担兼容和运行态辅助职责。
- `cost_guard` / `lifecycle_hooks` / `self_repair`：为预算、生命周期、恢复能力提供新骨架。

当前 runtime 还不是完整 supervisor。生命周期 owner 仍散落在 `TurnService`、`stream_task.rs`、session manager、pending permission、resume cursor、harness 等多个模块中。

### 4.5 Domain Modules

主要 domain module：

- `src-tauri/src/modules/session/`：产品 session 元数据、session.json 兼容存储、undo 等。
- `src-tauri/src/modules/projects/`：workspace/project 边界。
- `src-tauri/src/modules/identity/`：Soul / persona / identity settings。
- `src-tauri/src/modules/memory/`：SQLite/vector provider、compiler、ticker、learned traits、memory events。
- `src-tauri/src/modules/tools/`：builtin tools、MCP/tool registry、tool execution primitives。
- `src-tauri/src/modules/provider/`：LLM provider、resilience、provider routing。
- `src-tauri/src/modules/harness/`：suite/report/trace sidecar。
- `src-tauri/src/modules/security/`：redaction / safety policy 骨架。

这些模块应继续按 bounded context 演进。新能力不要再塞回 `commands/mod.rs` 或 `stream_task.rs`。

## 5. 当前核心数据流

### 5.1 Chat Turn

```text
User input
  -> src/api/conversations.ts::startChatTurn()
  -> Tauri command: start_agent_stream / run_agent_turn
  -> TurnService::stream_turn() or TurnService::run_turn()
  -> SessionContextResolver + prompt/memory/provider/tool preparation
  -> run_id created + RunEventLogger opened
  -> stream_task executes model/tool loop
  -> Tauri events emitted to frontend
  -> runtime-projection bridge translates events
  -> runtimeProjectionStore updates run/approval/memory projection
  -> Chat UI renders Conversation + RunProjection overlay
  -> stream_finalize writes final runtime facts and compatibility session state
```

### 5.2 History Replay

```text
UI requests session history page
  -> src/api/sessions.ts::getSessionHistoryPage()
  -> commands/session.rs::get_session_history_page()
  -> runtime/history.rs reads runtime/run-log/<session>/<run>.jsonl
  -> returns SessionHistoryPageResponse / SessionHistoryReplay
  -> frontend history-replay projects events back into messages
```

If event log is empty, backend may still fall back to legacy `session.json` for compatibility. This fallback must remain explicitly temporary.

### 5.3 Permission Recovery

```text
Tool needs approval
  -> PermissionService creates pending request
  -> runtime/pending_permission.rs persists request
  -> run event log records permission requested
  -> Tauri event reaches frontend projection bridge
  -> runtimeProjectionStore.approvals drives permission UI
  -> user responds through respond_permission
  -> pending request is cleared and event log records resolution
```

Permission UI 已接近 projection 驱动；**持久化** pending 与 event log 路径已存在（`pending_permission`、§5.3 数据流）。残余风险主要是 **reload / 重连时** 与 **live channel** 的时序与可观测性是否在所有入口等价——需在联合审计中按场景验证，而非假设「尚未实现」。

## 6. Facts 与事实源

| 事实                                   | 当前来源                                                 | 目标来源                                           |
| -------------------------------------- | -------------------------------------------------------- | -------------------------------------------------- |
| Session list / title / project binding | session manager + `session.json` metadata                | `SessionMeta` / session registry                   |
| Transcript / run events                | `session.json` + run event log + stream payload          | run event log + projection replay                  |
| Active run status                      | raw stream + runtime projection                          | runtime projection from event log/envelope         |
| Permission pending                     | live channel + pending permission file + projection      | pending permission record + event log + projection |
| Memory evidence                        | message memoryContext + memory events + direct IPC pages | memory domain projection + evidence API            |
| Harness report                         | harness report store / suite report                      | canonical run report derived from event log        |

Staff-level 原则：**写事实只进 canonical log；读事实只读 projection；兼容存储只能作为迁移 fallback。**

### 6.1 Harness 与 canonical run log（过渡契约）

在 **MIG-023** 完成前，`HarnessState` 仍通过 **独立 EventBus → JSONL trace、`HarnessRunReport`、`HarnessReportStore`** 形成 **治理侧旁路真相**（与产品 chat 的 `runtime/run-log` 并行）。这不否定 Staff 原则，而是明确：**当前 harness 报告不可从 append-only run log 自动重放派生**；合并路径应以 event log 为 canonical、harness 为视图或派生层，避免第三套手写 transcript。

**会话历史**：`get_session_history_page` 仅在 **无 run-log `*.jsonl` 文件** 时对首屏使用 `session.json` fallback；一旦该会话目录下存在任意 jsonl 文件，即不再用 `session.json` 覆盖首屏 transcript（见 `history::session_has_run_log_jsonl_files`）。

**流式写入 run log**：`RunEventLogger::append_with_correlation` 将 `StreamTokenPayload.correlation`（含 Teams 预留字段）合并到 `RunLogEntry` 顶层列，便于按 run 查询而不仅依赖 payload JSON。

**非流式 turn**：`run.rs` 对同一 `run_id` 使用固定 `CorrelationIds`（`session_id` + `run_id`）调用 `append_with_correlation`，与流式路径对齐。

**`runtime_event` + Evolution 落盘**：Tauri `emit("runtime_event", envelope)` 已携带完整 `RuntimeEventEnvelope`（含 `correlation`；前端见 `src/transport/contracts.ts`）；当调用方传入 `RunEventLogger` 时，现改为 `append_sync_from_envelope`，使 durable 行与广播 envelope 同源（含 team 等 correlation）。

- 2026-05-01：`agent-token` 频道由 `AgentStreamEmitter::emit_payload` 收敛到
  `runtime_event`（PR C-1）；前端 `runtime-projection-bridge.ts` 改为单条
  `listen('runtime_event')` + family router。`runtime/runtime_event.rs::dispatch`
  作为非-evolution 路径的 canonical helper。
- 2026-05-01：`memory_event` / `memory_after_turn` 收敛到 `runtime_event`
  envelope (`event_type=memory`, family ∈ {`lifecycle`, `after_turn`})；
  harness EventBus 上的 `AgentEvent::MemoryAfterTurn` 保留为治理侧独立
  真值源（PR C-3）。chat-runtime 单频道收敛工作 (C-1/C-2/C-3) 完成。

## 7. Gap 与二次真相清单

### P0 / P1 架构 Gap

- `session.json` 仍混合了 session metadata、transcript、identity、skills、memory toggle、project binding 等职责；这和 event log / SessionMeta 目标态冲突。
- `run_id` 与 `stream_id` 并存。前端 handle、后端 event log、stream payload 的 correlation contract 还未完全统一。
- `RuntimeEventEnvelope`、`StreamTokenPayload`、`RunLogEntry` 同时存在，尚未形成单一 wire/event contract。
- `runtimeProjectionStore` 已存在，但 `conversation-slice` 和 `App.tsx` 仍保存/合成大量 chat 展示事实。
- `stream_task.rs` 仍是高复杂度执行 god-file，承担 orchestration、tool loop、stream emission、error handling、部分 persistence。
- `commands/mod.rs` 仍是过大的 service aggregate，command boundary 与 application layer 边界不够薄。
- **Session supervisor（后端）** 已由 `src-tauri/src/modules/runtime/supervisor.rs`（MIG-020）承担 **会话级生命周期快照**；但 **TurnService / stream_task / permission 通道 / 前端 store** 仍可能各自保留编排细节——「单一 owner」在**全栈**上仍未完全成立，需对照调用链继续收口。
- harness 仍保留独立 report/event 事实，与 run event log 存在并行真相（过渡契约见 **§6.1**）。

### P2 优化 Gap

- memory UI 存在多读面：消息 `memoryContext`、projection recent events、以及仍经 `@/lib/tauri` 的页面/类型；**`MemoryBrowser` / `MemoryDebugTab` 主路径已走 `src/api/memory`**，与「仅 projection + 单一 evidence API」的终态仍有差距。
- `src/lib/tauri.ts` 仍是过厚兼容层，新 UI 容易绕过 `src/api/*` facade。
- projection checkpoint / seq 增量 replay 还没有成为 session 打开的主路径。
- provider resilience、cost guard、self repair 已有模块骨架，但还需要和 run event log / projection contract 形成统一可观测闭环。
- activation/license/execution mode 在 projection 中已有读模型痕迹，但产品级 runtime policy 还需要更清晰的 contract。

### 漂移风险

- 文档与代码漂移：旧架构文档仍描述 Svelte、旧 orchestrator、旧 context/budget 模块。
- **双文档与代码漂移**：本文 §8 命名目标与实现进度需定期对照 `codebase`；`docs/packs/**` 已弃用，不得以旧 Registry 行覆盖 §1.0 审计结论。
- 前后端 contract 漂移：TS DTO、Rust command response、runtime contract、event log schema 必须同源或由生成/测试保证。
- UI truth 漂移：同一个 permission/run/memory 状态不能由多个 store 分别推断。

## 8. vNext 架构目标

vNext 的目标架构是：

```text
Session
  = durable event log
  + runtime supervisor
  + frontend projection
```

### 必须完成的收敛

- MIG-016：run event log 成为 canonical runtime fact source。
- MIG-017：chat runtime UI 切到 projection truth，raw stream 只作为 transport。
- MIG-018：session history 通过 event log replay + paging 恢复。
- MIG-019：permission recovery 以 pending permission record + projection 驱动。
- MIG-020：runtime supervisor 显式 owning run lifecycle。
- MIG-021：resume/recovery contract 覆盖 crash、reload、permission wait、provider failure。
- MIG-022：tool attempt ledger 记录每次工具尝试、失败、重试、恢复。
- MIG-023：run report / harness report 从 canonical event log 派生。

### 架构守则

- 不再新增直接消费 raw Tauri event 的 UI surface。
- 不再把 transcript 作为 `session.json` 的长期事实源。
- 不再让 harness、projection、session manager 各自生成独立 run truth。
- 所有新 runtime event 必须能关联 `session_id`、`run_id`、必要时 `turn_id`、`stream_id`。
- 新能力与 vNext 收敛应遵循 [if2ai-vnext-session-runtime-blueprint.md](docs/design-docs/if2ai-vnext-session-runtime-blueprint.md)。`docs/packs/**` 为历史材料，**不**再作为任务编排入口。

## 9. Agents Teams 设计建议

### 9.1 产品定义

Agents Teams 不是“多个 chat tab”，也不是把 agent 名字塞进 session。建议定义为：

> Team 是一个持久化的多 Agent 协作图。它绑定 project/session/runtime facts，由 team supervisor 调度多个 agent member，以 planner / executor / reviewer / researcher 等 role 协作完成一个或多个 run。

核心能力：

- 多 agent member 与 role 编排。
- planner / executor / reviewer / critic / researcher 等角色分工。
- 串行、并行、handoff、review gate、quorum 等协作模式。
- 共享 team memory 与成员私有 memory 的边界。
- team-level tool permission、budget、provider、sandbox policy。
- 全量事件可回放，前端能展示每个 agent 的动作、失败、重试、审核与交接。

### 9.2 后端新增 bounded context

建议新增 `src-tauri/src/modules/team/`，不要把 team 塞进 `session`、`project` 或 `identity`。

建议核心模型：

| Model            | 说明                                                                     |
| ---------------- | ------------------------------------------------------------------------ |
| `TeamMeta`       | team id、name、description、project binding、created/updated             |
| `TeamMember`     | member id、identity/persona binding、role id、capability tags            |
| `AgentRole`      | planner/executor/reviewer/researcher/custom role 定义                    |
| `TeamPolicy`     | budget、tool allow/deny、approval mode、provider preference、parallelism |
| `TeamRun`        | 一次 team-level run，拥有多个 member run / delegation                    |
| `Delegation`     | parent run -> member assignment 的任务边                                 |
| `TeamArtifact`   | shared outputs、review notes、handoff summary                            |
| `TeamProjection` | 前端展示所需的 team read model                                           |

建议服务边界：

- `TeamService`：CRUD、member 管理、project binding。
- `TeamSupervisor`：team run 生命周期、delegation graph、pause/resume/cancel、quorum/review gate。
- `TeamPolicyEngine`：team/member 级 tool permission、budget、provider、memory scope。
- `TeamProjectionService`：从 runtime event log 派生 team projection。

### 9.3 Runtime Contract 扩展

现有 `CorrelationIds` 至少需要扩展：

```rust
pub struct CorrelationIds {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub run_id: Option<String>,
    pub stream_id: Option<String>,
    pub turn_index: Option<u32>,
    pub attempt_id: Option<String>,
    pub team_id: Option<String>,
    pub member_id: Option<String>,
    pub role_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub delegation_id: Option<String>,
}
```

（与 `src-tauri/.../contracts/common.rs` + `src/transport/contracts.ts` 对齐；字段均为可选，未启用 Teams 时省略。）

Team 事件不应绕过 run event log。每个 member 的 tool call、permission request、provider retry、failure attempt、review result 都必须进入 canonical log，并通过 team projection 呈现。

### 9.4 Frontend 设计

建议新增：

- `src/api/teams.ts`：模仿 `startChatTurn()`，提供 `startTeamRun()`、`stopTeamRun()`、`respondTeamPermission()`、`getTeamProjection()`、`getTeamHistoryPage()`。
- `src/transport/team-contracts.ts`：Team DTO / event contract。
- `src/stores/team-store.ts`：只保存 `activeTeamId`、选中 member、局部 UI preference，不保存 runtime truth。
- `src/runtime-projection/team-*`：team event translator / reducer / selectors。
- `src/modules/teams/TeamWorkspace.tsx`：一级工作区。

UI 结构建议：

- `TeamRail`：team 列表、active team、run status。
- `AgentRoster`：成员、角色、在线/忙碌/等待 permission 状态。
- `DelegationGraph`：任务分解与 handoff 图。
- `SharedTimeline`：按 event log replay 的团队时间线。
- `ReviewInbox`：review gate、approval、失败重试建议。
- `ArtifactPanel`：team 输出物、handoff summary、run report。

### 9.5 Memory 与 Permission

Agents Teams 必须显式引入 team scope：

```text
MemoryScope = session | project | team | global
```

建议默认：

- member 私有 scratchpad 不自动进入 team memory。
- team memory 需要 policy gate 或 explicit promote。
- reviewer 的 review result 默认进入 team event log，但是否进入 long-term memory 由 `TeamPolicy` 控制。
- tool permission 既要支持 team-level policy，也要保留 member-level escalation。

### 9.6 Teams 实施切片（命名参考，非 Pack 流程）

以下 ID 仅作**工作分解命名**；**不**对应 `docs/packs/**` 或 CHARTER 执行义务。

- `TEAM-001`：Team domain contracts + persistence skeleton，不接 UI。
- `TEAM-002`：Team API facade + projection contract + frontend store skeleton。
- `TEAM-003`：TeamSupervisor MVP，支持 planner -> executor -> reviewer 串行图。
- `TEAM-004`：team-aware runtime correlation + event log replay。
- `TEAM-005`：TeamWorkspace UI，展示 roster、timeline、delegation graph。
- `TEAM-006`：team memory scope + team permission policy。
- `TEAM-007`：tool attempt ledger 与 review gate 贯通。
- `TEAM-008`：harness/team run report，从 canonical event log 派生质量报告。

## 10. 创新性要点

以下能力应作为 If2Ai 的差异化方向，而不只是补齐基础架构：

- **Flight Recorder Runtime**：每个 session/team run 都是可回放的黑盒记录，可用于 debug、review、resume、training。
- **Projection-first Desktop UX**：前端不是订阅一堆事件，而是读取稳定 projection，天然支持 reload/reconnect/time-travel。
- **Explainable Tool Attempt Ledger**：工具失败、重试、替代方案、人工批准全部可解释，而不是只显示最终结果。
- **Policy-as-Data Agent Teams**：团队协作策略可配置、可审计、可复用，而不是硬编码在 UI flow。
- **Memory Promotion Gate**：从 run evidence 到 long-term/team memory 有明确证据和审批边界。
- **Reviewer-native Workflow**：planner/executor/reviewer 是一等协作角色，review 不是事后检查，而是 runtime graph 的节点。
- **Session Integrity Score**：基于 event log 完整性、missing event、permission dangling、tool retry exhaustion、provider failure 等生成会话完整性指标。

## 11. 优先级建议

下列 **MIG-*** 为本文档内对 vNext 收敛的**命名目标**；是否已在实现中满足，由 **§1.0 联合审计**（对照 `codebase`）判定。

### 立即优先

- 完成 MIG-016 P0/P1 closeout，确保 run event log schema、写入、读取、redaction 稳定。
- 推进 MIG-017/MIG-018，把 chat UI 和 history replay 切到 projection-first。
- 推进 MIG-019，确保 permission pending/recovery/reload 不断流。
- 开始 MIG-020，把 runtime supervisor 从散落逻辑收敛成显式 owner。

### 中期优先

- 拆分 `stream_task.rs`，将 model loop、tool loop、event emission、finalization、permission wait 分离。
- 瘦身 `commands/mod.rs`，让 AppState 聚合和 command adapters 不再成为业务层。
- 合并 `RuntimeEventEnvelope` / `StreamTokenPayload` / `RunLogEntry` 的 contract 来源。
- 让 harness report、suite report、run report 从 canonical event log 派生。

### Agents Teams 前置条件

在正式实现 Teams 前，至少应满足：

- run event log 可稳定 replay 单 session。
- projection store 是 chat runtime UI 的主要读模型。
- permission recovery 可跨 reload 恢复。
- tool attempt ledger 能记录失败/重试。
- correlation id contract 已有 `team_id` / `member_id` / `delegation_id` 预留。

## 12. 参考路径

- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`
- [`.qoder/specs/if2ai-agent-evolution-report.md`](.qoder/specs/if2ai-agent-evolution-report.md)（与 §1.0 审计配对）
- `src/App.tsx`
- `src/api/conversations.ts`
- `src/api/sessions.ts`
- `src/runtime-projection/`
- `src/stores/README.md`
- `src-tauri/src/modules/application/turn_service/`
- `src-tauri/src/modules/control_plane/`
- `src-tauri/src/modules/runtime/event_log.rs`
- `src-tauri/src/modules/runtime/history.rs`
- `src-tauri/src/modules/runtime/pending_permission.rs`
- `src-tauri/src/modules/runtime/contracts/`
