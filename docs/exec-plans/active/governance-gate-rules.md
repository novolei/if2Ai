# Governance Gate Rules

> If2Ai Staff remediation 中 `M4/M5` 阶段使用的统一治理硬规则源。
>
> 最后更新: 2026-04-20
> 适用范围:
> - [phase-m4-policy-harness-governance.yaml](./phase-m4-policy-harness-governance.yaml)
> - [phase-m5-self-evolution-and-strategy-promotion.yaml](./phase-m5-self-evolution-and-strategy-promotion.yaml)
> - [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

## 1. 目的

本文件定义 `promote / hold / reject` 的统一判定口径，避免出现“文档写一套、脚本做一套、UI 展示另一套”的三重真相。

## 2. 治理对象

本规则适用于以下两类对象：

1. `candidate strategy`
2. `runtime or policy change package`

若某变更会影响 execution mode、memory write policy、tool execution policy、reflection-to-candidate promotion、activation/routing 决策中的任一行为，则必须经过本治理规则。

## 3. 必要输入

进入 gate 之前，以下输入必须齐备：

1. `HarnessRunReport`
2. `baseline vs candidate compare`
3. `grader outputs`
4. `blocker evaluation`
5. `final recommendation`

缺少任一项，默认不得 `promote`。

## 4. 默认裁决规则

### 4.1 Promote

仅当以下条件全部满足时，才允许 `promote`：

1. 存在完整 `HarnessRunReport`
2. 存在同语料、同口径的 `baseline vs candidate compare`
3. `final recommendation == promote`
4. 没有 blocker 命中
5. 没有未解释的关键指标回退

### 4.2 Hold

满足以下任一情况，默认 `hold`：

1. evidence 不完整，但没有明确安全或质量灾难
2. compare 结果不稳定，尚需更多样本
3. recommendation 为 `hold`
4. 存在需要人工确认的 trade-off

### 4.3 Reject

满足以下任一情况，默认 `reject`：

1. recommendation 为 `reject`
2. 命中 blocker
3. 关键质量、安全、稳定性指标出现不可接受回退
4. candidate 无法解释其行为变化来源

## 5. Blocker 规则

以下情况必须视为 blocker：

1. 缺失 `HarnessRunReport`
2. 缺失 `baseline vs candidate compare`
3. 缺失 `final recommendation`
4. compare 语料不一致
5. 关键失败率、安全风险或回归指标超过项目允许阈值
6. rollback 路径不存在或无法验证

命中 blocker 时，结论只能是 `reject` 或 `hold`，不得 `promote`。

## 6. Recommendation 与 Gate 的关系

`recommendation` 是治理输入，不是展示装饰。

必须满足以下约束：

1. UI 显示的 recommendation 必须与 gate 脚本一致
2. 文档中的 recommendation 规则必须与 gate 脚本一致
3. 若脚本无法生成 recommendation，默认进入 `hold`
4. 没有 harness recommendation，一律不得 `promote`

## 7. Compare 规则

`compare` 必须满足以下条件：

1. baseline 与 candidate 使用同一语料层
2. grading 口径一致
3. 结果可追溯到具体 run / corpus / grader
4. 不允许单样本或挑样本 compare

不满足以上条件时，`compare` 视为无效。

## 8. Rollback 规则

若 candidate 已被启用且后续发现异常，必须满足：

1. 可定位当前 active strategy
2. 可定位上一个稳定版本
3. rollback 原因可记录
4. rollback 后可重新进入 compare/gate 评估

没有可验证 rollback path 的 candidate，不得 `promote`。

## 9. Executor 检查清单

执行 `M4/M5` 时，executor 必须逐项核对：

1. `harness/gate.py` 与本文件口径一致
2. Settings / Governance / Diagnostics 页面口径一致
3. runbook、phase YAML、file-level plan 都引用本文件
4. 缺 report / 缺 compare / 缺 recommendation 时，默认不会走到 `promote`

## 10. 变更规则

若后续需要修改治理规则：

1. 先更新本文件
2. 再更新 `harness/gate.py`
3. 再回写相关 runbook、YAML、UI 文案
4. 最后通过 compare + regression 验证没有出现规则漂移
