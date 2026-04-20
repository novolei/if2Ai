# Phase M Remediation Execution Index

> If2Ai Staff 整改计划的统一执行索引。
>
> 最后更新: 2026-04-20（M0 全部 7 个 slice 已完成，M1 已可入场；M0 产物清单见 §6.1）
> 用途: 指导 executor 按最低风险顺序执行 M0 ~ M5 的全部任务。

## 1. 适用范围

本索引覆盖以下正式 phase YAML：

1. [phase-m0-canonical-contracts-and-truth.yaml](./phase-m0-canonical-contracts-and-truth.yaml)
2. [phase-m1-backend-service-extraction.yaml](./phase-m1-backend-service-extraction.yaml)
3. [phase-m2-frontend-runtime-projection.yaml](./phase-m2-frontend-runtime-projection.yaml)
4. [phase-m3-memory-coordinator.yaml](./phase-m3-memory-coordinator.yaml)
5. [phase-m4-policy-harness-governance.yaml](./phase-m4-policy-harness-governance.yaml)
6. [phase-m5-self-evolution-and-strategy-promotion.yaml](./phase-m5-self-evolution-and-strategy-promotion.yaml)

每个 `Phase M*` 都有一份 executor 运行手册。执行规则如下：

1. 开工当前 phase 前，必须阅读对应 phase runbook。
2. 后续 phase 的 runbook 仅在进入该 phase 前强制阅读。
3. 若当前工作会修改跨 phase contract，再按需补读相邻 phase runbook。

对应 runbook：

- [phase-m0-executor-runbook.md](./phase-m0-executor-runbook.md)
- [phase-m1-executor-runbook.md](./phase-m1-executor-runbook.md)
- [phase-m2-executor-runbook.md](./phase-m2-executor-runbook.md)
- [phase-m3-executor-runbook.md](./phase-m3-executor-runbook.md)
- [phase-m4-executor-runbook.md](./phase-m4-executor-runbook.md)
- [phase-m5-executor-runbook.md](./phase-m5-executor-runbook.md)

## 1.1 超短导航表

| Phase | YAML                                                                                                         | Runbook                                                        | File-level plans                                                                                                                                                                                                                                                                                                                                                                                           |
| ----- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `M0`  | [phase-m0-canonical-contracts-and-truth.yaml](./phase-m0-canonical-contracts-and-truth.yaml)                 | [phase-m0-executor-runbook.md](./phase-m0-executor-runbook.md) | 三份完整：[truth-documents](./phase-m0-truth-documents-file-level-plan.md)（M0.0+M0.1+M0.2）, [runtime-activation-execution-contracts](./phase-m0-runtime-activation-execution-contracts-file-level-plan.md)（M0.3+M0.4+M0.5）, [responsibility-inventory-and-linkage](./phase-m0-responsibility-inventory-and-linkage-file-level-plan.md)（M0.6+M0.7）                                                    |
| `M1`  | [phase-m1-backend-service-extraction.yaml](./phase-m1-backend-service-extraction.yaml)                       | [phase-m1-executor-runbook.md](./phase-m1-executor-runbook.md) | [phase-m1-initial-slices-file-level-plan.md](./phase-m1-initial-slices-file-level-plan.md), [phase-m1-memory-and-stream-file-level-plan.md](./phase-m1-memory-and-stream-file-level-plan.md), [phase-m1-routing-activation-control-plane-file-level-plan.md](./phase-m1-routing-activation-control-plane-file-level-plan.md)                                                                               |
| `M2`  | [phase-m2-frontend-runtime-projection.yaml](./phase-m2-frontend-runtime-projection.yaml)                     | [phase-m2-executor-runbook.md](./phase-m2-executor-runbook.md) | [phase-m2-runtime-projection-foundation-file-level-plan.md](./phase-m2-runtime-projection-foundation-file-level-plan.md), [phase-m2-state-boot-shell-file-level-plan.md](./phase-m2-state-boot-shell-file-level-plan.md), [phase-m2-chat-and-visible-surfaces-file-level-plan.md](./phase-m2-chat-and-visible-surfaces-file-level-plan.md)                                                                 |
| `M3`  | [phase-m3-memory-coordinator.yaml](./phase-m3-memory-coordinator.yaml)                                       | [phase-m3-executor-runbook.md](./phase-m3-executor-runbook.md) | [phase-m3-object-model-and-coordinator-file-level-plan.md](./phase-m3-object-model-and-coordinator-file-level-plan.md), [phase-m3-policy-quality-and-recall-file-level-plan.md](./phase-m3-policy-quality-and-recall-file-level-plan.md), [phase-m3-frontend-reflection-and-tests-file-level-plan.md](./phase-m3-frontend-reflection-and-tests-file-level-plan.md)                                         |
| `M4`  | [phase-m4-policy-harness-governance.yaml](./phase-m4-policy-harness-governance.yaml)                         | [phase-m4-executor-runbook.md](./phase-m4-executor-runbook.md) | [phase-m4-execution-and-report-contracts-file-level-plan.md](./phase-m4-execution-and-report-contracts-file-level-plan.md), [phase-m4-graders-compare-and-corpus-file-level-plan.md](./phase-m4-graders-compare-and-corpus-file-level-plan.md), [phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md](./phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md)                   |
| `M5`  | [phase-m5-self-evolution-and-strategy-promotion.yaml](./phase-m5-self-evolution-and-strategy-promotion.yaml) | [phase-m5-executor-runbook.md](./phase-m5-executor-runbook.md) | [phase-m5-trajectory-failure-and-reflection-file-level-plan.md](./phase-m5-trajectory-failure-and-reflection-file-level-plan.md), [phase-m5-candidate-registry-and-offline-eval-file-level-plan.md](./phase-m5-candidate-registry-and-offline-eval-file-level-plan.md), [phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md](./phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md) |

