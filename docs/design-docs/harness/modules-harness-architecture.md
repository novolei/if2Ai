# If2Ai `modules/harness` 架构设计

**版本**: 1.0  
**最后更新**: 2026-04-18  
**状态**: Proposed  
**模块路径**: `src-tauri/src/modules/harness/`  
**一句话定位**: 将现有偏观测用途的 harness 子模块升级为 If2Ai Agent App 内部的评测控制平面，负责任务编排、trial 执行、trace 采集、grader 评分、回归沉淀与学习回流。

---

## 1. 背景与设计目标

### 1.0 运行时路径策略

`modules/harness` 的主 archive 应定义为用户本地 If2Ai data root 下的运行时目录，而不是 workspace 内目录。

逻辑路径：

`If2Ai user-local data root / artifacts / harness /`

当前 Unix / macOS 推荐默认实现：

`~/.if2ai/artifacts/harness/`

这与 workspace export bundle 要严格区分：

- runtime-local archive
  app 持续写入、长期保留、append-only
- workspace export bundle
  从 archive 选择性导出，用于 review / CI / sharing

### 1.1 当前现状

当前仓库中已经存在 `src-tauri/src/modules/harness/`，但它的职责仍局限于“事件总线 + telemetry + trace JSONL 落盘”：

- [mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/mod.rs)
- [event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs)
- [telemetry.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/telemetry.rs)
- [session_recorder.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/session_recorder.rs)

它已经具备：

- agent 事件广播
- per-session telemetry 聚合
- session trace JSONL 录制

但它还不具备：

- 任务级评测编排
- multi-trial 执行与稳定性统计
- trace / outcome / policy grader
- regression corpus
- 与 trajectory / reflection 的自动回流

### 1.2 为什么要把 Harness 做成产品模块

这是一个产品级子系统，而不是测试脚本，原因有三：

1. If2Ai 是 agent-first 桌面产品，不是单次 API 应答服务。
2. 质量关键不止是“输出是否像样”，而是：
   - 是否选择了正确工具
   - 是否遵守 workdir / permission 边界
   - 是否在恢复、压缩、重试后仍达成正确 outcome
3. 未来 memory、desktop control、self-improvement 都依赖结构化评测，而不是一次性测试。

### 1.4 用户域覆盖要求

`modules/harness` 的范围定义必须从第一天起覆盖两类主域：

1. `engineering / coding`
2. `general daily productivity`

第二类至少包含：

- 信息整理
- 沟通辅助
- 文件整理
- TODO / follow-up
- 记忆驱动协作
- 多平台消息协作

### 1.3 理论与事实依据

这个架构选择基于以下稳定事实：

- OpenAI 官方公开强调 agent eval 应以 trace grading 和 deterministic harness 为核心，而非只看 final output。
- Anthropic 官方将 agent eval 分层为 `task -> trial -> grader -> transcript -> outcome -> harness -> scaffold`，说明“任务、轨迹、环境结果”必须解耦。
- Microsoft Agent-Pex 证明 trace-level evaluation 与 spec-driven adversarial generation 能显著提高鲁棒性发现能力。
- Google DeepMind AlphaEvolve 说明 automated evaluator 不只是验收器，更是优化闭环的一部分。
- WebArena、OSWorld、GAIA、BrowseComp 等 benchmark 表明真实 agent 任务必须在“可执行环境 + outcome verification”中测，而不能只做字符串匹配。
- Meta-Harness 证明 harness 优化不是 prompt tuning，而是对“stateful harness program”的 end-to-end code search；有效优化依赖 proposer 对 code、scores 和 execution traces 的选择性访问，而不是压缩反馈。

这些依据决定了 If2Ai 的 harness 设计必须是：

- app-native
- trace-first
- outcome-aware
- policy-sensitive
- learning-compatible
- archive-queryable
- proposer-compatible

---

## 2. 子系统职责

`modules/harness` 的职责边界如下。

### 2.1 它负责什么

1. 任务定义与任务包加载
2. 试验环境 setup / teardown
3. 单次或多次 trial 运行
4. trace 标准化采集
5. 多 grader 打分与证据保留
6. 结果汇总、比较、趋势分析
7. regression corpus 写入
8. reflection / trajectory 回流
9. runtime-local archive 管理与 export bundle 派生
10. 面向一般事务任务的 UX / memory / messaging 质量优化

### 2.2 它不负责什么

1. 不直接承担业务 Agent Loop 本身
   这是 `modules/runtime` 与 `commands/agent.rs` 的职责。
2. 不替代 Tool 执行
   这是 `ToolExecutionBroker` 和 `ToolRegistry` 的职责。
