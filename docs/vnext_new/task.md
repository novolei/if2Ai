# If2Ai vNext Session Runtime Task Breakdown

> 版本：`1.0.0`
> 状态：`proposed`
> 与 spec.md / design.md 的关系：本文件是 spec（做什么）和 design（怎么做）的可执行拆分
> 任务编号规则：T-XXX
> 最后更新：`2026-04-23`

---

## 0. 约定

每个 Task 包含以下字段：

| 字段 | 说明 |
|------|------|
| **优先级** | P0（阻塞）/ P1（关键）/ P2（重要）/ P3（增强） |
| **Owner** | 系统架构 / Runtime Backend / Frontend Projection / Harness Eval / Migration Coordinator |
| **输入** | Task 开始前必须已有的前提 |
| **输出** | Task 完成后交付的产物 |
| **验收** | 可验证的通过条件（测试名或行为描述） |
| **依赖** | 必须先完成的 Task |
| **关联 MIG/GAP** | 对应的 Pack 或 Gap 标识 |

---

## 1. 总体排期策略

### 1.1 四阶段策略

| 阶段 | 目标 | 优先级范围 |
|------|------|-----------|
| **Phase A** | 立事实源与真相边界 | P0 |
| **Phase B** | 补齐 Session Continuity | P1 |
| **Phase C** | 补齐恢复与工具执行语义 | P2 |
| **Phase D** | 审计与评测收口 | P3 |

### 1.2 并行约束

- 每阶段最多 2 个 Task 并行
- 同一 Owner 的 Task 串行
- 跨 Owner 的 Task 可并行（如后端 + 前端）

### 1.3 Owner 角色

| Owner | 职责范围 |
|-------|---------|
| **系统架构** | Contract 统一、架构守则、跨模块协调 |
| **Runtime Backend** | Rust 后端模块（supervisor / attempt_ledger / run_report / stream_task 分解） |
| **Frontend Projection** | TypeScript 前端模块（projection store / chat cutover / selector 迁移） |
| **Harness Eval** | Harness / grading / eval 相关 |
| **Migration Coordinator** | session.json 拆分、迁移 fallback 策略、文档同步 |

### 1.4 依赖关系总图

```text
T-001 ─────┬── T-002 ──── T-003 ──── T-005 ──┬── T-019
            │                                  └── T-020 ── T-007
            ├── T-004 ──┬── T-006 ──┬── T-007
            │            │           ├── T-010 ──┬── T-011 ──┬── T-012
            │            │           │           │           └── T-013 ── T-014
            ├── T-008 ──┘           │           │
            ├── T-009               │           └── T-015 ── T-016 ── T-017
            └── T-018               └── T-020
```

---

## 2. Phase A：立事实源与真相边界

### T-001: GAP-002 Runtime Contract 统一

| 字段 | 值 |
|------|---|
| **优先级** | P0（阻塞后续所有 contract 依赖的 Task） |
| **Owner** | 系统架构 + Runtime Backend |
| **关联** | GAP-002 |
| **输入** | RuntimeEventEnvelope (contracts/common.rs)、StreamTokenPayload (stream_emitter.rs)、RunLogEntry (event_log.rs) 现状分析 |
| **输出** | (1) `StreamTokenPayload` 可无损转换为 `RuntimeEventEnvelope`；(2) `RunLogEntry` 从 `RuntimeEventEnvelope` 构造；(3) `correlation` 字段统一 `run_id` + `stream_id` 双写；(4) `attempt_id` 字段新增 |
| **验收** | `stream_payload_maps_to_runtime_envelope`：每个 StreamTokenPayload event_type 都可映射到 RuntimeEventType；`run_log_entry_from_envelope_round_trips`：envelope → RunLogEntry → JSON → 反序列化等价；`correlation_run_id_is_set_for_run_scoped_events` |
| **依赖** | 无 |

关键代码路径：
- `src-tauri/src/modules/runtime/contracts/common.rs` — 新增 `attempt_id` 到 CorrelationIds
- `src-tauri/src/modules/runtime/stream_emitter.rs` — StreamTokenPayload 新增 correlation 字段
- `src-tauri/src/modules/runtime/event_log.rs` — RunLogEntry 构造从 envelope 生成

