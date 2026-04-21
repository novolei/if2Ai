# Phase M5 Promotion Rollback And Diagnostics File-Level Plan

> 将 `M5` 的 `m5.6 + m5.7 + m5.8` 细化为文件级实施方案。
>
> 最后更新: 2026-04-21（M5 reconciliation 后补 delivery layers + 命名校准）

## 0. Reconciliation 概要（必读）

- `m5.6` Promotion gate skeleton 已在 M5-B 落地：`src-tauri/src/modules/learning/promotion_gate.rs`（不是 §5.2 中提到的 `strategy_promotion.rs` —— 命名等价、职责相同，文档以代码为准）。
- `m5.6` Rollback + Active 的 typed contract / state machine / service / IPC 由 **M5-C 第一轮**承担，落到 `src-tauri/src/modules/learning/strategy_rollout.rs`（不是 §5.3 中提到的 `strategy_rollback.rs` —— 同一职责合并到一个 rollout 服务里；如未来需要拆分，再独立成模块）。
- `m5.6` 当前**不**做 production active flip：Active 仅是 registry / governance truth，不修改 `commands/agent.rs` 主链。production active flip 留 M5-D。
- `m5.7` 前端 diagnostics surfaces：仍 pending；前端模块（governance UI）受 m4.6 deferred 影响，不在 M5-C 第一轮范围。
- `m5.8` safety / regression suites：仍 pending；M5-C 第一轮内只补 Active / RolledBack 自身的状态机单元测试，不补 corpus 级别 reject-path 回放。
- 不要再把 m5.6 当成"必须等 rollback 全部完成"才能开始 —— promotion skeleton 已 done，rollback contract 由 M5-C 第一轮单独推进。

## 1. 适用范围

本计划只覆盖：

1. `m5.6` Implement gated promotion and rollback semantics
2. `m5.7` Project strategy lifecycle into diagnostics surfaces
3. `m5.8` Add self-evolution safety and regression checks

目标是把 `candidate -> recommendation -> promote/hold/reject -> rollback` 做成真正受治理的闭环，并把结果透出到可回看的诊断表面。

## 2. 当前事实基线

### 2.1 当前 gate 有基础，但没有 strategy lifecycle 语义

当前 [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1) 主要是 compile/test/behavior gate，并没有正式表达：

1. strategy promotion
2. hold
3. rollback path
4. rollback reason

### 2.2 当前前端 settings 壳层适合承载 diagnostics 面

当前 [src/modules/settings/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/types.ts:1) 与 [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1) 已足够承载 strategy diagnostics 页面。

### 2.3 当前还没有“错误推广被 gate 拦下”的正式回归面

M5 真正高风险点不是“不会学习”，而是“会错误学习”。  
所以安全检查必须覆盖：

1. 错误 candidate generation
2. 错误 promote
3. rollback 可追踪
4. gate reject path

## 3. 实施原则

1. promotion/rollback 必须严格复用 `M4` gate 规则。
2. diagnostics surfaces 是治理面，不是产品营销面。
3. safety checks 必须优先覆盖 reject/rollback 路径。
4. 这一轮不允许“手工文档批准”与“脚本 gate 结论”不一致。

## 4. 严格执行顺序

1. `P0` preflight promotion inventory
2. `P1` gated promotion semantics
3. `P2` rollback semantics
4. `P3` diagnostics state
5. `P4` diagnostics pages/surfaces
6. `P5` gate/recommendation wiring
7. `P6` safety/regression suites
8. `P7` build + verification

禁止并行：

1. promote/rollback semantics
2. diagnostics UI
3. safety suite

因为这三项一起动时，最容易出现“能升但不能回看”“能回看但不能回滚”“脚本拒绝但 UI 还显示可升”的双标。

## 5. 文件级实施方案

## 5.1 `P0` Preflight Promotion Inventory

### 必查文件

- `src-tauri/src/modules/learning/strategy_registry.rs`
- `src-tauri/src/modules/learning/strategy_registry_service.rs`
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)
- [src/modules/settings/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/types.ts:1)
- [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1)

### 必做动作

1. 标出 strategy registry 当前的 rollout state 可变更点。
2. 标出 gate 当前哪些输出可直接复用。
3. 标出 settings 中 diagnostics 页面最自然的挂点。

## 5.2 `P1` gated promotion semantics

### 实际落点（M5-B 已交付）

- `src-tauri/src/modules/learning/promotion_gate.rs`（命名取代了原稿中的 `strategy_promotion.rs`；contract / 行为等价）

### 历史草稿命名（已弃用）

- ~~`src-tauri/src/modules/learning/strategy_promotion.rs`~~