3. 不直接训练模型
   它只输出高价值失败样本给 `TrajectoryManager` / RL 系统。
4. 不承担前端展示逻辑
   只暴露结果 API，由 UI 层自行可视化。
5. 不把一般事务任务的质量判断简化成“最终文本像不像答案”
   这些任务需要专门的 UX、证据、行动性、记忆和路由 grader。

---

## 3. 架构原则

### 3.1 Trace First

任何 grader 都必须优先建立在结构化 trace 之上，而不是解析自然语言日志。

原因：

- trace 可复放
- trace 可比较
- trace 能定位失败步骤
- trace 才能服务 learning

### 3.2 Outcome over Surface Text

agent 最终文本不等于成功。成功应尽量落在环境状态断言上。

示例：

- “我已经创建文件” 不等于文件真的存在
- “我已经恢复会话” 不等于 session 恢复成功
- “已遵守权限” 不等于 broker 没有发生越界

### 3.3 Deterministic by Default

若任务可以确定性执行，就不应引入高方差模型评分。

优先级：

1. 环境断言
2. 结构化 trace 断言
3. rule-based grading
4. model-based grading

### 3.4 Runtime Reuse over Parallel Abstraction

Harness 不应重复发明一套 agent runtime，而是复用：

- `SessionExecutionContext`
- `ToolExecutionBroker`
- `PermissionPolicy`
- `ConversationRuntime`
- `TrajectoryManager`
- `ReflectionEngine`

### 3.5 Progressive Enablement

Harness 需要支持三种运行模式：

1. `off`
   生产默认，不附加评测开销
2. `record`
   只采集 trace / telemetry
3. `eval`
   完整执行 task + graders + report

### 3.6 Online Guardrail, Offline Optimization

对一般事务任务，`modules/harness` 还必须遵守：

- 在线：记录、约束、护栏、fallback
- 离线：真正优化 prompt、policy、memory、task pack 和 candidate harness

理由：

- 事务型用户对可预测性、稳定性和误发消息风险更敏感。

---

## 4. 推荐的目标结构

建议将 `modules/harness` 演化为以下结构：

```text
src-tauri/src/modules/harness/
├── mod.rs
├── orchestrator.rs
├── config.rs
├── task.rs
├── trial.rs
├── trace.rs
├── outcome.rs
├── report.rs
├── registry.rs
├── fixtures/
│   ├── mod.rs
│   ├── workspace.rs
│   ├── session.rs
│   ├── provider.rs
│   └── memory.rs
├── graders/
│   ├── mod.rs
│   ├── task_success.rs
│   ├── trace_quality.rs
│   ├── policy_safety.rs
│   ├── resource_efficiency.rs
│   ├── outcome_snapshot.rs
│   └── user_experience.rs
├── regression/
│   ├── mod.rs
│   ├── corpus.rs
│   ├── serializer.rs
│   └── clustering.rs
├── event_bus.rs
├── telemetry.rs
├── session_recorder.rs
└── agent_loop_integration.rs
```

### 4.1 分层说明

#### A. 采集层

- `event_bus.rs`
- `telemetry.rs`
- `session_recorder.rs`
- `agent_loop_integration.rs`

职责：从 agent loop 中接出 raw events，形成可消费 trace。

#### B. 契约层

- `task.rs`
- `trace.rs`
- `outcome.rs`
- `report.rs`
- `config.rs`

职责：定义任务、轨迹、环境结果、报告格式的稳定 schema。

#### C. 执行层

- `orchestrator.rs`
- `trial.rs`
- `fixtures/*`

职责：执行 setup / run / inspect / teardown，控制 trial 生命周期。

#### D. 评分层

- `graders/*`
- `registry.rs`

职责：根据 trace 与 outcome 进行结构化评分。

评分层内部必须从一开始区分两类 grader family：

1. `engineering / control-plane graders`
   关注 tool choice、policy compliance、workdir isolation、dead loop、resource efficiency。
2. `general productivity graders`
   关注 intent alignment、evidence fidelity、actionability、tone/persona fit、memory alignment、continuity、messaging routing。

#### E. 演进层

- `regression/*`

职责：沉淀失败样本、比较历史版本、形成长期改进资产。

---

## 5. 与现有模块的集成边界

### 5.1 与 `commands/agent.rs` 的关系

`commands/agent.rs` 负责真实的用户请求执行，Harness 不应复制其业务逻辑。

Harness 集成点：

- turn start / finish
- streamed token updates
- tool call lifecycle
- compaction
- resume / degraded outcome
- trajectory record trigger

