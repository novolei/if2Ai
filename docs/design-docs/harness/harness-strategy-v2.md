# If2Ai Harness Strategy v2

**版本**: 2.0  
**最后更新**: 2026-04-18  
**状态**: Proposed for implementation  
**一句话定位**: 为 If2Ai Agent App 设计一个 Rust/Tauri 原生 harness control plane，直接挂接 agent runtime、control plane、memory、trajectory 和 self-improvement，而不是依赖开发期 Python 测试框架。

---

## 1. 为什么现在要升级

### 1.0 运行时路径策略

本设计文档中的 `artifacts/harness/` 默认指的是逻辑路径：

`If2Ai user-local data root / artifacts / harness /`

在当前 If2Ai 代码风格下，Unix / macOS 上推荐默认实现为：

`~/.if2ai/artifacts/harness/`

这类 archive 是最终用户本地、实时生成、持续累积的运行时数据，不应默认写入当前 workspace 仓库。

与之对应，若需要分享、审查、CI 对比，则应从 runtime-local archive 中导出到：

`<workspace>/artifacts/harness-export/<bundle_id>/`

也就是：

- runtime-local archive：主经验库
- workspace export bundle：导出物，不是主 archive

If2Ai 现在真正可用于集成 app-native harness 的资产，在 `src-tauri`：

- `src-tauri/src/commands/agent.rs`
  已具备真实 agent loop、tool execution、streaming、resume、context compaction、trajectory recording。
- `src-tauri/src/modules/control_plane/session_context.rs`
  已有 `SessionExecutionContext`，可作为任务环境与边界隔离的基础上下文。
- `src-tauri/src/modules/control_plane/tool_execution_broker.rs`
  已有统一 tool broker，可作为 trace、policy、tool grader 的主入口。
- `src-tauri/src/modules/control_plane/audit.rs`
  已有结构化 audit event 和 trace id，可作为 transcript / trace grading 的原始数据源。
- `src-tauri/src/modules/learning/trajectory.rs`
  已有 `TrajectoryManager`，说明 app 内已经有失败轨迹与训练数据沉淀入口。
- `src-tauri/src/modules/learning/reflection.rs`
  已有 `ReflectionEngine`，说明 harness 结果天然可以回流到自我改进。
- `src-tauri/src/modules/memory/retrieval.rs`
  已有 `ActiveRetrievalManager`，可为 memory-aware eval 提供观测面。

但它距离现代大厂 agent harness 还有 4 个明显缺口：

1. 评估对象还偏“最终输出”，缺少 trace / transcript / outcome 分层。
2. 缺少可复现实验环境与任务状态断言，很多任务仍靠字符串匹配。
3. 缺少 failure clustering、adversarial generation、multi-trial 稳定性评估。
4. 现有 trajectory / reflection 能力还没有被 harness 主动编排成学习闭环。

### 1.1 用户域覆盖要求

Harness v2 不能只围绕 coding 场景设计。

If2Ai 的主用户群还包括大量日常事务处理需求者，他们的高频任务包括：

- 信息整理与研究总结
- 邮件、消息、汇报草稿
- TODO 提炼与 follow-up
- 文件与目录整理
- 基于长期偏好的持续协作

因此，harness 的顶层设计必须明确覆盖两类主域：

1. `engineering / coding`
2. `general daily productivity`

---

## 2. 外部最佳实践基线

以下结论来自官方文章、官方文档和近两年主流 benchmark 论文。

### 2.0 Meta-Harness 的关键结论

