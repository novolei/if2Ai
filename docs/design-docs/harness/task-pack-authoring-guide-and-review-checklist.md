# If2Ai Task Pack Authoring Guide and Review Checklist

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 为 general productivity harness task pack 的编写、审查、维护与回归沉淀提供统一工作流、质量门槛和 review checklist，避免 pack 快速漂移成低质量 prompt case 集合。

---

## 1. 为什么需要这份文档

有了 pack spec 和 examples，还不够。

团队在真正开始写 pack 时，最容易出现的问题是：

- 把 task 写成 prompt collection
- fixture 不稳定
- expectation 太模糊
- grader 没法判定
- review 只看“像不像合理任务”，不看可执行性

这份文档的作用，就是把编写流程和 review 规则固定下来。

---

## 2. 适用范围

本文档适用于：

- `general-productivity` 域 task packs
- 第一批 authored packs
- 后续 regression 提炼出的事务型 task

当前重点覆盖：

- `information_work`
- `communication_assistant`
- `personal_operations`
- `memory_driven_collaboration`
- `messaging_collaboration`

---

## 3. Authoring 总流程

建议流程固定为 7 步：

1. 先写 task intent
2. 再列 evidence / memory / routing dependencies
3. 再设计 outcome 与 UX expectation
4. 再选 allow/deny tools 与风险等级
5. 再写 fixtures
6. 再写 task YAML / pack manifest
7. 最后做 self-review 和 reviewer review

顺序不要反过来。

最常见反模式就是先写 prompt，最后才补 expectation。

---

## 4. Authoring Step-by-Step

### 4.1 Step 1: 先定义用户真实意图

必须先写一句话：

`这个 task 帮用户推进的真实事务是什么？`

合格例子：

- “把多来源资料整理成决策 memo，帮助用户快速做判断”
- “根据会议纪要生成可直接发送的 follow-up 草稿”
- “延续已有项目上下文，输出下一步推进建议”

不合格例子：

- “总结一段文本”
- “写一封邮件”

原因：

- 这些描述太表层，无法指导 expectation 和 grading

### 4.2 Step 2: 列依赖事实

作者必须显式列出：

- 哪些 source/evidence 必须用
- 哪些 memory 可以用
- 哪些 memory 不能用
- 是否涉及路由 / 渠道 /线程
- 是否涉及 continuation / prior decisions

若这些事实列不出来，说明 task 还没有被 harness 化。

### 4.3 Step 3: 设计 expectation

必须至少回答：

1. outcome 要落在哪
2. intent 如何算命中
3. evidence 如何算忠实
4. result 是否必须可执行
5. tone/persona 是否有要求
6. continuity 是否必须保留
7. messaging 是否必须 draft-only

### 4.4 Step 4: 选择工具与风险等级

选择工具时要优先最小权限原则。

低风险事务型 task 的默认倾向：

- 允许：`read_file`、`glob_search`、`write_file`、`memory_retrieve`
- 默认拒绝：`bash`、真实发送工具、 destructive actions

若一个 task 需要高风险工具，必须额外写出理由。

### 4.5 Step 5: 设计 fixtures

fixtures 必须满足：

- 稳定
- 可重放
- 不含不必要噪声
- 能明确支撑 grader

最小 fixture 思维：

- workspace fixture 承载环境输入
- evidence fixture 承载事实边界
- memory fixture 承载长期上下文
- continuation fixture 承载 prior decisions / open loops
- messaging fixture 承载 route constraints

### 4.6 Step 6: 写 task YAML / `pack.toml`

只有在前 5 步都清楚后，才开始正式写文件。

写完后必须做：

- schema 校验
- 字段完整性校验
- risk/tool 一致性校验

### 4.7 Step 7: 做 review

review 不应只问“这个任务看起来合理吗”。

必须问：

- 这个任务真的可判定吗
- 这个任务真的能稳定重放吗
- 这个任务真的代表用户真实价值吗

---

## 5. Category-Specific Authoring 要点

### 5.1 `InformationWork`

重点：

- required sources 必须明确
- forbidden claims 必须明确
- 不能只测“总结是否通顺”

作者必须回答：

- 如果 agent 没读 source-b，也能靠常识写出貌似合理内容，grader 怎么抓到

### 5.2 `CommunicationAssistant`

重点：

- tone/profile 必须明确
- 不得引入用户未承诺的信息
- 要强调能否直接发送或直接 review

### 5.3 `PersonalOperations`

重点：

- 输出必须可执行
- 不能只是泛泛列点
- 必须有优先级或下一步结构

### 5.4 `MemoryDrivenCollaboration`

重点：

- required / forbidden memory keys 必须明确
- stale memory 是否 blocking 必须明确
- continuation 约束要写清楚

### 5.5 `MessagingCollaboration`

