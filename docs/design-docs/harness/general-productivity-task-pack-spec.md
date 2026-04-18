# If2Ai General Productivity Task Pack Spec

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 定义 If2Ai 面向日常事务处理用户的 harness task pack 标准，使 `information_work`、`communication_assistant`、`personal_operations`、`memory_driven_collaboration`、`messaging_collaboration` 可以被一致地编写、运行、评测和回归。

---

## 1. 为什么需要这份规格

`modules/harness` 已经在架构和契约层承认 `general daily productivity` 是主任务域，但如果没有 task pack spec，后续实现会很快退化成：

- 只有零散 task case
- 只有 prompt，没有环境和 expectation
- 只有文本评分，没有 evidence 和 continuity 约束
- 只有 coding pack，事务型 pack 长期缺位

因此需要一份可执行规格，把“事务型任务如何写成 harness task”固定下来。

---

## 2. 设计目标

这份规格解决 6 件事：

1. task pack 放在哪里、如何版本化
2. pack manifest 和 task spec 的文件结构
3. 事务型任务的 fixture 与 expectation 最小要求
4. 各 task category 的 authoring 模板
5. task pack 的校验、选择与运行规则
6. 与 runtime-local archive / workspace export bundle 的边界

---

## 3. 范围与边界

### 3.1 本文档覆盖什么

- `general productivity` 域 authored task packs
- pack manifest schema
- 单 task schema 的 pack authoring 约束
- evidence / memory / messaging / continuity fixture 规范
- 事务型 task 的验收模板

### 3.2 本文档不覆盖什么

- Rust 运行时代码实现
- grader 内部算法细节
- archive candidate frontier 结构
- coding task pack spec

后两者由以下文档负责：

- [modules-harness-archive-layout.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-archive-layout.md)
- [grader-scoring-rubric-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/grader-scoring-rubric-spec.md)

---

## 4. 路径与存储策略

### 4.1 Authored Task Packs

task pack 是版本化的设计资产，不属于 runtime archive。

推荐逻辑位置：

- repo 内标准 pack: `<workspace>/harness/task-packs/general-productivity/`
- 或 app bundle 内只读内建 pack: `app://harness/task-packs/general-productivity/`

它们用于：

- 研发编写与 review
- CI / 本地 eval
- 内建 benchmark 分发

### 4.2 Runtime Outputs

task pack 运行结果必须写入 runtime-local archive，而不是写回 authored pack。

逻辑路径：

- `If2Ai user-local data root / artifacts / harness / runs/...`

当前 Unix / macOS 默认：

- `~/.if2ai/artifacts/harness/runs/...`

### 4.3 Workspace Export Bundle

若要分享结果、做 PR review、贴 CI artifact，则从 runtime-local archive 导出到：

- `<workspace>/artifacts/harness-export/<bundle_id>/`

结论：

- task pack: 版本化输入资产
- archive run: 运行期事实资产
- export bundle: 派生分享资产

---

## 5. Task Pack 顶层目录布局

建议目录：

```text
harness/task-packs/general-productivity/
├── information-work/
│   ├── pack.toml
│   ├── tasks/
│   │   ├── multi-source-summary-001.yaml
│   │   └── evidence-compare-001.yaml
│   ├── fixtures/
│   │   ├── workspace/
│   │   ├── memory/
│   │   ├── evidence/
│   │   └── expected/
│   └── README.md
├── communication-assistant/
├── personal-operations/
├── memory-driven-collaboration/
└── messaging-collaboration/
```

设计要求：

1. 一个目录只承载一个主 category。
2. `pack.toml` 定义 pack 级元数据。
3. `tasks/` 存放 task 文件。
4. `fixtures/` 只存稳定、可序列化、可重放的输入资产。
5. `expected/` 可放结构化 expectation 示例，但最终仍以 task 文件字段为准。

---

## 6. Pack Manifest 规格

### 6.1 `pack.toml`

```toml
schema_version = 1
pack_id = "general-productivity/information-work"
pack_version = 1
name = "Information Work Pack"
task_domain = "GeneralProductivity"
category = "InformationWork"
owner = "harness"
status = "proposed"

[selection]
default_trial_count = 3
default_timeout_ms = 90000
recommended_weight = 1.0

[coverage]
requires_evidence_fixture = true
requires_user_experience_expectation = true
supports_memory = true
supports_messaging = false
```

### 6.2 必填字段语义

- `schema_version`
  pack schema 版本
- `pack_id`
  全局稳定 ID
- `pack_version`
  pack 语义版本
- `task_domain`
  对事务型 pack 必须为 `GeneralProductivity`
- `category`
  pack 主类别，必须与目录语义一致
