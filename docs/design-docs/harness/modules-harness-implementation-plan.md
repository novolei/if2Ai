# If2Ai `modules/harness` 实施计划与需求分解

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**一句话定位**: 为 `modules/harness` 提供可执行的 requirements、阶段任务、验收标准、风险与迁移路线，确保从设计到实现可逐步交付。

---

## 1. 实施目标

### 1.1 运行时路径约束

实施时必须明确：

- 主 archive 落在 user-local If2Ai data root
- 不默认落在 workspace
- workspace 仅承载 export bundle

当前推荐默认路径：

- runtime-local archive: `~/.if2ai/artifacts/harness/`
- workspace export bundle: `<workspace>/artifacts/harness-export/<bundle_id>/`

在不破坏现有 agent app 主路径的前提下，把 `modules/harness` 从“observability 辅助模块”演进为“评测控制平面”。

最终目标：

1. 可定义任务
2. 可运行 trials
3. 可采集统一 trace
4. 可构建 outcome snapshot
5. 可执行 graders
6. 可输出 report
7. 可沉淀 regression
8. 可回流 learning
9. 可作为 future proposer 的经验基底
10. 可持续优化一般事务处理型用户体验

---

## 2. 顶层需求

### 2.1 Functional Requirements

#### FR-1 任务建模

系统必须支持用结构化方式定义 agent evaluation task。

验收标准：

- 能表达 prompt、fixture、权限、工具 allowlist、期待结果
- 能表达 deterministic 和 stochastic 任务
- 能表达 coding 与一般事务双主域任务

#### FR-2 Trial 执行

系统必须支持单任务多次 trial 执行。

验收标准：

- 支持 `trial_count`
- 支持 timeout
- 支持 fail-fast 与全量执行两种策略

#### FR-3 Trace 统一

系统必须将现有 agent/runtime/control-plane 事件统一为标准 trace schema。

验收标准：

- 能接入 `AgentEvent`
- 能接入 `AuditEvent`
- 能保留 session / request / trace id 关联

#### FR-4 Outcome 断言

系统必须能在 trial 结束时构建结构化 outcome snapshot。

验收标准：

- 至少支持 workspace / session / control_plane 三类资源
- 能做 workspace 越界检测

#### FR-5 多维评分

系统必须支持多个 grader 同时对一次 trial 评分。

验收标准：

- grader 可注册
- grader 输出 score / pass / evidence / confidence
- 支持加权聚合

#### FR-5a General Productivity Graders

系统必须支持面向一般事务任务的专用 grader family。

验收标准：

- 至少覆盖 intent、evidence、actionability、tone、memory、continuity、messaging routing

#### FR-6 Regression 沉淀

系统必须支持把失败 trial 写入 regression corpus。

验收标准：

- 保存 task、trace、outcome、failure taxonomy
- 可复用于后续 task run

#### FR-7 Learning 回流

系统必须支持把高价值失败样本输出给 reflection / trajectory。

验收标准：

- 可调用 `ReflectionEngine`
- 可向 `TrajectoryManager` 导出筛选后的失败样本

#### FR-7c Online Guardrail / Offline Optimization Split

系统必须明确区分：

- 运行时 guardrail
- 离线 harness 优化

并限制高风险事务型任务只在离线闭环中做结构性优化。

#### FR-7a Runtime-Local Archive

系统必须将主 archive 写入用户本地 If2Ai data root，而不是 workspace。

#### FR-7b Workspace Export Bundle

系统必须支持从 runtime-local archive 选择性导出 bundle 到 workspace / CI artifact。

### 2.2 Non-Functional Requirements

#### NFR-1 低侵入

`record` 模式不应显著拉长用户主链路 latency。

#### NFR-2 可恢复

单个 grader 失败不应导致整个 app 崩溃。

#### NFR-3 可版本化

task / trace / report schema 必须显式带 version。

#### NFR-4 可观察

harness 自身也必须可观察，至少包括：

- run status
- failed task count
- trace conversion errors
- grader panic count

#### NFR-5 可迁移

设计不得绑定 Python 测试框架，也不得绑定某一提供商。

#### NFR-5a UX Stability

