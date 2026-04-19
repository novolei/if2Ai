# Phase M5 Executor Runbook

> 将 `phase-m5-self-evolution-and-strategy-promotion.yaml` 细化为 executor 可直接执行的自我进化闭环手册。
>
> 最后更新: 2026-04-20

## 1. 目的

`M5` 的目标不是让系统“自动改自己”，而是建立一个受治理的闭环：

`trace -> reflection -> candidate strategy -> offline compare -> promote/hold/reject -> rollback`

## 2. 本阶段必须产出的结果

1. trajectory scoring 存在
2. failure clustering 存在
3. reflection note 结构化
4. candidate strategy registry 存在
5. promote/hold/reject/rollback 语义受 harness gate 约束

## 3. 读取顺序

1. [phase-m5-self-evolution-and-strategy-promotion.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-self-evolution-and-strategy-promotion.yaml:1)
2. [memory-self-evolution-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/memory-self-evolution-design.md:1)
3. [harness-v2-governance-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/harness-v2-governance-design.md:1)
4. [phase-m5-trajectory-failure-and-reflection-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-trajectory-failure-and-reflection-file-level-plan.md:1)
5. [phase-m5-candidate-registry-and-offline-eval-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-candidate-registry-and-offline-eval-file-level-plan.md:1)
6. [phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md:1)

## 4. 严格执行顺序

1. `m5.0` preflight evolution inventory
2. `m5.1` trajectory scoring
3. `m5.2` failure clustering
4. `m5.3` reflection notes
5. `m5.4` candidate registry
6. `m5.5` offline eval
7. `m5.6` gated promotion + rollback
8. `m5.7` diagnostics surfaces
9. `m5.8` safety checks

## 5. Slice 详细执行说明

## 5.1 `m5.0` Preflight Evolution Inventory

### 必查文件

- [src-tauri/src/modules/learning/](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/learning/mod.rs:1)
- [src-tauri/src/modules/harness/](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/mod.rs:1)

## 5.2 `m5.1` Trajectory Scoring

### 必须落实

1. score 不是只看 token/latency
2. 要包含完成率、恢复质量、memory 对齐信号

## 5.3 `m5.2` Failure Clustering

### 最低分类

1. intent miss
2. policy issue
3. memory issue
4. recovery issue
5. tool misuse

## 5.4 `m5.3` Reflection Notes

### 最低字段

1. `issue_type`
2. `evidence`
3. `proposed_strategy`
4. `expected_gain`
5. `risk_level`

## 5.5 `m5.4` Candidate Registry

### 必须落实

1. `draft / candidate / active / rollback`
2. 来源可追踪：reflection / human feedback / curated rule

## 5.6 `m5.5` Offline Eval

### 必须落实

1. candidate strategy 必须经过 harness compare
2. 输出 `promote / hold / reject`

### checkpoint

`m5.5` 后必须确认至少一条 candidate 完成 compare。

## 5.7 `m5.6` Gated Promotion + Rollback

### 必须落实

1. 单次成功样本不能直接升级默认策略
2. rollback reason 与 rollback path 可追踪

## 5.8 `m5.7` Diagnostics Surfaces

### 必须落实

1. candidate / active / rollback 状态可见
2. compare 结果和最近 recommendation 可见

## 5.9 `m5.8` Safety Checks

### 必须覆盖

1. candidate generation tests
2. promote tests
3. rollback tests
4. 至少一条错误推广被 gate 拦下的案例

## 6. 交付物检查清单

- [ ] trajectory scoring 已出现
- [ ] failure clustering 已出现
- [ ] reflection notes 已结构化
- [ ] candidate registry 已出现
- [ ] offline eval 已可跑
- [ ] gated promotion 已出现
- [ ] rollback path 已出现
- [ ] safety/regression cases 已出现
- [ ] 已按 3 份文件级任务单完成 `m5.1 ~ m5.8`

## 7. 推荐提交顺序

1. `m5.1 + m5.2 + m5.3`
2. `m5.4 + m5.5`
3. `m5.6 + m5.7 + m5.8`

对应文件级任务单：

1. [phase-m5-trajectory-failure-and-reflection-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-trajectory-failure-and-reflection-file-level-plan.md:1)
2. [phase-m5-candidate-registry-and-offline-eval-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-candidate-registry-and-offline-eval-file-level-plan.md:1)
3. [phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md:1)

## 8. 完成标志

只有当系统能“受约束地学习”而不是“随机漂移”时，`M5` 才算完成。
