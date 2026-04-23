# If2Ai vNext Session Runtime Specification

> 版本：`1.0.0`
> 状态：`proposed`
> 适用范围：MIG-003 ~ MIG-023 + GAP-001 ~ GAP-008
> 强约束引用：[vNext Session Runtime Blueprint](../design-docs/if2ai-vnext-session-runtime-blueprint.md)
> 最后更新：`2026-04-23`

---

## 0. 术语表

| 术语 | 定义 |
|------|------|
| **Session** | 用户与 agent 的一次持久对话，绑定 project、identity、memory scope |
| **Run** | 一次 agent 执行过程，从 `run_started` 到 `run_completed`/`run_cancelled`/`stream_error` |
| **Turn** | 一次用户输入触发的完整执行链路，一个 turn 可包含多个 tool loop 迭代 |
| **Stream** | Run 的实时传输通道，当前用 `stream_id` 标识，目标统一为 `run_id` |
| **Event** | append-only 事实记录，写入 run event log，不可修改 |
| **Projection** | 从 event 流派生的只读视图，用于 UI / analytics / replay，不发明事实 |
| **Checkpoint** | Projection 在某个 `seq` 的持久化快照，用于加速冷启动 |
| **Canonical** | 唯一权威来源，不允许并行 truth 与之矛盾 |
| **Compatibility** | 为迁移期保留的旧路径，仅作 fallback，不引入新能力 |
| **Correlation** | 跨系统关联 ID 集（session_id / run_id / stream_id / turn_id / attempt_id） |

**"单一真相"判定标准**：任何 runtime surface 的最终展示状态必须只从一个 store 派生。若同一事实可从多个来源推断，则只有 canonical 来源的结果被视为合法。

---

## 1. Executive Summary

核心目标：`Session = durable event log + runtime supervisor + frontend projection`

三层分离：
- **SessionMeta**：产品层元数据，轻、可索引、可列表
- **Run/Event Log**：运行事实，append-only，可 replay，可分页，可审计
- **Projection**：面向 UI/model/analytics 的投影，不直接当事实源

核心不变量：
1. **写事实只进 canonical log** — 所有 runtime 事件必须进入 append-only run event log
2. **读事实只读 projection** — 所有 UI surface 只从 projection store 派生展示
3. **兼容存储只做 fallback** — session.json / harness trace 仅在 event log 缺失时降级读取

---

## 2. 架构原则

### 2.1 Event Log 是事实源

- append-only，按 session/run 组织 JSONL 文件
- `seq` 单调递增，不允许修改已写入的事件
- 最小事件族覆盖 run lifecycle / text / thinking / tool / permission / stream / memory
- 所有 runtime 事件必须能关联 `session_id`、`run_id`

### 2.2 Projection 只负责读，不发明事实

- `runtimeProjectionStore` 是所有 runtime UI 的唯一读模型
- Chat UI / Tool UI / Permission UI / Memory UI / Resume UI 全部只读 projection store
- 不允许任何 UI 组件直接消费 raw Tauri event 或 `agent-token` payload

### 2.3 Supervisor 统一 session 生命周期

- 当前 run 是否活跃、permission pending 是否存在、retry budget、recoverability 等
- 生命周期 owner 不得散落在 TurnService / stream_task / session manager 等多个模块

### 2.4 Tool execution 要有 attempt ledger

- 每次工具调用记录：第几次尝试、发起原因、policy 决策、用户拒绝、超时、重试、最终状态
- attempt ledger 是结构化索引，底层事实仍来自 event log

### 2.5 禁止事项（与 Blueprint §12 对齐）

1. 不允许把 `session.json` 继续扩展成 metadata/transcript/projection/resume 的混合事实源
2. 不允许让前端主聊天路径长期并行依赖 raw transport payload 与 projection store
3. 不允许让 permission pending 只存在于一次性同步阻塞等待中
4. 不允许让 tool retry/failure/policy decision 只体现在 UI 文案而没有 canonical attempt facts
5. 不允许让 harness/replay/eval 继续长期依赖分散 trace 拼装，而不向 event-log truth 收口

---

## 3. 数据模型规格

### 3.1 SessionMeta

产品层索引字段，从 `session.json` 提取迁移：