### 5.2 与 `control_plane` 的关系

关键依赖：

- [session_context.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/session_context.rs)
- [tool_execution_broker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs)
- [audit.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/audit.rs)

Harness 使用它们来回答三个关键问题：

1. agent 当时在哪个 workdir 执行
2. 权限决策是什么
3. 工具行为是否可归因

### 5.3 与 `runtime` 的关系

关键依赖：

- `ConversationRuntime`
- `Session`
- `PermissionPolicy`
- `PermissionOutcome`

Harness 不修改 runtime 业务规则，但要求 runtime 暴露统一事件与结果快照。

### 5.4 与 `learning` 的关系

关键依赖：

- `TrajectoryManager`
- `ReflectionEngine`

Harness 输出：

- regression 样本
- failure taxonomy
- reflection 候选
- 高价值 trajectory 子集

### 5.5 与 `memory` 的关系

关键依赖：

- `ActiveRetrievalManager`
- working memory / episodic compaction

Harness 用于验证：

- 检索是否命中正确上下文
- compaction 是否引入行为回退
- memory 插入是否污染后续任务

### 5.6 与 `messaging gateway` 的关系

对一般事务任务，Harness 还必须能验证：

1. 是否选择了正确的发送目标、频道或线程
2. 是否遵守“草稿、确认、发送”边界
3. 是否把用户上下文错误路由到不该出现的外部渠道

因此一旦 If2Ai 的 `messaging gateway` 成熟，`modules/harness` 需要消费其结构化消息路由事件，而不是只看最终生成文本。

---

## 6. 运行模式设计

### 6.1 `off`

默认生产模式：

- 不启动 session recorder
- 不执行 graders
- 仅允许极低成本 telemetry

### 6.2 `record`

开发与线上诊断模式：

- 启动 event bus
- 写 trace JSONL
- 聚合 telemetry
- 不运行完整 graders

使用场景：

- bug 复盘
- 用户投诉采样
- regression 样本抽取

### 6.3 `eval`

完整评测模式：

- 从 task pack 装载任务
- 执行多 trial
- 采集 trace 和 outcome
- 执行 graders
- 写入 report / regression / learning sink

---

## 7. 为什么沿用现有 observability harness 演进

Staff 级设计上，不建议把现有 harness 子模块废弃重写，原因如下：

1. `EventBus` 已经形成非侵入式事件采集模式。
2. `TelemetryCollector` 已有 per-session 聚合语义。
3. `SessionRecorder` 已有 trace 持久化基础。
4. 这些模块位于 app 内部，天然比外部 Python harness 更贴近真实 agent 行为。

因此推荐路径是：

`observability harness -> evaluation harness -> regression harness -> learning harness`

而不是：

`删除现有模块 -> 重新发明一套评测系统`

---

## 8. 非功能性要求

### 8.1 性能

- `record` 模式不能显著增加用户 turn latency
- 事件采集应以 append / fire-and-forget 为主
- grader 运行应在 `eval` 模式下进行，避免污染用户路径

### 8.2 稳定性

- trace schema 需要版本化
- 不允许 grader panic 影响主流程
- 单个 subscriber lag 不应阻塞 agent loop

### 8.3 可移植性

- 任务契约与报告格式必须与 provider 无关
- grader 不应依赖单一模型厂商
- fixture 设计应兼容本地、CI、未来远程 runner

### 8.4 安全性

- regression artifacts 需要隐私控制
- trace 存储默认做输入裁剪与必要脱敏
- 评测运行不应绕过现有 `PermissionPolicy`

### 8.5 可演进性

- grader registry 允许新增维度而不破坏旧任务
- task pack schema 允许版本升级
- outcome snapshot 允许新增资源类型

### 8.6 用户体验稳定性

对一般事务用户，系统必须保证：

- 同一任务在等价上下文下的语气和行动性波动可控
- resume / compaction 后不显著提高误解用户意图的概率
- 对消息发送类任务，误路由与越权发送风险接近零
- memory 命中错误不能被“回答得很流畅”掩盖

---

## 9. 关键决策

### 决策 1: Harness 是正式业务模块

结论：是。

理由：

- 它服务产品质量与自我改进，而不是只服务 CI。

### 决策 2: 以 trace 为中心，而不是 log 为中心

结论：必须。

理由：

- log 面向人读，trace 面向系统分析与重放。

### 决策 3: outcome grader 必须是一等公民

结论：必须。

理由：

- agent 产品的正确性本质是“环境是否达成目标”。

### 决策 4: learning sink 不与 RL 强耦合

结论：保持松耦合。

