# Memory 与自我进化设计

> 面向 If2Ai 的记忆增强、自我进化与安全策略升级闭环设计。
>
> 最后更新: 2026-04-19
> 状态: Proposed

## 1. 设计目标

If2Ai 的“更智能”不应只体现在上下文变长，而应体现在：

1. 记住稳定事实
2. 记住用户偏好
3. 记住策略经验
4. 少犯重复错误
5. 在安全前提下逐步改进行为

因此 memory 与 self-evolution 要被设计成一个双回路系统，而不是零散模块集合。

## 2. 当前问题

当前仓库中已经存在：

- working memory
- pinned memory
- compiled memory
- summary / ticker / compiler
- learning / reflection / trajectory 模块

但还缺：

- 明确的记忆对象模型
- 写入准入策略
- recall usefulness 评估
- reflection 到策略升级的正式链路
- 自我进化的验证与回滚机制

## 3. 总体架构

建议采用双回路。

### 3.1 在线回路

服务当前 turn：

- scoped retrieval
- pinned memory injection
- compiled memory injection
- working memory packing
- context budget aware selection

### 3.2 离线回路

服务未来 turn：

- trace replay
- failure clustering
- reflection synthesis
- candidate strategy generation
- harness validation

## 4. 记忆对象模型

建议引入四类核心对象。

### 4.1 Fact

稳定事实。

例子：

- 用户使用中文优先
- 项目默认工作目录
- 某 provider 已配置完成

属性建议：

- stability: high
- scope: session/project/global
- evidence_required: true

### 4.2 Preference

偏好类信息。

例子：

- 喜欢 Staff 级视角
- 输出偏好简洁中文
- 先给结论再给细节

属性建议：

- stability: medium
- overrideable: true
- source: user explicit / repeated behavior

### 4.3 Strategy

对任务更有效的做法。

例子：

- 对 review 请求先给 findings 再给 summary
- 对大仓库扫描先做目录盘点再下钻
- 对 memory 注入优先 facts/pinned，再注入 compiled

属性建议：

- learned_from: reflection / human feedback / curated rule
- rollout_state: draft / candidate / active / rollback

### 4.4 Episode

一次任务的经验摘要。

例子：

- 某次恢复中断成功
- 某类 browser 任务在 headed 模式下失败
- 某 session 中 memory recall 命中但帮助不大

属性建议：

- time-bound
- linked_to_trace
- suitable_for_clustering

## 5. 写入策略

不是所有信息都应进入长期记忆。

建议准入规则：

1. 必须有明确 evidence
2. 必须能归入对象类型
3. 必须有 scope
4. 必须经过去重与冲突处理
5. 对敏感内容必须做 scrub / deny / prompt

### 5.1 准入决策

建议统一为：

- `allow`
- `deny`
- `prompt`

同时附带：

- `reason_code`
- `evidence_id`
- `memory_type`
- `scope`

## 6. 召回策略

召回目标不是“尽量多”，而是“尽量有用”。

建议排序维度：

1. scope match
2. semantic / lexical relevance
3. recency
4. importance
5. stability

### 6.1 注入顺序

建议顺序：

1. hard rules
2. pinned memory
3. critical facts
4. active preferences
5. compiled memory
6. episode summary

### 6.2 usefulness 评估

每次 recall 都应尽量记录：

- 是否被注入
- 是否被模型使用
- 是否改善结果
- 是否造成噪声

这将反过来影响后续排序与压缩。

## 7. Reflection 与策略生成

Reflection 不能只是生成“总结”，而应至少产出三类结构化结果：

1. 失败原因
2. 可改进策略
3. 是否值得进入 candidate

### 7.1 Reflection 输入

- trace
- outcome snapshot
- grader failures
- memory events
- user-visible errors

### 7.2 Reflection 输出

建议结构：

```text
ReflectionNote
  -> issue_type
  -> evidence
  -> proposed_strategy
  -> expected_gain
  -> risk_level
```

## 8. 自我进化闭环

建议将自我进化定义为以下流程：

1. 执行任务
2. 采集 trace / outcome
3. 提炼 reflection
4. 生成 candidate strategy
5. 用 harness 跑 compare
6. 通过后灰度启用
7. 观察指标
8. 保留或回滚

## 9. 安全约束

自我进化最容易出问题的地方，是让策略改写绕开治理。

必须约束：

1. 不能直接覆盖核心系统规则
2. 不能绕过 harness gate
3. 不能把单次成功样本直接推广为默认策略
4. 不能把未解释的 memory 写入作为长期事实
5. 不能让跨 scope 记忆泄漏进入 candidate 策略

## 10. 与 Harness 的接口

Memory/self-evolution 必须使用 Harness 作为裁判。

关键接口建议：

- `memory_alignment_score`
- `recall_usefulness_score`
- `strategy_candidate_compare`
- `rollback_reason`

只要 candidate strategy 会影响：

- memory 写入
- memory 召回
- tool 策略
- prompt 结构

都必须进入 compare 流程。

## 11. 产品层表现

用户可见的不应是“复杂算法”，而应是：

- 这个 agent 记住了什么
- 为什么记住
- 如何纠正
- 为什么这次建议更贴合上下文

建议产品表现重点：

1. 记忆证据面板
2. pinned/compiled 可编辑可回滚
3. 策略更新不直接暴露为技术细节，而体现在更稳定行为上

## 12. 实施建议

### 阶段 1

- 固化 Fact / Preference / Strategy / Episode schema
- 完成写入决策统一模型
- 增加 recall usefulness 埋点

### 阶段 2

- 让 reflection 产出 candidate strategy
- 建立 strategy registry
- 对 memory policy 做 compare

### 阶段 3

- 支持灰度启用与回滚
- 建立跨版本策略收益档案

## 13. 验收标准

成功落地后，应达到：

1. memory 对关键任务成功率有正向可验证提升
2. recall usefulness 可量化
3. strategy candidate 具备 compare/gate 流程
4. 策略上线后可回滚
5. 记忆系统不会因为“更聪明”而更不可控

## 14. 结论

If2Ai 的记忆与自我进化，必须建立在“可解释 + 可评测 + 可回滚”的前提上。

只有这样，它才会是能力增强，而不是复杂度增强。
