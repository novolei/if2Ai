# If2Ai Remediation Program And Phase Roadmap

> 将 Staff 总蓝图转换为可执行的阶段化整改计划。
>
> 最后更新: 2026-04-20

## 1. 目标

把总蓝图中的：

- M0 ~ M5
- 90 天路线图
- P0 / P1 / P2 优先级
- 风险与防错

收敛为一个统一的项目级整改计划。

## 2. Program Phases

### Phase A: 可信基线

- 测试门恢复
- 文档真相恢复
- activation contract
- execution-mode contract
- workflow truth

### Phase B: 结构收口

- backend M1
- frontend M2
- 拆 God-file
- worker execution abstraction skeleton

### Phase C: memory / governance

- M3
- M4

### Phase D: 自我进化闭环

- M5

## 3. M0 ~ M5 映射

### M0

- canonical contracts
- canonical domain model
- boot truth
- execution-mode truth

### M1

- backend application service split
- worker contract / registry / execution seam skeleton

### M2

- frontend runtime projection split

### M3

- memory coordinator

### M4

- policy + harness governance
- worker trace / failure / compare 治理接入

### M5

- reflection + strategy promotion

## 4. Priority Buckets

### P0

- canonical domain model
- canonical runtime contract
- workflow truth
- activation contract
- execution-mode contract

### P1

- prompt plan
- policy coordinator
- memory coordinator
- runtime projection stores

### P2

- harness gate
- strategy compare
- deeper product surfaces

## 5. 风险控制

1. feature freeze window 应覆盖 M1/M2 关键切片。
2. 每个 phase 结束都必须有 harness regression。
3. 每个 phase 必须减少至少一个 God-file 的责任。
4. activation 与 execution-mode 不能停留在设计层，必须进入运行时。

## 6. 输出物

本 program 文档之后应继续下钻为：

1. `docs/exec-plans/active/phase-m0-*.yaml`
2. `docs/exec-plans/active/phase-m1-*.yaml`
3. `docs/exec-plans/active/phase-m2-*.yaml`

## 7. 验收

1. 每个 phase 都有清晰输入、输出、验收。
2. 子设计文档与 exec-plan 一一映射。