| 字段 | 类型 | 语义 | 迁移来源 |
|------|------|------|---------|
| `session_id` | `string` | 唯一标识 | session.json `id` |
| `project_id` | `string?` | 绑定项目 | session.json `project_id` |
| `title` | `string` | 显示标题 | session.json `title` |
| `created_at` | `string (RFC3339)` | 创建时间 | session.json `created_at` |
| `updated_at` | `string (RFC3339)` | 最后更新 | session.json `updated_at` |
| `last_run_status` | `enum` | 最近 run 状态 | 从 event log 派生 |
| `last_run_id` | `string?` | 最近 run 标识 | 从 event log 派生 |
| `last_message_preview` | `string?` | 预览文本 | 从 event log 派生 |
| `pinned` | `boolean` | 是否置顶 | session.json `pinned` |
| `identity_override` | `object?` | 身份覆盖 | session.json `identity` |
| `memory_enabled` | `boolean` | 记忆开关 | session.json `memory_enabled` |
| `history_stats` | `object?` | 历史统计 | 从 event log 派生 |

约束：SessionMeta 不包含 transcript / tool_calls / thinking 等 runtime 事实。

### 3.2 RunLogEntry

append-only 事件行：

| 字段 | 类型 | 约束 |
|------|------|------|
| `event_id` | `string` | UUID v4，全局唯一 |
| `session_id` | `string` | 非空 |
| `run_id` | `string` | 非空 |
| `seq` | `u64` | 同一 run 内单调递增，从 1 开始 |
| `event_type` | `string` | 闭集（见 §4 事件族） |
| `occurred_at` | `string (RFC3339)` | 发生时间 |
| `payload_json` | `serde_json::Value` | 事件负载 |
| `causation_id` | `string?` | 因果关系上游事件 ID |
| `correlation_id` | `string?` | 关联追踪 ID |
| `tool_call_id` | `string?` | 工具调用关联 |
| `attempt_id` | `string?` | 工具尝试关联 |

存储路径：`{app_data_dir}/runtime/run-log/{session_id}/{run_id}.jsonl`

### 3.3 ProjectionCheckpoint

| 字段 | 类型 | 语义 |
|------|------|------|
| `session_id` | `string` | 所属 session |
| `projection_kind` | `string` | 投影类型标识 |
| `last_applied_seq` | `u64` | 快照对应的最后 seq |
| `snapshot_json` | `string` | 序列化的 projection 快照 |
| `created_at` | `string (RFC3339)` | 保存时间 |

存储路径：`{app_data_dir}/runtime/projection-checkpoints/{session_id}/{kind}.json`

### 3.4 PermissionPending

| 字段 | 类型 | 语义 |
|------|------|------|
| `pending_id` | `string` | 唯一标识 |
| `session_id` | `string` | 所属 session |
| `run_id` | `string?` | 关联 run |
| `tool_call_id` | `string?` | 关联工具调用 |
| `tool_name` | `string` | 请求权限的工具名 |
| `request_payload` | `object?` | 请求详情 |
| `requested_at` | `string (RFC3339)` | 请求时间 |
| `expires_at` | `string (RFC3339)?` | 过期时间 |
| `resolved_at` | `string (RFC3339)?` | 解决时间 |
| `resolution` | `enum?` | `allowed` / `denied` |

存储路径：`{app_data_dir}/runtime/pending-permissions/{session_id}.json`

### 3.5 ToolAttempt

| 字段 | 类型 | 语义 |
|------|------|------|
| `attempt_id` | `string` | 唯一标识 |
| `session_id` | `string` | 所属 session |
| `run_id` | `string` | 所属 run |
| `tool_call_id` | `string` | 关联工具调用 |
| `tool_name` | `string` | 工具名 |
| `attempt_no` | `u32` | 同一 tool_call 下的尝试序号，从 1 开始 |
| `status` | `enum` | queued/authorizing/running/retrying/completed/failed/cancelled/blocked |
| `policy_decision` | `enum?` | allow/deny/prompt |
| `started_at` | `string (RFC3339)?` | 开始时间 |
| `ended_at` | `string (RFC3339)?` | 结束时间 |
| `failure_kind` | `string?` | 失败分类 |
| `retry_reason` | `string?` | 重试原因 |
| `request_id` | `string?` | 权限请求关联 |
| `evidence_id` | `string?` | 证据关联 |

### 3.6 CanonicalRunReport

从 event log 派生的运行报告：