理由：

- harness 先产出高质量失败资产，后续可被 reflection、trajectory learning、RL 复用。

### 决策 5: General Productivity 是主任务域，不是附属任务域

结论：必须视作主任务域。

理由：

- If2Ai 的目标用户中，大量价值来自日常事务处理，而不是只来自 coding。
- 这类任务最容易出现“看起来答得不错，但没有真正帮助用户推进事务”的伪成功。
- 如果架构只先为 coding 优化，后续补事务型 grader、task pack 和 archive taxonomy 的成本会非常高。

---

## 10. 实施任务分解

### Phase A: 重新定义模块边界

目标：把现有 harness 从 observability-only 升级为可扩展评测骨架。

Tasks:

1. 定义 `modules/harness` 的分层目录结构。
2. 保留现有 `event_bus / telemetry / session_recorder`，明确其成为采集层。
3. 新增 `task / trace / outcome / report / orchestrator / trial` 的设计文档与契约。

### Phase B: 标准化 trace schema

目标：统一来自 runtime、broker、audit 的事件。

Tasks:

1. 设计 `TraceRecord` 与 `TraceEvent`。
2. 定义与 `AgentEvent` 的映射。
3. 定义与 `AuditEvent` 的映射。
4. 增加 trace versioning 策略。

### Phase C: 引入任务与 trial 编排

目标：从 session recording 迈向 task evaluation。

Tasks:

1. 定义 `TaskSpec`。
2. 定义 `TaskPack` 与 `TaskSelectionPolicy`。
3. 定义 `TrialRunner` 生命周期。
4. 定义 setup / inspect / teardown fixture 协议。

### Phase D: 建 grader 系统

目标：把“录下来”升级为“可判定”。

Tasks:

1. 定义 `Grader` trait。
2. 实现最小 5 个 grader 的规格。
3. 定义评分聚合与 pass/fail 策略。
4. 定义 grader 证据输出。

### Phase E: 建一般事务任务 task packs 与 grader families

目标：让 harness 真正服务事务型用户，而不是停留在 coding/control-plane 质量治理。

Tasks:

1. 定义 `information_work` task pack。
2. 定义 `communication_assistant` task pack。
3. 定义 `memory_driven_collaboration` task pack。
4. 定义 `IntentAlignmentGrader`、`EvidenceFidelityGrader`、`ActionabilityGrader`、`MemoryAlignmentGrader`、`MessagingRoutingGrader` 的规格。
5. 定义 misroute、stale-memory、evidence-drift failure taxonomy。

### Phase F: 回归与学习闭环

目标：让 harness 成为产品进化基础设施。

Tasks:

1. 定义 regression artifact 格式。
2. 定义 failure taxonomy。
3. 定义 reflection export 契约。
4. 定义 trajectory export 过滤策略。

---

## 11. 本文档的后续配套文档

为了把实现前置设计做扎实，本文档应配套以下两份详细文档：

1. `modules-harness-core-contracts.md`
   详细定义 `TraceRecord / TaskSpec / Grader trait / OutcomeSnapshot / HarnessRunReport`
2. `modules-harness-implementation-plan.md`
   详细定义 requirements、阶段任务、验收标准、风险和迁移路径

---

## 12. Meta-Harness 对架构的增量要求

### 12.1 新增架构角色: Archive Layer

为了匹配 Meta-Harness 所依赖的“filesystem with all prior code, scores, traces”，建议把目标结构扩展为：

```text
src-tauri/src/modules/harness/
├── archive/
│   ├── mod.rs
│   ├── layout.rs
│   ├── query.rs
│   ├── frontier.rs
│   └── validation.rs
```

职责：

- 写 candidate run 目录
- 维护 frontier
- 支持 top-k / diff / trace show 查询
- 运行 lightweight validation

### 12.2 新增运行模式: `search`

除 `off / record / eval` 外，建议规划 `search` 模式：

- proposer 读取 archive
- 生成 candidate harness
- 跑 lightweight validation
- 交给 external evaluator
- append 到 archive

### 12.3 新增硬要求: Search/Test 隔离

架构上必须从一开始就支持：

- `validation_set`
- `search_set`
- `final_test_set`

并通过 metadata 和 archive policy 强制 final-test 对 proposer 不可见。

### 12.4 新增硬要求: Proposer-First Queryability

archive 不是“方便人类读报告”的附属物，而是 future coding agent proposer 的工作台。

因此目录与文件设计必须优先满足：

- `grep`
- `diff`
- `top-k`
- `frontier list`
- `show trace`

而不是优先满足复杂 UI 定制。