---

### T-002: MIG-003 前端 Projection Truth 收敛

| 字段 | 值 |
|------|---|
| **优先级** | P0 |
| **Owner** | Frontend Projection |
| **关联** | MIG-003 |
| **输入** | T-001 完成（contract 统一后 translator 可确保正确性）、MIG-016 event log（DONE） |
| **输出** | (1) runtime-projection-bridge 成为唯一 runtime event ingestion 入口；(2) chat 页面不直接理解底层 agent-token payload；(3) 新增 CanonicalRuntimeEvent variant 覆盖 contract 扩展 |
| **验收** | `npm run build` 通过；`npm test -- runtime-projection` 全通过；前端不再新增直接消费 raw agent-token 的代码路径 |
| **依赖** | T-001 |

关键代码路径：
- `src/runtime-projection/runtime-event-translator.ts` — 扩展 translator 支持 correlation 字段
- `src/runtime-projection/runtime-projection-bridge.ts` — 确认为唯一入口
- `src/runtime-projection/types.ts` — 新增 event variant

---

### T-003: MIG-017 Chat Truth Cutover（锚点 Task）

| 字段 | 值 |
|------|---|
| **优先级** | P0 |
| **Owner** | Frontend Projection |
| **关联** | MIG-017（锚点） |
| **输入** | T-002 完成、MIG-016 event log 稳定 |
| **输出** | (1) chat UI 只从 projection store 读消息；(2) raw listener 降级为 transport-only；(3) `projectConversationMessagesFromRuns` 签名改造为只接受 user messages |
| **验收** | `npm test -- runtime-projection` 全通过；`npm test -- chat-store` 全通过；`npm run build` 通过；chat 主路径不再长期并行依赖 raw listener 与 projection；刷新后可用 projection snapshot 恢复当前 run 状态 |
| **依赖** | T-002 |

关键代码路径：
- `src/runtime-projection/chat-run-projection.ts` — projectConversationMessagesFromRuns 签名改造
- `src/modules/chat/components/ChatWorkspace.tsx` — 消息来源切换到 projection-only
- `src/App.tsx` — raw agent-token listener 的 appendMessage 标记 deprecated
- `src/stores/conversation-slice.ts` — 收缩为 UI 临时态存储

迁移步骤（与 design.md §3.2 对齐）：
1. Bridge 成为唯一事件入口，raw listener appendMessage deprecated
2. ChatWorkspace 逐步切到 projection selector
3. conversation-slice 只保留非 runtime UI 状态
4. Raw listener 完全降级

---

### T-004: GAP-001 session.json 事实拆分

| 字段 | 值 |
|------|---|
| **优先级** | P0 |
| **Owner** | Runtime Backend + Migration Coordinator |
| **关联** | GAP-001 |
| **输入** | session.json 当前字段清单、event log schema |
| **输出** | (1) SessionMeta 结构定义并实现持久化；(2) metadata 与 transcript 分离；(3) history 读取优先 event log，session.json 作为 fallback |
| **验收** | `session_manager_metadata_without_runtime_transcript`：SessionMeta 不含 transcript 字段；`history_prefers_event_log`：history page 优先从 event log 读取；fallback 到 session.json 时记录 fallback 事件 |
| **依赖** | T-001 |

关键代码路径：
- `src-tauri/src/modules/session/` — SessionMeta 提取
- `src-tauri/src/modules/runtime/history.rs` — 读取优先级调整
- `src-tauri/src/commands/session.rs` — API 适配

---

### T-005: GAP-003 前端 Projection Single Truth

| 字段 | 值 |
|------|---|
| **优先级** | P0 |
| **Owner** | Frontend Projection |
| **关联** | GAP-003 |
| **输入** | T-003 完成（chat truth cutover 已完成） |
| **输出** | (1) assistant text/thinking/tool/completion 从 projection 派生，不经 conversation-slice；(2) raw listener 不直接 mutate final message transcript；(3) permission prompt 只读 runtimeProjectionStore.approvals |
| **验收** | `chat_messages_derive_from_projection_runs`：所有 assistant/tool 消息内容来自 RunProjection；`raw_listener_is_transport_only`：raw listener 无 appendMessage 调用；`permission_reads_projection_approvals`：permission UI 只读 projection store |
| **依赖** | T-003 |

