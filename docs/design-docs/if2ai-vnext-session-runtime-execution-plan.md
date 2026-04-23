# If2Ai vNext Session Runtime Execution Plan

> 配套文档：
> - 上位总纲：[If2Ai vNext Session Runtime Blueprint](./if2ai-vnext-session-runtime-blueprint.md)
>
> 目标：
> - 把蓝图转成可排期的实施顺序
> - 为每个阶段给出建议 owner
> - 提前列出关键风险、阻塞点与防错动作
>
> 适用范围：
> - `MIG-003`
> - `MIG-007`
> - `MIG-016` ~ `MIG-023`
>
> 强约束：
> - 本文档是 [If2Ai vNext Session Runtime Blueprint](./if2ai-vnext-session-runtime-blueprint.md) 的执行层补充
> - 若实施顺序、owner 归属、风险控制与具体 Pack 文案冲突，以 Blueprint 为准；Pack 应按需更新
>
> 最后更新：`2026-04-23`

---

## 1. 执行目标

这份执行文档不重复蓝图中的架构定义，而是回答 4 个问题：

1. 先做什么，后做什么
2. 每一步最适合谁来主导
3. 哪些步骤会互相阻塞
4. 哪些风险最容易让路线偏回“双真相 / 旁路状态 / 局部补丁”

一句话目标：

`让 If2Ai 以最小回归风险，逐步从 session object 驱动迁移到 event-log + supervisor + projection 驱动。`

---

## 2. 总体实施顺序

建议按 4 个阶段推进，而不是并行乱开 8 个 Pack。

### 阶段 A：立事实源与真相边界

目标：
- 建立 run event log
- 让前端主聊天路径真正朝 projection single truth 收敛

建议顺序：
1. `MIG-016`
2. `MIG-003`
3. `MIG-017`

为什么这样排：
- 没有 `MIG-016`，projection 还是没有稳定 replay truth。
- `MIG-003` 是老路线中的核心口子，必须尽早按 vNext 蓝图改写为“前端不直吃 raw transport”。
- `MIG-017` 是真正落到 chat surface 的 cutover 包，适合在事件真相和 projection 真相都明确后推进。

### 阶段 B：补齐 session continuity

目标：
- 建立 history replay / paging
- 让 permission pending 成为可恢复状态
- 引入 session supervisor

建议顺序：
1. `MIG-018`
2. `MIG-019`
3. `MIG-020`

为什么这样排：
- `MIG-018` 先把“如何恢复历史”做成正式能力。
- `MIG-019` 解决最脆弱的中断点：pending permission。
- `MIG-020` 再把 active run / blocked / recoverable 等会话生命周期真相统一收口。

### 阶段 C：补齐恢复与工具执行语义

目标：
- 让 resume 成为 typed contract
- 让 tool retry / failure / policy decision 变成 attempt-aware truth

建议顺序：
1. `MIG-007`
2. `MIG-021`
3. `MIG-022`

为什么这样排：
- `MIG-007` 是后端执行 contract 的老关键包，必须先按 vNext 路线升级成 attempt-aware contract。
- `MIG-021` 在 supervisor 和 replay 已存在时定义恢复语义最稳。
- `MIG-022` 再把工具时间线、重试、失败全部做成完整 ledger。

### 阶段 D：把审计与评测收口

目标：
- 让 harness / replay / eval 最终站上 canonical run report

建议顺序：
1. `MIG-023`
2. `MIG-008`

为什么这样排：
- `MIG-023` 先把 event log 转成 canonical run report。
- `MIG-008` 再让 harness replay / eval 真正改读 report，而不是继续拼零散 traces。

---

## 3. 建议 Owner 模型

这里说的是“角色 owner”，不是具体人名。建议每个阶段都明确主 owner，避免文档、前端、后端、harness 各做各的。

### 3.1 系统架构 Owner

职责：
- 审核阶段边界与 Pack 顺序
- 把关 Blueprint 是否被偏离
- 审核跨 Pack 的 contract 变更

建议负责：
- `MIG-003`
- `MIG-007`
- `MIG-020`
- `MIG-021`
- `MIG-023`

原因：
- 这些 Pack 都涉及跨层 contract 或系统真相变更，不适合只按局部实现思路推进。

### 3.2 Runtime / Backend Owner

职责：
- 负责 event log、supervisor、resume contract、tool attempt ledger 的 Rust 主实现

建议负责：
- `MIG-016`
- `MIG-019`
- `MIG-020`
- `MIG-021`
- `MIG-022`
- `MIG-023`

原因：
- vNext 的“事实源”最终必须落在 backend runtime truth。

### 3.3 Frontend / Projection Owner

职责：
- 负责 projection reducer/store、chat surface cutover、history paging UX、permission recovery UX

