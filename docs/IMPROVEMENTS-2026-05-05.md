# If2Ai 改善清单 — 2026-05-05

> **生成日期**：2026-05-05  
> **基线扫描**：完整 codebase（后端 71+ runtime 模块、前端 16+ runtime-projection、~35K LOC 核心区域）  
> **配套文档**：[`ARCHITECTURE.md`](../ARCHITECTURE.md) §9 摘要、[`CLAUDE.md`](../CLAUDE.md) §Architecture Boundaries  
> **流程**：通过本文识别的改善项应按 [`CLAUDE.md`](../CLAUDE.md) §Development Workflow 的 Superpowers SKILLS 流程认领与实施。

---

## 0. 摘要

本文档对当前 codebase 做了一次彻底的「设计真值 vs 实现真值」核对，按以下五个轴线整理改善点：

1. **二次真相 (Dual Truth)** — 同一事实存在多个写入或推断点。
2. **冗余设计 (Redundancy)** — 重复结构、不必要的抽象、过厚兼容层。
3. **错误设计 (Design Errors / Bugs)** — 已知导致行为错误或数据丢失的代码。
4. **God Files** — 超过硬阈值且承载多职责的巨型文件。
5. **Gap / 漂移 (Drift)** — 设计意图与实现间的差距、文档/代码不一致、未连线的预留。

每条改善点带有：**严重度**、**位置**、**根因**、**建议路径**。

**汇总**：

| 类别 | P0 | P1 | P2 | 合计 |
|------|-----|-----|-----|------|
| 二次真相 | 1 | 3 | 2 | 6 |
| 冗余设计 | 0 | 2 | 4 | 6 |
| 错误设计 | 1 | 1 | 2 | 4 |
| God Files | 3 | 4 | 0 | 7 |
| Gap / 漂移 | 1 | 4 | 5 | 10 |
| **合计** | **6** | **14** | **13** | **33** |

---

## 1. 二次真相（Dual Truth Sources）

> 违反设计原则 **P1 Single Source of Truth**。同一事实有多个写入点或并行推断点，导致 reload / multi-window / time-travel 时出现状态不一致。

### DT-01 [P0] Harness 仍是与 canonical run log 并行的真相

- **位置**：[`src-tauri/src/modules/harness/event_bus.rs`](../src-tauri/src/modules/harness/event_bus.rs)、[`harness/trace_aggregator.rs`](../src-tauri/src/modules/harness/trace_aggregator.rs)、[`harness/report_persistence.rs`](../src-tauri/src/modules/harness/report_persistence.rs)
- **现状**：Harness 拥有独立 `tokio::broadcast` (cap 256)，订阅者 `TelemetryCollector` / `SessionRecorder` / `TraceAggregator` 分别消费；`HarnessRunReport` 由 trace aggregator 拼装并持久化。**完全独立于 `runtime_event` envelope 与 `event_log.rs`**。
- **根因**：MIG-023「run report / harness report 从 canonical event log 派生」标记 partial。Harness 早于 vNext 收敛而生，未补做迁移。
- **影响**：
  - 同一 turn 在 run-log JSONL 与 harness JSONL trace 中可能出现**字段不同**的重复事件。
  - 评测/回放工具必须同时读两套源，难以一致。
  - 长期可能出现 schema 漂移。
- **建议路径**：
  1. 制定 `HarnessRunReport` 的 codegen / 派生规则，定义其每个字段如何由 `RunLogEntry` 计算得出。
  2. 第一阶段：让 `TraceAggregator` 改为 `event_log` 的 reader（旁路写入），保留 `HarnessReportStore` 持久化结构。
  3. 第二阶段：彻底删除 `event_bus.rs` 与各订阅者，让 telemetry / session_recorder 也改为 event_log reader 或转为 projection 视图。
  4. 在 [`ARCHITECTURE.md`](../ARCHITECTURE.md) §6.1「过渡契约」中标注收敛日期。

### DT-02 [P1] `session.json` 仍混合 metadata + transcript + identity + skills + memory toggle + project binding

- **位置**：[`src-tauri/src/modules/session/`](../src-tauri/src/modules/session/)
- **现状**：尽管 history 已 fallback gated（MIG-018），`session.json` 仍是产品 session 的事实文件，承载远超 metadata 的内容。
- **根因**：`SessionMeta` 抽离工作未完成；旧字段未拆迁。
- **影响**：
  - 任何 schema 变更需要双写保障。
  - 「写事实只进 canonical log」原则被部分违反。
  - 阻碍 multi-window 或并发写。
