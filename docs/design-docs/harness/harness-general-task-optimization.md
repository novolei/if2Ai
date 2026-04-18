# If2Ai Harness for General Daily Tasks

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 将 `modules/harness` 从 coding / control-plane 导向扩展为覆盖用户日常事务处理的通用 agent 优化框架，使信息整理、沟通辅助、日程跟进、文件处理和记忆驱动协作都能被持续评测与改进。

---

## 1. 为什么这份文档重要

If2Ai 的目标用户不只是工程师。大量真实使用场景来自日常事务处理，例如：

- 搜索后整理信息
- 总结会议、生成待办
- 帮用户起草消息、邮件、汇报
- 归档文件、整理目录、提炼重点
- 基于历史偏好持续协作
- 中断后恢复工作流

如果 harness 只覆盖 coding，它只能优化一小部分 agent 价值；如果 harness 能覆盖日常任务，它就会真正成为产品能力进化的核心基础设施。

---

## 2. 设计目标

这份文档定义 5 件事：

1. 一般性日常任务是否适合 harness 化
2. 哪些日常任务应进入 task taxonomy
3. 如何定义 success / outcome / quality
4. online guardrails 和 offline optimization 应如何分工
5. memory、messaging、workspace、多工具链路如何纳入评测

---

## 3. 核心结论

### 3.1 Harness 不只服务 coding

Harness 的本质不是“给代码打分”，而是：

- 让 agent 行为可重复
- 让任务结果可观察
- 让失败模式可归因
- 让系统能从真实任务中持续变好

因此，只要任务满足下面至少一部分条件，就适合进入 harness：

1. 可定义目标
2. 可观察执行过程
3. 可判断结果好坏
4. 可重复出现
5. 值得被持续优化

日常任务往往比 coding 更高频，所以长期价值更大。

### 3.2 日常任务的 harness 目标不是“完全自动化”

更准确的目标是：

- 让 agent 更稳定
- 让 agent 更少犯低级错误
- 让 agent 更会选工具
- 让 agent 更会利用记忆
- 让 agent 更贴近用户的真实意图

### 3.3 优化方式要分成在线与离线两层

这是整个设计里最重要的边界之一。

#### 在线优化

运行时实时介入：

- 记录 trace
- 检测异常模式
- 触发保护、降级或重试
- 根据已知 failure mode 选择更稳策略

#### 离线优化

基于累积任务样本持续改进：

- prompt architecture
- tool policy
- memory retrieval policy
- compaction policy
- task pack
- candidate harness

推荐原则：

- **在线负责保护与约束**
- **离线负责真正的系统优化**

---

## 4. 一般性任务的 taxonomy

建议把非 coding 日常任务明确纳入 `TaskCategory` 的扩展域。

### 4.1 信息整理类

定义：

- 搜索、阅读、汇总、对比、提炼

典型任务：

- 搜索某主题并生成三点摘要
- 比较多篇文档的差异
- 提取一个长 PDF 的关键行动项

关键评测点：

- 搜索策略是否合理
- 是否引用了足够证据
- 总结是否忠于来源
- 是否出现幻觉补全

### 4.2 沟通辅助类

定义：

- 帮用户起草、改写、跟进、总结沟通内容

典型任务：

- 写一封礼貌但简洁的跟进邮件
- 根据会议记录生成 Slack update
- 把长聊天记录整理成回复草稿

关键评测点：

- 语气是否符合用户偏好
- 是否遗漏关键上下文
- 是否引入不应承诺的信息
- 是否输出了可直接发送的成品

### 4.3 文件与工作区处理类

定义：

- 对本地文件、目录、文档、表格等进行读取、整理和改动

典型任务：

- 整理下载目录
- 重命名文件并建立目录结构
- 从多个文档抽取信息写入一个摘要文件

关键评测点：

- 是否在正确 workdir 内操作
- 是否发生越界写入
- 是否正确更新目标文件
- 是否避免了破坏性改动

### 4.4 日程与事务跟进类

定义：

- 帮用户追踪任务、提醒、整理待办和后续动作

典型任务：

- 根据今天的对话生成 TODO 列表
- 识别需要 follow-up 的事项
- 汇总项目当前状态和阻塞项

关键评测点：

- 待办是否可执行
- 是否按优先级排序
- 是否遗漏明确承诺的事项
- 是否正确沿用了历史上下文

### 4.5 记忆驱动协作类

定义：

- 基于用户长期偏好、历史习惯和以往任务持续协作

典型任务：

- 按用户偏好的语气写摘要
- 根据历史项目背景继续推进
- 复用用户既有决策风格做建议

关键评测点：

- 是否检索到了正确的长期记忆
- 是否错误引用旧事实
- 是否记忆污染了当前任务
- 是否在压缩后仍保持用户画像一致