### 修改文件

- `src-tauri/src/modules/learning/strategy_registry_service.rs`
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

### 第一版必须落实

1. candidate -> active 必须检查最新 recommendation
2. `promote` 只能在 gate 允许时发生
3. 没有 compare / recommendation 时默认 `hold`

### 必须显式表达

1. `promote`
2. `hold`
3. `reject`

而不是仅靠布尔值。

## 5.3 `P2` rollback semantics

### 实际落点（M5-C 第一轮）

- `src-tauri/src/modules/learning/strategy_rollout.rs`（同时承担 `PromotedCandidate -> Active` 与 `Active -> RolledBack` 两段状态机；命名取代了原稿中的 `strategy_rollback.rs`）

### 历史草稿命名（已弃用）

- ~~`src-tauri/src/modules/learning/strategy_rollback.rs`~~

### 修改文件

- `src-tauri/src/modules/learning/strategy_registry.rs`（扩 RolloutState + audit substruct）
- `src-tauri/src/modules/learning/strategy_registry_service.rs`（force_state_internal 接受新状态、set_state 拒绝 Active / RolledBack）
- `src-tauri/src/modules/learning/promotion_gate.rs`（仅在 activate 路径中复用 `check_eligibility`，不改自身职责）

### 第一版必须落实

1. rollback reason
2. rollback target
3. rollback timestamp
4. rollback initiated by

### 约束

rollback 不能只是改个 state label，必须能追踪“为什么回滚”和“回到哪一版”。

## 5.4 `P3` diagnostics state

### 新增文件

- `src/state/strategy-diagnostics-store.ts`

### 第一版必须承载

1. candidate list
2. active strategy
3. last recommendation
4. compare summary
5. rollback history
6. suppressed/rejected candidates

## 5.5 `P4` diagnostics pages/surfaces

### 新增文件

- `src/modules/settings/pages/StrategyDiagnosticsPage.tsx`
- `src/modules/settings/components/StrategyLifecycleCard.tsx`
- `src/modules/settings/components/StrategyCompareSummaryCard.tsx`
- `src/modules/settings/components/RollbackHistoryCard.tsx`

### 修改文件

- [src/modules/settings/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/types.ts:1)
- [src/modules/settings/data.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/data.ts:1)
- [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1)

### 必须落实

1. candidate / active / rollback 状态可见
2. compare 结果可见
3. 最近 recommendation 可见
4. rollback history 可见

### 原则

这必须是治理/诊断面，不是“AI 正在变聪明”的营销面板。

## 5.6 `P5` gate/recommendation wiring

### 修改文件

- `src/state/strategy-diagnostics-store.ts`
- `src/modules/settings/pages/StrategyDiagnosticsPage.tsx`
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)
- [docs/exec-plans/active/governance-gate-rules.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/governance-gate-rules.md:1)

### 必须落实

1. diagnostics 页面展示的 recommendation 与 gate 脚本口径一致
2. `promote / hold / reject` 的 UI 不允许和脚本冲突
3. rollout state 的迁移规则必须和治理文档一致

## 5.7 `P6` safety/regression suites

### 新增或修改测试文件

> 命名校准：原稿中的 `strategy_promotion.rs` / `strategy_rollback.rs` 实际落地为
> `promotion_gate.rs`（M5-B 交付）+ `strategy_rollout.rs`（M5-C 第一轮交付）。
> 后续 safety / regression 套件应附加到下列实际文件，不要再创建被弃用的命名。

- `src-tauri/src/modules/learning/promotion_gate.rs` 同文件单测
- `src-tauri/src/modules/learning/strategy_rollout.rs` 同文件单测
- `src-tauri/src/modules/learning/strategy_registry_service.rs` 同文件单测
- `src-tauri/src/modules/learning/candidate_evaluator.rs` 同文件单测
- [src-tauri/src/integration_tests.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/integration_tests.rs:1)
- `harness/corpus/self_evolution/`

### 必须覆盖

1. candidate generation test
2. promote allowed test
3. promote rejected-by-gate test
4. rollback test
5. 至少一条错误推广被拦下的回放案例

## 5.8 `P7` Build + Verification

### 必跑

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

### 建议补跑

- `python3 -m harness.runner --help`

### 必做人工检查

1. 至少一条 candidate 有完整 lifecycle 展示。
2. 一条 reject path 能在 diagnostics 页面和 gate 结果中对应起来。
3. rollback 原因和 rollback 目标可追踪。

## 6. 完成定义

只有当 reviewer 可以明确说出“系统现在如何受 gate 约束地升级、拒绝或回滚策略，并且这些结论在哪里可回看”时，这一段才算完成。
