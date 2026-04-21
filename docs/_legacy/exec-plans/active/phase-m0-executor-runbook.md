# Phase M0 Executor Runbook

> 将 `phase-m0-canonical-contracts-and-truth.yaml` 细化为 executor 可直接执行的首批切片顺序、产物模板、检查清单与风险控制规则。
>
> 最后更新: 2026-04-20（M0.0~M0.7 全部落地，§11 列出最终产物，executor 可基于此判定 M0 关闭）

## 1. 目的

本 runbook 只解决一件事：

> 让执行者在不二次解释蓝图的前提下，能直接启动 `M0`，并且明确知道每个 slice 要改什么、写什么、验什么、不能做什么。

`M0` 不是功能开发 phase。`M0` 的职责是把 If2Ai 后续所有整改要依赖的“真相”固定下来，避免 `M1/M2/M3/M4/M5` 建在漂移语义之上。

## 2. 本阶段必须产出的结果

`M0` 结束时，必须同时满足下面四件事：

1. canonical domain model 已落地为正式文档。
2. workflow truth registry 已落地为正式文档。
3. runtime / activation / execution-mode contracts 已有代码入口和 TS 对位类型入口。
4. `App.tsx` / `chat-ui.tsx` / `tauri.ts` / `commands/agent.rs` 的责任清单已经能直接作为 `M1/M2` 的拆分输入。

## 2.1 配套 file-level plans

为和 `M1~M5` 的执行粒度保持一致，`M0` 额外拆成以下 3 份 file-level plan：

1. [phase-m0-truth-documents-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-truth-documents-file-level-plan.md:1)
2. [phase-m0-runtime-activation-execution-contracts-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-runtime-activation-execution-contracts-file-level-plan.md:1)
3. [phase-m0-responsibility-inventory-and-linkage-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-responsibility-inventory-and-linkage-file-level-plan.md:1)

## 3. 读取顺序

### 3.1 执行者最小必读

执行者开工 `M0` 时，只要求先读下面 5 个入口：

1. [phase-m-remediation-execution-index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1)
2. [phase-m0-canonical-contracts-and-truth.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-canonical-contracts-and-truth.yaml:1)
3. 本 runbook
4. 当前 slice 对应的 file-level plan
5. 当前 slice 明确引用的真实代码入口文件

### 3.2 `M0` file-level plan 入口

1. [phase-m0-truth-documents-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-truth-documents-file-level-plan.md:1)
2. [phase-m0-runtime-activation-execution-contracts-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-runtime-activation-execution-contracts-file-level-plan.md:1)
3. [phase-m0-responsibility-inventory-and-linkage-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m0-responsibility-inventory-and-linkage-file-level-plan.md:1)

### 3.3 设计依据按需查阅

只有当当前 slice 需要核对定义或命名口径时，再回查以下设计文档：

1. [if2ai-staff-remediation-blueprint.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-staff-remediation-blueprint.md:1)
2. [canonical-domain-model-and-workflow-truth-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/canonical-domain-model-and-workflow-truth-design.md:1)
3. [runtime-contracts-and-event-projection-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/runtime-contracts-and-event-projection-design.md:1)
4. [activation-gate-and-license-lifecycle-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/activation-gate-and-license-lifecycle-design.md:1)
5. [request-intelligence-and-execution-mode-routing-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/request-intelligence-and-execution-mode-routing-design.md:1)

## 4. 落地决策

### 4.1 Rust contract 目录以现状模块树为准

蓝图文档里曾使用 `src-tauri/src/runtime/contracts/` 作为目标表达，但当前代码实际入口是 [main.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/main.rs:1) + [modules/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/mod.rs:1) + `src-tauri/src/modules/runtime/`。

因此 `M0` 落地时，Rust contract skeleton 必须创建在：

- `src-tauri/src/modules/runtime/contracts/mod.rs`
- `src-tauri/src/modules/runtime/contracts/common.rs`
- `src-tauri/src/modules/runtime/contracts/activation.rs`
- `src-tauri/src/modules/runtime/contracts/execution_mode.rs`
- `src-tauri/src/modules/runtime/contracts/memory.rs`

不要在 `src-tauri/src/runtime/` 再平行造一棵新树。

### 4.2 TS contract 入口从 `tauri.ts` 拆出

当前 [tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1) 体量约 2071 行，不得继续作为事实契约中心。

`M0` 的 TS 合同入口统一落在：