### 4.6 多平台消息协作类

定义：

- 在 Telegram / Slack / Discord / Email 等消息环境中完成任务

典型任务：

- 从群聊提炼决策
- 在指定线程回复一个 update
- 把外部消息转换成项目任务

关键评测点：

- session/thread 绑定是否正确
- 平台上下文是否保留
- 是否发送到了正确目标
- 是否遵守平台长度与格式限制

---

## 5. 为什么一般任务也适合 harness

### 5.1 Outcome 并不总是模糊

很多日常任务虽然不像 coding 那样有单元测试，但 outcome 并不等于不可判断。

例如：

- “把这段会议纪要整理为 5 条待办”
  outcome 可以判断是否产出了 5 条、是否包含关键决策、是否每条可执行
- “把总结写入 `meeting-summary.md`”
  outcome 可以判断文件是否存在、内容是否完整
- “写一封跟进邮件”
  outcome 可以判断结构、语气、事实一致性、是否可直接发送

### 5.2 一般任务更依赖用户体验维度

这类任务比 coding 更强调：

- clarity
- tone
- trustworthiness
- contextual fit
- memory alignment

所以 harness 需要把 UX 维度抬到和 correctness 同样重要的位置。

---

## 6. 面向日常任务的 grader 扩展

现有 harness 文档已经有：

- `task_success`
- `trace_quality`
- `policy_safety`
- `resource_efficiency`
- `user_experience`
- `learning_value`

对一般任务，建议把这 6 维再细化为更可实现的 grader 族。

### 6.1 `IntentAlignmentGrader`

问题：

- agent 是否真正理解用户要什么

关注点：

- 是否回答了用户实际任务
- 是否没有偏离目标
- 是否没有过度执行

### 6.2 `EvidenceFidelityGrader`

问题：

- 总结、归纳、对比是否忠于来源

关注点：

- 是否遗漏关键事实
- 是否加入未被来源支持的内容
- 是否引用了正确片段

### 6.3 `ActionabilityGrader`

问题：

- 输出是否可执行，而不只是“看起来合理”

关注点：

- TODO 是否具体
- follow-up 是否可操作
- 邮件/消息是否可直接发送

### 6.4 `ToneAndPersonaGrader`

问题：

- 输出语气是否符合用户偏好和场景

关注点：

- 是否过度正式或过度随意
- 是否符合历史沟通风格
- 是否在敏感情境下保持稳妥

### 6.5 `MemoryAlignmentGrader`

问题：

- agent 是否正确使用长期记忆

关注点：

- 是否命中正确记忆
- 是否错误使用过期记忆
- 是否记忆与当前任务冲突

### 6.6 `ContinuityGrader`

问题：

- 中断、恢复、压缩后任务是否保持连续

关注点：

- 是否重复工作
- 是否丢关键上下文
- 是否能接续上一轮产物

### 6.7 `MultiToolWorkflowGrader`

问题：

- 多工具链路是否高效、合理、无多余跳转

关注点：

- tool choice
- tool order
- unnecessary retries
- dead loops

---

## 7. 日常任务的 outcome 设计

### 7.1 输出类 outcome

适用于：

- 摘要
- 邮件草稿
- 回复草稿
- TODO 列表

判定方式：

- 结构约束
- 必要概念覆盖
- 事实一致性
- 可执行性

### 7.2 环境类 outcome

适用于：

- 文件写入
- 文件更新
- 目录整理
- 任务状态持久化

判定方式：

- 文件存在
- 文件内容变化
- 越界写入检测
- 会话状态变化

### 7.3 消息类 outcome

适用于：

- 多平台发送
- 线程回复
- 消息草稿准备

判定方式：

- 目标 thread/chat 是否正确
- 消息格式是否符合平台约束
- 长度限制是否满足

### 7.4 记忆类 outcome

适用于：

- profile 更新
- user preference 应用
- 长期上下文沿用

判定方式：

- 是否调用了正确 retrieval
- 是否写入了不该持久化的内容
- 是否形成了可复用事实

---

## 8. online vs offline optimization 边界

### 8.1 在线优化该做什么

建议只做低风险、强约束的事情：

1. 记录 trace
2. 检测循环或异常重试
3. 检测权限和 workdir 边界风险
4. 在已知 failure mode 上触发保守 fallback
5. 在 resume / compaction 时做状态保护

### 8.2 在线优化不该做什么

不建议在用户每次任务中：

1. 自动改写 harness 代码
2. 动态变更核心评测规则
3. 基于单次结果直接更新长期优化策略
4. 未经验证就切换到实验性 candidate harness

### 8.3 离线优化该做什么

应该由 archive + task pack + regression + proposer 驱动：

1. 持续发现高频失败模式
2. 更新 grader 和 task pack
3. 优化 prompt architecture
4. 优化 tool policy
5. 优化 memory retrieval policy
6. 优化 compaction / resume 策略