关键代码路径：
- `src/runtime-projection/chat-run-projection.ts` — 确认为唯一消息派生路径
- `src/App.tsx` — 移除 raw listener 的 direct state mutation
- `src/stores/conversation-slice.ts` — 移除 runtime truth 字段

---

## 3. Phase B：补齐 Session Continuity

### T-006: MIG-020 Session Supervisor Foundation

| 字段 | 值 |
|------|---|
| **优先级** | P1 |
| **Owner** | 系统架构 + Runtime Backend |
| **关联** | MIG-020 |
| **输入** | T-001 contract 统一完成、T-004 session.json 拆分完成、MIG-019 pending permission（DONE） |
| **输出** | (1) `supervisor.rs` bounded context 实现；(2) SupervisorSnapshot 持久化；(3) 生命周期 hook 接入 TurnService；(4) `get_supervisor_snapshot` API |
| **验收** | `supervisor_status_transitions_are_valid`：idle → running → blocked/recoverable_failed → completed 状态机正确；`supervisor_snapshot_persists_and_loads`：快照持久化往返正确；`active_blocked_recoverable_distinguishable`：三态可区分；`cargo test supervisor` 通过 |
| **依赖** | T-001, T-004 |

关键代码路径：
- `src-tauri/src/modules/runtime/supervisor.rs` — 新建
- `src-tauri/src/modules/application/turn_service/` — 生命周期 hook 接入
- `src-tauri/src/commands/session.rs` — API 暴露

---

### T-007: Supervisor Projection 前端实现

| 字段 | 值 |
|------|---|
| **优先级** | P1 |
| **Owner** | Frontend Projection |
| **关联** | MIG-020（前端侧） |
| **输入** | T-006 后端 supervisor snapshot API |
| **输出** | (1) `supervisor-projection.ts`；(2) RuntimeProjectionSnapshot 扩展 supervisor 字段；(3) reducer 扩展处理 `supervisor_state_changed` 事件；(4) UI 可显示 session 级生命周期状态 |
| **验收** | `npm test -- supervisor-projection` 通过；UI 可显示 active/blocked/recoverable_failed 状态标签 |
| **依赖** | T-006 |

关键代码路径：
- `src/runtime-projection/supervisor-projection.ts` — 新建
- `src/runtime-projection/types.ts` — RuntimeProjectionSnapshot 扩展
- `src/runtime-projection/runtime-event-reducer.ts` — 新增 case

---

### T-008: GAP-005 stream_task.rs 分解

| 字段 | 值 |
|------|---|
| **优先级** | P1 |
| **Owner** | Runtime Backend |
| **关联** | GAP-005 |
| **输入** | T-001 contract 统一完成、stream_task.rs 当前职责分析 |
| **输出** | (1) stream_task.rs (≤200 LOC) 顶层入口；(2) stream_orchestrator.rs 顶层循环；(3) model_loop.rs provider 交互；(4) tool_loop.rs 工具调度；(5) permission_wait.rs 权限等待；(6) stream_finalize.rs 已有 |
| **验收** | `stream_task_under_god_file_limit`：每个文件 ≤ 800 行；`stream_task_components_are_separately_testable`：各模块有独立 unit test；`event_order_equivalent_after_decomposition`：分解前后 event 顺序等价（对比 event log 快照） |
| **依赖** | T-001 |

关键代码路径：
- `src-tauri/src/modules/application/turn_service/stream_task.rs` — 拆分
- `src-tauri/src/modules/application/turn_service/` — 新增模块

---

### T-009: GAP-004 Command Boundary Thinning

| 字段 | 值 |
|------|---|
| **优先级** | P1 |
| **Owner** | Runtime Backend |
| **关联** | GAP-004 |
| **输入** | T-001 contract 统一完成、commands/mod.rs 当前聚合分析 |
| **输出** | (1) ServiceRegistry 结构体；(2) AppState 拆出 service registry；(3) 新 command 路由过 application service；(4) command adapter 薄化 |
| **验收** | `commands_app_state_is_thin_registry`：AppState 不直接持有业务逻辑；`commands_route_through_application_services`：所有 command handler 不超过 15 行；`cargo clippy -D warnings` 通过 |
| **依赖** | T-001 |

