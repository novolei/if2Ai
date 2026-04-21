# Phase M4 Governance Surfaces And Upgrade Gates File-Level Plan

> 将 `M4` 的 `m4.6 + m4.8` 细化为文件级实施方案。
>
> 最后更新: 2026-04-21（M4 reconciliation：m4.8 已 done；m4.6 显式延后）

## 0. Reconciliation 概要（必读）

- `m4.8` upgrade gate: done。落点是 `src-tauri/src/modules/harness/gate.rs`（`GatePolicy` / `GateDecision` / `Recommendation` / `evaluate_compare` / `evaluate_suite`）+ `harness_evaluate_compare` / `harness_evaluate_suite` IPC。默认 conservative — 缺 recommendation / compare 时返回 Hold/Reject，永远不会 Promote。
- `m4.6` governance UI: **deferred**。后端治理结论已通过 IPC 暴露（见上方 m4.8 与 `harness_list_reports` / `harness_load_report` / `harness_compare_reports` / `harness_aggregate_suite_report`），但前端 dashboard / 治理 store / settings 治理 section 暂未实现。
- 不允许在 M5 slice 内顺手实现 `m4.6`；如需推进，请单独走 follow-on phase（建议命名 `phase-m4f-governance-dashboard-frontend`），并显式恢复 m4.6 状态。
- `governance-gate-rules.md` 文档（§5.6 中提及的新增文件）至今未单独成文；其内容已通过 `gate.rs` 的 doc-comments 与 yaml 注释承载。如未来需要单独成文，请视为 enhancement，不视为 m4.8 未完成。
- 契约层级（与 §5.5 一致）：`HarnessRunReport` / `BaselineVsCandidate` / `SuiteReport` / `Recommendation` 四者平行；治理 store / 表面只投影治理结果，不重新计算评分。

## 1. 适用范围

本计划只覆盖：

1. `m4.6` Project governance surfaces into frontend
2. `m4.8` Gate policy upgrades on harness recommendation

目标是把治理结果从“后台存在”变成“开发者可回看、发布可裁决”的正式表面与升级规则。

## 2. 当前事实基线

### 2.1 当前前端只有开发者观测面，没有治理面

当前前端最接近 harness 的表面主要是 [src/components/chat/TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx:1)。

它现在主要显示：

1. telemetry
2. memory lifecycle
3. harness recording status

这仍属于“开发者观测面”，还不是 blueprint 里的：

1. run report surface
2. compare result surface
3. blocker summary
4. release readiness

### 2.2 当前 settings 壳层已具备承载治理页的条件

当前 [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1) 已有成熟 section shell，可新增治理相关页面，而无需重造 settings 架构。

### 2.3 当前升级规则尚未写死

虽然文档中已经反复强调：

1. 没有 recommendation 不可 promote
2. 没有 compare 不可升级

但这些规则目前还没有同时写入：

1. 文档
2. runner/gate code shape
3. 前端治理表面

## 3. 实施原则

1. 开发者观测面和治理面必须分离。
2. 最终用户聊天主界面不直接承载治理噪音。
3. 升级 gate 规则必须同时进入代码和执行文档。
4. 这一轮优先展示治理结论，不追求复杂 dashboard 视觉。

## 4. 严格执行顺序

1. `U0` preflight governance surface inventory
2. `U1` 建 governance state
3. `U2` 建 governance pages / cards
4. `U3` 接 settings 入口
5. `U4` 接 compare / blocker / recommendation projection
6. `U5` 写死 upgrade gate 规则
7. `U6` 回写执行文档与 gate 脚本
8. `U7` build + manual verification

禁止并行：

1. TelemetryDrawer 重构
2. Settings section 扩展
3. gate 规则改写

因为这三项一起动时，最容易把“观测”与“治理”重新混成一个层。

## 5. 文件级实施方案

## 5.1 `U0` Preflight Governance Surface Inventory

### 必查文件