- `src/transport/contracts.ts`

`M0` 只要求建立 skeleton 与命名基线，不要求在本 phase 完成所有 translator/reducer/store 改造。

### 4.3 文档真相必须优先于代码迁移

如果代码命名和文档真相冲突，`M0` 优先先把真相写清，不在本 phase 强行做大规模 rename。  
rename 是 `M1/M2` 的后续工作，不是 `M0` 的主任务。

## 5. 不允许做的事

1. 不要在 `M0` 启动 `App.tsx`、`chat-ui.tsx` 的大规模 UI 重构。
2. 不要在 `M0` 直接把 activation gate 做成完整功能。
3. 不要在 `M0` 落地 request classifier 的完整策略实现。
4. 不要在 `M0` 偷偷引入第二套 contract 命名。
5. 不要写“以后再说”的模糊文档。每个主 workflow 必须标 `canonical / partial / not_established`。

## 6. 首批 slice 严格顺序

执行顺序必须固定为：

1. `m0.0` preflight inventory
2. `m0.1` canonical domain model
3. `m0.2` workflow truth registry
4. `m0.3` runtime contract skeleton
5. `m0.4` activation contract + boot truth
6. `m0.5` execution-mode contract + reason taxonomy
7. `m0.6` god-file responsibility inventory
8. `m0.7` coverage matrix and plan linkage

其中：

- `m0.3` 之前不得写 contract code
- `m0.4` 之前不得定义 activation UI 语义
- `m0.5` 之前不得定义 execution-mode 前端字段
- `m0.6` 必须在 `m0.3~m0.5` 之后做，否则责任盘点无法基于新 contract truth

## 7. Slice 详细执行说明

## 7.1 `m0.0` Preflight Inventory

### 目标

在正式写文档前，先把现状证据采齐，避免后续凭印象写真相。

### 必查文件

- [ARCHITECTURE.md](/Users/ryanliu/Documents/IfAI/if2Ai/ARCHITECTURE.md:1)
- [AGENTS.md](/Users/ryanliu/Documents/IfAI/if2Ai/AGENTS.md:1)
- [src-tauri/src/commands/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/mod.rs:1)
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1)
- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1)
- [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1)
- [src-tauri/src/modules/session/manager.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/session/manager.rs:1)
- [src-tauri/src/main.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/main.rs:1)

### 必做动作

1. 记录当前主实体命名。
2. 记录当前 workflow 真实入口文件。
3. 记录 activation、execution mode、memory、harness 当前是否已有半实现。
4. 记录四个 god-file 的当前规模。

### 建议记录格式

放入临时工作笔记或 PR 描述，不一定要单独提交文件：

| item           | current_entry                              | status          | notes                   |
| -------------- | ------------------------------------------ | --------------- | ----------------------- |
| activation     | `commands/activation.rs` + onboarding flow | partial         | 还不是硬门禁            |
| execution_mode | none                                       | not_established | 尚无 canonical contract |

### 通过标准

后续 `m0.1~m0.6` 写作时，不出现“找不到真实入口”的情况。

## 7.2 `m0.1` Canonical Domain Model

### 输出文件

- `docs/staff-remediation/if2ai-canonical-domain-model.md`
- `docs/staff-remediation/README.md`

### 文件必须包含的章节

1. 文档目的
2. canonical naming rules
3. canonical entity catalog
4. entity lifecycle table
5. layer ownership table
6. naming drift and migration map
7. open questions

### canonical entity catalog 最少必须覆盖

1. `agent`
2. `session`
3. `project`
4. `run`
5. `worker`
6. `memory`
7. `automation`
8. `harness_eval`
9. `activation_license`
10. `execution_mode`

### 每个实体必须写的字段

| field                | required |
| -------------------- | -------- |
| canonical_name       | yes      |
| definition           | yes      |
| not_definition       | yes      |
| lifecycle            | yes      |
| owning_layers        | yes      |
| current_code_entry   | yes      |
| future_primary_owner | yes      |

### naming drift map 必须覆盖

至少整理下面几类漂移：

1. `turn / run / stream / task` 是否混用
2. `agent / worker / assistant` 是否混用
3. `activation / onboarding / setup / access` 是否混用
4. `mode / profile / route / scenario` 是否混用
5. `memory / summary / recall / knowledge` 是否混用

### 明确引用

- [canonical-domain-model-and-workflow-truth-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/canonical-domain-model-and-workflow-truth-design.md:1)
- [uclaw-if2ai-architecture-migration-report.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/uclaw-if2ai-architecture-migration-report.md:1)

