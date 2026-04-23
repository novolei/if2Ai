# If2Ai vNext Session Runtime Blueprint

> 状态：`proposed canonical blueprint`
>
> 目标：把 If2Ai 从“能跑的单进程 agent”升级成“可恢复、可审计、可重放、可扩展的产品级会话系统”。
>
> 适用范围：
> - `MIG-003`
> - `MIG-007`
> - `MIG-016` ~ `MIG-023`
>
> 强约束：
> - 上述 Pack 的设计、实现、review、验收，**必须完整参照本蓝图执行**
> - 若 Pack 内局部方案与本蓝图冲突，以本蓝图为准；如确需偏离，必须先更新本蓝图
>
> 配套执行计划：
> - [If2Ai vNext Session Runtime Execution Plan](./if2ai-vnext-session-runtime-execution-plan.md)
>
> 最后更新：`2026-04-23`

---

## 0. Executive Summary

目标不是“再做一层抽象”，而是把 `if2Ai` 从现在的“能跑的单进程 agent”升级成“可恢复、可审计、可重放、可扩展的产品级会话系统”。建议把未来 1-2 个迭代都收束到一个总目标上：

`Session = durable log + runtime supervisor + frontend projection`

也就是说，一个会话不再只是 `session.json + 当前流式 UI`，而是三层分离：

- `SessionMeta`：产品层元数据，轻、可索引、可列表。
- `Run/Event Log`：运行事实，append-only，可 replay，可分页，可审计。
- `Projection`：面向 UI / model / analytics 的投影，不直接当事实源。

---

## 1. 目标架构

建议把整体改造成下面这个骨架：

```text
Frontend
  ├─ AppShell / SessionShell
  ├─ Projection Store (唯一 UI 真相)
  ├─ History Pager
  └─ Action Facade (start / stop / approve / resume / retry)

Gateway / Host Boundary
  ├─ Session API
  ├─ Stream/Event Subscription API
  ├─ History Replay API
  └─ Permission / Control API

Session Supervisor
  ├─ Run Supervisor
  ├─ Permission Pending Registry
  ├─ Retry / Resume Coordinator
  ├─ Tool Attempt Tracker
  └─ Stream Fanout / Reconnect Recovery

Runtime Core
  ├─ Prompt / Memory / Provider / Tool execution
  ├─ TurnService / ToolExecutionBroker
  └─ Event emission (canonical runtime events)

Persistence
  ├─ session_meta
  ├─ run_log
  ├─ projection checkpoints
  ├─ permission_pending
  └─ tool_attempt ledger
```

你们现在已经有其中一半：

- `TurnService` / `stream_task` 是 runtime core。
- `runtime-projection-*` 是 projection 雏形。
- `gateway conversations facade` 是 host boundary 雏形。

真正缺的是中间那层：`Session Supervisor + Event Log`。

---

## 2. 核心原则

### 2.1 Event Log 才是事实源

现在 `if2Ai` 偏向直接持久化 session 对象，这在 MVP 阶段合理，但下一阶段会限制恢复能力。建议未来所有关键行为都先记事件，再投影状态。

最小事件族建议包括：

- `run_started`
- `text_delta`
- `thinking_started`
- `thinking_delta`
- `tool_call_queued`
- `tool_call_running`
- `tool_call_completed`
- `tool_call_failed`
- `permission_requested`
- `permission_resolved`
- `stream_retry_scheduled`
- `stream_error`
- `run_completed`
- `run_cancelled`
- `compact_boundary_emitted`
- `memory_after_turn_emitted`

这和你们现在的 `agent-token` 语义接近，但要升级成“可重放事实”，而不是单纯 UI 推送。

### 2.2 Projection 只负责读，不发明事实

`runtime-projection` 方向是对的。下一步是让：

- Chat UI
- Tool UI
- Permission UI
- Memory evidence UI
- Resume/recovery UI

全部只读 projection store，不再各自消费 transport payload。

### 2.3 Supervisor 统一 session 生命周期

把下面这些从“散落逻辑”收口为统一的 session supervisor：

