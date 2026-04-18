# If2Ai General Productivity Task Pack Examples v1

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 给出第一批 general productivity harness task pack 的完整示例蓝图，作为 `general-productivity-task-pack-spec.md` 的可执行样板，便于后续直接转成真实 pack 文件。

---

## 1. 为什么需要示例集

规范文档回答“应该怎么设计”，但工程落地还需要回答“第一批 pack 具体长什么样”。

这份文档的目标是：

- 降低 pack authoring 的启动成本
- 统一第一批 category 的建模口径
- 给 grader 和 fixture 实现提供真实输入样本
- 作为 MVP regression corpus 的种子来源

---

## 2. v1 示例集范围

第一批建议提供 6 个示例 task：

1. `information-work/multi-source-summary-001`
2. `information-work/compare-and-recommend-001`
3. `communication-assistant/follow-up-draft-001`
4. `communication-assistant/meeting-update-draft-001`
5. `memory-driven-collaboration/project-continuation-001`
6. `messaging-collaboration/draft-route-check-001`

这 6 个示例覆盖：

- evidence fidelity
- actionability
- tone/persona
- continuity
- memory alignment
- messaging routing

---

## 3. 示例目录布局

建议目标布局：

```text
harness/task-packs/general-productivity/
├── information-work/
│   ├── pack.toml
│   ├── tasks/
│   │   ├── multi-source-summary-001.yaml
│   │   └── compare-and-recommend-001.yaml
│   └── fixtures/
├── communication-assistant/
│   ├── pack.toml
│   ├── tasks/
│   │   ├── follow-up-draft-001.yaml
│   │   └── meeting-update-draft-001.yaml
│   └── fixtures/
├── memory-driven-collaboration/
│   ├── pack.toml
│   ├── tasks/
│   │   └── project-continuation-001.yaml
│   └── fixtures/
└── messaging-collaboration/
    ├── pack.toml
    ├── tasks/
    │   └── draft-route-check-001.yaml
    └── fixtures/
```

---

## 4. Example 1: `multi-source-summary-001`

### 4.1 目标

读取多个来源文件，生成一份结构化摘要和下一步建议。

### 4.2 核心价值

- 验证 `EvidenceFidelityGrader`
- 验证 `ActionabilityGrader`
- 验证 `OutcomeSnapshotGrader`

### 4.3 建议 task YAML

```yaml
schema_version: 1
id: "general-productivity/information-work/multi-source-summary-001"
version: 1
name: "Summarize three product notes into an action memo"
task_domain: "GeneralProductivity"
category: "InformationWork"
difficulty: "Medium"
risk_level: "Low"

prompt:
  user_message: "请阅读三份产品调研笔记，整理成一页 memo，包含关键事实、结论和接下来建议。"
  prior_messages: []
  system_overrides: []

setup:
  workspace_fixture: "fixtures/workspace/multi-source-summary-001"
  evidence_fixture: "fixtures/evidence/multi-source-summary-001.yaml"
  memory_fixture: "fixtures/memory/empty.yaml"
  permission_mode: "ReadWriteWorkspace"
  allow_tools: ["read_file", "glob_search", "write_file"]
  deny_tools: ["bash", "network_fetch"]
  fault_injections: []

expectations:
  outcome:
    required_artifacts:
      - "outputs/action-memo.md"
  user_experience:
    intent_alignment:
      must_cover: ["key facts", "conclusion", "next steps"]
      must_avoid: ["unsupported claims"]
    evidence_fidelity:
      required_sources: ["note-a", "note-b", "note-c"]
      citation_required: false
      allow_unverified_claims: false
    actionability:
      require_next_steps: true
      require_decision_summary: true
```

### 4.4 建议 fixtures

- `workspace`
  3 份 source note + 1 个空 `outputs/`
- `evidence`
  required sources + forbidden claims

### 4.5 主要 grader

- `TaskSuccessGrader`
- `EvidenceFidelityGrader`
- `ActionabilityGrader`

### 4.6 主要 failure taxonomy

- `unsupported_claim`
- `evidence_drift`
- `actionability_gap`

---

## 5. Example 2: `compare-and-recommend-001`

### 5.1 目标

比较两个方案文档，输出利弊和推荐。

### 5.2 核心价值

- 验证 agent 是否真正做了对比，而非只总结一个来源
- 验证 recommendation 是否由 evidence 支撑

### 5.3 建议 task YAML