建议负责：
- `MIG-003`
- `MIG-017`
- `MIG-018`
- `MIG-019`
- `MIG-022`

原因：
- 如果 projection 不成为唯一 UI 真相，后端 truth 再好也会被页面旁路掉。

### 3.4 Harness / Evaluation Owner

职责：
- 负责 canonical run report 接入 grader、replay、audit pipeline

建议负责：
- `MIG-023`
- `MIG-008`

### 3.5 文档 / Migration Coordinator

职责：
- 确保 Pack 文档、Blueprint、Execution Plan、REGISTRY 状态同步
- 任何 contract 变化都及时回写设计文档与 Pack

建议负责：
- 所有相关 Pack 的文档回写

---

## 4. Pack 级实施顺序与依赖细化

### 4.1 `MIG-016` Canonical Run Event Log Foundation

前置：
- `MIG-001`
- `MIG-015`

主 owner：
- Runtime / Backend Owner

协作方：
- 系统架构 Owner

完成定义：
- 事件事实可落盘
- run_id 成为稳定 session runtime identity 的一部分
- 失败不阻断主链

不要在本包做：
- 前端 UI 改读
- paging API
- harness report

### 4.2 `MIG-003` Runtime Event Projection Truth

前置：
- `MIG-016` 至少落地最小 event truth

主 owner：
- Frontend / Projection Owner

协作方：
- 系统架构 Owner

完成定义：
- chat 主链不再长期并行依赖 raw listener 与 projection bridge
- projection 成为前端唯一真相设计目标，而不是附属层

### 4.3 `MIG-017` Runtime Projection Chat Truth Cutover

前置：
- `MIG-003`
- `MIG-016`

主 owner：
- Frontend / Projection Owner

协作方：
- Runtime / Backend Owner

完成定义：
- chat surface 的 assistant / thinking / tool / completion 由 projection 驱动

### 4.4 `MIG-018` Session History Replay And Paging

前置：
- `MIG-016`
- `MIG-017`

主 owner：
- Frontend / Projection Owner

协作方：
- Runtime / Backend Owner

完成定义：
- history replay 可稳定重建 UI
- paging cursor 正确

### 4.5 `MIG-019` Pending Permission Recovery

前置：
- `MIG-016`
- `MIG-017`

主 owner：
- Runtime / Backend Owner

协作方：
- Frontend / Projection Owner

完成定义：
- pending permission 可恢复
- resolution 可审计

### 4.6 `MIG-020` Session Supervisor Foundation

前置：
- `MIG-019`
- 建议 `MIG-018` 至少完成最小 replay 路径

主 owner：
- 系统架构 Owner

协作方：
- Runtime / Backend Owner
- Frontend / Projection Owner

完成定义：
- active / blocked / recoverable failed 的状态真相集中统一

### 4.7 `MIG-007` Worker Tool Execution Contract

前置：
- 建议 `MIG-020` 完成，至少 supervisor 状态已存在

主 owner：
- 系统架构 Owner

协作方：
- Runtime / Backend Owner

完成定义：
- tool execution contract 开始朝 attempt-aware 语义升级

### 4.8 `MIG-021` Resume Contract And Run Recovery

前置：
- `MIG-018`
- `MIG-020`
- `MIG-007`

主 owner：
- Runtime / Backend Owner

协作方：
- 系统架构 Owner
- Frontend / Projection Owner

完成定义：
- resume 不再只是 cursor，而是 typed recoverability contract

### 4.9 `MIG-022` Tool Attempt Ledger And Timeline Contract

前置：
- `MIG-007`
- `MIG-021`

主 owner：
- Runtime / Backend Owner

协作方：
- Frontend / Projection Owner

完成定义：
- tool retries / failures / policy decisions 具有 canonical attempt facts

### 4.10 `MIG-023` Canonical Run Report From Event Log

前置：
- `MIG-016`
- `MIG-018`
- `MIG-021`
- `MIG-022`

主 owner：
- Harness / Evaluation Owner

协作方：
- Runtime / Backend Owner
- 系统架构 Owner

完成定义：
- run report 可独立于旧 trace 拼装生成

---

## 5. 风险清单

### 风险 1：Event Log 落地后，前端仍继续吃 raw payload

症状：
- `MIG-016` 做完了，但页面状态还是直接读 `agent-token`
- projection store 继续只是“附属显示层”

后果：
- 系统出现第三套真相：`session JSON + event log + raw listener state`

应对：
- `MIG-003` 必须在 `MIG-016` 后尽快启动
- review 时明确检查页面是否还直接理解 transport payload

### 风险 2：Session JSON 继续膨胀，event log 变成旁路摆设

症状：
- 新功能继续优先往 `session.json` 塞状态
- replay / resume / paging 仍主要依赖 session object

后果：
- 事实源切换失败，技术债继续扩大

