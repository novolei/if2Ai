# DT-01 — HarnessRunReport as a Derived View of the Canonical Run-Log

> **来源**：[`docs/IMPROVEMENTS-2026-05-05.md`](../../IMPROVEMENTS-2026-05-05.md) §1 DT-01 (MIG-023)
> **总览**：[`2026-05-05-improvements-wave1-overview.md`](2026-05-05-improvements-wave1-overview.md)
> **预计**：3 周 · 4–6 PR · 跨两个阶段 · **强烈建议独立 worktree**（与 GF-01/GF-02/GF-03 完全无文件冲突）
> **依赖**：无；建议在 ER-01 / DR-01 之后启动以避免并发改 `RuntimeEventType` 枚举

---

## 1. 总览

`docs/IMPROVEMENTS-2026-05-05.md` §1 DT-01 标注 MIG-023 为 partial：Harness 仍持有独立 `tokio::broadcast`（`event_bus.rs`，cap 256），而 vNext 已收敛 canonical run-log（`runtime/event_log.rs` JSONL，per-session/run，monotonic seq）。本计划用 **2 阶段、6 PR** 把 `HarnessRunReport` 从「EventBus 折叠产物」改造为「run-log 的纯投影」，并最终删除 EventBus、`AgentEvent` 与 `TurnServiceDeps.harness` 字段。

`ARCHITECTURE.md` §6.1 过渡契约明确：**事实只能写入 canonical event log**；`§7` facts table 把 telemetry / harness / report 列为 derived，不应有自己的 mutation 路径；`§8.1` MIG-023 status = partial 即指本项。

## 2. 问题

**ARCHITECTURE.md §6.1 引用**：

> "During migration, modules MAY subscribe to the canonical run-log; modules MUST NOT maintain a parallel write-side truth. EventBus broadcasts in `harness/event_bus.rs` are tolerated only until MIG-023 lands; new fact types MUST be appended to `event_log.rs` first."

**当前实现违反**：`harness/event_bus.rs::AgentEvent` 16 个 variant（TurnStarted / TurnFinished / LlmRequested/Responded / ToolCalled/Result / ContextCompacted / PermissionPrompted / PermissionResolved / ReflectionCompleted / PrepareStepExecuted / ExecutionModeJudged / StreamErrored / ResumeInvoked / MemoryAfterTurn）都是 fire-and-forget broadcast；`TraceAggregator::attach` 后台 task 折叠出 `HarnessRunReport`；`TelemetryCollector` / `SessionRecorder` 同样订阅。Run-log 通过 `RunEventLogger::append_with_correlation` 写 envelope ↔ `RunLogEntry` JSONL。两者**完全平行**。

### 2.1 字段对照（HarnessRunReport ↔ RunLogEntry）

| HarnessRunReport 字段 | EventBus 来源 | run-log 等价来源 (event_type:family + payload) | 派生方式 |
|---|---|---|---|
| `run_id` / `started_at` / `ended_at` | aggregator 启动+末事件时间戳 | 头/尾 `RunLogEntry.occurred_at`，按 `run_id` 分组 | min/max(occurred_at) over entries WHERE run_id=R |
| `session_id` / `project_id` | event.session_id() | `RunLogEntry.session_id` / payload.project_id | 直接读 |
| `task.turn_count` / `last_turn_succeeded` | TurnFinished 累加 | `conversation:turn_finished` 计数 + 末条 success | reduce |
| `task.outcome` | derived from blocking_failures + last success | 同上 + `system:run_failed_*` / `cancelled` | reduce |
| `aggregate.turns_completed/succeeded` | TurnStarted/Finished | `conversation:turn_*` 计数 | reduce |
| `aggregate.llm_calls` | LlmRequested | `conversation:llm_requested`（**今天 run-log 是否有该 family？需 §2.2 audit**）| count |
| `aggregate.input_tokens_total/output_tokens_total` | LlmRequested/Responded payload | `conversation:llm_*` payload.tokens | sum |
| `aggregate.tool_calls_total / *_by_name / successes_by_name` | ToolCalled/ToolResult | `tool:requested` / `tool:result` payload.tool_name + success | reduce |
| `aggregate.permission_prompts` | PermissionPrompted | `permission:opened` | count |
| `aggregate.permission_resolved_allow/deny` | PermissionResolved | `permission:decision` payload.decision | reduce |
| `aggregate.compaction_events` | ContextCompacted | `compression_event:*` 或 `conversation:compact` | count（需 audit） |
| `aggregate.memory_*` 9 项 | MemoryAfterTurn payload | `memory:after_turn` payload | reduce |
| `aggregate.prepare_step_*` 4 项 | PrepareStepExecuted payload | `system:prepare_step` 或新 family（需 audit） | reduce |
| `aggregate.execution_mode_judgments` | ExecutionModeJudged | `execution_mode:judged` | count |
| `aggregate.stream_errors` / `resume_invocations` | StreamErrored / ResumeInvoked | `system:stream_errored` / `system:resume_invoked` | count |
| `aggregate.total_turn_duration_ms` | TurnFinished.duration_ms | `conversation:turn_finished` payload.duration_ms | sum |
| `evidence.memory_after_turn` | MemoryAfterTurn 全文 | `memory:after_turn` payload 全文 | passthrough |
| `evidence.prepare_step` | PrepareStepExecuted | `system:prepare_step` payload | passthrough |
| `evidence.execution_mode` | ExecutionModeJudged | `execution_mode:judged` payload | passthrough |
| `evidence.permission_prompts/resolved` | PermissionPrompted/Resolved | `permission:opened/decision` | passthrough |
| `evidence.stream_errors/resume_invocations` | StreamErrored/ResumeInvoked | `system:stream_errored/resume_invoked` | passthrough |
| `blocking_failures[]` | aggregator 推断 (turn_failed / memory_rejected / prepare_step_denied) | 派生函数：扫 entries 应用同样规则 | reduce |
| `report_version` | 常量 `harness-run-report@m4.4` | 同 | const |