### 不能漏掉的判断

1. `execution_mode` 是 runtime truth，不是 scenario profile。
2. `activation_license` 是平台门禁实体，不是设置项。
3. `memory` 不是聊天记录全文。

### 完成定义

人工审查时，任何 reviewer 都可以回答“这个概念是什么、不是什么、由谁拥有、在哪一层结算”。

## 7.3 `m0.2` Workflow Truth Registry

### 输出文件

- `docs/staff-remediation/if2ai-workflow-truth.md`
- `docs/staff-remediation/README.md`

### 必须列出的 workflow

1. app boot
2. onboarding
3. activation gate
4. chat prompt dispatch
5. stream projection
6. permission approval
7. session recovery
8. memory capture
9. memory recall
10. memory write
11. harness record
12. harness replay
13. harness eval
14. execution mode routing
15. specialized surface entry
16. deactivation / revoke fallback

### 每条 workflow 必须有的列

| column           | meaning                                     |
| ---------------- | ------------------------------------------- |
| workflow_name    | canonical 名称                              |
| status           | `canonical` / `partial` / `not_established` |
| backend_entry    | Rust 入口文件                               |
| frontend_entry   | TS/React 入口文件                           |
| source_of_truth  | 当前哪一层算真                              |
| blockers         | 为什么没 canonical                          |
| next_phase_owner | `M1` / `M2` / `M3` / `M4` / `M5`            |

### 必须显式写清的 current-state judgement

1. activation gate 当前应标为 `partial`，除非已成为硬门禁。
2. execution mode routing 当前应标为 `not_established`，除非已有正式 classifier 与 contract。
3. stream projection 若仍大量直接在 [App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1) / [chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1) 处理，应标 `partial`。

### 人工 checkpoint

`m0.2` 完成后必须停一次，确认 workflow status 没有“为了好看而高估成熟度”。

## 7.4 `m0.3` Runtime Contract Skeleton

### 输出文件

- `src-tauri/src/modules/runtime/contracts/mod.rs`
- `src-tauri/src/modules/runtime/contracts/common.rs`
- `src-tauri/src/modules/runtime/contracts/activation.rs`
- `src-tauri/src/modules/runtime/contracts/execution_mode.rs`
- `src-tauri/src/modules/runtime/contracts/memory.rs`
- `src/transport/contracts.ts`

### Rust skeleton 最低要求

`mod.rs` 至少要导出：

- `common`
- `activation`
- `execution_mode`
- `memory`

`common.rs` 至少定义：

- `RuntimeEventEnvelope`
- `RuntimeEventType`
- `SchemaVersion`

`activation.rs` 至少定义：

- `ActivationStatus`
- `ActivationSnapshot`

`execution_mode.rs` 至少定义：

- `ExecutionMode`
- `RiskLevel`
- `ComplexityLevel`
- `ExecutionModeDecision`

`memory.rs` 至少定义：

- `MemoryKind`
- `MemoryScope`
- `MemoryDecision`
- `MemoryProjection`

### TS skeleton 最低要求

[contracts.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/transport/contracts.ts:1) 至少要与 Rust 对齐导出：

- `RuntimeEventEnvelope`
- `ActivationStatus`
- `ActivationSnapshot`
- `ExecutionMode`
- `ExecutionModeDecision`
- `MemoryProjection`

### 实施规则

1. 先建类型，再考虑 serializer。
2. 先建稳定命名，再考虑 legacy mapping。
3. 不要求本 slice 接入所有 commands，但必须有可编译入口。

### 编译要求

必须运行：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

### 风险提示

当前 crate 结构是 `main.rs -> mod modules; -> modules/runtime`。如果新增 contracts 模块后忘记在 `src-tauri/src/modules/runtime/mod.rs` 导出，后续 `M1/M2` 无法稳定引用。

## 7.5 `m0.4` Activation Contract And Boot Truth

### 输出文件

- `src-tauri/src/modules/runtime/contracts/activation.rs`
- `docs/staff-remediation/if2ai-workflow-truth.md`
- `docs/staff-remediation/README.md`

### 必须覆盖的 activation states

1. `checking_local`
2. `needs_activation`
3. `requesting_activation`
4. `pending_approval`
5. `redeeming`
6. `activated`
7. `offline_grace`
8. `expired`
9. `revoked`
10. `deactivated`