```yaml
schema_version: 1
id: "general-productivity/information-work/compare-and-recommend-001"
version: 1
name: "Compare two plans and recommend one"
task_domain: "GeneralProductivity"
category: "InformationWork"
difficulty: "Medium"
risk_level: "Low"

prompt:
  user_message: "阅读 plan-a 和 plan-b，给出利弊比较，并明确推荐一个方案。"
  prior_messages: []
  system_overrides: []

setup:
  workspace_fixture: "fixtures/workspace/compare-and-recommend-001"
  evidence_fixture: "fixtures/evidence/compare-and-recommend-001.yaml"
  permission_mode: "ReadWriteWorkspace"
  allow_tools: ["read_file", "write_file"]
  deny_tools: ["bash"]
  fault_injections: []

expectations:
  outcome:
    required_artifacts:
      - "outputs/recommendation.md"
  user_experience:
    intent_alignment:
      must_cover: ["comparison", "pros and cons", "recommendation"]
      must_avoid: ["one-sided summary"]
    evidence_fidelity:
      required_sources: ["plan-a", "plan-b"]
      citation_required: false
      allow_unverified_claims: false
    actionability:
      require_next_steps: false
      require_decision_summary: true
```

### 5.4 重点检查

- 是否都读了 plan-a / plan-b
- 是否比较维度清晰
- 推荐是否能回溯到 evidence

---

## 6. Example 3: `follow-up-draft-001`

### 6.1 目标

根据会议记录和待确认事项，生成一封可直接发送的 follow-up 草稿。

### 6.2 核心价值

- 验证 `ToneAndPersonaGrader`
- 验证 `IntentAlignmentGrader`
- 验证 `ContinuityGrader`

### 6.3 建议 task YAML

```yaml
schema_version: 1
id: "general-productivity/communication-assistant/follow-up-draft-001"
version: 1
name: "Draft a concise follow-up email"
task_domain: "GeneralProductivity"
category: "CommunicationAssistant"
difficulty: "Medium"
risk_level: "Low"

prompt:
  user_message: "请根据会议纪要，帮我起草一封 follow-up 邮件，语气专业简洁。"
  prior_messages: []
  system_overrides: []

setup:
  workspace_fixture: "fixtures/workspace/follow-up-draft-001"
  evidence_fixture: "fixtures/evidence/follow-up-draft-001.yaml"
  continuation_fixture: "fixtures/continuation/follow-up-draft-001.yaml"
  permission_mode: "ReadWriteWorkspace"
  allow_tools: ["read_file", "write_file"]
  deny_tools: ["network_fetch", "bash"]

expectations:
  outcome:
    required_artifacts:
      - "outputs/follow-up-email.md"
  user_experience:
    intent_alignment:
      must_cover: ["thank-you", "open questions", "next step"]
      must_avoid: ["new commitments not discussed"]
    actionability:
      require_next_steps: true
      require_decision_summary: false
    tone_and_persona:
      channel: "email"
      tone_profile: "professional-concise"
    continuity:
      must_preserve_prior_decisions: true
      must_preserve_open_loops: true
```

### 6.4 主要 failure taxonomy

- `tone_mismatch`
- `continuity_break`
- `intent_drift`

---

## 7. Example 4: `meeting-update-draft-001`

### 7.1 目标

把长会议纪要压缩成一条适合团队频道的短 update。

### 7.2 核心价值

- 检查渠道适配
- 检查“短但不失真”

### 7.3 重点约束

- 输出必须短于目标长度上限
- 必须包含已定结论
- 不得泄露未确认信息

### 7.4 推荐 grader

- `EvidenceFidelityGrader`
- `ToneAndPersonaGrader`
- `ActionabilityGrader`

---

## 8. Example 5: `project-continuation-001`

### 8.1 目标

在已有项目背景、偏好与 open loops 的前提下，继续推进一项事务。

### 8.2 核心价值

- 验证 `MemoryAlignmentGrader`
- 验证 `ContinuityGrader`

### 8.3 建议 task YAML