- 当前 run 是否活跃
- 当前连接数 / viewer 数
- permission pending 是否存在
- retry budget 是否耗尽
- 是否可 resume
- 最近一次失败发生在哪里
- 哪些 tool card 尚未 settle
- 断线后保活多久

### 2.4 Tool execution 要有 attempt ledger

工具调用不是一个点，而是一串尝试。每次工具都应该可回答：

- 第几次尝试
- 为什么发起
- 是否被 policy 阻止
- 是否被用户拒绝
- 是否超时
- 是否重试
- 最终 settle 状态
- 输出是否可信/被截断

这对“不断流”和“调试为什么挂住”非常关键。

---

## 3. 建议的数据模型

### 3.1 SessionMeta

只保留产品级索引字段：

- `session_id`
- `project_id`
- `title`
- `created_at`
- `updated_at`
- `last_run_status`
- `last_run_id`
- `last_message_preview`
- `pinned`
- `identity_override`
- `memory_enabled`
- `history_stats`

### 3.2 RunLogEntry

append-only，一条事件一行，推荐 JSONL 或 sqlite append table：

- `event_id`
- `session_id`
- `run_id`
- `seq`
- `event_type`
- `occurred_at`
- `payload_json`
- `causation_id`
- `correlation_id`
- `tool_call_id`
- `attempt_id`

### 3.3 ProjectionCheckpoint

用于加速恢复：

- `session_id`
- `projection_kind`
- `last_applied_seq`
- `snapshot_json`
- `created_at`

### 3.4 PermissionPending

- `pending_id`
- `session_id`
- `run_id`
- `tool_call_id`
- `tool_name`
- `request_payload`
- `requested_at`
- `expires_at`
- `resolved_at`
- `resolution`

### 3.5 ToolAttempt

- `attempt_id`
- `session_id`
- `run_id`
- `tool_call_id`
- `tool_name`
- `attempt_no`
- `status`
- `policy_decision`
- `started_at`
- `ended_at`
- `failure_kind`
- `retry_reason`
- `request_id`
- `evidence_id`

---

## 4. 前后端交互重构建议

### 4.1 现状

现在你们有：

- `startChatTurn`
- `listenToStream`
- `respondPermission`
- `runtime-projection-bridge`

这已经很好，但还是“动作 API + 流事件”的混合模式。

### 4.2 目标

升级为三类明确接口：

#### 4.2.1 Command API

负责发动作，不返回业务真相：

- `start_run(session_id, input, mode)`
- `stop_run(session_id, run_id)`
- `resolve_permission(pending_id, decision)`
- `resume_run(session_id, resume_cursor)`
- `retry_run(session_id, failed_run_id, strategy)`

#### 4.2.2 Event Subscription API

只订阅 canonical events：

- `subscribe_session_events(session_id, after_seq?)`
- `subscribe_run_events(run_id, after_seq?)`

#### 4.2.3 Projection API

给 UI 用来冷启动和断线恢复：

- `get_session_projection(session_id)`
- `get_history_page(session_id, before_seq?, limit)`
- `get_pending_permissions(session_id)`
- `get_run_summary(run_id)`

这样前端不需要自己从各处拼状态，只需要：

1. 先拿 projection snapshot。
2. 再订阅 `after_seq` 之后的增量事件。
3. reducer 合并。

这就是产品级会话系统的标准做法。

---

## 5. Session 历史与恢复策略

这里建议直接借鉴 `cc-haha-main` 的优点，但不要照搬它的技术形态。

### 5.1 应该保留的优点

- 历史分页加载
- 历史与 UI 解耦
- reconnect 后可以继续追事件
- compact boundary 可见
- sidechain / agent / tool lineage 可重建

### 5.2 更适合 If2Ai 的做法

因为你们 Rust 后端已经很强，不一定需要像它那样依赖外部 CLI transcript 文件。可以在 Rust 内部直接构建：

- `run_log` 作为底层事实
- `conversation_projection` 作为聊天展示投影
- `model_context_projection` 作为续跑上下文投影
- `tool_timeline_projection` 作为工具面板投影

### 5.3 恢复路径

冷启动某个 session 时：