### 2.2 关键 gap（S1.1 audit 任务）

`LlmRequested/Responded`（runtime 直接 emit Conversation 事件，但 token 维度是否 1:1 留存待确认）、`PrepareStepExecuted`（control_plane 路径是否 mirror 到 run-log）、`ExecutionModeJudged`（advisory）、`ContextCompacted` family 名称。Audit 输出列表决定 PR S1.2 是否需要补 emit。

## 3. 目标

1. **唯一真值 = run-log JSONL**。`HarnessRunReport` 是从 `RunLogEntry[]` 派生的纯投影。
2. 阶段 1 完成后：harness 行为零变化，但 `TraceAggregator::finalize` 内部走「读 run-log → reduce → report」路径。EventBus 仍存活以保护 telemetry / session_recorder。
3. 阶段 2 完成后：`event_bus.rs` / `AgentEvent` / `TurnServiceDeps.harness` EventBus 字段全部删除；telemetry / session_recorder 改为 run-log reader。
4. 不变量：`HarnessRunReport_from_eventbus(R) == HarnessRunReport_from_runlog(R)`（所有 R）；以 property test 守护。

## 4. 非目标

- **不修改 `HarnessRunReport` 公共 schema**（`HARNESS_RUN_REPORT_VERSION` 不动；compare/gate/grader/suite 兼容）。
- **不删除 `report_persistence.rs`** 与 `HarnessReportStore`（持久化层不动）。
- **不删 telemetry 概念**（只换数据来源）。
- **不删 harness 模块整体**（compare/corpus/gate/graders/report_persistence/run_report/suite_report 全部保留）。
- **不引入新事件 family**（除非 §2.2 audit 发现必须的运行时 emit 缺口；那种情况下用 `RuntimeEventType::Harness` 收纳）。

## 5. 设计

### 5.1 Stage 1 架构

```
  TurnService → RunEventLogger → run-log JSONL  (canonical)
                       │
                       └─→ EventBus.emit(AgentEvent)  (保留)
                                     │
                                     ├─ TelemetryCollector  (现状保留)
                                     ├─ SessionRecorder     (现状保留)
                                     └─ TraceAggregator     ← 改造点
                                            │
                                            ▼
                              在 finalize() 时改读 run-log JSONL：
                                RunLogReader::collect(session, run)
                                  → fold_to_report(entries) → HarnessRunReport
                                  → HarnessReportStore.save()
```

新增模块：`harness/runlog_projection.rs`
- `pub fn fold_run_log_to_report(entries: &[RunLogEntry]) -> HarnessRunReport`
- `pub fn collect_run_entries(base_dir, session_id, run_id) -> io::Result<Vec<RunLogEntry>>`
- 单元测试覆盖每个 aggregate 字段的派生规则（金标准 fixture）。

### 5.2 Stage 2 架构