重点：

- route target 必须明确
- `draft_only` 默认 true
- 真实发送任务需额外审批，不建议进入第一批 pack

---

## 6. Review Checklist

每个 pack / task PR reviewer 至少检查以下 20 项。

### 6.1 Intent 与用户价值

1. task 是否代表真实高频事务，而不是人为拼出来的 prompt
2. 任务目标是否具体到“帮助用户推进什么”
3. 是否避免了只测表层文本生成

### 6.2 Expectation 与 Grading

4. 是否显式定义了 `task_domain` 与 `category`
5. 是否有完整的 `user_experience` expectation
6. 是否至少有一个明确的 blocking 风险
7. 是否能映射到已有 grader family
8. reviewer 是否能看懂这个 task 最终怎样算成功

### 6.3 Fixtures

9. workspace fixture 是否稳定
10. evidence fixture 是否足够支撑 fidelity grading
11. memory fixture 是否区分 required / forbidden
12. continuation fixture 是否能支撑 continuity grading
13. messaging fixture 是否清楚表达 route / mode

### 6.4 Tool / Risk / Policy

14. allow/deny tools 是否符合最小权限原则
15. risk_level 是否和任务真实风险匹配
16. 是否错误允许了 `bash` 或真实发送工具

### 6.5 Reproducibility

17. task 是否能脱离作者个人环境重放
18. fixture 是否依赖易变外部状态
19. task id / version / pack id 是否稳定

### 6.6 Long-Term Maintainability

20. 这个 task 将来失败后，是否容易沉淀成 failure taxonomy 与 regression 样本

---

## 7. Reviewer 拒绝条件

出现以下任一情况，review 应直接要求修改，不建议“先合再补”：

1. 没有 evidence fixture，但 task 需要证据忠实度
2. 没有 `user_experience` expectation
3. routing task 没有 `draft_only` 或等价发送边界
4. memory task 没有 required / forbidden memory keys
5. task outcome 无法结构化判断
6. task 依赖作者本地特殊环境
7. 高风险工具被无理由允许

---

## 8. 常见反模式

### 8.1 Prompt Collection Anti-Pattern

表现：

- 一堆 prompt
- 几乎没有 fixture
- expectation 只有“输出 markdown”

问题：

- 不能稳定评测
- 不能服务 learning

### 8.2 Summary-Only Anti-Pattern

表现：

- 只要求“总结一下”
- 没有 evidence / actionability / fidelity 约束

问题：

- 最容易把事务型 harness 做成表面分数游戏

### 8.3 Hidden Human Knowledge Anti-Pattern

表现：

- 作者脑里知道正确答案，但 fixture 和 expectation 没写出来

问题：

- reviewer 与 grader 都无法判断

### 8.4 Over-Noisy Fixture Anti-Pattern

表现：

- fixture 放了过多无关文件或噪声上下文

问题：

- 不利于定位 failure
- 增加任务方差

### 8.5 Unsafe Messaging Anti-Pattern

表现：

- messaging task 默认允许真实发送

问题：

- 风险过高，不适合 v1

---

## 9. 提交模板建议

每个新 task PR 建议附一段简短说明：

```md
## Task Intent
这个 task 用于评测什么真实事务场景：

## Key Expectations
- intent:
- evidence:
- actionability:
- memory:
- routing:

## Primary Graders
- 

## Primary Failure Taxonomy
- 
```

这样 reviewer 不需要从 YAML 生啃语义。

---

## 10. Pack 维护策略

### 10.1 什么时候该升 `version`

以下情况应升 `task version`：

- 任务 intent 变了
- required evidence 变了
- blocking 条件变了
- outcome artifact 路径或语义变了

### 10.2 什么时候只改 fixture

以下情况可以只改 fixture：

- 修复拼写
- 去掉无关噪声文件
- 补充更稳定的 seed data，但不改变语义

### 10.3 什么时候该 retire

以下情况应考虑 retire：

- 任务不再代表真实用户价值
- task 被更高质量版本取代
- fixture 依赖已废弃系统

---

## 11. 作者自检清单

提交前，作者应自问：

1. 如果我是 grader，这个 task 成败是否可判
2. 如果我是 reviewer，我是否能从文件而不是作者口头说明中理解任务
3. 如果我是 future proposer，这个 task 的失败是否足够可学习
4. 如果这个 task 连续失败，我能否明确归类到 taxonomy

若有任一答案是否定，task 还没准备好。

---

## 12. 推荐配套文档

- 编写规范： [general-productivity-task-pack-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/general-productivity-task-pack-spec.md)
- 示例集： [task-pack-examples-v1.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/task-pack-examples-v1.md)
- rubric： [grader-scoring-rubric-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/grader-scoring-rubric-spec.md)