## 2. 执行者一页纸原则

### 2.1 执行者只需要回答 3 个问题

开工前，executor 必须能明确回答：

1. 我现在处在哪个 phase？
2. 这个 phase 当前唯一要完成的 slice 是什么？
3. 这个 slice 的完成标志是什么？

如果回答不了这 3 个问题，就不应开工。

### 2.2 执行者的最小阅读路径

对 executor 来说，文档读取顺序只能是：

1. 本索引
2. 当前 phase YAML
3. 当前 phase runbook
4. 当前 slice 对应的 file-level plan

蓝图和子设计文档属于“查依据”层，不是“开工入口”层。  
除非当前 slice 明确要求，executor 不应先去通读整套蓝图。

### 2.3 执行者每次只允许一种思维模式

1. `M0`：定真相，不做功能
2. `M1`：拆后端，不重写前端
3. `M2`：建前端投影，不扩 memory
4. `M3`：收 memory，不碰治理发布
5. `M4`：建 gate 和 governance，不做策略 promote
6. `M5`：做 candidate / promotion / rollback，不回头重写基础骨架

若某项工作同时跨越两种思维模式，默认说明 phase 边界已经被破坏，应先停下来复核。

### 2.4 半成品预警信号

出现以下任意 2 项，必须暂停并人工复核：

1. 新旧路径同时存在，但没有明确退场计划
2. contract 改了，但 UI、文档、脚本没有一起改
3. 当前 phase 尚未退出，就提前做下一个 phase
4. 为了推进进度，把 `partial` 写成 `canonical`
5. 改动后只能说“结构更好了”，却说不清系统具体哪里更稳

## 3. 总体执行顺序

严格顺序如下，不允许跳 phase：

1. `M0`
2. `M1`
3. `M2`
4. `M3`
5. `M4`
6. `M5`

原因：

1. `M0` 提供所有后续 phase 的真相与契约基线。
2. `M1` 先拆后端，避免前端先围绕旧 command 层继续固化错误边界。
3. `M2` 再建立前端投影层，消费 `M0/M1` 输出的 contracts 与 services。
4. `M3` 必须建立在 runtime contracts 与 frontend stores 已经稳定的前提下。
5. `M4` 必须建立在 memory/policy trace 可采集的前提下。
6. `M5` 必须建立在 harness compare 与 gate 已经具备裁决能力的前提下。

## 4. 执行红线

### 4.1 禁止跳过

禁止从 `M0` 直接做 `M2/M3/M4/M5`。

### 4.2 禁止并行的高风险项

以下项默认禁止并行：

1. `M1` 中 `agent.rs` service extraction 与 `M2` 中 `App.tsx` / `chat-ui.tsx` 投影重构
2. `M3` memory coordinator 与 `M4` harness governance gate
3. `M4` gate rule 改造 与 `M5` strategy promotion

原因：

这些工作同时推进时，最容易造成 contract 漂移或双重真相。

### 4.3 允许局部并行的安全项

以下可在同一 phase 内适度并行：

1. 文档真相更新与 contract skeleton 建设
2. backend service 抽离与 frontend types/translator 骨架
3. memory 前端投影与 memory 后端对象模型

前提：写集合不冲突，且不违反上面的高风险禁并规则。

## 5. 每个 Phase 的进入条件

### M0 进入条件

- 已有总蓝图和子设计文档
- 已阅读 `phase-m0-executor-runbook.md`
- 无需额外前置

### M1 进入条件

- `M0` 完成
- canonical domain model / workflow truth / contracts 已存在
- 已阅读 `phase-m1-executor-runbook.md`

### M2 进入条件

- `M1` 完成
- 后端 service 与 contracts 至少已有骨架
- 已阅读 `phase-m2-executor-runbook.md`

### M3 进入条件

- `M2` 完成
- frontend projection stores 已存在
- memory contract 已稳定
- 已阅读 `phase-m3-executor-runbook.md`

### M4 进入条件

- `M3` 完成
- memory decision / execution-mode / policy decisions 已能被 trace 采集
- 已阅读 `phase-m4-executor-runbook.md`

### M5 进入条件

- `M4` 完成
- compare / gate / recommendation 已具备基础能力
- 已阅读 `phase-m5-executor-runbook.md`

## 6. 每个 Phase 的退出条件

### M0 退出条件

1. canonical domain model 已落文档
2. workflow truth 已落文档
3. runtime / activation / execution-mode contracts 已有正式入口
4. God-file responsibility inventory 已完成