```
  TurnService → RunEventLogger → run-log JSONL  (唯一真源)
                                     │
                                     ├─ TraceAggregator (on-finalize reader)
                                     ├─ TelemetryCollector::tail(session, base_dir)
                                     │     · async task tail JSONL，apply 增量到 SessionTelemetry
                                     └─ SessionRecorder：删除（run-log 已是 canonical JSONL；
                                                              如需独立路径，改为「按 session 镜像 run-log」reader）

  EventBus / AgentEvent / agent_loop_integration::emit_*  → 删除
  TurnServiceDeps.harness: Option<Arc<HarnessState>>  → 替换为 Option<RunLogReaderDeps>
```

## 6. PR 序列

### Stage 1（保留 EventBus，引入 reader 路径）

#### DT-01-S1.1 — Schema audit + reconciliation test framework
- 新增 `harness/runlog_projection.rs` 骨架（types only，未接 fold）。
- 新增 `harness/tests/reconciliation.rs`：通过 `TurnService` mock 触发一段事件流，**同时**在末尾对比：
  - `eventbus_report = trace_aggregator.finalize()`
  - `runlog_report = fold_run_log_to_report(collect_run_entries(...))`
- 第一版 fold 故意未实现 → 测试 **xfail**，作为 §2.1 mapping audit 的可执行规约。
- 输出 audit 文档：每个 EventBus variant 是否在 run-log 有对应 entry；缺口列入 S1.2 修复清单。
- 行为零变化。

#### DT-01-S1.2 — Implement TraceAggregator runlog reader
- 实现 `fold_run_log_to_report` 全字段映射。
- 修补 §2.1 audit 暴露的 emit 缺口（在 runtime 侧补 `runtime_event` envelope；harness 不新增 family）。
- Reconciliation tests 全绿。
- `TraceAggregator` 仍 attach EventBus，仅新增 `finalize_from_runlog(base_dir, session, run)` 旁路 API。
- 行为零变化（旧 `finalize()` 仍是默认）。

#### DT-01-S1.3 — Switch report generation to runlog path
- IPC `harness_finalize_run` / `harness_finalize_and_rotate_run` 切到 `finalize_from_runlog`。
- EventBus 仍活；`TraceAggregator` 内部 in-memory state 仍存在但只供 fall-back（feature flag `harness_legacy_aggregator`，默认 off）。
- Reconciliation tests 仍守门。
- 此 PR 之后 Stage 1 完成。

### Stage 2（删除 EventBus）

#### DT-01-S2.1 — Telemetry → run-log tail reader
- `TelemetryCollector::attach(&EventBus)` → `TelemetryCollector::tail(base_dir, session_id)` 启动 tokio task 增量读 JSONL（poll + offset 持久化，参考 `notify` crate 或简单 `tokio::time::interval`）。
- 等价测试：旧 EventBus 路径 vs 新 tail 路径 metric 一致。

#### DT-01-S2.2 — SessionRecorder retire-or-tail
- 评估：run-log JSONL 已是 canonical；`SessionRecorder` 的 `<base_dir>/<session>.jsonl` 与 `runtime/run-log/<session>/<run>.jsonl` 内容是否冗余？
- 若冗余 → 删除 `SessionRecorder`，IPC `start_harness_recording` 退化为 noop（保留命令 surface 兼容）。
- 若仍需 session-scoped 镜像 → 改为 run-log reader 写出统一 jsonl。

#### DT-01-S2.3 — Delete EventBus
- 删除 `harness/event_bus.rs`、`AgentEvent`、`agent_loop_integration::emit_*`。
- `TurnServiceDeps.harness: Option<Arc<HarnessState>>` 字段 → 替换为 `Option<HarnessReaderDeps { report_store, telemetry_tail, base_dir }>`。
- 清理 `stream.rs / stream_finalize.rs / stream_event_loop.rs / stream_iteration.rs / stream_task.rs / stream_tool_execution.rs / run.rs` 中 `harness_event_bus_*` 字段。
- `HarnessState::new` 不再启动 EventBus subscriber，仅持 reader handles。
- 全套测试守门 + smoke：run a turn → harness_finalize → assert report 等价。

## 7. TDD 策略

**核心 invariant**：

```
∀ run R observed during a session S:
   let bus_report   = trace_aggregator_via_eventbus(R);
   let log_report   = fold_run_log_to_report(read_run_log(S, R));
   assert reports_equivalent(bus_report, log_report)
```

`reports_equivalent` 忽略 `started_at`/`ended_at`/`observed_at` 的 ms 级抖动（用 ±1s 容差），其余字段 deep-equal。