对一般事务型任务，系统必须支持衡量并逐步降低：

- 意图偏移
- 证据失真
- 过期 memory 误用
- resume / compaction 后连续性退化
- 消息误路由

---

## 3. 阶段性实施策略

### Phase 0: Design Freeze

目标：

- 先完成文档与契约冻结，避免边写边改导致模块漂移。

Deliverables:

- `modules-harness-architecture.md`
- `modules-harness-core-contracts.md`
- `modules-harness-implementation-plan.md`

Exit Criteria:

- 核心数据模型字段冻结到 v1
- trait 边界达成一致

### Phase 1: 采集层标准化

目标：

- 不改变产品行为，只统一事件语义。

Scope:

- `AgentEvent` 标准化
- `AuditEvent` 标准化映射
- `TraceRecord` adapter

Tasks:

1. 明确 `AgentEvent` 到 `TraceEvent` 的字段映射表
2. 明确 `AuditEvent` 到 `TraceEvent` 的字段映射表
3. 明确缺失字段和补充采集点
4. 为 trace schema 定义 versioning policy

Acceptance:

- 能从一次 agent turn 生成完整 `TraceRecord`
- 无需 grader 即可写出统一 trace artifact

### Phase 2: 任务与 fixture 层

目标：

- 让 harness 能真正跑 task。

Scope:

- `TaskSpec`
- `TaskPack`
- `Fixture` trait
- `TrialRunner`

Tasks:

1. 设计 `TaskPack` 的文件布局和加载方式
2. 定义 workspace/session/provider fixture 协议
3. 定义 `TrialRunner` 生命周期
4. 定义 setup / inspect / teardown 错误模型

Acceptance:

- 至少支持三类任务：
  - conversation_core
  - control_plane
  - memory_and_context
  - information_work

### Phase 3: Outcome snapshot

目标：

- 让“成功”落在环境事实，而不是话术。

Scope:

- workspace snapshot
- session snapshot
- control plane snapshot

Tasks:

1. 定义 snapshot builder
2. 定义 file diff / outside-workspace detection
3. 定义 session state extractor
4. 定义 permission outcome extractor

Acceptance:

- 至少能识别：
  - 目标文件已写入
  - 越界写入
  - session 恢复状态
  - 权限拒绝次数

### Phase 4: Grader 系统

目标：

- 形成最小可用评分系统。

Scope:

- `Grader` trait
- registry
- aggregation

Tasks:

1. 实现 `TaskSuccessGrader`
2. 实现 `ToolChoiceGrader`
3. 实现 `PermissionComplianceGrader`
4. 实现 `NoDeadLoopGrader`
5. 实现 `OutcomeSnapshotGrader`
6. 定义 `IntentAlignmentGrader` 规格
7. 定义 `EvidenceFidelityGrader` 规格
8. 定义 `MemoryAlignmentGrader` 规格
9. 定义 `MessagingRoutingGrader` 规格

Acceptance:

- 能对单 task 多 trial 输出稳定 report

### Phase 4A: General Productivity Task Packs

目标：

- 让 harness 覆盖事务型用户的真实主路径。

Scope:

- information_work
- communication_assistant
- personal_operations
- memory_driven_collaboration

Tasks:

1. 定义 task pack 布局与示例任务
2. 为 evidence fixture、memory seed、continuation fixture 定义最小 schema
3. 定义 research/summarization/follow-up 类任务的 outcome 和 UX expectation 模板
4. 定义 misroute、evidence drift、stale memory taxonomy

Acceptance:

- 至少有 1 个 `information_work` pack
- 至少有 1 个 `communication_assistant` pack
- 至少有 1 个 `memory_driven_collaboration` pack

### Phase 5: Regression 与比较

目标：

- 从一次性评测走向长期质量治理。

Scope:

- regression serialization
- corpus index
- historical comparison

Tasks:

1. 设计 regression artifact 存储布局
2. 设计 failure taxonomy
3. 支持按 task/version/model/config 比较
4. 定义 regression reopening 规则

Acceptance:

- 能从失败样本重新生成 task run

### Phase 6: Learning Sink

目标：

- Harness 与 learning 打通。

Scope:

- reflection export
- trajectory export
- failure clustering

