# Phase M5 Executor Runbook

> 将 `phase-m5-self-evolution-and-strategy-promotion.yaml` 细化为 executor 可直接执行的自我进化闭环手册。
>
> 最后更新: 2026-04-21（M5 closeout：每个 slice 已 done，详见下方 delivery 表）

## -1. 当前 delivery 状态（M5 closeout）

**Phase M5 整体 done。**

| Slice | 范围                                                                                                                                                        | 状态 |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| m5.1  | `trajectory_score.rs`（5 轴 + 复合 + compare 评分）                                                                                                         | done |
| m5.2  | `failure_taxonomy.rs` + `failure_clustering.rs`（闭集 + cluster pipeline）                                                                                  | done |
| m5.3  | `reflection_generator.rs`（trajectory + cluster 驱动的真实 producer；legacy adapter 保留为 explicit 路径）                                                  | done |
| m5.4  | `strategy_registry`（含 history 栈 + `StrategyDefinition` + `SupersedeRecord`）                                                                             | done |
| m5.5  | `candidate_evaluator`（compare-pair + suite-pair 双入口 + cross-store 校验 + step 级 idempotency）                                                          | done |
| m5.6  | `promotion_gate.PromotionBasis`（默认 `RequireBoth`）+ `strategy_rollout` singleton-active + supersede + rollback 校验 + active overlay 接入 prompt planner | done |
| m5.7  | `src/modules/settings/pages/StrategyDiagnosticsPage.tsx`（真实 IPC 消费，settings nav 已挂）                                                                | done |
| m5.8  | `src-tauri/tests/m5_self_evolution_integration.rs` 跨链路 + 107 lib 单测                                                                                    | done |

**production active flip 真实闭环路径**：
`candidate.definition (StrategyDefinition)` → `ActiveStrategyOverlayResolver` →
`ActiveStrategyOverlay` → `BuildPromptPlanRequest.active_strategy_overlay` →
`PromptBlockKind::ActiveStrategyOverlay` 块 → 真实 turn 的 system prompt。
仅 `application/turn_service.rs` 单 hook 接入，**不**重写 `commands/agent.rs` 主链。

**M5 完成后的 enhancement（非阻塞）** 见 yaml `notes` 段：自动触发 reflection /
executable strategy DSL / per-corpus-tier weighted gate / closed-set
evidence_ref scheme / auto-promote / auto-rollback automation。

## 0. M5 Readiness Calibration（开工前必读）

M5 入场建立在 **M4 backend complete, frontend governance deferred** 这一真实状态上。

### 0.1 M5 可直接复用的输入契约（已实现）

| 契约                  | 模块                                                | 主要 IPC                                                                                                                            |
| --------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `HarnessRunReport`    | `src-tauri/src/modules/harness/run_report.rs`       | `harness_finalize_run` / `harness_finalize_and_rotate_run` / `harness_load_report` / `harness_save_report` / `harness_list_reports` |
| `BaselineVsCandidate` | `src-tauri/src/modules/harness/compare.rs`          | `harness_compare_reports`                                                                                                           |
| `RegressionCorpus`    | `src-tauri/src/modules/harness/corpus.rs`           | `harness_load_corpus`                                                                                                               |
| `SuiteReport`         | `src-tauri/src/modules/harness/suite_report.rs`     | `harness_aggregate_suite_report`                                                                                                    |
| `Recommendation`      | `src-tauri/src/modules/harness/gate.rs`             | `harness_evaluate_compare` / `harness_evaluate_suite`                                                                               |
| `ReflectionNote`      | `src-tauri/src/modules/learning/reflection_note.rs` | （内嵌契约，未独立 IPC）                                                                                                            |

四个治理契约 + Recommendation 是平行关系。`Recommendation` **不是** `HarnessRunReport` 的字段——它由 gate 在消费 `BaselineVsCandidate` 或 `SuiteReport` 后渲染出来。M5 candidate eval 必须复用 `harness_evaluate_compare` / `harness_evaluate_suite`，不要在 M5 自己重新写一套裁决规则。

### 0.2 M5 不得假设已存在的能力

以下被 M4 显式延后或视为后续 enhancement，M5 slice 不得默认它们已就绪：

1. 前端 governance dashboard / 治理 store / settings 治理 section（m4.6 deferred）。
2. auto-promote / auto-rollback automation。
3. per-corpus-tier weighted policy（当前 `GatePolicy` 单一默认 conservative）。
4. closed-set evidence_ref scheme（当前 `evidence_ref` 仍是 free-form 字符串）。
5. 五层语料 YAML 全量填充（契约 + 聚合 + IPC 已就绪，YAML 内容补齐视为 enhancement）。