| Section | 字段 |
|---------|------|
| **Run Summary** | run_id, session_id, started_at, completed_at, status, task_outcome |
| **Stream Outcome** | provider, model, total_tokens, cost_usd, routing |
| **Tool Summary** | tool_count, completed_count, failed_count, retry_count, timeline |
| **Permission Summary** | requested_count, allowed_count, denied_count, pending_count |
| **Recoverability** | resume_available, resume_cursor, resume_reason, safe_to_retry_mutations, failed_attempt_ids, last_settled_tool_call_id |
| **Memory Summary** | decisions_count, accepted_count, rejected_count |

### 3.7 SupervisorSnapshot

Supervisor 拥有的 session 级生命周期权威快照：

| 字段 | 类型 | 语义 |
|------|------|------|
| `session_id` | `string` | 所属 session |
| `status` | `enum` | idle / running / blocked / recoverable_failed / completed / closed |
| `active_run_id` | `string?` | 当前活跃 run |
| `active_run_status` | `enum?` | streaming / completed / failed / cancelled |
| `pending_permission_count` | `usize` | 挂起权限数 |
| `last_error_kind` | `string?` | 最近错误分类 |
| `recoverable` | `boolean` | 是否可恢复 |
| `retry_budget_remaining` | `u32` | 剩余重试预算 |
| `disconnect_grace_until` | `string (RFC3339)?` | 断线保活截止时间 |
| `last_updated_at` | `string (RFC3339)` | 最后更新时间 |

存储路径：`{app_data_dir}/runtime/supervisor/{session_id}.json`

### 3.8 CorrelationIds

跨系统关联契约：

| 字段 | 类型 | 当前状态 | 目标状态 |
|------|------|---------|---------|
| `session_id` | `Option<String>` | 已有 | 必填（runtime event） |
| `project_id` | `Option<String>` | 已有 | 必填（runtime event） |
| `run_id` | `Option<String>` | 已有 | 必填（run-scoped event） |
| `stream_id` | `Option<String>` | 已有，主用 | 降级为 transport compatibility alias |
| `turn_index` | `Option<u32>` | 已有 | 保留 |
| `team_id` | `Option<String>` | 预留 | Agents Teams 扩展 |
| `member_id` | `Option<String>` | 预留 | Agents Teams 扩展 |
| `delegation_id` | `Option<String>` | 预留 | Agents Teams 扩展 |
| `attempt_id` | `Option<String>` | 新增 | Tool Attempt Ledger |

---

## 4. 事件族规格

### 4.1 最小事件族（与 Blueprint §2.1 对齐）

| event_type | 归属 | 必须字段 | 可选字段 |
|------------|------|---------|---------|
| `run_started` | Conversation | run_id, session_id | message_preview, execution_mode |
| `text_delta` | Conversation | run_id, text | — |
| `thinking_started` | Conversation | run_id | — |
| `thinking_delta` | Conversation | run_id, thinking | — |
| `tool_call_queued` | Tool | run_id, tool_call_id, tool_name | tool_args |
| `tool_call_running` | Tool | run_id, tool_call_id, tool_name | attempt_id |
| `tool_call_completed` | Tool | run_id, tool_call_id, tool_name, duration_ms | tool_result, evidence_id |
| `tool_call_failed` | Tool | run_id, tool_call_id, tool_name, failure_kind | attempt_id, retry_reason |
| `permission_requested` | Permission | session_id, tool_name, permission_mode | run_id, tool_call_id |
| `permission_resolved` | Permission | session_id, decision | pending_id, scope |
| `stream_retry_scheduled` | Conversation | run_id, retry_reason | retry_budget_remaining |
| `stream_error` | Conversation | run_id, reason | task_outcome, resume_available, resume_cursor |
| `run_completed` | Conversation | run_id, task_outcome | turn_cost, routing, session_totals, memory_items, prompt_diagnostics |
| `run_cancelled` | Conversation | run_id | — |
| `compact_boundary_emitted` | System | session_id, run_id | — |
| `memory_after_turn_emitted` | Memory | session_id, run_id | decisions, quality, conflicts |

### 4.2 扩展事件族（vNext 新增）

| event_type | 归属 | 语义 |
|------------|------|------|
| `attempt_started` | Tool | 某次工具尝试开始，携带 attempt_id / attempt_no |
| `attempt_settled` | Tool | 某次工具尝试终结（completed/failed/cancelled/blocked） |
| `attempt_retried` | Tool | 工具尝试重试，携带 retry_reason |
| `resume_contract_resolved` | System | Resume recoverability 计算完成 |
| `supervisor_state_changed` | System | Supervisor 状态迁移（idle/running/blocked/recoverable_failed） |
| `session_opened` | System | Session 打开 |
| `session_closed` | System | Session 关闭 |
| `session_compacted` | System | Session 压缩边界 |