Tasks:

1. 定义 reflection candidate schema
2. 定义 trajectory export filter
3. 定义 failure cluster summary
4. 设计人工 review 介入点

Acceptance:

- 能从一组失败 trial 产生：
  - reflection suggestions
  - trajectory export candidates

---

## 4. 模块级任务拆解

### 4.1 `orchestrator.rs`

职责：

- 组织 task pack 执行
- 控制 trial lifecycle
- 调度 grader
- 写 report

Spec:

- 输入：`HarnessRunRequest`
- 输出：`HarnessRunReport`
- 不直接操作 UI，不直接处理具体 provider API

Required APIs:

- `run_task_pack`
- `run_task`
- `run_trial`
- `compare_runs`

### 4.2 `task.rs`

职责：

- 持有任务定义与解析逻辑

Spec:

- 任务对象必须可 JSON / TOML / YAML 序列化
- 加载时必须验证 schema version

Required APIs:

- `validate`
- `validate_task_domain`
- `resolve_defaults`
- `materialize`

### 4.3 `trial.rs`

职责：

- 执行单次 trial

Spec:

- 生命周期：
  `prepare -> setup -> invoke runtime -> collect trace -> inspect outcome -> teardown`

Required APIs:

- `run_once`
- `run_many`
- `evaluate_stability`

### 4.4 `trace.rs`

职责：

- 标准化 trace schema 和 adapter

Spec:

- `TraceRecord` 必须可由 raw event stream 构建
- 支持 partial trace 和 terminal trace

Required APIs:

- `from_agent_events`
- `merge_audit_events`
- `finalize_summary`

### 4.5 `outcome.rs`

职责：

- 构建 outcome snapshot

Spec:

- 资源快照构建应独立于 grader

Required APIs:

- `capture_workspace`
- `capture_session`
- `capture_memory`
- `capture_control_plane`

### 4.6 `graders/*`

职责：

- 维度化评分

Spec:

- 每个 grader 仅负责单维度
- 不在 grader 内创建副作用

Required APIs:

- `supports`
- `grade`

补充要求：

- 一般事务 grader 必须支持 evidence extraction
- 必须支持 memory hit / miss / misuse 证据输出
- 对消息任务必须支持 target / recipient / draft-only 校验
- 对 continuity 任务必须支持 prior decision 与 open loops 校验

### 4.7 `regression/*`

职责：

- 失败样本治理

Spec:

- artifact 必须可重放
- 必须支持隐私脱敏

Required APIs:

- `store_failure`
- `load_case`
- `list_cases`
- `cluster_failures`

---

## 5. 风险与规避策略

### 风险 1: 事件语义碎片化

问题：

- 现在 agent、audit、session 各自有不同事件语义

后果：

- grader 写出来会高度耦合、难维护

规避：

- 先冻结 `TraceEvent` 统一 schema，再做 adapter

### 风险 2: 过早引入模型评分

问题：

- LLM-based grader 方差大、成本高、难追责

规避：

- v1 先以 deterministic grader 为主
- model-based grader 作为可选扩展

### 风险 3: Harness 污染主路径

问题：

- 如果 eval 逻辑直接混进用户 agent path，会导致性能与复杂度膨胀

规避：

- 明确 `off / record / eval` 模式
- 仅 `record` 可接入主路径
- `eval` 走显式调度

### 风险 4: regression corpus 泄漏隐私

问题：

- 真实用户 trace 可能含敏感内容

规避：

- 增加 artifact redaction policy
- 默认最小保留原则

### 风险 5: 设计过大导致无法落地

问题：

- 一次性做完整平台风险高

规避：

- 以 Phase 1-4 先构建 MVP
- Phase 5-6 再逐步引入 regression / learning

### 风险 8: 框架天然偏向 coding-only

问题：

- 如果前几 phase 只围绕 workdir、tool、compile outcome 建模，后续会默认把事务型任务降级成“文本生成任务”。

规避：

- 在 `TaskSpec` v1 就冻结 `TaskDomain` 与 `UserExperienceExpectation`
- Phase 2 开始即纳入 `information_work` 样例任务
- Phase 4 同步定义一般事务 grader family，而不是实现后期再补