关键代码路径：
- `src-tauri/src/commands/mod.rs` — 拆分
- `src-tauri/src/modules/application/service_registry.rs` — 新建

---

## 4. Phase C：补齐恢复与工具执行语义

### T-010: MIG-007 Tool Execution Contract 升级

| 字段 | 值 |
|------|---|
| **优先级** | P2 |
| **Owner** | 系统架构 + Runtime Backend |
| **关联** | MIG-007 |
| **输入** | T-006 supervisor 状态可用、T-008 stream_task 已分解 |
| **输出** | (1) ToolExecutionBroker 产出 `attempt_id` / `attempt_no`；(2) tool event 增加 attempt 关联字段；(3) tool execution contract 朝 attempt-aware 语义升级 |
| **验收** | `tool_execution_produces_attempt_id`：每次工具调用携带 attempt_id；`attempt_no_increments_on_retry`：重试时 attempt_no 递增；`cargo test tool_execution` 通过 |
| **依赖** | T-006, T-008 |

关键代码路径：
- `src-tauri/src/modules/tools/` — ToolExecutionBroker 改造
- `src-tauri/src/modules/application/turn_service/tool_loop.rs` — attempt tracking
- `src-tauri/src/modules/runtime/stream_emitter.rs` — event 增加 attempt 字段

---

### T-011: MIG-021 Resume Contract 实现

| 字段 | 值 |
|------|---|
| **优先级** | P2 |
| **Owner** | Runtime Backend |
| **关联** | MIG-021 |
| **输入** | T-006 supervisor 可用、T-010 attempt-aware contract |
| **输出** | (1) `ResumeRecoverability` 结构实现；(2) typed `ResumeReason` 枚举；(3) `safe_to_retry_mutations` 判定规则；(4) `resume_run` command API；(5) stream_complete/stream_error 带 recovery contract |
| **验收** | `mutating_tool_not_mislabelled_safe`：有副作用的工具不被标为安全 resume；`resume_contract_on_stream_terminal`：stream_complete/stream_error 带 recoverability；`resume_run_command_works`：resume_run API 可正常调用；`cargo test resume` 通过 |
| **依赖** | T-006, T-010 |

关键代码路径：
- `src-tauri/src/modules/runtime/supervisor.rs` — resume 逻辑
- `src-tauri/src/modules/application/turn_service/` — resume_run 接入
- `src-tauri/src/commands/agent/mod.rs` — resume_run command

---

### T-012: Resume Projection 前端实现

| 字段 | 值 |
|------|---|
| **优先级** | P2 |
| **Owner** | Frontend Projection |
| **关联** | MIG-021（前端侧） |
| **输入** | T-011 后端 resume contract API |
| **输出** | (1) `resume-projection.ts`；(2) UI 显示 resume CTA + 原因 + 风险标签；(3) `resume_run` 前端 facade |
| **验收** | `npm test -- resume-projection` 通过；用户可见 typed resume 理由（如 "Provider 超时"、"网络错误"）；风险标签正确（safe / unsafe / needs_confirmation） |
| **依赖** | T-011 |

关键代码路径：
- `src/runtime-projection/resume-projection.ts` — 新建
- `src/api/conversations.ts` — resume_run facade
- `src/modules/chat/components/ChatWorkspace.tsx` — resume CTA UI

---

### T-013: MIG-022 Tool Attempt Ledger 实现

| 字段 | 值 |
|------|---|
| **优先级** | P2 |
| **Owner** | Runtime Backend |
| **关联** | MIG-022 |
| **输入** | T-010 attempt-aware contract、T-011 resume contract |
| **输出** | (1) `attempt_ledger.rs` 实现；(2) attempt_id/attempt_no 稳定生成；(3) attempt ledger 状态机完整；(4) attempt 查询 API（by_run_id / by_tool_call_id / by_session_id） |
| **验收** | `attempt_no_monotonic_per_tool_call`：同一 tool_call_id 下 attempt_no 单调递增；`attempt_status_machine_complete`：queued → settled 所有路径可走通；`timeline_order_stable`：多次 retry 的 timeline 顺序稳定；`cargo test attempt_ledger` 通过 |
| **依赖** | T-010, T-011 |

