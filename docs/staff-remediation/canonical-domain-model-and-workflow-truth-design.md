# If2Ai Canonical Domain Model And Workflow Truth Design

> 将 If2Ai 的领域实体与主 workflow 真相收敛为正式设计资产。
>
> 最后更新: 2026-04-20

## 1. 目标

本设计文档解决两个根问题：

1. If2Ai 缺少统一的 canonical domain model。
2. If2Ai 缺少“已跑通 / 半闭环 / 未成立”的 workflow truth 机制。

本设计是所有后续重构的上位约束。没有这份真相文档，后续 prompt、memory、harness、activation、execution mode 都会继续各写各的叙事。

## 2. 设计原则

1. 先定义实体，再定义服务，再定义页面。
2. workflow truth 必须先于 UI 完整度。
3. 任何“实验中”能力都必须显式标记。
4. 一个概念只能有一个 canonical 名称。

## 3. Canonical Entities

### 3.1 agent

- 是什么：当前回答与执行策略的主体。
- 不是什么：不是一次 run，也不是用户 profile。
- 生命周期：create -> select -> update -> retire
- 所属层：application + runtime + UI projection

### 3.2 session

- 是什么：连续对话与短期执行上下文容器。
- 不是什么：不是 project，也不是 memory。
- 生命周期：new -> active -> switched -> archived -> deleted
- 所属层：application + persistence + UI projection

### 3.3 project

- 是什么：长期任务空间，聚合目标、文件上下文、关联 session、产出物。
- 不是什么：不是一次 turn 的上下文窗口。
- 生命周期：create -> active -> evolve -> archived

### 3.4 run

- 是什么：一次 runtime 执行实例。
- 不是什么：不是 session 本身，也不是 worker。
- 生命周期：created -> started -> streaming -> waiting -> completed | failed | cancelled

### 3.5 worker

- 是什么：run 中派生的执行单元。
- 不是什么：不是顶层用户对象。
- 生命周期：spawned -> running -> completed | failed | cancelled

### 3.6 memory

- 是什么：跨 turn / 跨 session 可复用上下文资产。
- 不是什么：不是全部聊天日志。
- 子类型：
  - working
  - session summary
  - recalled episodic
  - pinned
  - compiled
  - reflection note

### 3.7 automation

- 是什么：可重复执行的任务定义。
- 不是什么：不是普通对话消息。

### 3.8 harness_eval

- 是什么：用于比较 prompt / policy / memory / route strategy 的评测运行。
- 不是什么：不是普通用户 run。

### 3.9 activation_license

- 是什么：If2Ai 的激活授权实体。
- 不是什么：不是本地设置开关。
- 生命周期：requested -> issued -> redeemed -> refreshed -> revoked | expired | deactivated

### 3.10 execution_mode

- 是什么：chat 入口自动分类得到的运行时执行模式。
- 不是什么：不是用户可编辑的 scenario profile。
- canonical values：
  - `direct_execute`
  - `auto_plan_execute`
  - `plan_then_confirm`
  - `specialized_surface`

## 4. Workflow Truth Model

每条主 workflow 必须以三态声明：

1. `canonical`
2. `partial`
3. `not_established`

## 5. 必须维护的 Workflow Truth 列表

1. app boot
2. onboarding
3. activation gate
4. chat prompt dispatch
5. stream projection
6. permission approval
7. session recovery
8. memory capture / recall / write
9. harness record / replay / eval
10. execution mode routing
11. specialized surface entry
12. deactivation / revoke fallback

## 6. 产物要求

必须新增两份 canonical 文档：

1. `if2ai-canonical-domain-model.md`
2. `if2ai-workflow-truth.md`

每份文档都必须包含“最后更新”日期，并在 PR 中同步维护。

## 7. 与其他设计的依赖关系

- prompt plan 依赖 `execution_mode`、`session`、`agent`
- memory 依赖 `session`、`project`、`run`
- harness 依赖 `run`、`harness_eval`
- activation 依赖 `activation_license`
- runtime projection 依赖 `run`、`worker`、`execution_mode`

## 8. 执行切片

### Slice D0.1

- 产出 canonical entity 表
- 审核现有命名偏差
- 给出命名迁移表

### Slice D0.2

- 产出 workflow truth 表
- 标记 canonical / partial / not_established

### Slice D0.3

- 把 workflow truth 接入 exec-plan 审查模板

## 9. 验收

1. 后续所有子设计都引用同一组实体名称。
2. 任一主 workflow 都能明确指出当前状态。
3. 没有文档仍把 React 前端写成 Svelte 这类真相漂移。

## 10. 风险

1. 只写文档不改命名，导致真相继续分叉。
2. workflow 状态不更新，文档再次过时。