- `selection`
  pack 默认 trial / timeout / sampling 参数
- `coverage`
  声明 pack 是否必须带 evidence、memory、messaging expectation

### 6.3 校验规则

1. `task_domain` 不允许为 `Engineering`
2. `category` 必须属于 5 个事务型 category 之一
3. `requires_evidence_fixture = true` 时，pack 内每个 task 都必须绑定 evidence fixture
4. `supports_messaging = true` 时，pack 内每个 task 都必须声明 routing expectation 或 draft-only expectation

---

## 7. 单 Task 文件规格

推荐格式：`YAML`

示例：

```yaml
schema_version: 1
id: "general-productivity/information-work/multi-source-summary-001"
version: 1
name: "Summarize three source files into decision memo"
description: "Read three local source notes and produce a concise memo with next steps."
task_domain: "GeneralProductivity"
category: "InformationWork"
difficulty: "Medium"
risk_level: "Low"
tags:
  - summary
  - evidence
  - next-steps

prompt:
  user_message: "请阅读资料并整理成一份给管理层的简报，包含事实、结论和下一步建议。"
  prior_messages: []
  system_overrides: []

setup:
  workspace_fixture: "fixtures/workspace/info-pack-001"
  evidence_fixture: "fixtures/evidence/info-pack-001.yaml"
  memory_fixture: "fixtures/memory/empty.yaml"
  permission_mode: "ReadWriteWorkspace"
  allow_tools: ["read_file", "glob_search", "write_file"]
  deny_tools: ["bash", "network_fetch"]
  fault_injections: []

expectations:
  output:
    format: "markdown"
  trace:
    must_read_files:
      - "sources/q1-note.md"
      - "sources/q2-note.md"
  outcome:
    required_artifacts:
      - "outputs/decision-memo.md"
  policy:
    must_stay_in_workspace: true
  user_experience:
    intent_alignment:
      must_cover: ["facts", "decision", "next steps"]
      must_avoid: ["unsupported claims"]
    evidence_fidelity:
      required_sources: ["q1-note", "q2-note", "q3-note"]
      citation_required: false
      allow_unverified_claims: false
    actionability:
      require_next_steps: true
      require_decision_summary: true
    tone_and_persona:
      channel: "memo"
      tone_profile: "concise-executive"

grading:
  profile: "general_information_work_v1"

execution:
  trial_count: 3
  timeout_ms: 90000
  max_turns: 12
```

### 7.1 额外约束

在一般事务 task 中，以下字段必须显式出现：

- `task_domain`
- `category`
- `setup.evidence_fixture` 或等价 evidence 输入
- `expectations.user_experience`
- `grading.profile`

---

## 8. 事务型 Task 的 Fixture 规格

### 8.1 Workspace Fixture

适用于：

- 信息整理
- 文件写入
- 总结结果落盘

要求：

- 输入文件固定
- 目录布局稳定
- 可 checksum 校验

### 8.2 Evidence Fixture

这是事务型 task 的核心新增 fixture。

建议结构：

```yaml
fixture_id: "info-pack-001"
sources:
  - source_id: "q1-note"
    kind: "file"
    path: "sources/q1-note.md"
    trust_level: "primary"
  - source_id: "q2-note"
    kind: "file"
    path: "sources/q2-note.md"
    trust_level: "primary"
required_sources:
  - "q1-note"
  - "q2-note"
forbidden_claims:
  - "未经资料支持的季度目标变化"
```

用途：

- 约束 agent 应读取哪些来源
- 约束哪些事实不能凭空生成
- 供 `EvidenceFidelityGrader` 做比对

### 8.3 Memory Fixture

适用于：

- 记忆驱动协作
- 延续性 follow-up
- 用户偏好保持

建议最小字段：

```yaml
fixture_id: "memory-pref-001"
seed_memories:
  - key: "tone.preference"
    value: "brief and direct"
  - key: "project.current_priority"
    value: "onboarding redesign"
forbidden_memories:
  - "deprecated.project_name"
```

### 8.4 Continuation Fixture

适用于：

- 中断恢复
- follow-up
- compaction 后接续任务

建议最小字段：

```yaml
fixture_id: "continuation-001"
prior_decisions:
  - "本周优先交付 onboarding redesign"
open_loops:
  - "需要给设计团队发出 follow-up"
must_preserve:
  - "不要重新讨论已经定下的方向"
```

### 8.5 Messaging Fixture

适用于：

- Slack / Telegram / Email / Discord
- 回复草稿与真实发送分流

建议最小字段：

```yaml
fixture_id: "message-route-001"
channel: "slack"
expected_target: "#design"
expected_recipient: null
mode: "draft_only"
thread_context:
  thread_id: "thread-123"
  platform: "slack"
```