### 必须覆盖的 lifecycle actions

1. activation request
2. activation redeem
3. license refresh
4. revoke check
5. deactivate
6. local boot restore

### boot truth 文档必须画清楚的顺序

`startup -> onboarding -> activation gate -> main shell`

每一段都必须说明：

1. 进入条件
2. 退出条件
3. 谁决定跳转
4. 失败如何回流

### 参考引用必须进入文档

- [AppState.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/AppState.swift:33)
- [RootView.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/App/RootView.swift:33)
- [ActivationService.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Services/Activation/ActivationService.swift:32)
- [ActivationLifecycleManager.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Services/Activation/ActivationLifecycleManager.swift:8)

### 不得犯的错误

1. 不要把 activation 当成 settings page 的一个 toggle。
2. 不要遗漏 revoke/expire 回流 gate。
3. 不要把 onboarding 和 activation 混成一个状态机。

### checkpoint

`m0.4` 完成后，必须人工确认 activation contract 已能表示“反激活回退到 gate”这件事。

## 7.6 `m0.5` Execution-Mode Contract And Reason Taxonomy

### 输出文件

- `src-tauri/src/modules/runtime/contracts/execution_mode.rs`
- `src/transport/contracts.ts`
- `docs/staff-remediation/if2ai-workflow-truth.md`
- `docs/staff-remediation/README.md`

### 四个 canonical execution modes

1. `direct_execute`
2. `auto_plan_execute`
3. `plan_then_confirm`
4. `specialized_surface`

### reason taxonomy 最低字段

1. `reason_code`
2. `reason_label`
3. `applies_to_modes`
4. `risk_impact`
5. `complexity_impact`
6. `explainability_text`

### `ExecutionModeDecision` 最低字段

1. `execution_mode`
2. `risk_level`
3. `complexity_level`
4. `complexity_score`
5. `reason_codes`
6. `route_hint`
7. `requires_plan`
8. `classifier_policy_version`
9. `classifier_matched_rule_ids`
10. `classifier_slot_summary`
11. `classifier_ambiguous_escalated`
12. `classifier_escalation_source`

### 必须写进文档的原则

1. `execution_mode` 是 runtime truth。
2. `Chat / Coding / Research / Planning / Review` 是上层 scenario profile。
3. front-end 只能投影 classifier 结果，不能自行判定。

### UClaw 参考引用

- [task_complexity.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/coding/task_complexity.rs:4)
- [PHASE6-AGENT-EXECUTION-PRD-ARCHITECTURE.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/Docs/Migration/Phase%206%20Docs/PHASE6-AGENT-EXECUTION-PRD-ARCHITECTURE.md:149)
- [chat_ws.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/api/routes/chat_ws.rs:744)
- [ScenarioAgentStudioStore.swift](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/UClaw/Core/Store/ScenarioAgentStudioStore.swift:100)

### checkpoint

`m0.5` 完成后，必须人工确认“execution mode”和“scenario profile”没有被重新混成一套概念。

## 7.7 `m0.6` God-File Responsibility Inventory

### 输出文件

正式新增（已落地，2026-04-20）：

- [docs/staff-remediation/m0-god-file-responsibility-inventory.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/m0-god-file-responsibility-inventory.md:1)

并在 [README.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/README.md:1) 链接（已完成）。

### 必盘点文件

- [src/App.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/App.tsx:1) 约 2472 行
- [src/components/ui/chat-ui.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/ui/chat-ui.tsx:1) 约 4736 行
- [src/lib/tauri.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/lib/tauri.ts:1) 约 2071 行
- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1) 约 4034 行

### 每个文件必须盘出的维度

1. current responsibilities
2. responsibilities that belong elsewhere
3. `M1` or `M2` destination module
4. safe extraction order
5. risky entanglements

### 输出格式建议

| file               | current_responsibility                | should_move_to                                 | target_phase | extraction_risk |
| ------------------ | ------------------------------------- | ---------------------------------------------- | ------------ | --------------- |
| `src/lib/tauri.ts` | transport + domain types + IPC helper | `src/transport/contracts.ts` + bridge adapters | `M2`         | high            |

### 不可接受的产出

1. 只写“文件太大”。
2. 不写下一落点。
3. 不写拆分顺序。

## 7.8 `m0.7` Coverage Matrix And Plan Linkage

### 输出文件

