# Phase M4 Executor Runbook

> 将 `phase-m4-policy-harness-governance.yaml` 细化为 executor 可直接执行的治理层手册。
>
> 最后更新: 2026-04-20

## 1. 目的

`M4` 的任务是让 If2Ai 从“有观测”进入“可治理”：

`prepare_step_execution -> trace -> run report -> compare -> blocker -> gate`

## 2. 本阶段必须产出的结果

1. `prepare_step_execution` 进入真实执行链。
2. HarnessRunReport / grader / blocker / recommendation contracts 出现。
3. baseline-candidate compare 成立。
4. policy / memory / classifier 变更必须经过治理 gate。

## 3. 读取顺序

1. [phase-m4-policy-harness-governance.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-policy-harness-governance.yaml:1)
2. [harness-v2-governance-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/harness-v2-governance-design.md:1)
3. [backend-application-control-plane-refactor-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/backend-application-control-plane-refactor-design.md:1)
4. [if2ai-worker-adoption-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-worker-adoption-design.md:1)
5. [phase-m4-execution-and-report-contracts-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-execution-and-report-contracts-file-level-plan.md:1)
6. [phase-m4-graders-compare-and-corpus-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-graders-compare-and-corpus-file-level-plan.md:1)
7. [phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md:1)

## 4. 严格执行顺序

1. `m4.0` preflight governance inventory
2. `m4.1` prepare_step_execution real path
3. `m4.2` run report contracts
4. `m4.3` graders + blockers
5. `m4.4` evidence trace wiring
6. `m4.5` compare flow
7. `m4.6` governance UI
8. `m4.7` regression corpus
9. `m4.8` gate policy upgrades

## 5. Slice 详细执行说明

## 5.1 `m4.0` Preflight Governance Inventory

### 必查文件

- [src-tauri/src/modules/harness/](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/mod.rs:1)
- [harness/](/Users/ryanliu/Documents/IfAI/if2Ai/harness/README.md:1)
- [src-tauri/src/modules/control_plane/](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/mod.rs:1)

## 5.2 `m4.1` Prepare Step Execution

### 必须落实

1. boundary / permission / sandbox 三层联动
2. deny / approve / escalate 都有正式 path

## 5.3 `m4.2` Run Report Contracts

### 最低结构

1. `HarnessRunReport`
2. `TaskRunResult`
3. `AggregateMetrics`
4. `BlockingFailures`
5. `Recommendation`

## 5.4 `m4.3` Graders + Blockers

### skeleton 最少覆盖

1. `TaskSuccess`
2. `PermissionCompliance`
3. `MemoryAlignment`
4. `RecoveryResilience`
5. `ResourceEfficiency`

### 阻断规则必须明确

不能只是口头标准。

## 5.5 `m4.4` Evidence Trace Wiring

### 必须记录

1. execution_mode / reason_codes / route_hint
2. memory write / recall decisions
3. policy decisions

## 5.6 `m4.5` Compare Flow

### 必须落实

1. baseline 与 candidate 跑同一语料
2. 输出 diff 和 blocker

### checkpoint

`m4.5` 后必须确认 compare 不是单样本比较。

## 5.7 `m4.6` Governance UI

### 必须落实

1. 开发者观测面和治理面分离
2. run report / compare / blocker 可回看

## 5.8 `m4.7` Regression Corpus

### 必须建立分层

1. smoke
2. critical path
3. memory-sensitive
4. tool-risk
5. resume-recovery

## 5.9 `m4.8` Gate Policy Upgrades

### 必须落实

1. `promote / hold / reject` 条件写死
2. 没有 harness recommendation 不可 promote

## 6. 交付物检查清单

- [ ] `prepare_step_execution` 已进入真实链
- [ ] HarnessRunReport contract 已出现
- [ ] graders/blockers 已出现
- [ ] compare flow 已出现
- [ ] governance UI 已出现
- [ ] regression corpus 已分层
- [ ] gate 规则已写入文档/代码语义
- [ ] 已按 3 份文件级任务单完成 `m4.1 ~ m4.8`

## 7. 推荐提交顺序

1. `m4.1 + m4.2`
2. `m4.3 + m4.4 + m4.5 + m4.7`
3. `m4.6 + m4.8`

对应文件级任务单：

1. [phase-m4-execution-and-report-contracts-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-execution-and-report-contracts-file-level-plan.md:1)
2. [phase-m4-graders-compare-and-corpus-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-graders-compare-and-corpus-file-level-plan.md:1)
3. [phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md:1)

## 8. 完成标志

只有当团队可以用统一口径回答“这个 candidate 为什么能升、为什么不能升”时，`M4` 才算完成。