### 0.3 遇到上述空缺的处置

如 M5 slice 必须依赖以上未完成项才能推进，**停止该 slice**，开 follow-on phase 处理后再继续：

- 前端治理面 → 建议命名 `phase-m4f-governance-dashboard-frontend`
- gate policy 加权 / closed-set evidence_ref → 建议命名 `phase-m4g-gate-policy-enhancement`
- 语料填充 → 建议命名 `phase-m4h-regression-corpus-content`

不允许在 M5 slice 内顺手补做这些事——会破坏 phase 边界与 contract 漂移控制。

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

### 3.1 执行者最小必读

执行者开工 `M5` 时，只要求先读下面 5 个入口：

1. [phase-m-remediation-execution-index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1)
2. [phase-m5-self-evolution-and-strategy-promotion.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-self-evolution-and-strategy-promotion.yaml:1)
3. 本 runbook
4. 当前 slice 对应的 file-level plan
5. 当前 slice 明确引用的真实代码入口文件

### 3.2 `M5` file-level plan 入口

1. [phase-m5-trajectory-failure-and-reflection-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-trajectory-failure-and-reflection-file-level-plan.md:1)
2. [phase-m5-candidate-registry-and-offline-eval-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-candidate-registry-and-offline-eval-file-level-plan.md:1)
3. [phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m5-promotion-rollback-and-diagnostics-file-level-plan.md:1)

### 3.3 设计依据按需查阅

只有当当前 slice 需要核对学习闭环或治理规则时，再回查以下设计文档：

1. [memory-self-evolution-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/memory-self-evolution-design.md:1)
2. [harness-v2-governance-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/harness-v2-governance-design.md:1)

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

### 已落实（M5-A done）

1. `Draft / Candidate / Compared / Recommended / Rejected / Deprecated` 闭集（M5-A）。
2. `PromotionReady / PromotionBlocked / PromotedCandidate` 闭集扩展（M5-B）。
3. `Active / RolledBack` 闭集扩展 + rollback audit chain（M5-C 第一轮，本轮交付）。
4. 来源可追踪：`StrategySource = reflection / manual / curated_rule / other`（M5-A）。

### 落点

- `src-tauri/src/modules/learning/strategy_registry.rs`
- `src-tauri/src/modules/learning/strategy_registry_store.rs`
- `src-tauri/src/modules/learning/strategy_registry_service.rs`

## 5.6 `m5.5` Offline Eval

### 必须落实

1. candidate strategy 必须经过 harness compare
2. 输出 `promote / hold / reject`

### checkpoint

`m5.5` 后必须确认至少一条 candidate 完成 compare。

## 5.7 `m5.6` Gated Promotion + Rollback

### Promotion skeleton（M5-B done）

1. `promotion_gate.rs::check_eligibility` + `PromotionGateService.apply_eligibility / mark_promoted_candidate`
2. 硬规则：缺 Recommendation / 缺 compare / decision != promote / stale recommendation / operator terminal 全部 Blocked
3. `PromotedCandidate` 仅是 registry 标记 —— 仍非 production active

### Rollback + Active（M5-C 第一轮，本轮交付）

1. `Active / RolledBack` 进入 RolloutState 闭集
2. rollback audit chain：`rollback_reason / rollback_target / rollback_initiated_by / rollback_at`（typed contract）
3. `strategy_rollout.rs` 提供 `activate_promoted_candidate / rollback_active_strategy / list_active_strategies` 服务 + 严格状态机
4. IPC：`learning_activate_promoted_candidate / learning_rollback_active_strategy / learning_get_active_strategies`
5. **本轮 Active 仅是 governance truth**：不切换 production policy、不触碰 commands/agent.rs 主链；activate 仍要求 candidate 处于 `PromotedCandidate` 才允许；rollback 必须带非空 reason

### 文件级 plan 命名校准

- file-level plan §5.2 提到的 `strategy_promotion.rs` 实际落地为 `promotion_gate.rs`（命名等价、职责相同）。
- file-level plan §5.3 提到的 `strategy_rollback.rs` 落地为 `strategy_rollout.rs`（同时承担 promotion->Active 与 Active->RolledBack；如未来需要拆分，再分裂为独立模块）。

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