关键代码路径：
- `src-tauri/src/modules/runtime/attempt_ledger.rs` — 新建
- `src-tauri/src/modules/application/turn_service/tool_loop.rs` — 写入时机
- `src-tauri/src/commands/` — 查询 API

---

### T-014: Attempt Timeline Projection 前端实现

| 字段 | 值 |
|------|---|
| **优先级** | P2 |
| **Owner** | Frontend Projection |
| **关联** | MIG-022（前端侧） |
| **输入** | T-013 后端 attempt ledger API |
| **输出** | (1) `attempt-projection.ts`；(2) RunProjection 扩展 attempts 字段；(3) 前端 timeline 准确显示多次 retry；(4) tool card 展示 attempt 历史 |
| **验收** | `npm test -- attempt-projection` 通过；tool card 展示 attempt 历史（第 1 次、第 2 次重试等）；timeline 正确反映 retry 间隔 |
| **依赖** | T-013 |

关键代码路径：
- `src/runtime-projection/attempt-projection.ts` — 新建
- `src/runtime-projection/types.ts` — RunProjection 扩展
- `src/modules/chat/components/` — tool card 展示改造

---

## 5. Phase D：审计与评测收口

### T-015: MIG-023 Canonical Run Report 生成器

| 字段 | 值 |
|------|---|
| **优先级** | P3 |
| **Owner** | Harness Eval + Runtime Backend |
| **关联** | MIG-023 |
| **输入** | T-011 resume contract、T-013 attempt ledger、MIG-016 event log（DONE）、MIG-018 history replay（DONE） |
| **输出** | (1) `run_report.rs` 实现；(2) 从 event log 生成 CanonicalRunReport；(3) report 包含完整 runtime sections |
| **验收** | `report_independent_of_legacy_trace`：report 独立于旧 trace 拼装路径仍可生成；`report_contains_all_sections`：包含 run summary / stream outcome / tool summary / permission summary / recoverability / memory summary；`cargo test run_report` 通过 |
| **依赖** | T-011, T-013 |

关键代码路径：
- `src-tauri/src/modules/runtime/run_report.rs` — 新建
- `src-tauri/src/modules/runtime/event_log.rs` — 读取源
- `src-tauri/src/modules/runtime/attempt_ledger.rs` — 数据源

---

### T-016: GAP-007 Harness Event Log Truth Cutover

| 字段 | 值 |
|------|---|
| **优先级** | P3 |
| **Owner** | Harness Eval |
| **关联** | GAP-007 |
| **输入** | T-015 run report 可用 |
| **输出** | (1) harness report 优先从 canonical event log 派生；(2) 旧 trace 路径标记为 fallback only |
| **验收** | `harness_report_prefers_canonical_run_report`：harness report 优先使用 canonical report；`legacy_harness_store_is_fallback_only`：旧 store 只在 canonical 不可用时降级 |
| **依赖** | T-015 |

关键代码路径：
- `src-tauri/src/modules/harness/` — 改造
- harness suite 配置 — 优先级调整

---

### T-017: Harness Replay/Eval 接入 Canonical Report

| 字段 | 值 |
|------|---|
| **优先级** | P3 |
| **Owner** | Harness Eval |
| **关联** | MIG-008 |
| **输入** | T-016 harness truth 已收口 |
| **输出** | (1) grader / replay / eval 改读 canonical run report；(2) 评估结果与旧路径等价 |
| **验收** | `harness_suite_results_equivalent`：同一 suite 基于 canonical report 的评估结果与旧路径等价（允许浮点误差）；`harness_suite_runs_cleanly`：`./scripts/pack suite` 通过 |
| **依赖** | T-016 |

---

## 6. 跨 Phase Task

### T-018: GAP-008 Contract Drift Guardrails

| 字段 | 值 |
|------|---|
| **优先级** | P1（持续） |
| **Owner** | 系统架构 |
| **关联** | GAP-008 |
| **输入** | Rust contracts / TS translator / event log schema |
| **输出** | (1) drift 测试：Rust event kind 与 TS translator 支持集对比；(2) schema snapshot 策略；(3) CI / pack verify 集成 |
| **验收** | `unmapped_runtime_event_fails_fast`：新增的 Rust event_type 未在 TS translator 映射中时测试失败；CI 中 `pack verify` 包含此检查 |
| **依赖** | T-001 |