1. 先读 `SessionMeta`
2. 读最新 `ProjectionCheckpoint`
3. 从 `last_applied_seq` 之后 replay event
4. 得到最新 projection
5. 若有 active run，挂订阅继续接 event
6. 若有 pending permission，恢复弹窗
7. 若 run 处于 recoverable failed state，显示 resume CTA

### 5.4 不要再做的事

- 不要把“模型上下文 messages”和“UI 展示 messages”视为同一个数组
- 不要把“当前 session JSON”同时当 metadata、transcript、projection、resume source

---

## 6. 工具调用与不断流策略

建议把“不断流”拆成 4 层：

### 6.1 Provider stream resilience

你们已有基础：

- start retry
- event retry
- resume cursor
- stream error settlement

继续加强：

- retry reason typed 化
- retry budget per run
- jitter/backoff policy
- provider failover hook

### 6.2 Tool attempt resilience

每个 tool call 进入统一状态机：

- `queued`
- `authorizing`
- `running`
- `retrying`
- `completed`
- `failed`
- `cancelled`
- `blocked`

并支持：

- timeout retry
- idempotent retry policy
- user approval retry
- fallback tool/provider path

### 6.3 UI continuity

前端应该永远能明确看到：

- 当前 run 是否还活着
- 当前卡在哪个 tool / permission / retry
- 是否已经自动重试
- 还剩几次重试
- 是否建议用户 intervene

### 6.4 Resume continuity

若 run 中断，系统要能回答：

- 能否 resume
- 从哪里 resume
- resume 后会不会重复执行 mutating tool
- 哪些 tool result 已持久化，哪些只是临时态

这一层必须依赖 attempt ledger 和 event log，不能只靠 UI memory。

---

## 7. 权限系统重构建议

当前 `permission_service` 是同步等待前端响应，这对于 Tauri 内部很顺，但不适合作为长期架构真相。

建议演进成：

### 7.1 Pending Permission Workflow

- runtime 发 `permission_requested`
- supervisor 落库 `PermissionPending`
- frontend 读 pending projection 展示
- 用户动作走 `resolve_permission(pending_id, decision)`
- supervisor 记录 `permission_resolved`
- runtime 继续

### 7.2 这样带来的好处

- 前端刷新不丢 prompt
- 多窗口 / viewer 可见同一 pending
- 可以做超时、自动 deny、审计记录
- 可以支持 session-scoped remember rule
- 可以支持“稍后处理”而不是只能同步卡死

---

## 8. 建议的分期路线

### 8.1 P0：把“真相路径”立住

建议 2-3 个 Pack。

1. `MIG-003-runtime-event-projection-truth`
   原因：`frontend never renders raw transport payloads; it renders replayable runtime projections only`

2. `MIG-007-worker-tool-execution-contract`
   原因：统一 tool lifecycle、attempt、policy、failure、retry contract

3. `EVLOG-001-event-log-foundation`
   目标：引入 append-only run log，不替换现有 session 存储，只旁挂记录。

### 8.2 P1：把“session continuity”补齐

再做 3 个 Pack。

1. `EVLOG-002-history-replay-and-paging`
   目标：从 run log 重建 history page 和 conversation projection。

2. `PERM-001-pending-permission-recovery`
   目标：pending permission 可恢复、可重连、可审计。

3. `SUP-001-session-supervisor-foundation`
   目标：统一 active run / reconnect / retry budget / recoverability 状态。

### 8.3 P2：把“产品级恢复与审计”做扎实

1. `SUP-002-resume-contract-and-run-recovery`
   目标：resume 不只是 cursor，而是完整 recoverability contract。

2. `PROJ-001-tool-timeline-and-attempt-ledger-ui`
   目标：前端完整展示工具调用、重试、失败、策略决策。

3. `HAR-001-run-report-from-event-log`
   目标：harness / grading / eval 基于 canonical run report，而不是分散证据。

---

## 9. 每个 Pack 的验收标准

### 9.1 `EVLOG-001`

- 每个 streaming run 都会产生稳定 `run_id`
- 至少 `run_started / tool_call_update / stream_complete / stream_error / permission_requested / permission_resolved` 可持久化
- crash 后可读出最近 run 的 event log