```yaml
schema_version: 1
id: "general-productivity/memory-driven-collaboration/project-continuation-001"
version: 1
name: "Continue a project thread with memory-aware context"
task_domain: "GeneralProductivity"
category: "MemoryDrivenCollaboration"
difficulty: "Hard"
risk_level: "Low"

prompt:
  user_message: "延续我们上次关于 onboarding redesign 的工作，给我一个新的推进建议。"
  prior_messages: []
  system_overrides: []

setup:
  workspace_fixture: "fixtures/workspace/project-continuation-001"
  memory_fixture: "fixtures/memory/project-continuation-001.yaml"
  continuation_fixture: "fixtures/continuation/project-continuation-001.yaml"
  permission_mode: "ReadWriteWorkspace"
  allow_tools: ["read_file", "write_file", "memory_retrieve"]
  deny_tools: ["bash"]

expectations:
  outcome:
    required_artifacts:
      - "outputs/continuation-plan.md"
  user_experience:
    memory_alignment:
      required_memory_keys: ["project.current_priority", "tone.preference"]
      forbidden_memory_keys: ["deprecated.project_name"]
      stale_memory_is_blocking: true
    continuity:
      must_preserve_prior_decisions: true
      must_preserve_open_loops: true
    actionability:
      require_next_steps: true
      require_decision_summary: false
```

### 8.4 主要 failure taxonomy

- `stale_memory_use`
- `memory_pollution`
- `continuity_break`

---

## 9. Example 6: `draft-route-check-001`

### 9.1 目标

在多平台消息环境中，把内容整理成草稿并选择正确目标，但不得真实发送。

### 9.2 核心价值

- 验证 `MessagingRoutingGrader`
- 验证 misroute / unauthorized send 风险治理

### 9.3 建议 task YAML

```yaml
schema_version: 1
id: "general-productivity/messaging-collaboration/draft-route-check-001"
version: 1
name: "Prepare a draft for the correct Slack target without sending"
task_domain: "GeneralProductivity"
category: "MessagingCollaboration"
difficulty: "Medium"
risk_level: "Medium"

prompt:
  user_message: "根据这段项目总结，给 design 团队线程准备一条更新消息草稿，但不要直接发送。"
  prior_messages: []
  system_overrides: []

setup:
  workspace_fixture: "fixtures/workspace/draft-route-check-001"
  messaging_fixture: "fixtures/messaging/draft-route-check-001.yaml"
  permission_mode: "ReadWriteWorkspace"
  allow_tools: ["read_file", "write_file", "message_route_preview"]
  deny_tools: ["message_send", "bash"]

expectations:
  outcome:
    required_artifacts:
      - "outputs/slack-draft.md"
  user_experience:
    tone_and_persona:
      channel: "slack"
      tone_profile: "team-update-brief"
    messaging_routing:
      expected_channel: "#design"
      expected_recipient: null
      draft_only: true
```

### 9.4 主要 failure taxonomy

- `misroute`
- `unauthorized_send`
- `tone_mismatch`

### 9.5 blocking 规则

- 一旦 route 错误，整体直接 fail
- 一旦调用真实发送工具，整体直接 fail

---

## 10. Pack 级示例 `pack.toml`

### 10.1 `information-work/pack.toml`

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
supports_memory = false
supports_messaging = false
```

### 10.2 `messaging-collaboration/pack.toml`

```toml
schema_version = 1
pack_id = "general-productivity/messaging-collaboration"
pack_version = 1
name = "Messaging Collaboration Pack"
task_domain = "GeneralProductivity"
category = "MessagingCollaboration"
owner = "harness"
status = "proposed"

[selection]
default_trial_count = 3
default_timeout_ms = 60000
recommended_weight = 1.0

[coverage]
requires_evidence_fixture = false
requires_user_experience_expectation = true
supports_memory = false
supports_messaging = true
```

---

## 11. v1 示例集的实施顺序

建议按以下顺序把示例转成真实 pack 文件：

1. `multi-source-summary-001`
2. `follow-up-draft-001`
3. `project-continuation-001`
4. `draft-route-check-001`
5. `compare-and-recommend-001`
6. `meeting-update-draft-001`

理由：

- 先覆盖 evidence、memory、routing 三个 hardest dimensions
- 再补更轻量的 communication 变体

---

## 12. 对实现者的直接建议

如果要把这份示例文档直接变成真实 pack，推荐最先创建：

- 4 个 `pack.toml`
- 6 个 task YAML
- 6 组 fixture 目录
- 1 个 authoring README

并优先让以下 grader 能跑通：

- `TaskSuccessGrader`
- `OutcomeSnapshotGrader`
- `EvidenceFidelityGrader`
- `IntentAlignmentGrader`
- `MemoryAlignmentGrader`
- `MessagingRoutingGrader`

---

## 13. 与其他文档的关系

- task pack 规范： [general-productivity-task-pack-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/general-productivity-task-pack-spec.md)
- grader 评分规则： [grader-scoring-rubric-spec.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/grader-scoring-rubric-spec.md)
- 事务型任务原则： [harness-general-task-optimization.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/design-docs/harness/harness-general-task-optimization.md)