### 8.4 为什么这样切分

因为一般任务场景里，用户最敏感的是：

- 结果稳不稳
- 会不会突然跑偏
- 会不会乱发消息
- 会不会错误记住东西

所以运行时必须更保守，系统进化更多放到离线闭环里。

---

## 9. 与 memory system 的关系

日常任务比 coding 更依赖 memory。

### 9.1 Harness 应评测什么

1. retrieval 是否命中正确记忆
2. retrieval 是否过度召回
3. 是否误用了旧偏好
4. 是否把临时上下文误写成长期事实
5. compaction 后是否还保留关键用户画像

### 9.2 推荐的 memory task pack

建议增加专门 task pack：

- `memory/user-preference-alignment`
- `memory/longitudinal-project-context`
- `memory/avoid-stale-memory`
- `memory/conclusion-quality`

---

## 10. 与 messaging gateway 的关系

如果未来多平台消息协作是重点，harness 不能只看单轮输出，还要看：

- session 绑定是否正确
- thread continuity 是否正确
- 平台适配是否正确
- 发送前是否做了安全审查

建议增加 task pack：

- `messaging/thread-reply-routing`
- `messaging/daily-digest`
- `messaging/follow-up-detection`
- `messaging/channel-tone-adaptation`

---

## 11. 面向一般任务的 task pack 设计建议

### Pack A: Daily Information Work

包含：

- 搜索并总结
- 多文档对比
- 提炼重点
- 写简报

### Pack B: Communication Assistant

包含：

- 邮件起草
- Slack/Telegram update
- 跟进消息
- 会议总结转回复

### Pack C: Personal Operations

包含：

- TODO 提炼
- 日程跟进
- 状态更新
- 文件归档

### Pack D: Memory-Driven Collaboration

包含：

- 偏好对齐
- 长期项目延续
- 历史约束复用
- 避免记忆污染

---

## 12. 适用于一般任务的优化指标

除了已有指标，建议对一般任务增加：

- `intent_alignment_rate`
- `actionability_rate`
- `evidence_fidelity_rate`
- `tone_alignment_rate`
- `memory_alignment_rate`
- `continuity_success_rate`
- `message_misroute_rate`
- `stale_memory_error_rate`

这些指标比“编译通过率”更贴近事务型用户的真实体验。

---

## 13. 关键设计决策

### 决策 1: 日常任务必须进入 task taxonomy

结论：必须。

理由：

- 目标用户里高频、长期、真实价值最大的任务很多都在这一类。

### 决策 2: UX grader 在一般任务里是一等公民

结论：是。

理由：

- 一般任务的失败很多不是“没做”，而是“做得不可信、不好用、不贴用户”。

### 决策 3: 在线优化以 guardrail 为主

结论：必须。

理由：

- 事务型用户对稳定性和可预测性更敏感。

### 决策 4: memory eval 必须被制度化

结论：必须。

理由：

- 对事务型用户来说，错误记忆往往比工具失败更伤体验。

---

## 14. 实施任务

### Phase A: taxonomy 扩展

1. 把 general daily tasks 纳入 `TaskCategory`
2. 增加信息整理、沟通辅助、事务跟进、记忆协作等 pack

### Phase B: grader 扩展

1. 定义 `IntentAlignmentGrader`
2. 定义 `EvidenceFidelityGrader`
3. 定义 `ActionabilityGrader`
4. 定义 `ToneAndPersonaGrader`
5. 定义 `MemoryAlignmentGrader`
6. 定义 `ContinuityGrader`

### Phase C: memory / messaging integration

1. 增加 memory-aware task fixtures
2. 增加 messaging-aware task fixtures
3. 增加 session/thread continuity assertions

### Phase D: optimization policy split

1. 定义 online guardrail policy
2. 定义 offline optimization loop
3. 定义哪些改进可以自动上线，哪些必须人工 review

---

## 15. 验收标准

当以下条件满足时，可认为 harness 已经不再只服务 coding：

1. 至少有 4 组面向日常任务的 task pack
2. 至少有 5 个面向一般任务的专用 grader
3. memory task pack 可识别 stale memory 错误
4. messaging task pack 可识别 misroute / wrong-thread 错误
5. online / offline optimization 边界被文档和实现共同约束

---

## 16. 与整体 harness 文档的关系

这份文档是对以下文档的“任务域扩展”：

- [harness-strategy-v2.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/harness-strategy-v2.md)
- [modules-harness-architecture.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-architecture.md)
- [modules-harness-core-contracts.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-core-contracts.md)
- [modules-harness-implementation-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-implementation-plan.md)

它不替代这些文档，而是保证 harness 设计从第一天起就覆盖非 coding 主用户群。