- [src/components/chat/TelemetryDrawer.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/components/chat/TelemetryDrawer.tsx:1)
- [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1)
- [src/modules/settings/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/types.ts:1)
- [src/modules/settings/data.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/data.ts:1)
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)
- [docs/staff-remediation/harness-v2-governance-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/harness-v2-governance-design.md:1)

### 必做动作

1. 标出当前哪些内容属于开发者观测面。
2. 标出 settings 中最适合挂治理页的 section 位置。
3. 盘出当前 gate 脚本如何表达通过/失败。

## 5.2 `U1` 建 governance state

### 新增文件

- `src/state/governance-store.ts`

### 修改文件

- 视 `M2` 现状接入 `src/state/index.ts`

### 第一版必须承载

1. latest run report
2. latest compare report
3. blocker summary
4. recommendation
5. release readiness / gate status

### 约束

治理 store 只投影治理结果，不重新计算评分。

## 5.3 `U2` 建 governance pages / cards

### 新增文件

- `src/modules/settings/pages/GovernanceDashboardPage.tsx`
- `src/modules/settings/components/RunReportCard.tsx`
- `src/modules/settings/components/CompareResultCard.tsx`
- `src/modules/settings/components/BlockerSummaryCard.tsx`
- `src/modules/settings/components/ReleaseReadinessCard.tsx`

### 必须落实

1. run report 可读
2. compare result 可读
3. blocker summary 可读
4. recommendation / readiness 可读

### 第一版不追求

1. 复杂图表
2. 多维 drill-down

先保证治理信息可见、可回看。

## 5.4 `U3` 接 settings 入口

### 修改文件

- [src/modules/settings/SettingsApp.tsx](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/SettingsApp.tsx:1)
- [src/modules/settings/types.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/types.ts:1)
- [src/modules/settings/data.ts](/Users/ryanliu/Documents/IfAI/if2Ai/src/modules/settings/data.ts:1)

### 必须落实

1. 新增治理 section
2. governance dashboard 不与 general/usage/remote 混淆
3. developer telemetry drawer 保持存在，但不再冒充治理页

## 5.5 `U4` 接 compare / blocker / recommendation projection

### 修改文件

- `src/state/governance-store.ts`
- `src/modules/settings/pages/GovernanceDashboardPage.tsx`
- 视需要扩展 `src/transport/contracts.ts`
- 视需要扩展 `src/lib/tauri.ts`

### 必须落实

1. 页面读 store，不直接临时 `invoke` 拼结果
2. recommendation 明确显示 `promote / hold / reject`
3. blocker 明确列出原因，而不是只显示红/绿状态

## 5.6 `U5` 写死 upgrade gate 规则

### 新增文件

- `docs/exec-plans/active/governance-gate-rules.md`

### 修改文件

- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)
- [docs/staff-remediation/harness-v2-governance-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/harness-v2-governance-design.md:1)

### 第一版必须明确

1. 什么时候 `promote`
2. 什么时候 `hold`
3. 什么时候 `reject`
4. 缺 report / 缺 compare / 缺 recommendation 时的默认行为

### 强制默认

没有 harness recommendation，一律不得 promote。

## 5.7 `U6` 回写执行文档与 gate 脚本

### 修改文件

- [docs/exec-plans/active/phase-m4-executor-runbook.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m4-executor-runbook.md:1)
- [docs/exec-plans/active/phase-m-remediation-execution-index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1)
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

### 必须落实

1. 文档与 gate 脚本口径一致
2. executor 能看到升级 gate 的硬规则
3. 不允许文档写一套、脚本做一套

## 5.8 `U7` Build + Manual Verification

### 必跑

- `npm run build:web`

### 建议补跑

- `python3 -m harness.runner --help`

### 必做人工检查

1. TelemetryDrawer 仍是观测面，不是治理面。
2. Settings 中已能看到治理 dashboard。
3. recommendation / blocker / readiness 已能清楚解释升级结论。

## 6. 完成定义

只有当 reviewer 可以明确回答“现在开发者去哪里看观测，去哪里看治理，以及为什么这个 candidate 能不能升级”时，这一段才算完成。