### 4.3 事件负载契约

- 每个 event 的 `payload_json` 必须是合法 JSON 对象
- 字段命名：后端 `snake_case`（serde），前端 `camelCase`（translator 转换）
- `causation_id` 指向触发当前事件的上游事件 ID
- `correlation_id` 用于跨 run / 跨 system 追踪

### 4.4 事件序约束

1. `run_started` 必须是同一 `run_id` 的第一个事件（`seq = 1`）
2. `run_completed` / `run_cancelled` 必须是同一 `run_id` 的最后一个事件
3. `permission_resolved` 必须出现在对应 `permission_requested` 之后
4. `attempt_settled` 必须出现在对应 `attempt_started` 之后
5. 同一 `run_id` 的 `seq` 必须单调递增，不允许间断

---

## 5. 前端接口规格

### 5.1 Command API — 发动作，不返回业务真相

| API | 输入 | 输出 | 副作用 |
|-----|------|------|--------|
| `start_run(session_id, input, mode?)` | session_id: string, input: string, mode?: PermissionMode | run_id: string | 创建 run，开始 event log，启动 stream |
| `stop_run(session_id, run_id)` | session_id: string, run_id: string | void | 发送 cancel 信号，记录 `run_cancelled` |
| `resolve_permission(pending_id, decision)` | pending_id: string, decision: allow/deny, scope?: once/session | void | 清除 pending，记录 `permission_resolved` |
| `resume_run(session_id, resume_cursor)` | session_id: string, resume_cursor: string | run_id: string | 从恢复点继续执行 |
| `retry_run(session_id, failed_run_id, strategy?)` | session_id: string, failed_run_id: string, strategy?: RetryStrategy | run_id: string | 基于失败 run 重新发起执行 |

错误码：`SESSION_NOT_FOUND` / `RUN_ALREADY_ACTIVE` / `PERMISSION_NOT_FOUND` / `RESUME_NOT_AVAILABLE` / `RETRY_NOT_SAFE`

### 5.2 Event Subscription API — 订阅 canonical events

| API | 输入 | 输出 |
|-----|------|------|
| `subscribe_session_events(session_id, after_seq?)` | session_id: string, after_seq?: u64 | `RuntimeEventEnvelope` 流 |
| `subscribe_run_events(run_id, after_seq?)` | run_id: string, after_seq?: u64 | `RuntimeEventEnvelope` 流 |

订阅生命周期：`open` → `active` → `closed` / `errored`

- `after_seq` 用于增量订阅（断线恢复时从 checkpoint seq 之后继续）
- 订阅期间如果 run 结束，流自然关闭（发送 terminal event 后 close）

### 5.3 Projection API — UI 冷启动与断线恢复

| API | 输入 | 返回结构 |
|-----|------|---------|
| `get_session_projection(session_id)` | session_id: string | `{ runs, approvals, memory, activation, executionMode, supervisor }` |
| `get_history_page(session_id, before_seq?, limit?)` | session_id: string, before_seq?: string, limit?: u32 | `{ entries, next_cursor, has_more }` |
| `get_pending_permissions(session_id)` | session_id: string | `PermissionPending[]` |
| `get_run_summary(run_id)` | run_id: string | `CanonicalRunReport` |
| `get_supervisor_snapshot(session_id)` | session_id: string | `{ status, active_run_id, pending_permission_count, recoverable, ... }` |

缓存策略：
- Projection API 返回的是 server-side 最新快照
- 前端拿到后 + `after_seq` 增量订阅 = 完整恢复
- 不依赖 localStorage 缓存 projection

---

## 6. MIG-017 Chat Truth Cutover 规格（锚点章节）

### 6.1 当前双真相问题定义

当前 chat 展示是 `Conversation + RunProjection` 合成：

1. **Conversation 路径**：`App.tsx` 订阅 raw `agent-token` → `conversation-slice.appendMessage()` → `ChatWorkspace` 渲染
2. **Projection 路径**：`runtime-projection-bridge` 订阅 `agent-token` → `translator` → `reducer` → `runtimeProjectionStore` → `projectConversationMessagesFromRuns()` → overlay

