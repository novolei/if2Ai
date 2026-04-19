# Phase M Remediation Execution Index

> If2Ai Staff 整改计划的统一执行索引。
>
> 最后更新: 2026-04-20
> 用途: 指导 executor 按最低风险顺序执行 M0 ~ M5 的全部任务。

## 1. 适用范围

本索引覆盖以下正式 phase YAML：

1. [phase-m0-canonical-contracts-and-truth.yaml](./phase-m0-canonical-contracts-and-truth.yaml)
2. [phase-m1-backend-service-extraction.yaml](./phase-m1-backend-service-extraction.yaml)
3. [phase-m2-frontend-runtime-projection.yaml](./phase-m2-frontend-runtime-projection.yaml)
4. [phase-m3-memory-coordinator.yaml](./phase-m3-memory-coordinator.yaml)
5. [phase-m4-policy-harness-governance.yaml](./phase-m4-policy-harness-governance.yaml)
6. [phase-m5-self-evolution-and-strategy-promotion.yaml](./phase-m5-self-evolution-and-strategy-promotion.yaml)

每个 `Phase M*` 都有一份 executor 运行手册，开工前必须同时阅读：

- [phase-m0-executor-runbook.md](./phase-m0-executor-runbook.md)
- [phase-m1-executor-runbook.md](./phase-m1-executor-runbook.md)
- [phase-m2-executor-runbook.md](./phase-m2-executor-runbook.md)
- [phase-m3-executor-runbook.md](./phase-m3-executor-runbook.md)
- [phase-m4-executor-runbook.md](./phase-m4-executor-runbook.md)
- [phase-m5-executor-runbook.md](./phase-m5-executor-runbook.md)

## 2. 总体执行顺序

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

## 3. 执行红线

### 3.1 禁止跳过

禁止从 `M0` 直接做 `M2/M3/M4/M5`。

### 3.2 禁止并行的高风险项

以下项默认禁止并行：

1. `M1` 中 `agent.rs` service extraction 与 `M2` 中 `App.tsx` / `chat-ui.tsx` 投影重构
2. `M3` memory coordinator 与 `M4` harness governance gate
3. `M4` gate rule 改造 与 `M5` strategy promotion

原因：

这些工作同时推进时，最容易造成 contract 漂移或双重真相。

### 3.3 允许局部并行的安全项

以下可在同一 phase 内适度并行：

1. 文档真相更新与 contract skeleton 建设
2. backend service 抽离与 frontend types/translator 骨架
3. memory 前端投影与 memory 后端对象模型

前提：写集合不冲突，且不违反上面的高风险禁并规则。

## 4. 每个 Phase 的进入条件

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

## 5. 每个 Phase 的退出条件

### M0 退出条件

1. canonical domain model 已落文档
2. workflow truth 已落文档
3. runtime / activation / execution-mode contracts 已有正式入口
4. God-file responsibility inventory 已完成

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

## 6. 推荐执行节奏

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

## 7. Executor 操作规则

1. 每次只激活一个 `phase_status: active` 的 M-phase。
2. 完成一个 phase 后，再切换到下一个。
3. 每个 slice 完成后，必须核对：
   - 设计文档引用是否满足
   - acceptance 是否可运行或可人工核查
   - review checklist 是否逐项通过
4. 若某 slice 需要改写 contract，必须回写 `M0` 产物和对应子设计文档。

## 8. 人工检查点

建议的强制人工 checkpoint：

1. `M0.2` 后：确认 truth 与 naming 无歧义
2. `M0.5` 后：确认 activation / execution-mode contracts 成熟
3. `M1.5` 后：确认 stream emitter 与 legacy 兼容不破主路径
4. `M2.6` 后：确认 activation gate boot flow 正常
5. `M3.5` 后：确认 recall 顺序与 usefulness 口径
6. `M4.5` 后：确认 baseline/candidate compare 可用
7. `M5.5` 后：确认 promote/hold/reject 不会误升策略

## 9. 失败回退策略

若某 phase 中途失败：

1. 不推进下一个 phase
2. 记录失败点与受影响 contract
3. 将当前 phase 标记为 blocked 或 draft-rework
4. 在修复前禁止开始后续依赖 phase

特别说明：

- `M1` 失败时，禁止做 `M2`
- `M4` 失败时，禁止做 `M5`

## 10. 与现有文档的关系

本索引是执行层总导航，不替代：

1. 总蓝图
2. 子设计文档
3. 单 phase YAML

它的职责只有一个：

> 让 executor 明确知道什么先做、什么不能并行、什么没有通过前不能继续做。