应对：
- 新增与 session continuity 相关的数据优先设计为 event / projection / pending / ledger
- review 时禁止把 session JSON 当成唯一恢复真相继续扩展

### 风险 3：Supervisor 没有收口成功，状态继续散落

症状：
- active run 在 store 里一份、backend 一份、UI 组件里再一份
- reconnect / stop / pending permission 各自维护状态

后果：
- session lifecycle 看起来存在，实际没有单一判断点

应对：
- `MIG-020` 必须是集中收口包，而不是再建一个旁路 store
- review 时要求列出“此状态最终由谁拥有”

### 风险 4：Resume 只做 UI 提示，不做 recoverability truth

症状：
- 页面上出现“Resume”按钮
- 但后端没有 typed `resume_reason` / `safe_to_retry_mutations`

后果：
- 恢复能力看起来增强，实际风险更高

应对：
- `MIG-021` 的验收必须要求 typed recoverability contract
- mutating tool 场景必须显式测试

### 风险 5：Tool timeline 只做展示，不做 attempt ledger

症状：
- UI 上多了 retry 提示
- 但 backend 没有 attempt_id / attempt_no / failure_kind

后果：
- timeline 只是表演层，没有审计价值

应对：
- `MIG-022` 必须先做 contract，再做 UI
- 若 contract 未落地，不接受只改前端时间线样式

### 风险 6：Harness 继续依赖零散 trace，错过 event-log 收口窗口

症状：
- `MIG-023` 只是在旧 trace aggregator 上加字段
- grader 仍不读 canonical run report

后果：
- replay / eval / audit 永远无法统一

应对：
- `MIG-023` 必须明确“旧路径兼容”与“新路径主真相”的边界
- `MIG-008` 只能在 `MIG-023` 后推进

### 风险 7：文档与 Pack 脱节

症状：
- Blueprint 写一套
- Pack 写一套
- 实现再做第三套

后果：
- 路线图失效，团队成员按各自理解推进

应对：
- 文档 / Migration Coordinator 必须在每个 Pack 完成后回写：
  - Blueprint 是否需更新
  - Execution Plan 是否需调整顺序或 owner
  - REGISTRY 状态是否变化

---

## 6. 实施时的检查点

每推进一个阶段，建议做一次 checkpoint，而不是只看代码是否 merge。

### Checkpoint A：事实源检查

检查问题：
- 新 runtime truth 是否已优先写入 event log
- 是否仍有核心状态只存在于 process-local memory

### Checkpoint B：UI 真相检查

检查问题：
- 主聊天界面是否仍直接依赖 raw transport payload
- projection store 是否真的承接了最终展示语义

### Checkpoint C：会话连续性检查

检查问题：
- 刷新后能恢复什么
- 重连后能恢复什么
- pending permission 是否还能处理
- recoverable failed run 是否可识别

### Checkpoint D：执行语义检查

检查问题：
- tool retry 是否有 canonical attempt facts
- policy / approval / timeout / failure 是否能追溯

### Checkpoint E：评测闭环检查

检查问题：
- harness / replay 是否已站上 canonical run report
- 是否仍在拼接分散 traces

---

## 7. 建议的工作方式

### 7.1 不建议的方式

- 8 个 Pack 同时开工
- 前后端各自按理解实现，再后面对齐
- 先做 UI 展示，后补 runtime truth
- 先做“可用” resume，再后补 recoverability contract

### 7.2 建议的方式

- 每个阶段最多并行 2 个 Pack
- 每个阶段先确认 owner 与上位 contract
- 后端 truth 先落，前端 projection 再切
- 每个阶段结束做一次 checkpoint 回写文档

---

## 8. 推荐排期节奏

如果按 4 个阶段推，建议是：

- 第 1 轮：
  - `MIG-016`
  - `MIG-003`

- 第 2 轮：
  - `MIG-017`
  - `MIG-018`

- 第 3 轮：
  - `MIG-019`
  - `MIG-020`

- 第 4 轮：
  - `MIG-007`
  - `MIG-021`

- 第 5 轮：
  - `MIG-022`

- 第 6 轮：
  - `MIG-023`
  - `MIG-008`

这不是唯一顺序，但这是我认为在“最小回归风险 + 最大真相收口”之间最稳的顺序。

---

## 9. 结论

这条路线的关键不是多写几个 store、多加几条事件、多补几个 UI 卡片，而是始终守住 3 个判断：

1. 新能力是不是在把系统推向 `event-log truth`
2. 新 UI 是不是在把前端推向 `projection truth`
3. 新恢复能力是不是在把 session 推向 `supervisor truth`

只要这三条持续成立，`MIG-003`、`MIG-007`、`MIG-016` ~ `MIG-023` 就会是同一条路线的分阶段落地，而不是一串松散的重构任务。