- **建议路径**：
  1. 定义 `SessionMeta` schema（id、title、icon、created_at、updated_at、project_id、active_identity_id、active_skill_set_ids）。
  2. 把 transcript 字段从 `session.json` 移除（已可由 event_log 派生）。
  3. memory toggle / skills 选择 / identity binding 改写到独立配置文件或 SQLite。
  4. 旧 `session.json` 标记 read-only fallback；定明 sunset 日期。

### DT-03 [P1] `chat-ui.tsx` 通过 prop drilling 接收 messages，而非读 `runtimeProjectionStore`

- **位置**：[`src/components/ui/chat-ui.tsx`](../src/components/ui/chat-ui.tsx)（5,043 LOC）、[`src/App.tsx`](../src/App.tsx)（3,310 LOC）
- **现状**：App.tsx 调用 `projectConversationMessagesFromRuns(s.runs)` 后通过 `messages` prop 注入 ChatUI；ChatUI 自己不订阅 projection store。
- **根因**：MIG-017 收敛 95% 完成，最后一公里未做。
- **影响**：违反 P3「Projection-first UI」；阻碍 multi-window；阻碍 ChatUI 拆分（强耦合 App 状态）。
- **建议路径**：在 ChatUI 内部使用 `useRuntimeProjectionSelector(s => projectConversationMessagesFromRuns(s.runs))`，移除 messages prop。

### DT-04 [P1] Memory UI 多读面

- **位置**：消息内 `memoryContext`（嵌入字段）、`runtimeProjectionStore.memory.recent`（projection 视图）、`@/api/memory` hooks（独立查询）、若干旧页面仍直接 import `@/lib/tauri`
- **现状**：MemoryBrowser / MemoryDebugTab 已切到 `@/api/memory`；其他 surface 与消息内嵌仍各读各的。
- **根因**：MIG-006 「Frontend shell truth」partial。
- **建议路径**：
  1. 抽出 `useMemoryEvidence(runId)` 单一 hook，封装来源选择。
  2. 移除消息内 `memoryContext` 字段（改为通过 hook 按 runId 查询）。
  3. 用 lint 规则禁止 component import `@/lib/tauri` 的 memory DTO。

### DT-05 [P2] Activation / Execution Mode 在 projection 与直 fetch 之间游离

- **位置**：[`src/runtime-projection/runtime-projection-bridge.ts`](../src/runtime-projection/runtime-projection-bridge.ts) `refreshActivationSnapshot()` / `refreshExecutionModeDecision()`、相关 settings 页面
- **现状**：projection 已有读模型痕迹，但仍提供 fetch seam 让某些 UI 直接拉取。
- **建议路径**：定义产品级 ActivationPolicy / ExecutionModePolicy contract；让 projection 成为唯一读模型，fetch seam 仅作为 cold start 初始化。

### DT-06 [P2] `RuntimeEventEnvelope` / `StreamTokenPayload` / `RunLogEntry` 三种 wire schema

- **位置**：[`src-tauri/src/modules/runtime/contracts/common.rs`](../src-tauri/src/modules/runtime/contracts/common.rs)、[`src-tauri/src/modules/runtime/event_log.rs`](../src-tauri/src/modules/runtime/event_log.rs)、[`src/transport/contracts.ts`](../src/transport/contracts.ts)
- **现状**：`append_with_correlation` 已在 envelope ↔ run-log entry 之间桥接，但仍是三种类型并存。
- **建议路径**：
  1. 定义一种 canonical schema（建议 envelope 为唯一来源，RunLogEntry 是其持久化形态）。
  2. 用 codegen（schemars / typeshare / openapi）生成 Rust + TS 双端，杜绝手写漂移。

---

## 2. 冗余设计（Redundant Designs）

### RD-01 [P1] `lib/tauri.ts` 3,414 行遗留 DTO + transport bridge

- **位置**：[`src/lib/tauri.ts`](../src/lib/tauri.ts)
- **现状**：仍持有 100+ 个 session / project / browser / harness DTO；新 UI 容易绕过 `src/api/*` facade 直接 import 这里的类型。
- **根因**：M2.1 DTO 切分未完成。
- **建议路径**：
  1. 按 feature 切分到 `src/transport/<feature>.ts`（sessions、projects、browser、harness、updater、…）。
  2. 在 `lib/tauri.ts` 仅保留 `invoke` re-export 与已 deprecated 的 listen helpers。
  3. ESLint 规则：`src/modules/**` 与 `src/components/**` 禁止 import `src/lib/tauri.ts` 的 DTO。