`conversation-slice` 仍持有的职责：
- `conversations`: 按 sessionId 存储完整消息列表
- `sessionLoading`: 加载状态
- `streamAbortHandles`: 中止句柄
- `sessionTitleStates` / `sessionTodos`: UI 临时态

问题：同一 assistant 消息同时存在于 `conversation-slice` 和 `runtimeProjectionStore.runs`，二者可能不一致。

### 6.2 目标单一真相规格

1. **chat messages 完全从 projection runs 派生** — `projectConversationMessagesFromRuns()` 是唯一消息来源
2. **raw listener 降级为 transport-only** — `App.tsx` 的 `agent-token` 订阅只负责把 event 送入 projection bridge，不再直接 `appendMessage`
3. **assistant text / thinking / tool cards / completion banner** 全部由 `RunProjection` 字段驱动
4. **permission prompt 只读 `runtimeProjectionStore.approvals`** — 不再走独立 IPC 路径

### 6.3 迁移中间态规格

**CompatibilityAdapter 模式**：
- `conversation-slice` 保留为 display adapter，但不再作为 runtime truth 源
- 投影覆盖顺序：projection wins over legacy
- 新组件禁止直接订阅 raw Tauri runtime events
- 现有组件逐步迁移到 projection selector

**迁移阶段**：
1. Bridge 成为唯一事件入口 — raw listener 的 `appendMessage` 调用标记 deprecated
2. ChatWorkspace 逐步替换 conversation-slice selector → projection selector
3. conversation-slice 只保留非 runtime 的 UI 状态（loading, abort handle）
4. 最终 conversation-slice 退役所有 runtime truth 职责

### 6.4 验收标准

- `npm test -- runtime-projection` 全通过
- `npm test -- chat-store` 全通过
- `npm run build` 通过
- chat 主路径不再长期并行依赖 raw listener 与 projection
- 刷新后可用 projection snapshot 恢复当前 run 文本、thinking、tool 状态
- 至少新增一条 reducer / projection 测试覆盖主聊天路径

---

## 7. 各 MIG Pack 验收标准汇总

### 7.1 MIG-003 — Runtime Event Projection Truth

- 聊天界面不直接依赖 raw `agent-token` payload
- 页面刷新后通过 snapshot + replay 恢复 UI
- tool card / thinking / completion banner 都由 projection 驱动

### 7.2 MIG-016 — Event Log Foundation（DONE）

- 每个 streaming run 产生稳定 `run_id`
- 至少 `run_started / tool_call_update / stream_complete / stream_error / permission_requested / permission_resolved` 可持久化
- crash 后可读出最近 run 的 event log
- `seq` 在混合 append/append_sync 下单调递增

### 7.3 MIG-017 — Chat Surface 由 Projection 驱动（锚点）

见 §6.4

### 7.4 MIG-018 — History Replay & Paging（DONE）

- 分页前后无重复无丢失
- 同事件集合的投影稳定
- 跨 run 事件顺序正确
- tool 首见顺序稳定

### 7.5 MIG-019 — Pending Permission Recovery（DONE）

- pending 权限持久化与恢复往返正确
- session ID 路径安全化
- 权限 prompt 刷新后仍可见

### 7.6 MIG-020 — Session Supervisor Foundation

- active / blocked / recoverable_failed 三态可区分
- 前端可读取 supervisor snapshot
- supervisor 是唯一生命周期 owner（不再散落）

### 7.7 MIG-007 — Worker Tool Execution Contract

- ToolExecutionBroker 产出 `attempt_id` / `attempt_no`
- tool event 增加 attempt 关联字段
- tool execution contract 朝 attempt-aware 语义升级

### 7.8 MIG-021 — Resume Contract & Run Recovery

- resume 操作有 typed reason
- mutating tool 不被误标为安全 resume
- UI 能说明"为什么可以/不可以 resume"

### 7.9 MIG-022 — Tool Attempt Ledger & Timeline

- 多次 retry 的 timeline 顺序稳定
- attempt_id / attempt_no 稳定生成
- attempt ledger 状态机完整覆盖 queued → settled

### 7.10 MIG-023 — Canonical Run Report from Event Log

- report 独立于旧 trace 拼装路径仍可生成
- report 包含完整 runtime sections（run summary / tool / permission / recoverability）
- harness 评估可基于 canonical report

### 7.11 MIG-008 — Harness Replay/Eval on Canonical Report

- grader / replay / eval 改读 canonical run report
- 评估结果与旧路径等价（允许浮点误差）
- harness suite 基于 canonical report 运行通过

