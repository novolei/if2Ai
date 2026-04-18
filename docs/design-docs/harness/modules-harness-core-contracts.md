# If2Ai `modules/harness` 核心契约设计

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 定义 `modules/harness` 的稳定数据契约与 trait 契约，作为后续 Rust 实现、CI 接入、UI 可视化和 learning pipeline 的统一基线。

---

## 1. 设计目标

### 1.1 路径语义约束

所有 `ArchiveRef`、artifact path、trace path 在契约层都应理解为：

- 主路径：runtime-local archive 内相对路径
- 导出路径：workspace export bundle 内相对路径

默认情况下，主路径应解析到：

`If2Ai user-local data root / artifacts / harness /`

而不是 workspace 根目录。

这份文档聚焦 5 类核心契约：

1. `TaskSpec`
2. `TraceRecord`
3. `OutcomeSnapshot`
4. `Grader` trait
5. `HarnessRunReport`

要求：

- 对 app 内真实 runtime 可直接落地
- 对 provider、模型、平台尽量无关
- 可版本化
- 可用于回归、比较、学习回流
- 可被 future proposer 通过 filesystem/CLI 高效查询

---

## 2. `TaskSpec` 设计

### 2.1 设计意图

`TaskSpec` 代表“一个可复现、可评分、可比较的 agent 任务定义”。

它不是简单 prompt，而是以下要素的组合：

- 输入
- 环境
- 边界
- 期望 outcome
- grader 配置
- trial 稳定性要求

### 2.2 Rust 契约

```rust
pub struct TaskSpec {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub description: String,
    pub category: TaskCategory,
    pub difficulty: TaskDifficulty,
    pub risk_level: RiskLevel,
    pub prompt: TaskPrompt,
    pub setup: TaskSetup,
    pub expectations: TaskExpectations,
    pub grading: GradingPlan,
    pub execution: ExecutionPlan,
    pub task_domain: TaskDomain,
    pub tags: Vec<String>,
}
```

```rust
pub enum TaskDomain {
    Engineering,
    GeneralProductivity,
    Hybrid,
}
```

### 2.3 字段语义

#### `id`

- 全局稳定 ID
- 用于 regression 追踪、趋势比较、report 聚合
- 一旦发布，不可随意重用

建议格式：

- `control-plane/workdir-isolation/001`
- `memory/compaction/002`

#### `version`

- task 语义版本号
- 当期望 outcome 或环境 setup 发生不兼容变化时增加

#### `category`

建议枚举：

```rust
pub enum TaskCategory {
    ConversationCore,
    ToolUse,
    ControlPlane,
    MemoryAndContext,
    ResumeAndRecovery,
    DesktopAgent,
    SelfImprovement,
    Regression,
    InformationWork,
    CommunicationAssistant,
    PersonalOperations,
    MemoryDrivenCollaboration,
    MessagingCollaboration,
}
```

#### `difficulty`

用来做 sampling 与评测分层：

```rust
pub enum TaskDifficulty {
    Easy,
    Medium,
    Hard,
    LongHorizon,
}
```

#### `risk_level`

决定默认运行策略、隔离级别、是否允许 destructive tool：