---

## 6. 验收标准

### MVP 验收

以下条件全部满足时，可认为 `modules/harness` v1 可进入实现：

1. 有稳定的 `TaskSpec` v1
2. 有稳定的 `TraceRecord` v1
3. 有稳定的 `OutcomeSnapshot` v1
4. 有稳定的 `Grader` trait v1
5. 有最小 5 个 grader 规格
6. 有 clear 的 fixture 生命周期
7. 有 report 与 regression artifact 格式
8. 至少有 1 个一般事务 task pack 与对应 grader 规格

### 第一阶段实现完成验收

1. 可运行至少 3 个真实 task
2. 可输出统一 trace report
3. 可正确检测至少一种越界行为
4. 可对 compaction/resume 至少做基础评分
5. 可对至少一种 `information_work` 或 `communication_assistant` 任务输出 UX 维度评分

---

## 7. 推荐交付顺序

建议工程实现顺序：

1. `trace.rs`
2. `task.rs`
3. `trial.rs`
4. `outcome.rs`
5. `graders/*`
6. `report.rs`
7. `regression/*`
8. `orchestrator.rs`

理由：

- trace 是事实基础
- task 是输入契约
- trial 是运行骨架
- outcome / grader 建立判定能力
- report / regression 是长期沉淀
- orchestrator 最后拼装可降低返工

---

## 8. 下一步建议

设计完成后，下一轮实现不应直接全模块开工，而应先做一个垂直 slice：

推荐首个 slice：

- `TraceRecord v1`
- `TaskSpec v1`
- `TaskSuccessGrader`
- `PermissionComplianceGrader`
- `OutcomeSnapshot` 里的 `workspace + control_plane`

原因：

- 这条链路最能验证架构是否闭环
- 也最贴近 If2Ai 当前 control plane / workdir 边界问题

推荐第二个 slice：

- `TaskDomain`
- `InformationWork` / `CommunicationAssistant` task category
- `IntentAlignmentGrader`
- `MemoryAlignmentGrader`
- `ContinuityExpectation`

原因：

- 这条链路能尽早避免 harness 架构被固化成 coding-only
- 也最贴近 If2Ai 事务型主用户的真实收益

---

## 9. Meta-Harness 驱动的新增实施要求

### 9.1 新增 FR

#### FR-8 Experience Archive

系统必须支持把 candidate code、trace、grade、outcome 写入统一经验档案。

验收标准：

- 目录结构稳定
- 机器可读
- 支持 top-k / frontier / diff 查询

#### FR-9 Search/Test 隔离

系统必须显式区分 validation、search-set 与 final-test。

验收标准：

- proposer 不可见 final-test 结果
- archive policy 可控制 proposer visibility

### 9.2 新增 NFR

#### NFR-6 最小外循环结构

基础设施不得过早固化 mutation rule、parent selection、critique template。

原因：

- Meta-Harness 的核心发现之一是 outer loop 应保持 minimum necessary structure，把诊断与改写交给 proposer。

### 9.3 新增阶段

#### Phase 7: Archive / Frontier / Search Interface

目标：

- 让当前 harness 成为 future proposer 可直接消费的 substrate。

Tasks:

1. 设计 archive 目录结构
2. 设计 candidate artifact schema
3. 设计 Pareto frontier snapshot
4. 设计 CLI/query API
5. 设计 validation gate
6. 设计 search-set / final-test 隔离

Acceptance:

- 对任意 candidate，可查询 code、score、trace、outcome 和 diff

### 9.4 新增模块任务

推荐新增：

- `archive/*`

职责：

- proposer-friendly experience filesystem

Required APIs:

- `write_candidate_run`
- `read_candidate_run`
- `list_frontier`
- `diff_candidates`
- `top_k_by_axis`

### 9.5 新增风险

#### 风险 6: 只有 summary 没有 raw trace

问题：

- future proposer 会失去最关键的诊断能力来源

规避：

- raw trace artifact 设为强制项

#### 风险 7: search/test 泄漏

问题：

- proposer 若看到 final-test 结果，会污染泛化评估

规避：

- `SearchRole` 强制化
- final-test artifact 默认 proposer 不可见