关键代码路径：
- `src/runtime-projection/runtime-event-translator.ts` — 映射完整性
- 新增 `tests/contract-drift.test.ts` — 自动对比

---

### T-019: GAP-006 Memory UI Read Model Unification

| 字段 | 值 |
|------|---|
| **优先级** | P2 |
| **Owner** | Frontend Projection |
| **关联** | GAP-006 |
| **输入** | T-005 projection single truth 完成 |
| **输出** | (1) 统一 memory read model；(2) MemoryBrowser / MemoryDebugTab 通过 `src/api/memory.ts` facade 访问；(3) 消息 `memoryContext` 改从 projection 派生 |
| **验收** | `memory_ui_uses_single_read_model`：所有 memory UI 组件走同一 facade；`memory_context_derives_from_projection`：消息 memoryContext 来自 projection store |
| **依赖** | T-005 |

关键代码路径：
- `src/api/memory.ts` — facade 确认
- `src/modules/memory/` — UI 组件改造

---

### T-020: Projection Checkpoint 冷启动集成

| 字段 | 值 |
|------|---|
| **优先级** | P2 |
| **Owner** | Frontend Projection + Runtime Backend |
| **关联** | 跨 MIG |
| **输入** | T-005 projection-first 完成、T-007 supervisor projection 可用 |
| **输出** | (1) 后端 `get_session_projection` API 实现；(2) ProjectionCheckpoint 持久化；(3) 前端冷启动主路径 = checkpoint + incremental replay；(4) 刷新后 projection 完整恢复 |
| **验收** | `cold_start_recovers_full_projection`：刷新后 projection 完整恢复；`cold_start_under_500ms`：冷启动时间 < 500ms（session < 100 runs） |
| **依赖** | T-005, T-007 |

关键代码路径：
- `src-tauri/src/modules/runtime/` — checkpoint 持久化 + API
- `src/api/sessions.ts` — get_session_projection facade
- `src/runtime-projection/history-replay.ts` — 冷启动流程

---

## 7. 依赖关系矩阵

| Task | 依赖 | 阻塞 |
|------|------|------|
| T-001 | — | T-002, T-004, T-006, T-008, T-009, T-018 |
| T-002 | T-001 | T-003 |
| T-003 | T-002 | T-005 |
| T-004 | T-001 | T-006 |
| T-005 | T-003 | T-019, T-020 |
| T-006 | T-001, T-004 | T-007, T-010, T-011 |
| T-007 | T-006 | T-020 |
| T-008 | T-001 | T-010 |
| T-009 | T-001 | — |
| T-010 | T-006, T-008 | T-011, T-013 |
| T-011 | T-006, T-010 | T-012, T-013, T-015 |
| T-012 | T-011 | — |
| T-013 | T-010, T-011 | T-014, T-015 |
| T-014 | T-013 | — |
| T-015 | T-011, T-013 | T-016 |
| T-016 | T-015 | T-017 |
| T-017 | T-016 | — |
| T-018 | T-001 | — |
| T-019 | T-005 | — |
| T-020 | T-005, T-007 | — |

---

## 8. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| Event Log 落地后前端仍吃 raw payload | projection 层形同虚设 | T-002/T-003 必须在 T-001 后尽快启动；review 时检查是否新增 raw listener 消费 |
| session.json 继续膨胀 | 事实源分裂 | T-004 禁止新 transcript 字段写入 session.json；review 时检查 |
| Supervisor 没有收口成功 | 生命周期仍然散落 | T-006 review 时要求列出"此状态最终由谁拥有"；supervisor 必须是唯一 snapshot owner |
| Resume 只做 UI 提示不做 truth | 恢复不可审计 | T-011 验收要求 typed recoverability contract + event log 记录 |
| Harness 继续依赖零散 trace | 评测不可复现 | T-015/T-016 必须在 T-013 后推进；report 必须可独立生成 |
| 文档与 Pack 脱节 | 实施偏离蓝图 | 每个 task 完成后 Migration Coordinator 回写对应 Pack 状态 |

---