实现：
- `harness/tests/reconciliation.rs` — property test (`proptest` 可选；初版用 fixed scenarios)。
- 场景集：empty run、single tool call、tool failure、permission prompt+resolve、memory after-turn 写入、multi-turn、stream error、cancellation。
- Stage 1 期间作为 dual-source 校验；Stage 2 拆掉 EventBus 后改为「金标准 fixture → fold → assert」，invariant 形态保留。

## 8. 文件清单

| PR | 主要新增 | 主要修改 | 删除 |
|---|---|---|---|
| S1.1 | `harness/runlog_projection.rs`（骨架）；`harness/tests/reconciliation.rs` | `harness/mod.rs` re-export | — |
| S1.2 | （fold 实现） | `runtime/...` 补缺口 emit；`runlog_projection.rs` 全实现 | — |
| S1.3 | — | `harness/trace_aggregator.rs`（finalize_from_runlog）；`commands/harness.rs`（IPC 切换） | — |
| S2.1 | `harness/telemetry_tail.rs` | `harness/telemetry.rs`（API 切换）；`HarnessState::new` | — |
| S2.2 | （可能 `harness/recorder_tail.rs`） | IPC `start_harness_recording` | `harness/session_recorder.rs`（视决策）|
| S2.3 | `HarnessReaderDeps` struct | `turn_service/{mod,run,stream,stream_*}.rs`；`commands/{mod,harness,request_intelligence}.rs`；`harness/{mod,agent_loop_integration}.rs` | `harness/event_bus.rs`；`AgentEvent`；`agent_loop_integration::emit_*` |

## 9. 验证

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml harness --lib
cargo test --manifest-path src-tauri/Cargo.toml runtime::event_log --lib
cargo test --manifest-path src-tauri/Cargo.toml turn_service --lib
npm test
npm run build:web
```

Smoke：start chat turn（开 harness） → 完成 → `harness_finalize_run` IPC → 检查 `<root>/<run_id>.json` 字段与 reconciliation fixture 一致；检查 `~/.if2ai/runtime/run-log/<sid>/<rid>.jsonl` 存在且 fold 出等价 report。

## 10. 风险与缓解

| 风险 | 缓解 |
|---|---|
| **EventBus 订阅者 buffer 与文件 reader 时序差异**（最大风险）| Stage 1 全程 dual-source 校验；reconciliation test 是合并阻塞门；Stage 2 拆 EventBus 前必须连续 N 次 dual-source 一致 |
| run-log 缺某些 EventBus variant 对应 family | S1.1 audit 显式列出；S1.2 修补 emit；不私自加 harness-only family |
| `LlmRequested/Responded` token 计数在 run-log 中无 1:1 envelope | 优先在 runtime `conversation` family 补 token payload；fallback 用 `usage.rs` 统计 |
| `SessionRecorder` 删除破坏 harness-cli/外部消费者 | S2.2 评估再决定；若有外部消费，保留为「run-log JSONL 镜像 reader」 |
| `TurnServiceDeps.harness` 删字段触发广面 churn | S2.3 单一 PR 集中 churn；保持 `Option<HarnessReaderDeps>` 形态最小化签名变化 |
| 独立 worktree 与 GF-02 work_loop 拆分 rebase 冲突 | DT-01 S2.3 触及的 `stream_*.rs` 与 GF-02 高度重叠：S2.3 优先在 GF-02 合入后再启动 |
| `report_version` 漂移 | 不改常量；schema 等价性 reconciliation test 守护 |

## 11. PR 描述模板

```markdown
## 改善 ID
DT-01 (Stage <1|2>, PR S<x.y>) — MIG-023

## 变更摘要
<1–3 句>

## Before / After
- 数据流：EventBus → aggregator (in-memory) | run-log → fold → report
- HarnessRunReport schema：未变（HARNESS_RUN_REPORT_VERSION = m4.4）
- Reconciliation invariant：bus_report == runlog_report on N fixtures

## 验证
- [ ] cargo fmt / clippy -D warnings
- [ ] cargo test harness::tests::reconciliation
- [ ] cargo test harness / runtime::event_log / turn_service
- [ ] npm test / build:web
- [ ] Smoke：harness_finalize_run → diff stored report vs golden

## 关联
- Plan: docs/superpowers/plans/2026-05-05-dt01-harness-derive-from-runlog.md
- Wave: docs/superpowers/plans/2026-05-05-improvements-wave1-overview.md
- Spec: ARCHITECTURE.md §6.1, §7, §8.1 MIG-023
```
