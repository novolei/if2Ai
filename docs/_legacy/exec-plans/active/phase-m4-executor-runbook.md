# Phase M4 Executor Runbook

> 将 `phase-m4-policy-harness-governance.yaml` 细化为 executor 可直接执行的治理层手册。
>
> 最后更新: 2026-04-21（M4 doc reconciliation closeout —
> backend 治理链 m4.1 ~ m4.5 / m4.7 / m4.8 已 done；m4.6 前端 governance UI 显式延后；M5 入场已解锁）

## 1. 目的

`M4` 的任务是让 If2Ai 从“有观测”进入“可治理”：

`prepare_step_execution -> trace -> run report -> compare -> blocker -> gate`

## 2. 本阶段当前真实状态（M4 doc reconciliation 后）

`M4` 现在的官方完成口径是 **backend complete, frontend governance deferred**。

后端治理链已完成并通过 IPC 暴露：

1. `prepare_step_execution` 已进入真实执行链（m4.1）。
2. `HarnessRunReport` / `EvidenceBundle` / grader / blocker 契约已出现（m4.2 / m4.3 / m4.4）。
3. `BaselineVsCandidate` compare 契约 + `harness_compare_reports` IPC 已出现（m4.5）。
4. `RegressionCorpus` / `SuiteReport` / `aggregate_suite_report` IPC 已出现（m4.7）。
5. `GatePolicy` / `GateDecision` / `Recommendation` + `harness_evaluate_compare` / `harness_evaluate_suite` IPC 已出现，默认 conservative（m4.8）。

显式延后（不再视为 phase blocker）：

- `m4.6` 前端 governance dashboard / 治理 store / settings 治理 section。建议拆到独立 follow-on phase（例如 `phase-m4f-governance-dashboard-frontend`），不得在 M5 slice 内顺手实现。
- auto-promote 与 rollback automation（属于 M5 的 candidate 闭环范围，但 M5 入场时不得预设其已存在）。
- per-corpus-tier weighted policy。
- closed-set evidence_ref scheme（M4.5 报告中 evidence_ref 仍是 free-form 字符串）。
- 五层语料 YAML 全量填充（契约 + 聚合可用即可，语料填充视为 enhancement）。

治理契约层级（必须按此口径理解，不要再写成 “Recommendation 是 HarnessRunReport 的字段”）：

| 契约                  | 模块                                            | 角色                                |
| --------------------- | ----------------------------------------------- | ----------------------------------- |
| `HarnessRunReport`    | `src-tauri/src/modules/harness/run_report.rs`   | run-level trace/report              |
| `BaselineVsCandidate` | `src-tauri/src/modules/harness/compare.rs`      | 两份 run report 之间的 diff         |
| `RegressionCorpus`    | `src-tauri/src/modules/harness/corpus.rs`       | 语料输入                            |
| `SuiteReport`         | `src-tauri/src/modules/harness/suite_report.rs` | corpus/suite 聚合输出               |
| `Recommendation`      | `src-tauri/src/modules/harness/gate.rs`         | gate 决策输出（与上述并列，非内嵌） |

## 3. 读取顺序

### 3.1 执行者最小必读

执行者开工 `M4` 时，只要求先读下面 5 个入口：

1. [phase-m-remediation-execution-index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1)
2. [phase-m4-policy-harness-governance.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-policy-harness-governance.yaml:1)
3. 本 runbook
4. 当前 slice 对应的 file-level plan
5. 当前 slice 明确引用的真实代码入口文件

### 3.2 `M4` file-level plan 入口

1. [phase-m4-execution-and-report-contracts-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-execution-and-report-contracts-file-level-plan.md:1)
2. [phase-m4-graders-compare-and-corpus-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-graders-compare-and-corpus-file-level-plan.md:1)
3. [phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md:1)

### 3.3 设计依据按需查阅

只有当当前 slice 需要核对治理口径或执行后端边界时，再回查以下设计文档：

1. [harness-v2-governance-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/harness-v2-governance-design.md:1)
2. [backend-application-control-plane-refactor-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/backend-application-control-plane-refactor-design.md:1)
3. [if2ai-worker-adoption-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-worker-adoption-design.md:1)

## 4. 严格执行顺序（历史口径，仅供回溯）

> M4 reconciliation 之后，本节顺序是历史顺序，不是当前要执行的待办。
> 当前 M4 已落地到 “backend complete, frontend governance deferred”，
> 本节保留以便回看每个 slice 的产出时机。

1. `m4.0` preflight governance inventory — done
2. `m4.1` prepare_step_execution real path — done
3. `m4.2` run report contracts — done
4. `m4.3` graders + blockers — done
5. `m4.4` evidence trace wiring — done
6. `m4.5` compare flow — done
7. `m4.6` governance UI — **deferred**（不得在 M5 顺手做）
8. `m4.7` regression corpus — done（契约 + IPC + 聚合；YAML 语料填充视为 enhancement）
9. `m4.8` gate policy upgrades — done（默认 conservative）

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

## 6. 交付物检查清单（reconciliation 后的真实状态）

- [x] `prepare_step_execution` 已进入真实链
- [x] HarnessRunReport contract 已出现
- [x] graders/blockers 已出现
- [x] compare flow 已出现（`BaselineVsCandidate` + `harness_compare_reports`）
- [ ] governance UI 已出现 — **显式延后**，不阻塞 M5；如需推进请走独立 follow-on phase
- [x] regression corpus 已分层（5 个 tier 闭集 + suite 聚合 + IPC，YAML 语料填充视为 enhancement）
- [x] gate 规则已写入文档/代码语义（`GatePolicy::default()` conservative、`harness_evaluate_*` IPC）
- [x] 已按 3 份文件级任务单完成 `m4.1 ~ m4.5 / m4.7 / m4.8`；`m4.6` 见上方 deferred 项

## 7. 推荐提交顺序

1. `m4.1 + m4.2`
2. `m4.3 + m4.4 + m4.5 + m4.7`
3. `m4.6 + m4.8`

对应文件级任务单：

1. [phase-m4-execution-and-report-contracts-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-execution-and-report-contracts-file-level-plan.md:1)
2. [phase-m4-graders-compare-and-corpus-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-graders-compare-and-corpus-file-level-plan.md:1)
3. [phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-governance-surfaces-and-upgrade-gates-file-level-plan.md:1)

## 8. 完成标志

`M4` 当前已被官方定义为 **backend complete, frontend governance deferred**。
判断依据：

1. 团队可以用统一口径回答 “这个 candidate 为什么能升、为什么不能升”——通过 `harness_compare_reports` + `harness_evaluate_compare` / `harness_evaluate_suite` IPC 的 `Recommendation` 输出。
2. 缺少 recommendation / compare 时，`GatePolicy::default()` 强制返回 `Hold` 或 `Reject`，不会 `Promote`。
3. 治理表面（governance UI）的缺位仅影响人类回看体验，不影响 candidate 闭环裁决，因此 M5 入场不被阻塞。