### RD-02 [P1] `commands/mod.rs` 巨型 AppState 聚合

- **位置**：[`src-tauri/src/commands/mod.rs`](../src-tauri/src/commands/mod.rs)
- **现状**：AppState 持有数十个 service / manager 字段，命令边界与 application layer 边界不够薄。
- **建议路径**：
  1. 拆 `AppState` 成 domain bundles（`MemoryDeps` / `ProviderDeps` / `RuntimeDeps`）。
  2. 命令组按 surface 落到 `commands/<surface>.rs`，仅 import 自己需要的 bundle。
  3. 避免 commands 之间出现 indirect import。

### RD-03 [P2] `conversation.rs` (1471) + `run_delegate.rs` (644) 紧耦合且边界模糊

- **位置**：[`src-tauri/src/modules/runtime/conversation.rs`](../src-tauri/src/modules/runtime/conversation.rs)、[`src-tauri/src/modules/runtime/run_delegate.rs`](../src-tauri/src/modules/runtime/run_delegate.rs)
- **现状**：conversation 是同步 turn executor；run_delegate 是 async↔sync streaming bridge。两者通过 `RunDelegate` trait 协作，但调用关系不直观。
- **建议路径**：
  1. 明确文档：「conversation = pure runtime」「run_delegate = streaming adapter」。
  2. 把 `RunDelegate` trait 提到独立 `runtime/delegate_trait.rs`。
  3. 评估是否可合并（如果 streaming 是唯一 caller）。

### RD-04 [P2] `prompt_planner/` vs `prompt_coordinator.rs` 双层 prompt 装配

- **位置**：[`src-tauri/src/modules/application/prompt_planner/`](../src-tauri/src/modules/application/prompt_planner/) + [`prompt_coordinator.rs`](../src-tauri/src/modules/application/prompt_coordinator.rs)
- **现状**：prompt_planner 已是较完整的 block / merge / compaction 框架；coordinator 仍承担 user input + system + memory 合成。
- **建议路径**：审计两者职责重叠；如果 planner 已能完成 coordinator 工作，将 coordinator 标记 deprecated 或纯收口。

### RD-05 [P2] `evolution_event-store.ts` 与 `runtime-projection-store.ts` 角色重叠

- **位置**：[`src/state/evolution-event-store.ts`](../src/state/evolution-event-store.ts) (99 LOC, placeholder)
- **现状**：注释为「Future evolution events」placeholder；与 `runtimeProjectionStore` 已经可承载的 evolution family 事件可能重叠。
- **建议路径**：要么删除 placeholder，要么明确 evolution 事件不进入 runtime projection 的边界依据。

### RD-06 [P2] `chat-store.ts` 仅 facade 但与 `conversation-slice.ts` 命名分裂

- **位置**：[`src/stores/chat-store.ts`](../src/stores/chat-store.ts) (114 LOC) + [`conversation-slice.ts`](../src/stores/conversation-slice.ts) (307 LOC)
- **现状**：chat-store 是 facade；conversation-slice 是实际 zustand store。Facade 加了一层但收益有限；commit `b6bf7d1` 已停止 re-export deprecated transcript actions。
- **建议路径**：合并为单文件 `chat-state.ts`，明确「chat UI 状态」唯一来源。

---

## 3. 错误设计（Design Errors / Bugs）

### ER-01 [P0] `setActiveModel` 静默丢失 `auth_variant`（数据丢失）

- **位置**：[`src/api/models.ts`](../src/api/models.ts) `setActiveModel` + [`src-tauri/src/commands/provider.rs:171`](../src-tauri/src/commands/provider.rs)
- **现状**：前端 `ActiveModel.auth_variant` 字段已加（FIX-11 fixup, commit `f1c26ed`），但后端 `model_set_active` 命令不接收 `auth_variant` 参数；保存时静默丢字段。
- **影响**：多认证 variant provider（如 Moonshot-CN 国内/国际）保存后下次启动认证错配。
- **追溯**：SP-F1 followup F1F-5（[`docs/superpowers/plans/2026-05-02-sp-f1-followups.md`](superpowers/plans/2026-05-02-sp-f1-followups.md)）。
- **建议路径**：
  1. 后端 `model_set_active` 命令签名增加 `auth_variant: Option<String>`。
  2. `ActiveModel` 持久化结构补字段。
  3. 加测试覆盖：set → get round-trip 校验 auth_variant 不丢。