### 6.1 M0 已落地产物清单（M1+ 必须直接消费）

完成时间：2026-04-20。M1 / M2 executor 不得回头改写以下文件（如需改写，必须重新打开 M0 phase）：

- [docs/staff-remediation/if2ai-canonical-domain-model.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-canonical-domain-model.md:1) — M0.1 canonical 实体 / 命名映射
- [docs/staff-remediation/if2ai-workflow-truth.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-workflow-truth.md:1) — M0.2 workflow 真相 + M0.4 boot truth 状态机
- [src-tauri/src/modules/runtime/contracts/](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/contracts/mod.rs:1) — M0.3+M0.4+M0.5 Rust skeleton（`common / activation / execution_mode / memory`）
- [src/transport/contracts.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/transport/contracts.ts:1) — M0.3+M0.4+M0.5 TS twin
- [docs/staff-remediation/m0-god-file-responsibility-inventory.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/m0-god-file-responsibility-inventory.md:1) — M0.6 四大 god-file 责任清单（M1 / M2 拆分输入）

输入指引：
- M1 executor 入场时必须先读 §6.1 中的 canonical domain model + workflow truth + Rust contracts skeleton + god-file inventory（`commands/agent.rs` 段）。
- M2 executor 入场时必须先读 §6.1 中的 canonical domain model + workflow truth + TS contracts twin + god-file inventory（`App.tsx / chat-ui.tsx / tauri.ts` 段）。

### M1 退出条件

1. `commands/agent.rs` 明显瘦身
2. provider / prompt / memory injection / stream emitter / request intelligence / activation services 已出现
3. control-plane seam 已存在

### M2 退出条件

1. `tauri.ts` 回归 thin bridge
2. translator / reducer / stores 已建立
3. `App.tsx` 与 `chat-ui.tsx` 责任明显下降
4. activation gate 与 execution-mode explainability 进入正式 UI

### M3 退出条件

1. MemoryCoordinator 出现
2. WritePolicy / QualityGate / RecallAssembler 出现
3. memory 前后端都能解释为什么写、为什么不写、为什么召回

### M4 退出条件

1. `prepare_step_execution` 进入真实链路
2. HarnessRunReport / graders / blocker rules 存在
3. baseline-candidate compare 与 gate 规则存在

### M5 退出条件

1. candidate strategy registry 存在
2. reflection -> candidate -> compare -> promote/hold/reject 闭环成立
3. rollback path 存在

## 7. 推荐执行节奏

### Wave 1

- 先完整做完 `M0`
- 人工 checkpoint 审核 contracts / truth / naming

### Wave 2

- 完整做完 `M1`
- 不启动 `M2` 之前，先检查 `agent.rs` 是否真的降责

### Wave 3

- 做 `M2`
- 检查 `App.tsx`、`chat-ui.tsx`、`tauri.ts` 是否都明显收缩

### Wave 4

- 做 `M3`
- 只在 memory contracts 稳定后再推进治理层

### Wave 5

- 做 `M4`
- 建立 gate 后再允许进入 `M5`

### Wave 6

- 做 `M5`
- 只允许 candidate strategy 走 compare + gate，不允许直接 promote

## 8. Executor 操作规则

1. 每次只激活一个 `phase_status: active` 的 M-phase。
2. 完成一个 phase 后，再切换到下一个。
3. 每个 slice 完成后，必须核对：
   - 设计文档引用是否满足
   - acceptance 是否可运行或可人工核查
   - review checklist 是否逐项通过
4. 若某 slice 需要改写 contract，必须回写 `M0` 产物和对应子设计文档。
5. 不允许在同一轮执行里同时追两个 slice 的“部分完成”状态；要么完成当前 slice，要么显式标记 blocked。
6. 若 executor 打开当前文档 5 分钟后仍说不清“下一步改哪个文件”，应退回到当前 slice 的 file-level plan，而不是继续扩读更多设计文档。

## 9. 人工检查点

建议的强制人工 checkpoint：

1. `M0.2` 后：确认 truth 与 naming 无歧义
2. `M0.5` 后：确认 activation / execution-mode contracts 成熟
3. `M1.5` 后：确认 stream emitter 与 legacy 兼容不破主路径
4. `M2.6` 后：确认 activation gate boot flow 正常
5. `M3.5` 后：确认 recall 顺序与 usefulness 口径
6. `M4.5` 后：确认 baseline/candidate compare 可用
7. `M5.5` 后：确认 promote/hold/reject 不会误升策略

## 10. 失败回退策略

若某 phase 中途失败：

1. 不推进下一个 phase
2. 记录失败点与受影响 contract
3. 将当前 phase 标记为 blocked 或 draft-rework
4. 在修复前禁止开始后续依赖 phase

特别说明：

- `M1` 失败时，禁止做 `M2`
- `M4` 失败时，禁止做 `M5`

## 11. 与现有文档的关系

本索引是执行层总导航，不替代：

1. 总蓝图
2. 子设计文档
3. 单 phase YAML

它的职责只有一个：

> 让 executor 明确知道什么先做、什么不能并行、什么没有通过前不能继续做。