```rust
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

#### `prompt`

建议显式拆为：

```rust
pub struct TaskPrompt {
    pub user_message: String,
    pub prior_messages: Vec<SeedMessage>,
    pub system_overrides: Vec<String>,
}
```

原因：

- 真实 agent 表现高度依赖 prior context
- 只存最终 user prompt 会丢失对话状态

### 2.4 `TaskSetup`

```rust
pub struct TaskSetup {
    pub workspace_fixture: Option<WorkspaceFixtureRef>,
    pub session_fixture: Option<SessionFixtureRef>,
    pub provider_fixture: Option<ProviderFixtureRef>,
    pub memory_fixture: Option<MemoryFixtureRef>,
    pub permission_mode: Option<PermissionMode>,
    pub allow_tools: Vec<String>,
    pub deny_tools: Vec<String>,
    pub env_overrides: BTreeMap<String, String>,
    pub fault_injections: Vec<FaultInjection>,
}
```

#### 为什么必须有 `allow_tools` 和 `deny_tools`

因为 agent eval 既要测能力，也要测 policy compliance。

例如：

- 任务允许 `read_file` 但不允许 `bash`
- 如果 agent 调了 `bash`，即使最后结果正确，也应被安全 grader 扣分

#### `fault_injections`

用于恢复性和鲁棒性评估：

```rust
pub enum FaultInjection {
    InterruptAfterToolResult,
    ProviderTimeoutOnce,
    EmptyToolResult { tool_name: String },
    PermissionPromptDenyOnce { tool_name: String },
    ForceCompaction,
}
```

### 2.5 `TaskExpectations`

```rust
pub struct TaskExpectations {
    pub output: OutputExpectation,
    pub trace: TraceExpectation,
    pub outcome: OutcomeExpectation,
    pub policy: PolicyExpectation,
    pub user_experience: UserExperienceExpectation,
}
```

拆成四层是因为：

- 有些任务文本重要
- 有些任务 trace 重要
- 有些任务环境结果重要
- 有些任务权限合规重要
- 一般事务任务还会把 UX、tone、证据忠实度、行动性拉到核心位置

```rust
pub struct UserExperienceExpectation {
    pub intent_alignment: Option<IntentAlignmentExpectation>,
    pub evidence_fidelity: Option<EvidenceFidelityExpectation>,
    pub actionability: Option<ActionabilityExpectation>,
    pub tone_and_persona: Option<TonePersonaExpectation>,
    pub memory_alignment: Option<MemoryAlignmentExpectation>,
    pub continuity: Option<ContinuityExpectation>,
    pub messaging_routing: Option<MessagingRoutingExpectation>,
}
```

建议最小字段如下：

```rust
pub struct IntentAlignmentExpectation {
    pub must_cover: Vec<String>,
    pub must_avoid: Vec<String>,
}

pub struct EvidenceFidelityExpectation {
    pub required_sources: Vec<String>,
    pub citation_required: bool,
    pub allow_unverified_claims: bool,
}

pub struct ActionabilityExpectation {
    pub require_next_steps: bool,
    pub require_decision_summary: bool,
}

pub struct TonePersonaExpectation {
    pub channel: Option<String>,
    pub tone_profile: Option<String>,
}

pub struct MemoryAlignmentExpectation {
    pub required_memory_keys: Vec<String>,
    pub forbidden_memory_keys: Vec<String>,
    pub stale_memory_is_blocking: bool,
}

pub struct ContinuityExpectation {
    pub must_preserve_prior_decisions: bool,
    pub must_preserve_open_loops: bool,
}