## 9. Checkpoint 定义

### Checkpoint A（Phase A 完成后）：事实源检查

- [ ] 新 runtime truth 是否已优先写入 event log（而非 session.json）
- [ ] 前端主聊天界面是否已切到 projection truth
- [ ] conversation-slice 是否已不再持有 runtime truth

### Checkpoint B（Phase B 完成后）：会话连续性检查

- [ ] 刷新后能恢复什么（projection checkpoint + incremental replay）
- [ ] 重连后能恢复什么（after_seq subscription）
- [ ] supervisor 是否成为唯一生命周期 owner

### Checkpoint C（Phase C 完成后）：执行语义检查

- [ ] tool retry 是否有 canonical attempt facts（attempt_id / attempt_no / status machine）
- [ ] resume 是否有 typed recoverability（ResumeReason + safe_to_retry_mutations）

### Checkpoint D（Phase D 完成后）：评测闭环检查

- [ ] harness/replay 是否已站上 canonical run report
- [ ] 是否仍在拼接分散 traces（应已标记为 fallback only）
- [ ] contract drift guardrails 是否在 CI 中生效

---

## 10. 排期建议

| 轮次 | 并行 Task 1 | 并行 Task 2 | 说明 |
|------|-----------|-----------|------|
| 1 | T-001 (Contract 统一) | T-004 (session.json 拆分) | Phase A 基础 |
| 2 | T-002 (Projection Truth) | T-006 (Supervisor) | 前后端并行 |
| 3 | T-003 (Chat Cutover) | T-008 (stream_task 分解) | 锚点 Task + 后端 god-file |
| 4 | T-005 (Projection Single Truth) | T-009 (Command Thinning) | Phase A 收尾 |
| 5 | T-007 (Supervisor 前端) | T-010 (Tool Contract) | Phase B 收尾 + Phase C 启动 |
| 6 | T-011 (Resume Contract) | T-018 (Drift Guardrails) | 恢复语义 + 持续保障 |
| 7 | T-012 (Resume 前端) | T-013 (Attempt Ledger) | 前后端并行 |
| 8 | T-014 (Attempt 前端) | T-015 (Run Report) | Phase C 收尾 + Phase D 启动 |
| 9 | T-016 (Harness Cutover) | T-019 (Memory UI) | Phase D + 跨 Phase |
| 10 | T-017 (Harness Eval) | T-020 (Checkpoint 冷启动) | 最终收尾 |

---

## 11. 文档关系

| Task | 对应 Spec 章节 | 对应 Design 章节 |
|------|---------------|-----------------|
| T-001 | §8.2 GAP-002 | §4.3 Contract 统一 |
| T-002 | §7.1 MIG-003 | §3.2 Step 1 |
| T-003 | §6 MIG-017（锚点） | §3.2 Chat Truth Cutover |
| T-004 | §8.1 GAP-001 | §8.1 session.json 拆分 |
| T-005 | §8.3 GAP-003 | §3.2 Step 3-4 |
| T-006 | §7.6 MIG-020 | §2.2 SessionSupervisor |
| T-007 | §7.6 MIG-020 | §3.1 Supervisor Projection |
| T-008 | §8.5 GAP-005 | §2.5 stream_task 分解 |
| T-009 | §8.4 GAP-004 | §2.6 Command Thinning |
| T-010 | §7.7 MIG-007 | §2.3 ToolAttempt Ledger |
| T-011 | §7.8 MIG-021 | §2.4 Resume Contract |
| T-012 | §7.8 MIG-021 | §3.1 Resume Projection |
| T-013 | §7.9 MIG-022 | §2.3 ToolAttempt Ledger |
| T-014 | §7.9 MIG-022 | §3.1 Attempt Projection |
| T-015 | §7.10 MIG-023 | §7.7 Run Report 生成策略 |
| T-016 | §8.7 GAP-007 | §8.3 Harness 迁移 |
| T-017 | §7.11 MIG-008 | §8.3 Harness 迁移 |
| T-018 | §8.8 GAP-008 | §4.4 Drift Guardrails |
| T-019 | §8.6 GAP-006 | §3.5 Memory Read Model |
| T-020 | §5.3 Projection API | §3.3 Checkpoint 冷启动 |