在完整阅读 [Meta-Harness: End-to-End Optimization of Model Harnesses](https://arxiv.org/pdf/2603.28052) 后，我们需要把几个判断升级为一等设计原则：

1. harness 是 stateful program，而不是 prompt 片段。
2. harness engineering 的核心问题是长时程 credit assignment。
3. raw code、raw scores、raw execution traces 的选择性访问，比压缩后的 summary 更适合 harness 优化。
4. outer loop 应保持 minimum necessary structure，只保留 archive、frontier、validation、external evaluation、append-only history。
5. evaluation 必须独立于 proposer，lightweight validation 也应独立于 proposer。

因此，If2Ai 的 harness 设计不仅要支持“评测当前 agent”，还必须预留“future proposer / optimizer 读取经验库并生成更好 harness”的基础设施。

### 2.1 OpenAI 的信号

- OpenAI 在 2026-02 发布的 [Harness engineering](https://openai.com/index/harness-engineering/) 强调，代码库首先要对 agent 可读，harness 不只是测功能，而是塑造“agent legibility”。
- OpenAI 官方 [Agent evals](https://platform.openai.com/docs/guides/agent-evals) 与 [Trace grading](https://platform.openai.com/docs/guides/trace-grading) 明确把 trace 当成一等公民，建议对整条工作流做可复现评分，而不是只看最终回答。
- [PaperBench](https://openai.com/index/paperbench/) 用层级 rubric 拆成 8,316 个可评分子任务，说明复杂 agent 任务需要“分解式 grading”。
- [MLE-bench](https://openai.com/index/mle-bench/) 展示了真实工程环境、开放脚手架和资源约束下的 agent 评估。
- [EVMbench](https://openai.com/index/introducing-evmbench/) 用 Rust harness 做 deterministic replay、隔离环境和 unsafe capability 限制，说明高风险任务必须依赖确定性环境和安全边界。

### 2.2 Anthropic 的信号

- Anthropic 在 2026-01 的 [Demystifying evals for AI agents](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents) 给出了最清晰的概念分层：
  `task -> trial -> grader -> transcript -> outcome -> harness -> scaffold`。
- 这篇文章特别强调 outcome 不等于 agent 的最终文本，而是环境里是否真的达成目标。
- Anthropic 在 2025-09 的 [Writing effective tools for AI agents](https://www.anthropic.com/engineering/writing-tools-for-agents) 强调工具设计必须通过 evaluation-driven iteration 打磨。
- Anthropic 在 2025-12 的 [Bloom](https://www.anthropic.com/research/bloom) 进一步表明：行为评估不能只手写 case，还要能自动生成场景，测频率、严重度和泛化。

### 2.3 Google / DeepMind 的信号

- DeepMind 在 2025-05 的 [AlphaEvolve](https://deepmind.google/blog/alphaevolve-a-gemini-powered-coding-agent-for-designing-advanced-algorithms/) 明确采用“proposal -> automated evaluators -> evolutionary selection”闭环。
- 这说明对 If2Ai 来说，harness 不该只是验收器，还应成为 prompt、tool policy、memory policy 迭代的优化器。

### 2.4 Microsoft 的信号

- Microsoft Research 的 [Agent-Pex](https://www.microsoft.com/en-us/research/project/agent-pex-automated-evaluation-and-testing-of-ai-agents/) 强调 specification-driven evaluation、trace-level evaluation 和 adversarial test generation。
- AutoGen v0.4 官方博客把 [AutoGen Bench](https://www.microsoft.com/en-us/research/blog/autogen-v0-4-reimagining-the-foundation-of-agentic-ai-for-scale-extensibility-and-robustness/) 作为 agent 开发基础设施的一部分，而不是外围工具。

### 2.5 论文平台上的高价值 benchmark

- [SWE-bench](https://arxiv.org/abs/2310.06770): 真实仓库 issue 修复，适合代码 agent。
- [WebArena](https://arxiv.org/abs/2307.13854): 真实网页环境、长链路任务、功能正确性评估。
- [GAIA](https://arxiv.org/abs/2311.12983): 通用助手能力，覆盖推理、多模态、工具使用。
- [OSWorld](https://arxiv.org/abs/2404.07972): 真实桌面与跨应用任务，最接近未来 If2Ai 桌面 agent 场景。
- [BrowseComp](https://arxiv.org/abs/2504.12516): 浏览型 agent 的 persistence 与信息检索能力。
- [FinRetrieval](https://arxiv.org/abs/2603.04403): 发布完整 tool traces，说明 trace 数据本身就是研究资产。

### 2.6 需要警惕的地方

- 2025-2026 的多篇论文指出 SWE-bench 可能存在污染或记忆效应，如 [Does SWE-Bench-Verified Test Agent Ability or Model Memory?](https://arxiv.org/abs/2512.10218)。
- 结论不是“不用 benchmark”，而是：
  1. 不要只押单一公开 benchmark。
  2. 必须建设项目私有、持续更新、贴近产品工作流的 eval set。

---

## 3. 对 If2Ai 最合适的策略框架

### 3.1 总体原则

If2Ai 适合采用 5 层、且完全 app-native 的 Harness 架构：

1. **Static Gates**
   `cargo check / cargo test / lint / symbol gate`
2. **Trace Evals**
   对 agent transcript、tool sequence、budget、policy decision 打分
3. **Outcome Evals**
   对文件、数据库、session、memory、tool side effects 做环境断言
4. **Scenario Generation**
   自动扩展 adversarial / regression / long-horizon / permission edge cases
5. **Learning Loop**
   将失败轨迹聚类后回流到 `trajectory` / `reflection` / `self-improvement`

并在任务域上明确支持：

- `engineering domain`
- `general productivity domain`

### 3.2 为什么这套最适合当前代码库

因为 If2Ai 当前的主产品面不是单一问答，而是：

- 桌面 agent
- 多工具调用
- 会话与工作目录强绑定
- 权限策略与控制平面很重要
- 未来还有 memory、skills、self-improvement

这决定了最终字符串正确性只是最外层信号。真正决定质量的是：

- 是否选择了对的工具
- 是否在对的工作目录执行
- 是否遵守权限与边界
- 是否在预算内完成
- 是否把环境改成了正确状态
- 失败是否可复现、可归因、可学习

对于一般事务任务，还必须额外对以下问题负责：

- 是否对齐了用户真实意图
- 是否忠于来源证据
- 是否产出了可执行结果
- 是否语气贴合用户和渠道
- 是否正确使用长期记忆
- 是否在 resume / compaction 后保持连续性

---

## 4. 推荐的目标架构

```text
Task Spec
  -> Harness Orchestrator (Rust)
  -> Trial Runner (Runtime-backed)
  -> Trace Collector (Audit + Agent Events)
  -> Graders
       -> output grader
       -> trace grader
       -> outcome grader
       -> safety grader
       -> cost grader
  -> Aggregator
  -> Regression Store
  -> Reflection / Learning Sink
```

### 4.1 新的核心数据模型

建议在 app 内新增 `src-tauri/src/modules/harness/`，并统一以下概念：

```text
src-tauri/src/modules/harness/
├── mod.rs
├── orchestrator.rs
├── task.rs
├── trial.rs
├── trace.rs
├── graders/
├── fixtures/
├── regression/
└── report.rs
```

建议核心数据模型：

- `TaskSpec`
  定义输入、环境 setup、工具 allowlist、目标 outcome、标签、风险等级。
- `TrialResult`
  一次实际运行结果；同一 task 默认跑 `n=3` trials。
- `TraceRecord`
  统一汇总：
  - agent stream events
  - `AuditEmitter` 事件
  - tool broker dispatch
  - permission decisions
  - resume cursor
  - compaction events
- `OutcomeSnapshot`
  任务结束时的文件系统、session、memory、数据库、artifacts 状态。
- `GradeBundle`
  多 grader 汇总结果，保留每维分数和失败证据。

### 4.2 Grader 维度

建议固定 6 个一级维度：

1. `task_success`
   任务是否完成。
2. `trace_quality`
   工具链路、计划质量、错误恢复、是否无效循环。
3. `policy_safety`
   权限、路径边界、敏感工具、安全策略是否遵守。
4. `resource_efficiency`
   token、wall time、tool count、retry count、compaction count。
5. `user_experience`
   输出是否清晰、是否承认不确定性、是否保留可执行结果。
6. `learning_value`
   失败样本是否可归类，是否能生成 reflection / memory patch 候选。

### 4.3 Trial 策略

不同任务不要一刀切。

- `deterministic` 任务：1 trial 即可
  例如 session persistence、tool registry、权限边界
- `semi_open` 任务：3 trials
  例如搜索、总结、规划
- `high_variance` 任务：5 trials + best/worst spread
  例如多工具研究任务、开放式 coding task

聚合时同时保存：

- `pass@k`
- `mean score`
- `variance`
- `worst-case failure mode`

### 4.4 一般事务任务的任务模型

对 `general daily productivity` 域，task 不能再被简化成“给一个 prompt，看最终回答像不像”。

v2 策略要求一般事务任务至少显式建模以下四类事实：

1. `intent`
   用户真正想完成什么，而不只是表层措辞。
2. `evidence`
   agent 依赖了哪些来源、文件、消息、memory 命中、工具结果。
3. `action`
   最终结果是否能直接推动用户下一步，而不是停留在泛泛总结。
4. `continuity`
   在 resume / compaction / follow-up 之后，任务是否保持上下文连续性。

因此一般事务 task pack 建议至少分成 5 大类：

1. `information_work`
   搜索、阅读、总结、对比、提炼。
2. `communication_assistant`
   邮件、消息、汇报、会议 follow-up 草稿。
3. `personal_operations`
   TODO 提炼、提醒、状态更新、下一步行动整理。
4. `memory_driven_collaboration`
   结合用户偏好、项目背景、历史决策做持续协作。
5. `messaging_collaboration`
   多平台消息路由、频道/收件人选择、发前确认与内容适配。

---

## 5. 适配 If2Ai 的测试金字塔

### Layer A: 代码与模块门禁

继续保留当前 `compile_gate` / `test_gate` / `symbol_gate`，但补充：

- `clippy_gate`
- `rustdoc_gate` 或 public API doc gate
- `tauri_command_contract_gate`
- `schema_drift_gate` for config / session / tool payload

### Layer B: Agent Loop Trace Evals

直接围绕 `src-tauri/src/commands/agent.rs`、`ToolExecutionBroker` 和 `AuditEmitter` 采集：

- `tool_called`
- `tool_result`
- `thinking`
- `permission decision`
- `compaction`
- `resume`
- `request_id`
- `task_outcome`

这里最值得新增的 grader：

- `NoDeadLoopGrader`
- `PermissionComplianceGrader`
- `WorkdirIsolationGrader`
- `ToolChoiceGrader`
- `CompactionQualityGrader`
- `ResumeIntegrityGrader`

### Layer C: 环境 outcome evals

这层要替代大量字符串断言。

示例：

- 文件是否真实写入目标工作区
- session message 顺序是否正确
- project/session 隔离是否保持
- memory retrieval 是否命中预期片段
- tool side effects 是否只发生在 allowlist 边界内

### Layer D: 真实工作流 suites

建议把 suite 分为 11 大类：

1. `control_plane`
2. `conversation_core`
3. `tool_use`
4. `memory_and_context`
5. `desktop_agent`
6. `self_improvement`
7. `information_work`
8. `communication_assistant`
9. `personal_operations`
10. `memory_driven_collaboration`
11. `messaging_collaboration`

其中后 5 类不是“将来再补”的外围内容，而是 If2Ai 面向事务型用户时必须长期持有的主 suite。

### Layer E: 对抗与生成式 evals

借鉴 Bloom / Agent-Pex：

- 从 design doc / tool schema / permission policy 自动反推出 edge cases
- 对已有失败轨迹做 paraphrase / constraint mutation / tool noise mutation
- 自动生成：
  - 缺失上下文
  - 误导性文件名
  - 冲突权限
  - 工具返回空结果
  - 中途中断再恢复

---

## 6. 与当前实现的差距映射

### 已有优势

- Rust runtime 已能输出丰富事件
- control plane 已有 trace id、audit event、session context
- 已有 learning / trajectory / reflection 模块雏形
- memory retrieval 已在 app 内可复用
- 已有 integration tests，可作为 app-native harness 的最底层回归样本来源

### 当前主要不足

1. 缺少 `modules/harness` 这样的 Rust 原生评测编排层。
2. `agent.rs`、`ToolExecutionBroker`、`AuditEmitter` 的事件还没统一成标准 trace schema。
3. 缺少 app 内的 `TaskSpec -> TrialRunner -> Graders -> Report` 闭环。
4. 缺少 outcome snapshot 抽象，暂时很难系统性断言工作目录、session、memory、tool side effects。
5. 还没有失败聚类、趋势比较和 regression corpus 管理。

---

## 7. 最优实施路线

### Phase 1: 建立 Rust 原生 trace-aware harness 骨架

目标：在 app 内落一个最小可用的 `modules/harness`。

实施：

- 新增 `src-tauri/src/modules/harness/`
- 定义 `TaskSpec`, `TrialResult`, `TraceRecord`, `GradeBundle`, `HarnessRunReport`
- 将 Rust agent runtime 的事件导出为结构化 JSON trace
- 统一接入 `AuditEmitter`、`ToolExecutionBroker`、stream token payload
- 增加 `TraceGrader` trait 和 2-3 个基础 grader

### Phase 2: 建 outcome-based task runner

目标：让 harness 直接运行真实 If2Ai agent task。

实施：

- 新增 Rust 版 `EnvironmentFixture`
- 支持 `setup -> run -> inspect -> teardown`
- 支持 file/session/project/memory 断言器
- 用工作目录快照保证可复现
- 为一般事务任务补 `evidence snapshot`、`message route snapshot`、`continuity snapshot`

### Phase 3: 建 regression corpus

目标：每次线上/手测失败都能沉淀为 eval。

实施：

- 增加 `artifacts/evals/regressions/`
- 每个 failure 保存：
  - prompt
  - tool trace
  - runtime config
  - expected outcome
  - failure taxonomy
- CI 对关键 regression 必跑
- 对事务型失败增加 `intent drift`、`evidence drift`、`memory misuse`、`misroute` taxonomy

### Phase 4: 建 adversarial generation

目标：不仅测已知失败，也主动找新失败。

实施：

- 从 tool schema、permission rules、path policy 自动生成任务变体
- 对历史失败样本做 mutation
- 引入 spec-driven negative tests

### Phase 5: 接入 self-improvement

目标：harness 成为学习飞轮，而不是报告生成器。

实施：

- 失败任务自动产出 reflection 草稿
- 调用 `ReflectionEngine` 产出 pattern candidates
- 按 failure cluster 汇总 prompt/tool/memory 改进候选
- 将高价值失败样本导入 `TrajectoryManager`

---

## 8. If2Ai 建议采用的任务契约

建议使用 Rust struct，而不是把 app runtime 绑定到 Python suite schema：

```rust
pub struct TaskSpec {
    pub id: String,
    pub category: TaskCategory,
    pub prompt: String,
    pub risk_level: RiskLevel,
    pub trial_count: u8,
    pub setup: TaskSetup,
    pub expected_outcome: ExpectedOutcome,
    pub grader_weights: Vec<GraderWeight>,
}
```

`TaskSetup` 最少应包含：

- workspace fixture
- session fixture
- provider fixture
- allow/disallow tools
- permission mode
- forced compaction / interrupt / retry injection

对于一般事务任务，还建议补：

- evidence fixture
- channel / recipient fixture
- memory seed fixture
- follow-up continuation fixture
- tone / persona expectation profile

`ExpectedOutcome` 最少应包含：

- output assertions
- trace assertions
- file system assertions
- session assertions
- memory assertions
- policy assertions

---

## 9. 指标体系

项目层面建议长期跟踪这些核心指标：

- `task_success_rate`
- `worst_case_pass_rate`
- `policy_violation_rate`
- `dead_loop_rate`
- `mean_tool_calls`
- `p95_latency`
- `token_per_success`
- `resume_success_rate`
- `compaction_regression_rate`
- `regression_reopen_rate`
- `intent_alignment_rate`
- `evidence_fidelity_violation_rate`
- `actionability_pass_rate`
- `memory_alignment_rate`
- `continuity_success_rate`
- `messaging_misroute_rate`

其中最重要的不是单次最高分，而是：

- 方差是否下降
- 最坏情况是否改善
- 安全违规是否接近零
- 新功能上线是否拉低老 suite
- 对事务型用户来说，误发消息、误引证据、误用记忆是否持续下降

---

## 10. 对项目的直接建议

If2Ai 现在最优解不是直接照搬某一家，而是做一个混合框架：

- 用 **OpenAI** 的 `trace grading + rubric decomposition + deterministic harness`
- 用 **Anthropic** 的 `task/trial/grader/transcript/outcome` 概念分层
- 用 **Google DeepMind** 的 `automated evaluator -> optimizer loop`
- 用 **Microsoft** 的 `spec-driven evaluation + adversarial generation`
- 用 **WebArena / OSWorld / SWE-bench / GAIA** 的公开 benchmark 思路指导任务构造
- 同时避免单 benchmark 绑架，建立 If2Ai 私有 regression corpus

这套混合方案是当前代码库的局部最优，因为它：

- 直接嵌进现有 Rust runtime / control plane / learning 模块
- 不把 app 核心能力依赖在开发期 Python harness 上
- 可以逐层交付，而不是“大重构后再见”
- 能直接服务未来的 memory、desktop control plane、self-improvement

---

## 11. 推荐的近期落地清单

未来两个迭代优先级建议如下：

1. 在 `src-tauri/src/modules/harness/` 建最小骨架。
2. 在 Rust agent runtime 导出结构化 trace artifacts。
3. 接通 `AuditEmitter`、`ToolExecutionBroker`、`TrajectoryManager`。
4. 补第一批 `information_work` 与 `communication_assistant` task packs。
5. 定义 `IntentAlignmentGrader`、`EvidenceFidelityGrader`、`MemoryAlignmentGrader`、`MessagingRoutingGrader` 的 v1 规格。
4. 新增 5 个 grader：
   `ToolChoice`, `PermissionCompliance`, `WorkdirIsolation`, `NoDeadLoop`, `OutcomeSnapshot`.
5. 建立 `artifacts/evals/regressions/` 并把已知 bug 失败样本产品化。
6. 为 `memory`, `context compression`, `resume` 建独立 task packs。
7. 为后续 RL / self-improvement 预留 failure cluster export。

---

## 12. 参考来源

- OpenAI, [Harness engineering: leveraging Codex in an agent-first world](https://openai.com/index/harness-engineering/), 2026-02
- OpenAI Platform, [Agent evals](https://platform.openai.com/docs/guides/agent-evals), 2026 可访问版本
- OpenAI Platform, [Trace grading](https://platform.openai.com/docs/guides/trace-grading), 2026 可访问版本
- OpenAI, [PaperBench](https://openai.com/index/paperbench/), 2025-04
- OpenAI, [MLE-bench](https://openai.com/index/mle-bench/), 2024-10 页面版本
- OpenAI, [Introducing EVMbench](https://openai.com/index/introducing-evmbench/), 2026-03
- Anthropic, [Demystifying evals for AI agents](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents), 2026-01
- Anthropic, [Writing effective tools for AI agents](https://www.anthropic.com/engineering/writing-tools-for-agents), 2025-09
- Anthropic, [Introducing Bloom](https://www.anthropic.com/research/bloom), 2025-12
- Google DeepMind, [AlphaEvolve](https://deepmind.google/blog/alphaevolve-a-gemini-powered-coding-agent-for-designing-advanced-algorithms/), 2025-05
- Microsoft Research, [Agent-Pex](https://www.microsoft.com/en-us/research/project/agent-pex-automated-evaluation-and-testing-of-ai-agents/), 2026 可访问版本
- Microsoft Research, [AutoGen v0.4](https://www.microsoft.com/en-us/research/blog/autogen-v0-4-reimagining-the-foundation-of-agentic-ai-for-scale-extensibility-and-robustness/), 2024-11 页面版本
- SWE-bench, [arXiv:2310.06770](https://arxiv.org/abs/2310.06770)
- WebArena, [arXiv:2307.13854](https://arxiv.org/abs/2307.13854)
- GAIA, [arXiv:2311.12983](https://arxiv.org/abs/2311.12983)
- OSWorld, [arXiv:2404.07972](https://arxiv.org/abs/2404.07972)
- BrowseComp, [arXiv:2504.12516](https://arxiv.org/abs/2504.12516)
- FinRetrieval, [arXiv:2603.04403](https://arxiv.org/abs/2603.04403)
- Does SWE-Bench-Verified Test Agent Ability or Model Memory?, [arXiv:2512.10218](https://arxiv.org/abs/2512.10218)
- Lee et al., [Meta-Harness: End-to-End Optimization of Model Harnesses](https://arxiv.org/pdf/2603.28052), 2026-03

---

## 13. Meta-Harness 对 If2Ai 的具体设计修正

### 13.1 从“评测系统”升级为“评测 + 经验基底”

吸收论文后，`modules/harness` 的定位必须扩展成两层：

1. **evaluation control plane**
   运行 task、采 trace、做 grading、写 report
2. **experience substrate**
   保存 candidate code、score、trace、outcome、diff、frontier，供 future proposer / optimizer 选择性检索

### 13.2 从单一 score 升级为 Pareto 优化面

Meta-Harness 维护 Pareto frontier，而不是只追一个 aggregate score。这对 If2Ai 很重要，因为我们未来至少有 4 个彼此冲突的优化目标：

- task success
- context tokens / token cost
- latency
- policy safety

因此框架层必须支持：

- 单次运行的多 axis 打点
- frontier materialization
- top-k by axis
- run-to-run diff

### 13.3 从 task pack 升级为 search/validation/test 三分法

Meta-Harness 的重要隐含前提是：proposer 可见历史，但不应污染 final evaluation。

因此 If2Ai 需要把任务进一步区分为：

- `validation`
  用于快速过滤不合法 candidate
- `search_set`
  proposer 可见，驱动外循环搜索
- `final_test`
  proposer 不可见，用于真正的泛化验收

### 13.4 从 summary-first 升级为 raw-trace-first

论文的主论点之一是：summary 会丢失 credit assignment 所需信息。

因此我们的规则应明确写死：

- summary 只做索引
- raw trace 永远保留
- candidate code 永远保留
- diff artifact 永远保留
- outcome snapshot 永远保留

### 13.5 从“agent 评测”升级为“harness 候选评测”

当前文档主要面向 runtime 级 agent eval。吸收 Meta-Harness 后，需要再补一个对象层次：

- `Runtime Harness`
  当前产品中的 conversation + tool + memory + policy 过程
- `Candidate Harness`
  未来被 proposer 修改后的 harness 实现单元
- `Meta-Harness`
  搜索、验证、评测、归档 candidate harness 的外层系统

换句话说，If2Ai 的 `modules/harness` 设计，最终要能同时支持：

1. 评测一个 agent run
2. 比较多个 harness candidate
3. 为 future harness optimizer 提供 archive