pub struct MessagingRoutingExpectation {
    pub expected_channel: Option<String>,
    pub expected_recipient: Option<String>,
    pub draft_only: bool,
}
```

这些 expectation 的目标不是把事务任务重新做成模板生成器，而是把“用户真正关心的完成条件”结构化下来。

### 2.6 `ExecutionPlan`

```rust
pub struct ExecutionPlan {
    pub trial_count: u8,
    pub timeout_ms: u64,
    pub max_turns: Option<u32>,
    pub stability_policy: StabilityPolicy,
}
```

```rust
pub struct StabilityPolicy {
    pub min_pass_trials: u8,
    pub max_score_variance: Option<f32>,
    pub fail_fast: bool,
}
```

### 2.7 设计要求

Requirements:

1. `TaskSpec` 必须能脱离具体模型厂商理解。
2. `TaskSpec` 必须支持 deterministic 和 stochastic 任务。
3. `TaskSpec` 必须支持 environment fixture。
4. `TaskSpec` 必须可序列化并可版本化。
5. `TaskSpec` 必须能表达 coding 与一般事务双主域任务。
6. 一般事务任务必须能显式约束 intent、evidence、actionability、memory 和 messaging routing。

---

## 3. `TraceRecord` 设计

### 3.1 设计意图

`TraceRecord` 是 harness 的事实来源。

任何 grader 若能只用 trace 完成，就不应回退到日志文本。

### 3.2 Rust 契约

```rust
pub struct TraceRecord {
    pub trace_id: String,
    pub schema_version: u32,
    pub task_id: Option<String>,
    pub trial_id: String,
    pub session_id: String,
    pub request_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub events: Vec<TraceEvent>,
    pub summary: TraceSummary,
}
```

### 3.3 `TraceEvent`

```rust
pub enum TraceEvent {
    TurnStarted(TurnStartedEvent),
    TurnFinished(TurnFinishedEvent),
    LlmRequested(LlmRequestedEvent),
    LlmResponded(LlmRespondedEvent),
    ToolCalled(ToolCalledEvent),
    ToolResult(ToolResultEvent),
    PolicyDecision(PolicyDecisionEvent),
    ContextCompacted(ContextCompactedEvent),
    ResumeOffered(ResumeOfferedEvent),
    ResumeApplied(ResumeAppliedEvent),
    ReflectionCompleted(ReflectionCompletedEvent),
    OutcomeResolved(OutcomeResolvedEvent),
}
```

### 3.4 为什么不直接复用 `AgentEvent`

现有 `AgentEvent` 很有价值，但它仍是“采集层事件”，不是最终评测 schema。

我们需要 `TraceRecord`，因为：

1. 它要同时吸收 `AgentEvent` 与 `AuditEvent`
2. 它要有固定 schema version
3. 它要为 graders 提供统一字段
4. 它要支持 task/trial 关联

### 3.5 `TraceSummary`

```rust
pub struct TraceSummary {
    pub success: bool,
    pub total_turns: u32,
    pub llm_calls: u32,
    pub total_tool_calls: u32,
    pub total_errors: u32,
    pub total_duration_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub permission_prompts: u32,
    pub compaction_count: u32,
    pub resume_count: u32,
    pub degraded_reason: Option<String>,
}
```

### 3.6 数据来源映射

#### 来自 `AgentEvent`

- turn start / finish
- llm requested / responded
- tool called / result
- context compacted
- reflection completed

#### 来自 `AuditEvent`

- policy decision made
- tool execution started / failed / finished
- effective workdir
- permission mode

#### 来自 session / runtime state

- task outcome
- resume cursor
- degraded reason

### 3.7 设计要求

Requirements:

1. trace 必须可 JSON 序列化。
2. trace 必须支持版本化迁移。
3. trace summary 必须可被 grader 快速消费。
4. trace event 不允许依赖不可稳定解析的自然语言 message。

---

## 4. `OutcomeSnapshot` 设计

### 4.1 设计意图

`OutcomeSnapshot` 表示 trial 结束时环境状态。

它回答的是：

- 文件是否改对了
- session 是否正确持久化
- memory 是否命中或被污染
- 是否真的完成目标

### 4.2 Rust 契约

```rust
pub struct OutcomeSnapshot {
    pub workspace: WorkspaceSnapshot,
    pub session: SessionSnapshot,
    pub memory: Option<MemorySnapshot>,
    pub control_plane: ControlPlaneSnapshot,
    pub artifacts: Vec<ArtifactRef>,
}
```

### 4.3 资源快照

#### `WorkspaceSnapshot`

```rust
pub struct WorkspaceSnapshot {
    pub root: PathBuf,
    pub created_files: Vec<PathBuf>,
    pub modified_files: Vec<PathBuf>,
    pub deleted_files: Vec<PathBuf>,
    pub outside_workspace_modifications_detected: bool,
}
```

#### `SessionSnapshot`

```rust
pub struct SessionSnapshot {
    pub session_id: String,
    pub message_count: usize,
    pub assistant_message_count: usize,
    pub last_task_outcome: Option<String>,
    pub resume_available: bool,
}
```

#### `MemorySnapshot`

```rust
pub struct MemorySnapshot {
    pub retrieval_queries: u32,
    pub retrieval_hits: u32,
    pub inserted_memories: u32,
    pub compaction_applied: bool,
}
```

#### `ControlPlaneSnapshot`

```rust
pub struct ControlPlaneSnapshot {
    pub effective_workdir: PathBuf,
    pub permission_mode: PermissionMode,
    pub permission_denials: u32,
    pub policy_violations_detected: u32,
}
```

### 4.4 为什么 outcome snapshot 必须是独立对象

原因：

1. trace 是“过程”
2. outcome 是“结果”

很多错误只看 trace 看不出来：

- agent 正确调用了工具，但没真正写入目标文件
- agent 输出了成功话术，但 session 状态没持久化

### 4.5 设计要求

Requirements:

1. snapshot 尽量结构化，不依赖 diff 文本解析。
2. snapshot 构建应可局部复用，不要求每类任务都填满所有字段。
3. snapshot 支持未来扩展桌面 UI 状态、browser DOM state 等资源。

---

## 5. `Grader` trait 设计

### 5.1 设计意图

Grader 是 harness 的核心判定插件。

它必须支持：

- 可组合
- 可并行
- 可解释
- 有证据输出

### 5.2 Rust 契约

```rust
#[async_trait]
pub trait Grader: Send + Sync {
    fn id(&self) -> &'static str;
    fn dimension(&self) -> GraderDimension;
    fn supports(&self, task: &TaskSpec) -> bool;
    async fn grade(&self, input: &GradeInput) -> HarnessResult<GradeResult>;
}
```

```rust
pub struct GradeInput<'a> {
    pub task: &'a TaskSpec,
    pub trace: &'a TraceRecord,
    pub outcome: &'a OutcomeSnapshot,
    pub runtime_metadata: &'a RuntimeMetadata,
}
```

```rust
pub struct GradeResult {
    pub grader_id: String,
    pub dimension: GraderDimension,
    pub score: f32,
    pub passed: bool,
    pub confidence: f32,
    pub evidence: Vec<GradeEvidence>,
    pub summary: String,
}
```

### 5.3 `GraderDimension`

```rust
pub enum GraderDimension {
    TaskSuccess,
    TraceQuality,
    PolicySafety,
    ResourceEfficiency,
    UserExperience,
    LearningValue,
}
```

### 5.4 `GradeEvidence`

```rust
pub enum GradeEvidence {
    TraceRef { event_index: usize, reason: String },
    FileRef { path: PathBuf, reason: String },
    SessionFact { key: String, value: String },
    PolicyFact { decision: String, reason: String },
    MemoryFact { key: String, value: String, reason: String },
    MessageRouteFact { target: String, reason: String },
    MetricFact { name: String, value: f64, threshold: Option<f64> },
}
```

### 5.5 为什么 grader 必须输出 `confidence`

原因：

- deterministic grader 的置信度应接近 1
- model-based grader 的置信度会更低
- 聚合层可以根据 confidence 做加权或报警

### 5.6 最小 grader 集

第一版必须设计 5 个基础 grader：

1. `TaskSuccessGrader`
2. `ToolChoiceGrader`
3. `PermissionComplianceGrader`
4. `NoDeadLoopGrader`
5. `OutcomeSnapshotGrader`

同时必须设计一组一般事务 grader family 规格：

1. `IntentAlignmentGrader`
2. `EvidenceFidelityGrader`
3. `ActionabilityGrader`
4. `ToneAndPersonaGrader`
5. `MemoryAlignmentGrader`
6. `ContinuityGrader`
7. `MessagingRoutingGrader`

#### `TaskSuccessGrader`

目的：

- 判断任务总体是否完成

主要证据：

- output expectation
- outcome snapshot
- task_outcome

#### `ToolChoiceGrader`

目的：

- 判断 agent 是否选择了适当工具集合

主要证据：

- trace 中工具序列
- allow/deny tools
- 是否出现冗余或违禁工具

#### `PermissionComplianceGrader`

目的：

- 判断 agent 是否遵守 control plane 边界

主要证据：

- `AuditEvent::policy_decision_made`
- `PermissionOutcome`
- `effective_workdir`

#### `NoDeadLoopGrader`

目的：

- 检测无效工具循环、重复思考循环、预算耗尽式失败

主要证据：

- 重复 tool call pattern
- max turns 命中
- 无增量 outcome

#### `OutcomeSnapshotGrader`

目的：

- 验证环境结果是否达成

主要证据：

- workspace snapshot
- session snapshot
- memory snapshot

#### `IntentAlignmentGrader`

目的：

- 判断 agent 是否真正回应了用户意图，而不是只围绕表层措辞组织文本。

主要证据：

- `TaskSpec.expectations.user_experience.intent_alignment`
- trace 中的 retrieval / tool / plan 选择
- 最终输出与 action items

#### `EvidenceFidelityGrader`

目的：

- 判断总结、汇报、草稿是否忠于实际来源，而不是生成貌似合理但未经证实的内容。

主要证据：

- 检索命中
- 文件读取记录
- search/browser tool 结果
- 输出中的引用与事实断言

#### `MemoryAlignmentGrader`

目的：

- 判断 agent 是否命中了正确记忆，并避免把过期或无关记忆误用到当前任务。

主要证据：

- memory retrieval trace
- memory snapshot
- 与 task fixture 中 required / forbidden memory keys 的比对

#### `MessagingRoutingGrader`

目的：

- 判断消息是否被路由到正确渠道、正确收件人，以及是否遵守 draft-only / confirm-first 边界。

主要证据：

- route selection trace
- messaging gateway 事件
- `MessagingRoutingExpectation`

### 5.7 评分聚合

建议聚合契约：

```rust
pub struct GradeBundle {
    pub results: Vec<GradeResult>,
    pub weighted_score: f32,
    pub passed: bool,
    pub blocking_failures: Vec<String>,
}
```

聚合规则：

1. 存在 blocking grader 失败时，总体直接失败
2. 否则按权重求分
3. 记录最低分维度作为首要失败原因
4. `MessagingRoutingGrader` 在真实发送任务中默认是 blocking grader
5. `EvidenceFidelityGrader` 在研究、总结、汇报类任务中可被配置为 blocking grader
6. `MemoryAlignmentGrader` 若命中 `stale_memory_is_blocking`，必须直接进入高优先级 failure taxonomy

---

## 6. `HarnessRunReport` 设计

### 6.1 Rust 契约

```rust
pub struct HarnessRunReport {
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub task_results: Vec<TaskRunResult>,
    pub summary: HarnessSummary,
}
```

```rust
pub struct TaskRunResult {
    pub task_id: String,
    pub task_version: u32,
    pub trials: Vec<TrialResult>,
    pub aggregate: TaskAggregateResult,
}
```

```rust
pub struct TaskAggregateResult {
    pub pass_rate: f32,
    pub mean_score: f32,
    pub variance: f32,
    pub worst_failure_mode: Option<String>,
    pub regression_detected: bool,
}
```

### 6.2 为什么 report 要保留 variance

agent 的关键问题不只是平均分低，而是高方差。

高方差会直接导致：

- 用户体验不稳定
- regression 难定位
- RL 训练样本噪声变大

---

## 7. 约束与迁移策略

### 7.1 向后兼容

现有 `AgentEvent`、`SessionRecorder` 不需要立刻废弃。

迁移方式：

1. 先增加 `TraceRecord` 适配层
2. 再让 `SessionRecorder` 可落 `TraceRecord`
3. 最后让 graders 改用新 schema

### 7.2 可测试性要求

每个契约都必须支持：

- JSON round-trip
- deterministic fixture 构造
- snapshot assertion

### 7.3 可移植性要求

这些契约不能假设：

- 某一家 LLM provider
- 某一种 UI 平台
- 某一种外部 benchmark

---

## 8. 后续实施任务

### Task Group A: Schema

1. 定义 `TaskSpec` 与其子结构
2. 定义 `TraceRecord` / `TraceEvent`
3. 定义 `OutcomeSnapshot`
4. 定义 `HarnessRunReport`

### Task Group B: Trait

1. 定义 `Grader` trait
2. 定义 `Fixture` trait
3. 定义 `TrialRunner` trait
4. 定义 `ReportSink` trait

### Task Group C: Adapters

1. 从 `AgentEvent` 到 `TraceEvent` 的 adapter
2. 从 `AuditEvent` 到 `TraceEvent` 的 adapter
3. 从 session / workspace 状态到 `OutcomeSnapshot` 的 adapter

---

## 9. Meta-Harness 增量契约

### 9.1 `SearchRole`

为避免 search/test 泄漏，建议在 `TaskSpec` v1 就预留：

```rust
pub enum SearchRole {
    Validation,
    SearchSet,
    FinalTest,
}
```

### 9.2 `ArchivePolicy`

```rust
pub struct ArchivePolicy {
    pub persist_trace: bool,
    pub persist_outcome: bool,
    pub proposer_visible: bool,
    pub redact_sensitive_fields: bool,
}
```

### 9.3 `ArchiveRef`

```rust
pub struct ArchiveRef {
    pub artifact_type: ArchiveArtifactType,
    pub relative_path: String,
}

pub enum ArchiveArtifactType {
    CandidateCode,
    TraceJsonl,
    OutcomeJson,
    GradeJson,
    DiffPatch,
    FrontierIndex,
}
```

### 9.4 `OptimizationAxis`

为了支持 Pareto frontier，而不是只有单一总分，建议预留：

```rust
pub enum OptimizationAxis {
    TaskSuccess,
    Cost,
    ContextTokens,
    Latency,
    PolicySafety,
}
```

并允许 `GradeResult` / `GradeBundle` 输出 axis 值，供 frontier 聚合使用。

### 9.5 `FrontierSnapshot`

```rust
pub struct FrontierSnapshot {
    pub candidate_ids: Vec<String>,
    pub axes: Vec<OptimizationAxis>,
}
```

### 9.6 为什么这些契约必须现在就预留

因为 Meta-Harness 证明：真正有效的 harness optimization 不是“先做评测，未来再补 archive”，而是从第一天起就把：

- raw trace
- raw code
- scores
- diffs
- frontier

放进统一经验基底里。否则后续 proposer 会被迫建立第二套日志和搜索设施，导致系统分裂。