---

## 8. GAP 规格汇总

### 8.1 GAP-001 — session.json 事实拆分

- metadata 与 transcript 分离的精确字段映射见 §3.1
- history 读取优先 event log，fallback 到 session.json
- 新字段写入只进 event log / SessionMeta，不再扩展 session.json

### 8.2 GAP-002 — Runtime Contract 统一

- `RuntimeEventEnvelope` / `StreamTokenPayload` / `RunLogEntry` 三者关系：
  - `StreamTokenPayload` 可无损转换为 `RuntimeEventEnvelope`
  - `RunLogEntry` 从 `RuntimeEventEnvelope` 构造
  - `run_id` 作为主 correlation，`stream_id` 仅做 transport compatibility alias

### 8.3 GAP-003 — 前端 Projection Single Truth

- assistant text/thinking/tool/completion 从 projection selector 派生
- raw listener 不直接 mutate final message transcript
- permission prompt 只读 `runtimeProjectionStore.approvals`

### 8.4 GAP-004 — Command Boundary Thinning

- `commands/mod.rs` 的 `AppState` 聚合拆出 service registry
- command adapter 薄化：参数校验 → service 调用 → error mapping
- 新 command 路由过 application service，不过 runtime 直接

### 8.5 GAP-005 — stream_task.rs 分解

- 1249 LOC 拆为 ≤800 行的多个模块（stream_orchestrator / model_loop / tool_loop / permission_wait / stream_finalizer）
- 分解前后 event 顺序等价
- 各模块可独立测试

### 8.6 GAP-006 — Memory UI Read Model Unification

- 消息 `memoryContext` / projection recent events / MemoryBrowser / MemoryDebugTab 统一为单一 memory read model
- 新 UI 通过 `src/api/memory.ts` facade 访问，不直接 IPC

### 8.7 GAP-007 — Harness Event Log Truth Cutover

- harness report 优先从 canonical event log 派生
- 旧 trace 路径标记为 fallback only
- grader / replay / eval 改读 canonical run report

### 8.8 GAP-008 — Contract Drift Guardrails

- Rust event kind 与 TS translator 支持集对比测试
- schema snapshot / generated fixture 策略
- CI / pack verify 集成
- 未映射的 runtime event 必须在测试中 fail-fast

---

## 9. 跨领域约束

1. 不再新增直接消费 raw Tauri event 的 UI surface
2. 不再把 transcript 作为 `session.json` 的长期事实源
3. 不再让 harness / projection / session manager 各自生成独立 run truth
4. 所有新 runtime event 必须能关联 `session_id`、`run_id`、必要时 `turn_id`、`stream_id`
5. 所有新 Pack 和 MIG-003/MIG-007 必须按 vNext blueprint 更新任务实施，不得绕过 canonical runtime
6. 编码硬规则：无 `unwrap()`/`expect()`/`todo!()`，跨模块用 `crate::modules::*`，所有 `pub fn` 有 `///` 注释

---

## 10. 文档关系

| 本文档章节 | 对应 Design 文档章节 | 对应 Task 文档 |
|-----------|---------------------|---------------|
| §6 MIG-017 Chat Truth 规格 | §3.2 Chat Truth Cutover 设计 | T-003 锚点 Task |
| §5 前端接口规格 | §4 交互协议设计 | T-002 / T-003 / T-005 / T-020 |
| §3 数据模型规格 | §7 持久化设计 | T-001 / T-004 / T-006 |
| §7 MIG 验收标准 | — | 每个 Task 的验收字段 |
| §8.1 GAP-001 session.json 拆分 | §8.1 session.json 拆分迁移 | T-004 |
| §8.2 GAP-002 Contract 统一 | §4.3 Contract 统一设计 | T-001 / T-018 |
| §8.3 GAP-003 Projection Single Truth | §3.2 Chat Truth Cutover | T-005 |
| §8.4 GAP-004 Command Thinning | §2.6 Command Boundary Thinning | T-009 |
| §8.5 GAP-005 stream_task 分解 | §2.5 stream_task 分解设计 | T-008 |
| §8.6 GAP-006 Memory UI 统一 | §3.5 Memory Read Model 设计 | T-019 |
| §8.7 GAP-007 Harness Truth Cutover | §8.3 Harness truth 迁移 | T-016 / T-017 |
| §8.8 GAP-008 Contract Drift Guardrails | §4.4 Drift Guardrails 设计 | T-018 |