- `docs/staff-remediation/README.md`
- `docs/exec-plans/index.md`
- `docs/exec-plans/active/phase-m-remediation-execution-index.md`

### 必做动作

1. 在 staff remediation 目录索引中加入 `M0` 新增产物。
2. 在 exec-plans 总索引中给 `M0` 增加 runbook 链接或明确引用。
3. 在执行总索引中指明 `M0` 开工时必须同时打开本 runbook。

### 验收标准

任一 executor 只看索引就能找到：

1. 总蓝图
2. 子设计
3. 当前 phase YAML
4. 当前 phase runbook

## 8. 交付物检查清单

执行者在提交 `M0` 前，必须逐项勾完：

- [ ] `if2ai-canonical-domain-model.md` 已创建并包含 canonical entity table
- [ ] `if2ai-canonical-domain-model.md` 已包含 naming drift map
- [ ] `if2ai-workflow-truth.md` 已创建并覆盖 16 条 workflow
- [ ] 每条 workflow 都有状态列和入口列
- [ ] Rust contracts skeleton 已建在 `src-tauri/src/modules/runtime/contracts/`
- [ ] `src/transport/contracts.ts` 已建立
- [ ] activation state machine 已进入 contract 语义
- [ ] execution-mode decision fields 已进入 contract 语义
- [ ] `m0-god-file-responsibility-inventory.md` 已完成
- [ ] `docs/staff-remediation/README.md` 已补索引
- [ ] `docs/exec-plans/index.md` 已补 phase/runbook 关系
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` 已通过

## 9. 推荐提交方式

建议按下面顺序拆 commit 或 PR 内逻辑提交：

1. 文档真相提交：`m0.1 + m0.2`
2. contract skeleton 提交：`m0.3 + m0.4 + m0.5`
3. responsibility inventory + plan linkage 提交：`m0.6 + m0.7`

如果团队只允许单 PR，也必须在 PR 描述里按上面三段分区展示。

## 10. 完成标志

`M0` 只有在下面条件同时满足时才能标记为完成：

1. `phase-m0-canonical-contracts-and-truth.yaml` 中所有 slice 已可核验。
2. 本 runbook 第 8 节清单全部完成。
3. `phase-m-remediation-execution-index.md` 中 `M0` 退出条件全部满足。
4. 人工 checkpoint 已通过，不存在"为了推进 M1/M2 而模糊化真相"的情况。

## 11. M0 最终产物清单（M1+ 必须直接消费）

完成时间：2026-04-20。M0 全部 7 个 slice 已落地，以下产物即 M1 / M2 的输入源：

| slice | 产物                                                                                                                                                                                                                                                                                                                                                                                         | 类别                                      |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| M0.1  | [docs/staff-remediation/if2ai-canonical-domain-model.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-canonical-domain-model.md:1)                                                                                                                                                                                                                                       | 文档真相                                  |
| M0.2  | [docs/staff-remediation/if2ai-workflow-truth.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-workflow-truth.md:1)                                                                                                                                                                                                                                                       | 文档真相                                  |
| M0.3  | [src-tauri/src/modules/runtime/contracts/common.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/contracts/common.rs:1) + [src-tauri/src/modules/runtime/contracts/memory.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/contracts/memory.rs:1) + [src/transport/contracts.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/transport/contracts.ts:1) | Rust + TS skeleton                        |
| M0.4  | [src-tauri/src/modules/runtime/contracts/activation.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/contracts/activation.rs:1) + [if2ai-workflow-truth.md §3.17 boot truth](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-workflow-truth.md:1)                                                                                                      | Activation contract + boot truth          |
| M0.5  | [src-tauri/src/modules/runtime/contracts/execution_mode.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/contracts/execution_mode.rs:1) + 对位 TS                                                                                                                                                                                                                       | Execution-mode contract + reason taxonomy |
| M0.6  | [docs/staff-remediation/m0-god-file-responsibility-inventory.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/m0-god-file-responsibility-inventory.md:1)                                                                                                                                                                                                                       | 责任清单（M1 / M2 拆分输入）              |
| M0.7  | 索引收口：[docs/staff-remediation/README.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/README.md:1)、[docs/exec-plans/index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/index.md:1)、[phase-m-remediation-execution-index.md §6.1](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1)、本 runbook §11         | 链接 / 索引                               |

进入 M1 之前，executor 必须能在不读蓝图的前提下从上表直接拿到全部输入；如做不到，说明 M0.7 索引未收口完整，应回到本 phase。