### ER-02 [P1] `chat-ui.tsx` 残留 raw `invoke('get_models')` 与 `listen('if2ai://models-changed')`

- **位置**：[`src/components/ui/chat-ui.tsx:298, 327`](../src/components/ui/chat-ui.tsx)
- **现状**：违反 [`CLAUDE.md`](../CLAUDE.md) Hard Rule「不绕过 src/api/*」；FIX-11 漏迁。
- **影响**：
  - 阻碍未来 transport 替换（sidecar / cloud gateway）。
  - 模型列表 invalidation 逻辑分散，可能与 `useMemoryInvalidationKey` 风格的 hook 不一致。
- **建议路径**：在 [`src/api/models.ts`](../src/api/models.ts) 暴露 `useAvailableModels()` hook + `subscribeModelsChanged()`，迁移所有 4 处调用点（chat-ui.tsx:302、HomeScreen.tsx:75、ProvidersSettingsPage.tsx:285、ModelSettingsPage.tsx:358）。

### ER-03 [P2] App.tsx 切 session 时缺少原子 snapshot swap，存在快速切换 race

- **位置**：[`src/App.tsx`](../src/App.tsx) `loadConversationHistory()` 调用点
- **现状**：切换 session 时新 history 通过 `loadConversationHistory()` 装载；但没有原子 snapshot swap，快速切换可能导致旧 session 后到的事件被错误归到新 session 的 projection。
- **建议路径**：
  1. 在 `runtimeProjectionStore` 提供 `swapSession(newSessionId)` 原子操作。
  2. translator/reducer 拒绝处理 `correlation.session_id !== current` 的事件。

### ER-04 [P2] `App.tsx:1392-1394` 注释指 retired 通道

- **位置**：[`src/App.tsx`](../src/App.tsx)
- **现状**：注释仍写「agent-token / permission-request / memory_event」；实际已收敛到 `runtime_event`（PR C-1/C-2/C-3/D-1）。
- **追溯**：F1F-3。
- **建议路径**：注释改为「canonical `runtime_event` envelope routed via projection bridge」。

---

## 4. God Files

> 违反原则：内聚边界不清；改一处影响多处；难以并行开发；阻碍 projection-first / 分层守则的执行。

### GF-01 [P0] `src/components/ui/chat-ui.tsx` — **5,043 LOC**

- **承载**：消息渲染（virtual list + map）+ chat input + slash 补全 + model picker + permission mode picker + branch picker + memory chip + turn cost chip + routing chip + tool calls + artifacts + thinking blocks + todo panel + project preview + context bar + file explorer sidebar + browser card iframe。
- **拆分建议**：
  1. `chat-ui/MessageList.tsx`（virtual list + 渲染）
  2. `chat-ui/ChatComposer.tsx`（input + slash 补全）
  3. `chat-ui/pickers/`（ModelPicker、BranchPicker、PermissionModePicker、…）
  4. `chat-ui/chips/`（MemoryChip、TurnCostChip、RoutingChip）
  5. `chat-ui/sidebars/FileExplorerSidebar.tsx`
  6. `chat-ui/cards/BrowserCard.tsx`、`ToolCallCard.tsx`、`ArtifactCard.tsx`、`ThinkingBlock.tsx`
  7. `chat-ui/panels/TodoPanel.tsx`、`ProjectPreviewPanel.tsx`、`ContextBar.tsx`
- 每个文件 ≤ 500 LOC；ChatUI 主组件 ≤ 200 LOC。

### GF-02 [P0] `src-tauri/src/modules/application/turn_service/work_loop.rs` — **3,226 LOC**

- **承载**：work-loop 路由 + skill resolution + 决策。
- **拆分建议**：
  1. `work_loop/mod.rs` — orchestrator（≤ 400 LOC）
  2. `work_loop/skill_resolution.rs` — skill 选择
  3. `work_loop/routing.rs` — 路由决策
  4. `work_loop/decision.rs` — `WorkLoopDecision` 构造
  5. `work_loop/tests.rs`

### GF-03 [P0] `src/App.tsx` — **3,310 LOC**

- **承载**：boot、session、stream、permission、memory overlay、abort handle、title stage、todo、undo/redo、…
- **拆分建议**：
  1. `boot/runBootSequence.ts`
  2. `session/loadConversationHistory.ts`
  3. `session/SessionEffects.tsx`（session 切换的 effect 集中）
  4. `permission/PermissionOverlayHost.tsx`
  5. `app-effects/RuntimeProjectionWiring.tsx`
  6. App.tsx 主体仅做组合 ≤ 400 LOC。

### GF-04 [P1] `src/lib/tauri.ts` — **3,414 LOC**

见 RD-01。

### GF-05 [P1] `stream_finalize.rs` — **1,503 LOC**

- **承载**：guardrail 检查 + outcome 构建 + learning 调度 + memory after-turn dispatch + persistence 收尾。
- **拆分建议**：
  1. `stream_finalize/mod.rs` — orchestrator
  2. `stream_finalize/guardrails.rs`
  3. `stream_finalize/outcome.rs`
  4. `stream_finalize/learning_dispatch.rs`
  5. `stream_finalize/persistence.rs`

### GF-06 [P1] `stream_iteration.rs` — **1,078 LOC**、`stream_task.rs` — **996 LOC**

- 已经是 turn_service 内的 secondary 拆分；建议持续拆出 model loop / tool loop / event emission / permission wait 子模块。

### GF-07 [P1] `runtime/conversation.rs` — **1,471 LOC**

- 与 `run_delegate.rs` 关系紧耦合（见 RD-03）。建议分离 agentic loop core / message conversion / streaming hooks。

---

## 5. Gap / 漂移（Drift）

### DR-01 [P0] MIG-020 Supervisor 仅是「snapshot owner」非「lifecycle owner」

- **位置**：[`src-tauri/src/modules/runtime/supervisor.rs`](../src-tauri/src/modules/runtime/supervisor.rs)
- **现状**：supervisor 持有 `SupervisorSnapshot` 与 5 个 lifecycle hooks，但实际生命周期决策仍散落在 TurnService、stream_task、permission_service、resume_cursor、frontend store。
- **建议路径**：
  1. 把 `start_run` / `block_permission` / `unblock_permission` / `run_completed` / `close_session` 的**调用点收口**到极少数地方（建议唯一调用方为 TurnService）。
  2. 让 supervisor 同时主动 emit `runtime_event(Supervisor, …)` 而不只是被动持久化。
  3. 写测试：supervisor snapshot 与 run-log seq 的一致性 invariant。

### DR-02 [P1] Permission recovery cross-reload integration test 缺失

- **位置**：[`src-tauri/src/modules/application/turn_service/tests.rs`](../src-tauri/src/modules/application/turn_service/tests.rs)（仅 344 LOC）
- **现状**：MIG-019 已完成，但缺端到端测试覆盖「permission requested → reload → permission still visible → user responds → turn 继续」。
- **建议路径**：新增 `permission_recovery_e2e.rs`：
  1. 构造 turn 请求权限。
  2. 模拟进程重启（重建 AppState）。
  3. 调用 `list_pending_permissions(session_id)`，断言含原 request。
  4. 调用 `respond_permission`，断言 turn 解锁继续。

### DR-03 [P1] Projection checkpoint / seq 增量 replay 不是 session 打开主路径

- **现状**：reload 时仍读 history page → projection；没有「checkpoint + 增量 seq」warm-start。
- **影响**：长 session 打开慢；UI 闪现空态。
- **建议路径**：
  1. 在 supervisor snapshot 中记录最新 seq + 关键 projection 字段（活动 run、待决权限）。
  2. 前端 boot 时先恢复 checkpoint 渲染，再异步 fetch 增量 seq。

### DR-04 [P1] `CorrelationIds` 11 字段使用约定缺文档

- **现状**：当前代码每个 run 仅填 4–6 字段；Teams 字段（team_id、member_id、role_id、parent_run_id、delegation_id）已预留但未启用。
- **建议路径**：
  1. 在 [`contracts/common.rs`](../src-tauri/src/modules/runtime/contracts/common.rs) 文档化每个字段的「何时必填、何时可选、谁负责设置」。
  2. 添加 unit test：填写错误组合（如 member_id 无 team_id）应被拒绝。

### DR-05 [P1] Provider resilience / cost guard / self repair 未与 projection 形成闭环

- **位置**：[`runtime/cost_guard`](../src-tauri/src/modules/runtime/cost_guard)、[`runtime/self_repair.rs`](../src-tauri/src/modules/runtime/self_repair.rs)、[`provider/resilience.rs`](../src-tauri/src/modules/provider/resilience.rs)
- **现状**：模块骨架已有，但事件未通过 `runtime_event` envelope 暴露；UI 没有可观测视图。
- **建议路径**：
  1. 定义 `RuntimeEventType::Resilience` family；每次 retry / circuit-break / cost-cap 触发都 emit。
  2. projection reducer 增加 `runs[].resilience` 切面。
  3. UI 在 RoutingChip 旁加 resilience 指示。

### DR-06 [P2] 4 处遗留 raw `invoke('model_list_available')`

- **位置**：见 ER-02；属于同一根因，独立追踪以便单独 PR。
- **建议路径**：合并到 ER-02 的修复 PR。

### DR-07 [P2] `chat-store` 注释禁止 vs `conversation-slice` `@deprecated` 允许「窄使用」措辞冲突

- **追溯**：F1F-7。
- **建议路径**：在 transcript 单一真相完成后统一措辞为「禁止任何 transcript 写入」。

### DR-08 [P2] `commit 9c02d8b` 标题写「3」实际改 4 个 raw 站点

- **追溯**：F1F-6（仅审计记录，不改代码）。
- **建议路径**：写入未来 PR 描述模板「请审视 commit subject 与实际 diff」。

### DR-09 [P2] `RUNTIME_EVENT_CHANNEL` re-export 在 lib/tauri.ts 死引用

- **追溯**：F1F-2。
- **位置**：[`src/lib/tauri.ts`](../src/lib/tauri.ts) L78 周边。
- **建议路径**：随 RD-01 一并清理。

### DR-10 [P2] `conversation.rs:626` 与 `run_delegate.rs:515` 的 TODO

- **现状**：两条 TODO 标记未来 session/message 重构，非关键路径阻塞，但已存在多个 commit。
- **建议路径**：纳入下一次 runtime 重构 plan。

---

## 6. 文档与流程类（Meta）

### MT-01 [P1] 旧 doc 仍引用 retired Pack 流程

- **现状**：CLAUDE.md 已切到 Superpowers（本次更新）；但 `AGENTS.md`、`docs/packs/CHARTER.md`、`docs/packs/REGISTRY.md`、`.cursor/rules/*` 仍按 Pack 描述。
- **建议路径**：
  1. AGENTS.md 同步更新（reflect Superpowers loop）。
  2. `docs/packs/` 整体加顶层 `DEPRECATED.md`，标注全部为历史档案。
  3. `.cursor/rules/superpowers-workflow.mdc` 已更新；其他 rule 检视。

### MT-02 [P1] 双文档「联合审计」需要例行节奏

- **现状**：`ARCHITECTURE.md` 与 `.qoder/specs/if2ai-agent-evolution-report.md` 互为审计基线；缺少周期性 sync 机制。
- **建议路径**：用 cron 或 scheduled remote agent，每 2 周扫描 codebase 与两文档 diff，自动产出 drift 列表。

### MT-03 [P2] 改善项追踪缺统一表

- **建议路径**：本文为初稿；后续每次变更应在「Status」列追加日期与 commit。把本文档 ID（DT-* / RD-* / ER-* / GF-* / DR-* / MT-*）写入相关 PR 描述，便于反向链接。

---

## 7. 实施建议路线图

> **覆盖原则**：本路线图必须覆盖 §1–§6 全部 33 项改善点（DT-01~06、RD-01~06、ER-01~04、GF-01~07、DR-01~10、MT-01~03）。下表先按时间分波，最后用「覆盖矩阵」逐 ID 反查。

### 7.1 第一波（P0，阻塞 vNext 闭环 + 数据安全）— 2026-05 内

| ID | 标题 | 建议形式 | 依赖 |
|----|------|----------|------|
| **ER-01** | 修复 `setActiveModel` `auth_variant` 数据丢失 | bugfix PR + round-trip 测试 | — |
| **GF-01** | 拆分 `chat-ui.tsx`（7 个子模块） | 多 PR 渐进；每次拆 1 个子模块；末次合并 projection-first 重构 | DT-03 |
| **GF-02** | 拆分 `work_loop.rs`（skill_resolution / routing / decision） | brainstorming → writing-plans → 多 PR | — |
| **GF-03** | 拆分 `App.tsx`（boot / session / permission / wiring sub-effects） | 渐进；每次 1 个 sub-effect 抽出 | — |
| **DT-01** | MIG-023：让 `HarnessRunReport` 从 canonical run-log 派生 | brainstorming → plan → 两阶段（aggregator 切 reader → 删 EventBus） | — |
| **DR-01** | Supervisor 收口为唯一 lifecycle owner | brainstorming → plan；包含 invariant 测试 | — |

### 7.2 第二波（P1）— 2026-06

| ID | 标题 | 建议形式 |
|----|------|----------|
| **DT-02** | `session.json` 拆分到 `SessionMeta`；transcript / identity / skills / memory toggle 抽出 | brainstorming → plan → schema 迁移 PR + sunset 公告 |
| **DT-03** | ChatUI 内部直读 `useRuntimeProjectionSelector`，移除 messages prop | 与 GF-01 末次 PR 合并 |
| **DT-04** | Memory UI 收敛到 `useMemoryEvidence(runId)` 单 hook；删除消息内 `memoryContext` | 多 PR：定义 hook → 切换 surface → 移除字段 |
| **ER-02** | 4 处 raw `invoke('get_models')` / `'model_list_available')` 与 `listen('if2ai://models-changed')` 迁到 `src/api/models.ts` | 单 PR；新增 `useAvailableModels()` + `subscribeModelsChanged()` |
| **ER-03** | `runtimeProjectionStore.swapSession()` 原子操作 + reducer 拒绝错配 session 事件 | 单 PR + race 测试 |
| **ER-04** | App.tsx:1392-1394 注释更新为 canonical `runtime_event` envelope | 与 GF-03 渐进 PR 顺带 |
| **RD-01** | `lib/tauri.ts` DTO 按 feature 切分到 `src/transport/<feature>.ts` + ESLint 规则禁止业务层引用 | 多 PR 渐进 |
| **RD-02** | `commands/mod.rs` AppState 拆为 domain bundles（`MemoryDeps` / `ProviderDeps` / `RuntimeDeps`） | brainstorming → plan |
| **RD-03** | `conversation.rs` ↔ `run_delegate.rs` 边界明确；`RunDelegate` trait 抽到 `delegate_trait.rs` | 单 PR + 文档 |
| **GF-04** | `lib/tauri.ts` 瘦身（与 RD-01 一并执行） | 见 RD-01 |
| **GF-05** | 拆分 `stream_finalize.rs`（guardrails / outcome / learning_dispatch / persistence） | brainstorming → plan → 多 PR |
| **GF-06** | 持续拆 `stream_iteration.rs` / `stream_task.rs`（model loop / tool loop / event emission / permission wait） | 渐进 |
| **GF-07** | `runtime/conversation.rs` 拆 agentic loop core / message conversion / streaming hooks | 与 RD-03 协同 |
| **DR-02** | 新增 `permission_recovery_e2e.rs`（reload 跨进程 e2e） | 单 PR |
| **DR-03** | Projection checkpoint warm-start：supervisor snapshot 记录 seq + 关键投影字段；前端 boot 先渲染 checkpoint | brainstorming → plan |
| **DR-04** | `CorrelationIds` 11 字段使用约定文档化 + 校验 unit test | 单 PR |
| **DR-05** | `RuntimeEventType::Resilience` family + projection `runs[].resilience` + UI RoutingChip 指示 | brainstorming → plan |
| **MT-01** | AGENTS.md 同步 Superpowers；`docs/packs/` 加 `DEPRECATED.md`；`.cursor/rules/*` 检视 | 单 PR |

### 7.3 第三波（P2 + Teams 前置 + 长期治理）— 2026-07+

| ID | 标题 | 建议形式 |
|----|------|----------|
| **DT-05** | Activation / Execution Mode policy contract 定义；projection 为唯一读模型，fetch seam 仅作 cold start | brainstorming → plan |
| **DT-06** | `RuntimeEventEnvelope` / `StreamTokenPayload` / `RunLogEntry` 收敛到 codegen 单源（schemars / typeshare） | brainstorming → plan → spike → 渐进切换 |
| **RD-04** | 审计 `prompt_planner/` vs `prompt_coordinator.rs` 职责重叠；coordinator deprecated 或纯收口 | 审计 plan + 重构 PR |
| **RD-05** | 删除 `evolution-event-store.ts` placeholder，或明确其与 runtime projection 的边界 | 单 PR |
| **RD-06** | 合并 `chat-store.ts` + `conversation-slice.ts` → 单文件 `chat-state.ts` | 单 PR |
| **DR-06** | 4 处 raw `invoke('model_list_available')` 收敛（与 ER-02 合并 PR） | 见 ER-02 |
| **DR-07** | `chat-store` / `conversation-slice` deprecated 措辞统一为「禁止任何 transcript 写入」 | 与 RD-06 合并 |
| **DR-08** | PR 描述模板增加「commit subject 与实际 diff 一致」检查项 | 单 PR：`.github/pull_request_template.md` |
| **DR-09** | `RUNTIME_EVENT_CHANNEL` 等 dead re-export 清理 | 与 RD-01 合并 |
| **DR-10** | `conversation.rs:626` / `run_delegate.rs:515` TODO 落地（session id intrinsic + InputMessage From impl） | 与 GF-07 / RD-03 合并 |
| **MT-02** | ARCHITECTURE.md ↔ evolution spec 周期性 sync 计划（cron 或 scheduled remote agent） | 用 `/schedule` 配置；输出到 `docs/superpowers/plans/` |
| **MT-03** | 改善项追踪表：在 PR 描述中引用本文 ID；本文新增「Status」列追加日期 + commit | 单 PR：本文表头扩列 |
| **TEAM-001~008** | Agents Teams 实施切片（见 [`ARCHITECTURE.md`](../ARCHITECTURE.md) §10.6） | brainstorming → 多 plan → 长期项目 |

### 7.4 覆盖矩阵（反查）

下表确认 §1–§6 每条改善点都被路线图覆盖：

| 类别 | ID | 第一波 | 第二波 | 第三波 |
|------|----|:------:|:------:|:------:|
| 二次真相 | DT-01 | ✅ | | |
| 二次真相 | DT-02 | | ✅ | |
| 二次真相 | DT-03 | | ✅ | |
| 二次真相 | DT-04 | | ✅ | |
| 二次真相 | DT-05 | | | ✅ |
| 二次真相 | DT-06 | | | ✅ |
| 冗余设计 | RD-01 | | ✅ | |
| 冗余设计 | RD-02 | | ✅ | |
| 冗余设计 | RD-03 | | ✅ | |
| 冗余设计 | RD-04 | | | ✅ |
| 冗余设计 | RD-05 | | | ✅ |
| 冗余设计 | RD-06 | | | ✅ |
| 错误设计 | ER-01 | ✅ | | |
| 错误设计 | ER-02 | | ✅ | |
| 错误设计 | ER-03 | | ✅ | |
| 错误设计 | ER-04 | | ✅ | |
| God Files | GF-01 | ✅ | | |
| God Files | GF-02 | ✅ | | |
| God Files | GF-03 | ✅ | | |
| God Files | GF-04 | | ✅ | |
| God Files | GF-05 | | ✅ | |
| God Files | GF-06 | | ✅ | |
| God Files | GF-07 | | ✅ | |
| Gap / 漂移 | DR-01 | ✅ | | |
| Gap / 漂移 | DR-02 | | ✅ | |
| Gap / 漂移 | DR-03 | | ✅ | |
| Gap / 漂移 | DR-04 | | ✅ | |
| Gap / 漂移 | DR-05 | | ✅ | |
| Gap / 漂移 | DR-06 | | | ✅ |
| Gap / 漂移 | DR-07 | | | ✅ |
| Gap / 漂移 | DR-08 | | | ✅ |
| Gap / 漂移 | DR-09 | | | ✅ |
| Gap / 漂移 | DR-10 | | | ✅ |
| Meta | MT-01 | | ✅ | |
| Meta | MT-02 | | | ✅ |
| Meta | MT-03 | | | ✅ |
| **合计** | **33 项** | **6** | **15** | **12** |

---

## 8. 验证规则（用于未来 verification-before-completion）

修复任一改善项时，至少执行：

1. `cargo fmt --check --manifest-path src-tauri/Cargo.toml`
2. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
3. `cargo test --manifest-path src-tauri/Cargo.toml <相关 filter> --lib`
4. （前端改动）`npm test` + `npm run build:web`
5. （涉及 Tauri 通道）手工 smoke：start chat turn → 触发权限 → reload → 完成 turn → reload 再读 history。
6. 在 PR 描述中引用本文 ID（DT-*, RD-*, ER-*, GF-*, DR-*, MT-*）。

---

**文档维护者**：架构改善需经 Superpowers `brainstorming` → `writing-plans` → 实施 → `verification-before-completion` → `requesting-code-review`，每次完成更新本文「Status」列与 [`ARCHITECTURE.md`](../ARCHITECTURE.md) §9 摘要。