---

## 9. 五类 Task Category 的 Authoring 规范

### 9.1 `InformationWork`

必填：

- evidence fixture
- intent alignment expectation
- evidence fidelity expectation
- actionability expectation

推荐 outcome：

- 生成摘要文件
- 生成决策 memo
- 生成结构化行动项

默认 blocking：

- `EvidenceFidelityGrader`

### 9.2 `CommunicationAssistant`

必填：

- tone/persona expectation
- actionability expectation
- continuity expectation

推荐 outcome：

- 产出可直接发送或可直接 review 的草稿

默认 blocking：

- `IntentAlignmentGrader`
- `ToneAndPersonaGrader`

### 9.3 `PersonalOperations`

必填：

- actionability expectation
- continuity expectation

推荐 outcome：

- TODO 列表
- 优先级排序
- follow-up 清单

默认 blocking：

- `ActionabilityGrader`

### 9.4 `MemoryDrivenCollaboration`

必填：

- memory fixture
- memory alignment expectation
- continuity expectation

默认 blocking：

- `MemoryAlignmentGrader`

### 9.5 `MessagingCollaboration`

必填：

- messaging fixture
- messaging routing expectation
- tone/persona expectation

默认 blocking：

- `MessagingRoutingGrader`

---

## 10. Pack 选择与运行策略

### 10.1 默认 Trial Policy

推荐默认：

- `InformationWork`: `trial_count = 3`
- `CommunicationAssistant`: `trial_count = 3`
- `PersonalOperations`: `trial_count = 3`
- `MemoryDrivenCollaboration`: `trial_count = 3`
- `MessagingCollaboration`: `trial_count = 3`，真实发送场景必须 `draft_only`

### 10.2 Stability 要求

事务型任务除了 pass/fail，还要关心：

- 意图是否稳定命中
- evidence 是否偶发漂移
- tone 是否波动过大
- message route 是否出现一次性严重错误

因此 pack authoring 时建议默认：

- `min_pass_trials = 2/3`
- `max_score_variance <= 0.20`
- message routing 任务若单次 misroute，直接总体失败

### 10.3 Offline First

事务型 pack 默认用于离线 eval，不直接在用户主链路实时自优化。

运行时只允许：

- record
- guardrail signal
- draft-only enforcement

---

## 11. Validation 规则

pack validator 至少要检查：

1. schema version 是否受支持
2. `task_domain/category` 是否匹配
3. fixture 路径是否存在
4. `expectations.user_experience` 是否完整
5. allow/deny tools 是否与风险等级冲突
6. messaging task 是否声明 `draft_only` 或明确发送门槛
7. memory task 是否声明 `required_memory_keys/forbidden_memory_keys`

校验失败时，不允许 pack 进入 `eval` 模式。

---

## 12. MVP 推荐 Pack 集

第一阶段建议先落 6 个 pack：

1. `information-work/multi-source-summary`
2. `information-work/compare-and-recommend`
3. `communication-assistant/follow-up-draft`
4. `communication-assistant/meeting-update-draft`
5. `memory-driven-collaboration/project-continuation`
6. `messaging-collaboration/draft-route-check`

理由：

- 能覆盖事务型主用户最高频路径
- 能尽早验证 evidence、memory、routing 三个 hardest problems

---

## 13. Authoring Checklist

每个事务型 task 在提交前都应回答：

1. 用户真正要推进的事务是什么
2. 哪些 source/evidence 是必须读取的
3. 哪些结论如果没有证据就不允许出现
4. 结果是否能直接推动下一步
5. 是否涉及 tone / persona / routing
6. 是否涉及长期 memory
7. 若 resume / compaction 发生，哪些 prior decisions 必须保留

---

## 14. 后续实施任务

### Task Group A: Pack Loader

1. 定义 `pack.toml` parser
2. 定义 task YAML parser
3. 定义 fixture resolver
4. 定义 validation errors

### Task Group B: Fixture Adapters

1. evidence fixture adapter
2. memory fixture adapter
3. continuation fixture adapter
4. messaging fixture adapter

### Task Group C: Template Library

1. 产出 5 个 category 的最小模板
2. 产出 6 个 MVP 示例 pack
3. 建 authoring README 与 review checklist

---

## 15. 与其他文档的关系

- 策略定位： [harness-strategy-v2.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/harness-strategy-v2.md)
- 子系统架构： [modules-harness-architecture.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-architecture.md)
- 核心契约： [modules-harness-core-contracts.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/modules-harness-core-contracts.md)
- 一般任务优化原则： [harness-general-task-optimization.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/harness-general-task-optimization.md)