### 9.2 `MIG-003`

- 聊天界面不直接依赖 raw `agent-token` payload
- 页面刷新后通过 snapshot + replay 恢复 UI
- tool card / thinking / completion banner 都由 projection 驱动

### 9.3 `PERM-001`

- permission prompt 在刷新后仍可见
- pending prompt 超时后有明确状态
- resolve 产生审计事件

### 9.4 `SUP-001`

- session disconnect 后可保活一段时间
- reconnect 可恢复订阅
- active run / failed recoverable / blocked pending permission 三种状态可区分

### 9.5 `SUP-002`

- resume 操作有 typed reason
- mutating tool 不会因 resume 被无脑重复执行
- UI 能说明“为什么可以 resume / 为什么不可以”

---

## 10. 我建议你们先做的具体顺序

如果只选最有价值的前三步，我建议是：

1. `EVLOG-001-event-log-foundation`
   原因：不立事实源，后面 replay、resume、审计都容易继续漂。

2. `MIG-003-runtime-event-projection-truth`
   原因：不先消灭前端双真相，后面 event log 接进来也会变成第三套真相。

3. `PERM-001-pending-permission-recovery`
   原因：这是最容易直接提升产品完整性的点，用户最容易感知。

---

## 11. 对当前代码状态的判断

这次审核里更确定：`if2Ai` 不是缺“执行能力”，而是缺“运行事实与生命周期治理”。

你们现在最有价值的资产其实是：

- Rust runtime 已经很强
- `TurnService` 已经接近 canonical spine
- `runtime-projection` 已经有正确方向
- `stream_complete` payload 已经带了很多高价值语义

这意味着你们不是要推倒重做，而是要把这些好能力接到一个更稳定的 session architecture 上。

---

## 12. Pack 执行强约束

以下 Pack 在 Goal、Spec、Files(scope)、Reads、Contract、Verify 的所有实现与 review 中，**必须完整参照本蓝图**：

- `MIG-003`
- `MIG-007`
- `MIG-016`
- `MIG-017`
- `MIG-018`
- `MIG-019`
- `MIG-020`
- `MIG-021`
- `MIG-022`
- `MIG-023`

具体执行要求：

1. 不允许把 `session.json` 继续扩展成 metadata / transcript / projection / resume 的混合事实源。
2. 不允许让前端主聊天路径长期并行依赖 raw transport payload 与 projection store。
3. 不允许让 permission pending 只存在于一次性的同步阻塞等待中。
4. 不允许让 tool retry / failure / policy decision 只体现在 UI 文案而没有 canonical attempt facts。
5. 不允许让 harness / replay / eval 继续长期依赖分散 trace 拼装，而不向 event-log truth 收口。

若某 Pack 只能完成局部落地，也必须保证方向上朝本蓝图收敛，而不是引入新的旁路真相。

---

## 13. 与现有 MIG Pack 的关系

- `MIG-003`：本蓝图是其上位真相，`MIG-003` 负责把 chat 主路径真正切到 projection truth。
- `MIG-007`：本蓝图是其上位真相，`MIG-007` 负责把 tool/worker execution contract 收口到 attempt-aware runtime contract。
- `MIG-016` ~ `MIG-023`：是本蓝图的拆分执行包，不是独立路线。

因此，`MIG-003` / `MIG-007` / `MIG-016` ~ `MIG-023` 的实现完成，不应被理解为“各自独立成功”，而应被理解为：

`If2Ai vNext Session Runtime Blueprint` 的分阶段落地。

---

## 14. Blueprint Checklist

后续 review 任一相关 Pack 时，至少检查：

- 是否建立了新的事实源，还是继续依赖混合状态对象
- 是否让 projection 更接近唯一 UI 真相
- 是否让 session lifecycle 更集中，而不是更分散
- 是否让 permission / retry / resume 更可恢复
- 是否让 tool execution 更可审计
- 是否让 harness / replay 更接近 event-log truth

只要答案偏向“更分散、更多双真相、更多隐式状态”，就说明该实现偏离本蓝图。
