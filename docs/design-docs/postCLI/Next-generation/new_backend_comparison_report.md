# uclaw-rs vs if2Ai 后端架构比较与可迁移资产盘点

> 生成日期: 2026-04-18
> 分析方法: 基于源码文件级深度阅读（第二遍：逐行扫描关键 orchestrator/memory/search/pipeline/router/contracts 模块）
> 版本: uclaw-rs v0.5.64 vs if2Ai feature/consolidate-codebase
> 审阅级别: Staff 系统架构师与策略师

---

## 一、uclaw-rs 源模块 → if2Ai 对应模块/目标落点 映射表

| uclaw-rs 源模块 | if2Ai 对应模块/目标落点 | 分类 | 原因 | 风险 |
|---|---|---|---|---|
| `agent_runtime/cortex.rs` (全局监督进程、decay tick、event 订阅) | `modules/runtime/` 无对应 | **adapt** | Cortex 是通用后台观测器抽象，与 if2Ai 的 harness 观测目标一致 | 需去掉 persona/中文人格绑定 |
| `agent_runtime/scheduler.rs` (ProcessControlRegistry + Scheduler, Worker/Branch 派发与超时) | `modules/scheduler/` (cron) + `modules/runtime/` (agent loop) | **adapt** | if2Ai 有 cron scheduler 但缺少 Worker/Branch 级别的进程生命周期管理 | 与 Tauri 事件循环集成需要适配 |
| `agent_runtime/runner.rs` (ProcessRunner trait + ProcessState 状态机) | `modules/runtime/` (agent turn loop) | **adapt** | 进程状态机是通用抽象，但 uclaw-rs 的 ProcessType/Channel/Branch/Worker 是多通道语义 | 需要精简为 if2Ai 的单一 agent 路径 |
| `agent_runtime/conversation_actor.rs` (ConversationActor, memory recall, TaskWorker 委派) | `modules/runtime/conversation.rs` | **rewrite** | 已有等价模块但架构不同；uclaw-rs 的未接入主路径（代码注释注明 dormant） | 代码虽存在但未验证，直接迁移可能引入未修复 bug |
| `agent_runtime/compactor.rs` | `modules/runtime/compact.rs` | **skip** | if2Ai 已有 compact 实现 | 无 |
| `agent_runtime/working_memory.rs` | `modules/memory/working_memory.rs` | **skip** | if2Ai 已有 working memory | 无 |
| `agent_runtime/supervisor.rs` (Worker 预算检查、冷却追踪) | `modules/runtime/budget.rs` | **adapt** | Supervisor 的预算检查与 if2Ai budget 模块有重叠但粒度不同 | 需对齐预算模型 |
| `agent_runtime/task_worker.rs` + `worker.rs` + `branch.rs` | 无对应 | **rewrite** | Worker/Branch 是后台任务委派模式，if2Ai 目前无此能力 | 实现复杂度高，需定义清晰接口 |
| `tools/dispatcher.rs` (ToolDispatcher, ApprovalGate, 审计, 风险策略) | `modules/tools/registry.rs` (ToolRegistry) | **adapt** | if2Ai ToolRegistry 已有注册/调度/超时，但缺少 ApprovalGate 审计链路和风险策略引擎 | 不引入第二套 registry，将 ApprovalGate 等作为扩展层挂到现有 ToolRegistry |
| `tools/policy.rs` (PolicyEngine, RiskBasedPolicyEngine, ToolPolicyDecision) | `modules/runtime/permissions.rs` | **adapt** | 风险分级策略是通用能力，if2Ai 有 PermissionMode 但缺少细粒度风险策略 | 需与现有权限模型合并 |
| `tools/approval.rs` (ApprovalGate, ToolApprovalRequestedPayload) | 无对应 | **direct** | 授权门是独立功能，无冲突，可直接迁移 trait + 实现 | 低 |
| `tools/audit.rs` (ToolAuditStore) | 无对应 | **direct** | 工具审计存储是独立功能，可直接迁移 | 低 |
| `tools/descriptor.rs` (ToolDescriptor, ToolRetryPolicy, ToolRiskLevel) | `modules/tools/registry.rs` ToolEntry | **adapt** | if2Ai 已有 ToolEntry；uclaw-rs 的 descriptor 更结构化（含风险等级、重试策略） | 需合并到 ToolEntry |
| `tools/tool_invocation_state.rs` (ToolInvocationLifecycle, ToolInvocationStatus) | 无对应 | **direct** | 工具调用生命周期跟踪是独立功能 | 低 |
| `tools/workspace_guard.rs` (WorkspaceGuard) | `modules/tools/context.rs` (ToolContext) | **adapt** | if2Ai 已有 ToolContext 含 workdir；WorkspaceGuard 是路径沙箱边界 | 需合并到 context |
| `tools/security.rs` (AutonomyLevel, CommandRiskLevel, SecurityPolicy) | `modules/security/` | **adapt** | 命令风险分级与安全策略是通用能力 | 需与 if2Ai 安全模型对齐 |
| `tools/loop_guard.rs` (ToolLoopGuard, LoopGuardDecision) | 无对应 | **direct** | 工具调用循环检测（防 LLM 重复调用同一工具）是独立能力 | 低 |
| `tools/tool_capability_catalog.rs` (工具能力目录、audit profile) | 无对应 | **direct** | 工具能力编目与 profile 暴露审计是独立功能 | 低 |
| `tools/contract.rs` (ToolContract, PolicyGate, ToolContractRegistry) | 无对应 | **adapt** | 工具契约层（预执行检查、权限声明）是额外安全层 | 可能与现有 permissions 重叠 |
| `memory/memory_manager.rs` (MemoryManager, 三层记忆, rerank, should_store, dedup) | `modules/memory/` (MemoryProvider, SqliteMemoryProvider, VectorMemoryProvider) | **adapt** | if2Ai 已有 provider 层但缺少 MemoryManager 协调层（prepare_context, after_turn, rerank 策略） | 需适配 if2Ai 的 MemoryProvider trait 接口 |
| `memory/working_memory.rs` | `modules/memory/working_memory.rs` | **skip** | if2Ai 已有 | 无 |
| `memory/embedder.rs` + `memory/embedding_table.rs` | `modules/memory/embedding/` | **skip** | if2Ai 已有 FastEmbed embedding | 无 |
| `memory/search.rs` (MemorySearch, SearchConfig, SearchMode, SearchSort) | `modules/memory/` (MemoryProvider::search) | **skip** | if2Ai MemoryProvider trait 已有 recall/search | 无 |
| `memory/decay.rs` (记忆衰减策略) | 无对应 | **direct** | 记忆衰减是独立功能 | 低 |
| `memory/quality_gate.rs` (记忆质量门控) | 无对应 | **direct** | 记忆写入质量检查 | 低 |
| `memory/write_policy.rs` (MemoryWritePolicyEngine) | `modules/memory/policy.rs` | **skip** | if2Ai 已有 memory policy | 需对比实现差异 |
| `memory/ingestion.rs` (后台记忆摄取循环) | 无对应 | **adapt** | 异步摄取循环是独立功能 | 需要适配 if2Ai 内存模型 |
| `mcp/manager.rs` (McpManager, 连接管理, 工具注册到 ToolDispatcher) | `modules/runtime/mcp.rs` + `modules/runtime/mcp_client.rs` | **adapt** | if2Ai 已有 MCP client，但缺少统一 Manager 将 MCP 工具注册到 ToolRegistry | 需适配 if2Ai ToolRegistry 而非 uclaw-rs ToolDispatcher |
| `mcp/connection.rs` + `mcp/adapter.rs` (McpConnection, McpToolAdapter) | `modules/runtime/mcp_client.rs` | **skip** | if2Ai 已有 MCP client 实现 | 需对比接口差异 |
| `mcp/runtime.rs` (McpRuntime, 健康检查投影) | 无对应 | **direct** | MCP 运行时健康监控 | 低 |
| `mcp/server_descriptor.rs` (McpServerDescriptor, McpServerCatalogFile) | 无对应 | **direct** | MCP 服务器描述/编目 | 低 |
| `llm/routing.rs` (RoutingConfig, 动态模型路由, 难度升级, fallbacks, thinking_effort) | `modules/provider/` | **adapt** | if2Ai 有 provider config 但缺少动态难度感知路由和 fallback 链 | 需适配 if2Ai 配置模型 |
| `llm/dual_scorer.rs` (DualLayerScorer, QueryDifficulty) | 无对应 | **direct** | 查询难度评估是独立能力 | 低 |
| `llm/cooldown.rs` (ProviderCooldownTracker, call_with_retry) | 无对应 | **direct** | Provider 限速冷却与重试是独立能力 | 低 |
| `llm/classifier.rs` (EightDimClassifier) | 无对应 | **direct** | 8 维度查询分类器 | 低 |
| `llm/model_caps.rs` + `llm/model_registry.rs` | 无对应 | **direct** | 模型能力注册表 | 低 |
| `channels/` (channel gateway, adapter, pipeline, router, dedupe, wechat_bridge) | `modules/channel/` | **adapt** | if2Ai 有 channel 模块但 uclaw-rs 的 channel 体系是面向 IM/IRC/Nostr 等外部通道的 | 大部分是产品语义（微信/IRC），需要剥离 |
| `tasks/cron.rs` (CronScheduler, CronConfig, CronJobType, CronStore) | `modules/scheduler/` | **skip** | if2Ai 已有 scheduler | 需对比接口差异 |
| `tasks/types.rs` (Task, TaskPriority, TaskStatus, TaskSubtask) | 无对应 | **direct** | 任务类型定义 | 低 |
| `plugins_v2/` (PluginManifest, PluginSupervisor, PluginIsolation, HookPipeline) | `modules/plugins/` | **adapt** | if2Ai 有 plugins 模块，需对比实现 | 需对比具体差异 |
| `swarm/runtime.rs` (SwarmRuntime, create_run, execute_run, intervene, metrics) | 无对应 | **rewrite** | Swarm 是多 Agent 任务编排运行时，if2Ai 目前无此能力 | 实现复杂度高，与 if2Ai 定位需重新评估 |
| `swarm/planner.rs` (GoalDecompositionPlanner, SwarmPlanner trait) | 无对应 | **rewrite** | 目标分解规划器 | 同上 |
| `swarm/executor.rs` (NativeTaskExecutor, SwarmExecutionProvider) | 无对应 | **rewrite** | 多 provider 执行引擎 | 同上 |
| `swarm/merger.rs` (merge_task_outputs, resolve_merge_policy) | 无对应 | **rewrite** | 多 worker 输出合并策略 | 同上 |
| `swarm/reviewer.rs` (RuleBasedReviewer, SwarmReviewer trait) | 无对应 | **rewrite** | 规则审查器 | 同上 |
| `task_dag/mod.rs` (TaskDag, TaskDagStore, ready_nodes) | 无对应 | **direct** | DAG 任务依赖管理是通用能力 | 中 — 需与 scheduler 集成 |
| `sandbox/` (SandboxExecutor trait, NoopSandboxExecutor, NonoSandboxAdapter) | `modules/runtime/sandbox.rs` | **adapt** | if2Ai 有 sandbox 模块，需对比实现 | 需对比 |
| `observability/` (llm metrics, tracing) | `modules/harness/` | **adapt** | if2Ai 有 harness 模块做遥测 | 需对比接口 |
| `event_bus/mod.rs` (EventBus, EventEnvelope, broadcast channel) | `modules/harness/` 或 `modules/control_plane/` | **direct** | 事件总线是通用抽象 | 低 |
| `orchestrator/context_engine.rs` (ContextEngine, TokenBudget, TurnReport) | `modules/runtime/budget.rs` + `modules/runtime/prompt.rs` | **adapt** | if2Ai 有 budget 和 prompt 模块 | 需对比 |
| `orchestrator/prompt_builder.rs` (PromptBuilder) | `modules/runtime/prompt.rs` | **skip** | if2Ai 已有 | 需对比差异 |
| `orchestrator/fallback_policy.rs` (FallbackPolicy, FallbackStage) | 无对应 | **direct** | LLM fallback 策略 | 低 |
| `orchestrator/session_summary.rs` (SessionSummaryStore, StructuredSummary) | 无对应 | **direct** | 会话摘要自动提取 | 低 |
| `orchestrator/graph_orchestrator_facade.rs` | 无对应 | **skip** | 图编排门面，与 swarm 耦合 | 仅在迁移 swarm 时有用 |
| `policy/` (UnifiedExecutionPolicy, BoundaryResolver, PermissionEngine, SandboxPolicyBuilder) | `modules/runtime/permissions.rs` + `modules/security/` | **adapt** | 统一执行策略是跨权限/沙箱/边界的协调层 | 实现复杂，需评估是否与现有模块重叠过多 |
| `cost/` (CostTracker, ContextCompressionPipeline, ModelPricing) | `modules/runtime/budget.rs` | **adapt** | if2Ai 有 budget 但缺少成本追踪和上下文压缩管线 | 成本追踪可独立迁移 |
| `journal/` (subagent execution journal) | `modules/harness/` | **direct** | 执行日志/回放能力 | 低 |
| `agents/manager.rs` (AgentManager) + `agents/metrics.rs` | 无对应 | **direct** | 多 Agent 管理器 | 中 — 与 swarm 耦合 |
| `presence/` (在线状态) | 无对应 | **skip** | IM 通道语义 | 产品语义，不适用 |
| `messaging/` (ProcessId, ChannelId, WorkerId, BranchId, ProcessType) | 无对应 | **adapt** | 消息类型定义 | 需精简去除通道语义 |
| `runtime/sidecar/` (SidecarRuntimeClient) | 无对应 | **skip** | uclaw-rs 的 sidecar 是独立 LLM 子进程，if2Ai 直接 HTTP 调用 | 架构不同 |
| `compose/` (消息组合/模板) | 无对应 | **skip** | 产品语义/通道相关 | 不适用 |
| `hub/` (技能市场相关) | `modules/skills/` | **skip** | if2Ai 已有 skills 模块 | 需对比 |
| `clawhub/` | 无对应 | **skip** | uclaw-rs 专属产品语义 | 不适用 |
| `oasis/` | 无对应 | **skip** | 产品语义 | 不适用 |
| `uinvest/` | 无对应 | **skip** | 产品语义（投资分析） | 不适用 |
| `coding/` | 无对应 | **skip** | 编码辅助专属功能 | 不适用 |
| `prompt/` (persona, mood) | 无对应 | **skip** | UClaw 人格/情绪系统 | 产品语义，不适用 |

---

## 二、执行摘要

**uclaw-rs 是一个面向 IM/多通道/多 Agent 的 Agent 后端**，运行在 actix-web 上，以 HTTP API + WebSocket 方式服务 Swift 客户端。它积累了大量通用平台能力，但也绑定了大量 UClaw 产品语义（人格、微信桥、IRC、Nostr、投资分析等）。

**if2Ai 是 Tauri 2 桌面应用**，后端直接嵌入 Tauri 作为 Rust 模块，通过 Tauri IPC 与前端 React 通信。架构更紧凑，模块职责更清晰。

**最有价值的可迁移资产**集中在以下领域：
1. **工具安全治理层**（ApprovalGate + Audit + RiskPolicy + LoopGuard）— if2Ai 缺少
2. **LLM 路由与难度感知**（RoutingConfig + DualScorer + CooldownTracker）— if2Ai 缺少
3. **Memory 协调层**（MemoryManager prepare_context/after_turn/rerank/dedup）— if2Ai 有 provider 但缺少协调
4. **MCP Manager**（连接管理 + 工具自动注册到统一 registry）— if2Ai 有 client 但缺少 manager
5. **进程生命周期管理**（ProcessRunner + Scheduler + ProcessControlRegistry）— if2Ai 缺少
6. **Swarm 多 Agent 编排** — 实现复杂但与 if2Ai 定位需重新评估
7. **统一执行策略**（policy 模块）— 与 if2Ai 现有 permissions 需合并

---

## 三、比较方法与证据范围

| 领域 | uclaw-rs 证据文件 | if2Ai 证据文件 |
|---|---|---|
| 启动与编排 | `main.rs`, `app/mod.rs` (未读到), `Cargo.toml` | `src-tauri/src/main.rs` |
| Agent 循环 | `agent_runtime/runner.rs`, `conversation_actor.rs`, `scheduler.rs` | `modules/runtime/conversation.rs`, `modules/commands/agent.rs` |
| 工具注册/调度 | `tools/dispatcher.rs`, `tools/policy.rs`, `tools/approval.rs`, `tools/audit.rs` | `modules/tools/registry.rs`, `modules/tools/context.rs` |
| 记忆系统 | `memory/memory_manager.rs`, `memory/working_memory.rs`, `memory/search.rs` | `modules/memory/mod.rs`, `modules/memory/providers/` |
| 任务调度 | `tasks/cron.rs`, `task_dag/mod.rs` | `modules/scheduler/` |
| MCP 运行时 | `mcp/manager.rs`, `mcp/connection.rs`, `mcp/adapter.rs` | `modules/runtime/mcp.rs`, `modules/runtime/mcp_client.rs` |
| 插件系统 | `plugins_v2/` (manifest, supervisor, isolation, hooks) | `modules/plugins/` |
| 会话/持久化 | `storage/`, `journal/` | `modules/session/manager.rs` |
| 通道网关 | `channels/` (adapter, pipeline, router, dedupe) | `modules/channel/` |
| LLM 路由 | `llm/routing.rs`, `llm/dual_scorer.rs`, `llm/cooldown.rs` | `modules/provider/`, `modules/runtime/config.rs` |
| 多 Agent | `swarm/runtime.rs`, `swarm/planner.rs`, `swarm/executor.rs` | 无 |
| 观测/回放 | `observability/`, `event_bus/`, `journal/` | `modules/harness/` |

---

## 四、分领域逐项对比（第二遍：基于源码深度阅读）

### 1. 统一编排器主链（ChatOrchestrator + ContextEngine + PromptBuilder）

**这是本次深度阅读中最重要的发现之一。** uclaw-rs 的编排层不是一个松散的模块集合，而是一个 **4 层结构化主链**：

#### Layer 1: ChatTurnRequest（18+ 字段的统一请求契约）

[`chat_orchestrator.rs:76-136`](uclaw-rs/src/orchestrator/chat_orchestrator.rs#L76-L136) 定义了 `ChatTurnRequest`，包含：
- 核心标识：`run_id`, `task_id`, `trace_id`, `session_id`, `agent_id`
- 上下文：`project_id`, `effective_workdir`, `project_context`
- 用户输入：`user_input`, `model`, `thinking_level`
- 工具：`tools: Vec<ToolSpec>`, `tool_policy`
- Skill/MCP：`selected_skill_ids`, `selected_mcp_refs`, `step_id`
- 控制标志：`force_tool_choice`, `plan_first_required`, `compression_mode`, `suppress_session_decisions`, `reasoning_required`

**关键模式**：这是一个 **Turn-level 完整契约**，而非简单的 "user message"。它包含了该 turn 需要的所有上下文、工具、策略、诊断追踪信息。

#### Layer 2: `run_turn_sidecar`（7 步编排流）

[`chat_orchestrator.rs:513-940`](uclaw-rs/src/orchestrator/chat_orchestrator.rs#L513-L940) 的主链：

```
1. 加载 episodic summary（支持 suppress_session_decisions 裁剪）
2. 冷启动种子消息（cold start recovery）
3. MemoryManager.prepare_context() → ContextPayload（三层记忆）
4. apply_runtime_context_compression() → 四级压缩管线
5. compute_effective_skill_ids() → is_casual_chat_turn 防护
6. plan_skill_shadow() → Skill 诊断 + Prompt Parity 校验
7. build_messages_with_notes() → PromptBuilder.build()
8. ContextBuiltInfo 快照（前端 context_built 事件）
9. cutover 路由 → client.run_turn()
10. 指标事件记录
```

**关键能力**：
- **Skill Shadow Diagnostics**（[`chat_orchestrator.rs:996-1071`](uclaw-rs/src/orchestrator/chat_orchestrator.rs#L996-L1071)）：在 turn 执行前，为每个 skill 做 `plan()` 调用，收集 prompt contributions、tool requests、preflight 检查结果，用于诊断 skill 是否真正参与了 prompt 构建。
- **Prompt Parity Verification**（[`chat_orchestrator.rs:674-700`](uclaw-rs/src/orchestrator/chat_orchestrator.rs#L674-L700)）：对比 host 端（前端）和 shadow 端（后端）的 prompt plan block hash，检测前后端 prompt 一致性。
- **Context Compression Pipeline**（[`chat_orchestrator.rs:1108-1205`](uclaw-rs/src/orchestrator/chat_orchestrator.rs#L1108-L1205)）：四级压缩 — `none → light(RuleCompressor) → standard(Rule+ShingleDeduplicator) → aggressive(Rule+Shingle+LLM Summarizer+LayeredDeriver)`，带 fallback 链和 degradation evidence。
- **Casual Chat Guard**（[`chat_orchestrator.rs:1579-1616`](uclaw-rs/src/orchestrator/chat_orchestrator.rs#L1579-L1616)）：基于启发式规则（<15 chars + 匹配 greeting 模式）跳过 skill 注入，避免 "hi" 触发完整的 skill shadow 计算。
- **ContextBuiltInfo**（[`chat_orchestrator.rs:148-167`](uclaw-rs/src/orchestrator/chat_orchestrator.rs#L148-L167)）：turn 上下文构建后的元数据快照（memory hit count、token 估算、skill shadow、prompt parity、compression evidence），供前端展示和调试。

#### Layer 3: ContextEngine（比例 Token 预算 + 瀑布式重分配）

[`context_engine.rs:1-323`](uclaw-rs/src/orchestrator/context_engine.rs) 实现了：

- **TokenBudget**：基于 tiktoken（cl100k_base）的精确 token 计数，fallback 到 chars/4 启发式
- **ContextBudgetSlots**：四槽比例分配 — System 10%, Summary 20%, Retrieved 30%, Recent 40%
- **Waterfall Reallocation**：当某个槽未用完时，剩余 token 按比例重分配到其他槽（特别是 Recent）
- **ContextEngine trait**：`bootstrap → ingest → assemble → compact → after_turn` 生命周期
- **DefaultContextEngine**：assemble() 构建 working memory + episodic summary，compact() 滚动窗口裁剪

**关键设计**：比例预算 + 瀑布重分配是一个 **通用的上下文窗口管理策略**，不依赖于 uclaw-rs 的 sidecar 架构。

#### Layer 4: PromptBuilder（动态内省 + 18 字段结构化注入）

[`prompt_builder.rs:1-564`](uclaw-rs/src/orchestrator/prompt_builder.rs) 实现了：

- **PromptBuildOptions**（18 字段）：`locale`, `timezone`, `skills`, `session_summary`, `recall_hint`, `retrieved_memories`, `working_memory`, `working_event_context`, `agent_notes`, `project_context`, `plan_first_required`, `skill_prompt_contributions`, `task_state_text`, `narration_policy`, `summary_budget_class`, `retrieved_budget_chars`, `recent_budget_chars`, `session_message_count`
- **注入块顺序**：`[Runtime Context] → [Active Skills] → [Project Context] → [Execution Plan Format] → [Conversation Summary] → [Relevant Memory] → [Agent Notes] → [Task State] → [Working Memory Timeline] → [Recent Conversation] → [Memory Recall Hint] → [Search Tool Available] → [Dynamic Introspection Policy]`
- **Dynamic Introspection Policy**：基于用户输入复杂度评分（长度 + 关键词 + 不确定性 + 情绪），自动选择 Off/Light/Full 三种内省模式
- **预算感知**：`retrieved_budget_chars` 和 `recent_budget_chars` 由 ContextBudgetSlots 计算而来，PromptBuilder 据此裁剪注入内容

#### if2Ai 差距分析

if2Ai 的 [`conversation.rs`](src-tauri/src/modules/runtime/conversation.rs) 是一个 **简化的同步 turn loop**：
- 有 `run_turn()` 状态机（user → LLM → tool → result → repeat）
- 有 `ContextBudget` 概念（[`budget.rs`](src-tauri/src/modules/runtime/budget.rs)）
- 有 `compact_session()`（[`compact.rs`](src-tauri/src/modules/runtime/compact.rs)）
- 有 `SystemPromptBuilder`（[`prompt.rs`](src-tauri/src/modules/runtime/prompt.rs)）

但缺少：
1. **ChatTurnRequest 级别的完整 turn 契约**（18+ 字段）
2. **比例预算 + 瀑布重分配**（当前 budget 是固定百分比）
3. **Skill Shadow Diagnostics**（skill 贡献诊断 + prompt parity）
4. **Context Compression Pipeline**（四级压缩 + fallback 链）
5. **Casual Chat Guard**（trivial turn 跳过 skill）
6. **ContextBuiltInfo**（上下文构建元数据快照）
7. **Dynamic Introspection Policy**（基于复杂度评分的 Off/Light/Full）

**分类: A — 最值得迁移的编排模式**（但不是直接复制代码，而是迁移 **架构模式**）：
- 提取 **TurnRequestAssembler**（对应 ChatTurnRequest 契约）
- 提取 **PromptBuilder**（去除 UClaw 产品语义：mood/pulse/reflect、agent-notes 路径、CAPTURE 协议）
- 提取 **比例预算 + 瀑布重分配**（纯算法，无产品语义）
- 提取 **Context Compression Pipeline**（RuleCompressor → ShingleDeduplicator → LLM Summarizer，纯算法）
- 提取 **Dynamic Introspection Policy**（复杂度评分逻辑，去除 mood 语义）
- 保留 if2Ai 的 `ConversationRuntime` 作为主 loop 实现

### 2. 记忆系统（MemoryManager + Hybrid Retrieval）

#### uclaw-rs MemoryManager 协调层

[`memory_manager.rs:1-1075`](uclaw-rs/src/memory/memory_manager.rs) 实现了完整的 7 步流程：

```
Step 1: ensure_warm() — 冷启动恢复（从磁盘种子 working memory）
Step 2: detect_recall_intent() — 意图门控（zh/en 30+ marker 匹配）
Step 3: build_enriched_query() — N2 查询增强（混入 session goal）
Step 4: search_memories_for_context_native() — 本地语义检索
Step 5: apply_rerank() — 双轨 rerank（Local/Jina/Llm/Hybrid + fallback 链）
Step 6: after_turn() — 更新 working memory
Step 7: should_store_user_message() — 写入策略（敏感度可调阈值）
```

**关键能力**：
- **RerankStrategy**（4 种）：Local（cosine×0.62 + keyword×0.23 + category_boost + recency_boost）、Jina（cross-encoder API，4s timeout）、Llm（sidecar 3s timeout）、Hybrid（Jina → Llm → Local 三级 fallback）
- **should_store**（敏感度 0.55）：基于 high_value_markers（40+ 中英文模式）+ explicit_memory_markers + trivial_patterns 过滤 + noisy_markers 过滤
- **is_near_duplicate**（三级去重）：vector score ≥ 0.82 → LanceDB distance ≤ 0.18 → lexical overlap ≥ 70%
- **detect_recall_intent**（30+ zh/en markers）：`你之前`、`上次`、`you said`、`continue`、`where were we` 等
- **category_boost**：identity +0.12, goal +0.08, todo +0.07, decision +0.06, preference +0.05
- **recency_boost**：≤1 天 +0.06, ≤7 天 +0.03, ≤30 天 +0.01

#### uclaw-rs MemorySearch（混合检索 + RRF 融合）

[`search.rs:1-707`](uclaw-rs/src/memory/search.rs) 实现了：

- **RetrievalIntent**（5 种）：Code/Task/Fact/Person/General，每种有独立的 vector/fts/graph 权重
- **环境可调权重**：`UCLAW_RETRIEVAL_CODE_VECTOR_WEIGHT` 等环境变量覆盖默认值
- **reciprocal_rank_fusion_weighted**：标准 RRF 公式 `Σ 1/(k + rank + 1) × weight`，k=60
- **Graph Traversal**（BFS）：从高 importance seed 出发，relation_type multiplier（RelatedTo/PartOf 可展开），max_depth 按 intent 调整（Code=1, Task=2, Fact/Person=3）
- **SearchMode**：Hybrid（FTS + Vector + Graph → RRF）/ Recent / Important / Typed

**关键对比**：
- if2Ai 有 **ActiveRetrievalManager**（[`retrieval.rs`](src-tauri/src/modules/memory/retrieval.rs)），基于 keyword 意图分类（代码/任务/事实/人物/通用）
- if2Ai 的 **VectorMemoryProvider** 使用 FastEmbed + LanceDB
- if2Ai 的 **HybridMemoryProvider**（HRR）支持代数推理
- 但 if2Ai 缺少：**RRF 融合**、**Intent-tuned 权重**、**Graph traversal**、**多级 rerank fallback 链**

#### if2Ai 已有的优势

- **Scope 隔离**（session/project/global 三级作用域）
- **Promotion**（session → project → global 记忆晋升）
- **Weibull 重要性衰减**
- **Audit + 前端事件推送**

#### 迁移策略

**不是迁移 uclaw-rs 的整个 MemoryManager，而是**：
1. 在 if2Ai 上新增 `modules/memory/manager.rs`（协调层）
2. 将 `prepare_context` / `after_turn` 流程适配到 if2Ai 的 `MemoryProvider` trait
3. 将 **RRF 融合算法**（纯数学，无产品语义）迁移到 if2Ai
4. 将 **RetrievalIntent + 环境可调权重**迁移
5. 将 **should_store + is_near_duplicate** 迁移
6. 将 **detect_recall_intent** 迁移
7. 保留 if2Ai 的 VectorMemoryProvider 作为存储引擎，不使用 uclaw-rs 的 MemoryStore

**分类: A — 必须迁移的协调层**（provider 层保留 if2Ai 实现，迁移 orchestration）

### 3. LLM 路由 + Model Capability Registry

#### uclaw-rs LLM 路由系统

基于 `llm/routing.rs`、`llm/dual_scorer.rs`、`llm/cooldown.rs`、`llm/classifier.rs`、`llm/model_caps.rs`：

- **RoutingConfig**：ProcessType + TaskType + Difficulty 三级路由
- **DualLayerScorer**：双层难度评估（查询特征 + 历史表现）
- **ProviderCooldownTracker**：基于 DashMap 的限速冷却追踪
- **call_with_retry**：重试 + fallback 链
- **Error Classification**：`is_retriable_error` / `is_context_overflow_error` / `is_rate_limit_error`
- **EightDimClassifier**：8 维度查询分类
- **ModelCapabilityRegistry**：模型能力注册表（tool support, context window, thinking mode）

#### if2Ai 现状

- `modules/provider/` 有基础 provider config
- 缺少动态难度感知路由
- 缺少 fallback 链
- 缺少冷却追踪
- 缺少模型能力注册

#### 迁移策略

1. **ProviderCooldownTracker** → 直接迁移（DashMap-based，独立）
2. **Error Classification** → 直接迁移（纯函数）
3. **DualLayerScorer** → 适配后迁移（去除 ProcessType 绑定）
4. **ModelCapabilityRegistry** → 直接迁移（通用能力）
5. **call_with_retry + fallback 链** → 迁移到 `modules/provider/` 的 dispatch 层

**分类: A — 高价值、低侵入**

### 4. Channels Gateway（InboundPipeline + SessionRouter）

#### uclaw-rs InboundPipeline

[`pipeline.rs:1-311`](uclaw-rs/src/channels/pipeline.rs) 实现了标准的 ingress 管线：

```
1. Adapter lookup（平台适配器）
2. Webhook signature verification（委托给 adapter）
3. Parse → ChannelEnvelope（标准化）
4. Dedupe（幂等去重，provider_message_id）
5. Rate limiting（DashMap token bucket，per session_key）
6. should_invoke_agent（DM = true, Group = @mention 检测）
```

**关键设计**：
- 纯函数式管线，每步返回明确的 PipelineError
- Token bucket 基于 DashMap（高并发）
- 有 polling 模式变体（跳过 webhook 签名）
- 完整测试覆盖（invalid_signature, valid_message, duplicate, rate_limit）

#### uclaw-rs SessionRouter

[`router.rs:1-286`](uclaw-rs/src/channels/router.rs) 实现了：

```
1. /stop 命令检测 → cancel_turn（abort handle）
2. Owner vs Guest 模式判定
3. Debounce 聚合（mpsc channel + timeout）
4. 窗口期内消息累积 → 到期后 dispatch
5. 并发安全（DashMap per session）
```

**关键设计**：
- 使用 `tokio::sync::mpsc` channel 做消息缓冲
- `AbortHandle` 做 turn 取消
- 50ms debounce 窗口（可配置）
- Owner/Guest 权限分派

#### 分析

这两个模块是 **IM 通道的 ingress 层**。它们的 **工程模式**（管线、去重、限流、debounce、取消）是通用的，但 **具体实现**（webhook 签名、ChannelEnvelope、AdapterRegistry、bot_username）深度绑定 IM 通道语义。

对于 if2Ai（Tauri 桌面应用）：
- 不需要 webhook 签名验证（IPC 调用）
- 不需要 ChannelEnvelope（Tauri 事件）
- 不需要 AdapterRegistry（无外部平台）
- **但需要**：debounce 聚合（快速连续输入合并）、/stop 取消（agent turn 中止）、rate limiting（防滥用）

**分类: B — 模式可借鉴，实现不直接迁移**。在 if2Ai 的 `modules/channel/` 中提取：
- **Turn Debouncer**（debounce 窗口 + 消息聚合）
- **Turn Canceller**（abort handle 管理）
- **Rate Limiter**（DashMap token bucket）

### 5. Run Snapshot / Startup Recovery / Replay

#### uclaw-rs 事件体系

[`contracts.rs:1-305`](uclaw-rs/src/runtime/contracts.rs) 定义了完整的 `RuntimeEvent` 枚举（20+ 变体）：

```
TurnStart, MessageStart/End/Update, ThinkingDelta, TextDelta,
ToolCallStart/ExecutionUpdate/CallEnd, TurnEnd,
AgentStart/End, AutoCompactionStart/End,
SessionLifecycle, SessionTitled, MemoryCaptured,
Error, ModelCompatApplied, ModelEmptyReply
```

**关键能力**：
- **MemoryCapturedItem**（结构化记忆捕获）：`text`, `kind`（preference/identity/constraint/decision/promise/factual）, `scope`, `confidence`, `conflict_key`, `evidence_excerpt`, `why_captured`, `project_id`
- **StreamEventEnvelope**：`seq`, `ts_ms`, `trace_id`, `session_id`, `turn_id`, `event`, `payload`
- **完整的事件生命周期**：从 TurnStart → MessageStart → TextDelta → ToolCall → ToolCallEnd → TurnEnd

#### Run Recovery

ChatOrchestrator 的 `run_turn_sidecar` 中：
- 每次 turn 有唯一的 `run_id`, `turn_id`, `trace_id`
- `cold_start` 恢复（`session_messages_cold` 种子 working memory）
- `SessionState` 管理（turn_abort handle）
- Shadow 模式下的并行 native 执行（审计用）

#### if2Ai 现状

- `modules/harness/` 有基础的 harness 状态管理
- `modules/session/manager.rs` 有 JSON 持久化
- 缺少事件总线（EventBus）
- 缺少执行日志（Journal）
- 缺少 Run Snapshot 机制

#### 迁移策略

1. **RuntimeEvent 枚举** → 迁移到 `modules/harness/events.rs`（通用事件定义）
2. **StreamEventEnvelope** → 迁移（序列化友好）
3. **MemoryCapturedItem** → 迁移（去除 UClaw 特定 kind，保留通用 kind）
4. **EventBus**（broadcast channel）→ 直接迁移
5. **Subagent Execution Journal** → 迁移到 `modules/harness/journal.rs`

**分类: A — 高价值、低侵入**

---

## 五、Staff 架构师建议：四大 "优化后迁移" 方案

### 建议 1：统一编排器主链（4 层架构）

**现状**：if2Ai 的编排逻辑分散在 `main.rs`（IPC 命令注册）、`commands/agent.rs`（agent turn 入口）、`runtime/conversation.rs`（turn loop）。

**目标架构**（迁移 uclaw-rs 的 4 层模式）：

```
TurnRequestAssembler → PromptBuilder → ToolLoopCoordinator → RunProjector
```

| 层 | 职责 | 迁移来源 | if2Ai 保留 |
|---|---|---|---|
| TurnRequestAssembler | 构建完整 turn 契约（工具列表、skill IDs、project context、workdir） | ChatTurnRequest 模式 | Tauri IPC 参数解析 |
| PromptBuilder | 组装系统 prompt（运行时上下文 + 技能 + 记忆 + 项目 + 动态内省） | PromptBuilder 去除产品语义 | if2Ai 的 SystemPromptBuilder 基础 |
| ToolLoopCoordinator | agent turn 循环（LLM → tool → permission → result → repeat） | conversation_actor 模式 | if2Ai ConversationRuntime |
| RunProjector | turn 后序列（memory capture、summary update、snapshot、event emit） | after_turn + MemoryCaptured | if2Ai 的 after_turn + harness |

**关键迁移项**：
- 比例 Token 预算 + 瀑布重分配（纯算法）
- Context Compression Pipeline（Rule → Shingle → LLM Summarizer）
- Casual Chat Guard（trivial turn 跳过 skill）
- Dynamic Introspection Policy（Off/Light/Full 复杂度评分）

**不迁移**：
- mood/pulse/reflect 区块（产品语义）
- agent-notes 文件路径（`~/.uclaw/` 语义）
- CAPTURE 记忆协议（`[CAPTURE: ...]` 格式）
- sidecar/native cutover 路由（架构不同）
- skill shadow diagnostics（依赖 uclaw-rs skill 注册体系）

### 建议 2：LLM 路由 + Model Capability Registry 迁移策略

**阶段 1**（独立、高价值）：
1. 迁移 `ProviderCooldownTracker` → `modules/provider/cooldown.rs`
2. 迁移 Error Classification 函数 → `modules/provider/errors.rs`
3. 迁移 `DualLayerScorer` → `modules/provider/scorer.rs`

**阶段 2**（适配后）：
4. 新增 `ModelCapabilityRegistry` → `modules/provider/caps.rs`
5. 将 `call_with_retry + fallback 链` 整合到 provider dispatch
6. 在 settings UI 中添加难度路由配置

**阶段 3**（长期）：
7. 评估 `EightDimClassifier` 是否需要（取决于 if2Ai 的查询分类需求）

### 建议 3：Memory Manager + Hybrid Retrieval 迁移策略

**原则**：保留 if2Ai 的 provider 层（SqliteMemoryProvider + VectorMemoryProvider + HRR），迁移 uclaw-rs 的协调层。

```
if2Ai 现有: MemoryProvider trait → Sqlite/Vector/HRR 实现
新增: MemoryManager (协调层) → prepare_context / after_turn / rerank / dedup
迁移算法: RRF 融合 + RetrievalIntent + should_store + is_near_duplicate + detect_recall_intent
```

**迁移顺序**：
1. 新增 `modules/memory/manager.rs`（MemoryManager 协调器）
2. 迁移 `detect_recall_intent`（30+ zh/en markers，无产品语义）
3. 迁移 `should_store_user_message`（敏感度阈值，去除 UClaw 特定 noisy_markers）
4. 迁移 `is_near_duplicate`（三级去重，纯算法）
5. 迁移 `reciprocal_rank_fusion_weighted`（RRF，纯数学）
6. 迁移 `RetrievalIntent` + 环境可调权重
7. 适配 `prepare_context` 到 if2Ai 的 `MemoryProvider` trait（使用 recall_scoped 而非 search_memories_for_context_native）
8. 适配 `after_turn` 到 if2Ai 的 `store_scoped`

### 建议 4：Run Snapshot / Startup Recovery / Replay

**目标**：为 if2Ai 增加可观测性和恢复能力。

**迁移内容**：
1. `RuntimeEvent` 枚举 → `modules/harness/events.rs`（去除 UClaw 特定变体）
2. `StreamEventEnvelope` → `modules/harness/envelope.rs`
3. `MemoryCapturedItem` → `modules/memory/captured.rs`（保留 kind/scope/confidence/why_captured）
4. EventBus（broadcast channel）→ `modules/harness/event_bus.rs`
5. Subagent Execution Journal → `modules/harness/journal.rs`

**if2Ai 新增能力**：
- Turn-level snapshot（run_id + turn_id + trace_id + ContextBuiltInfo）
- Cold start recovery（从持久化消息种子 working memory）
- Turn cancellation（AbortHandle 管理）

---

## 六、更新后的模块分类

### 分类 A：直接迁移（独立、高价值、低侵入）

| 模块 | 源文件 | 落点 | 理由 |
|---|---|---|---|
| 比例 Token 预算 + 瀑布重分配 | `context_engine.rs` | `modules/runtime/budget.rs` 扩展 | 纯算法，无产品语义 |
| Context Compression Pipeline | `chat_orchestrator.rs` apply_runtime_context_compression | `modules/runtime/compact.rs` 扩展 | Rule→Shingle→LLM 管线，通用 |
| RRF 融合算法 | `search.rs` reciprocal_rank_fusion_weighted | `modules/memory/` 新增 rrf.rs | 纯数学，无产品语义 |
| RetrievalIntent + 权重 | `search.rs` | `modules/memory/` 新增 intent.rs | 通用意图分类 |
| detect_recall_intent | `memory_manager.rs` | `modules/memory/` 新增 recall_intent.rs | 30+ zh/en markers |
| should_store_user_message | `memory_manager.rs` | `modules/memory/policy.rs` 扩展 | 敏感度阈值，通用 |
| is_near_duplicate | `memory_manager.rs` | `modules/memory/` 新增 dedup.rs | 三级去重，纯算法 |
| MemoryManager 协调层 | `memory_manager.rs` | `modules/memory/` 新增 manager.rs | prepare_context / after_turn |
| Dynamic Introspection Policy | `prompt_builder.rs` | `modules/runtime/prompt.rs` 扩展 | 复杂度评分，去除 mood |
| ApprovalGate | `tools/approval.rs` | `modules/tools/` 新增 approval.rs | 工具授权门 |
| ToolAuditStore | `tools/audit.rs` | `modules/tools/` 新增 audit.rs | 工具审计 |
| ToolLoopGuard | `tools/loop_guard.rs` | `modules/tools/` 新增 loop_guard.rs | 防循环调用 |
| ProviderCooldownTracker | `llm/cooldown.rs` | `modules/provider/` 新增 cooldown.rs | 限速冷却 |
| Error Classification | `llm/routing.rs` | `modules/provider/` 新增 errors.rs | 错误分类 |
| DualLayerScorer | `llm/dual_scorer.rs` | `modules/provider/` 新增 scorer.rs | 难度评估 |
| RuntimeEvent 枚举 | `contracts.rs` | `modules/harness/events.rs` | 事件定义 |
| EventBus | `event_bus/mod.rs` | `modules/harness/event_bus.rs` | 事件总线 |

### 分类 B：适配后迁移

| 模块 | 源文件 | 落点 | 适配要点 |
|---|---|---|---|
| TurnRequestAssembler | ChatTurnRequest 模式 | `modules/commands/agent.rs` 扩展 | 去除 UClaw 特定字段 |
| PromptBuilder 结构化注入 | `prompt_builder.rs` | `modules/runtime/prompt.rs` 重写 | 去除 mood/pulse/agent-notes 语义 |
| Turn Debouncer | `router.rs` debounce 模式 | `modules/channel/` 新增 debouncer.rs | 适配 Tauri IPC 而非 webhook |
| Turn Canceller | `router.rs` cancel_turn | `modules/channel/` 新增 canceller.rs | 适配 Tauri |
| Rate Limiter | `pipeline.rs` token bucket | `modules/channel/` 新增 rate_limiter.rs | 通用 |
| ModelCapabilityRegistry | `llm/model_caps.rs` | `modules/provider/` 新增 caps.rs | 通用 |
| call_with_retry + fallback | `llm/routing.rs` | `modules/provider/` dispatch 层 | 适配 if2Ai provider |

### 分类 C：不迁移（产品语义过重或架构不同）

| 模块 | 原因 |
|---|---|
| channels adapter 体系 | IM 产品语义（微信/IRC/Nostr） |
| mood/pulse/reflect | UClaw 人格/情绪系统 |
| agent-notes 文件路径 | `~/.uclaw/` 产品语义 |
| CAPTURE 记忆协议 | UClaw 特定格式 |
| sidecar cutover 路由 | if2Ai 直接 HTTP 调用 |
| skill shadow diagnostics | 依赖 uclaw-rs skill 体系 |
| 全部 swarm 模块 | 与 if2Ai 定位不符 |

---

## 七、最值得迁移的模块清单（已并入第六节分类 A）

## 八、需要适配后迁移的模块清单（已并入第六节分类 B）

## 九、不建议迁移的模块清单（已并入第六节分类 C）

---

## 十、if2Ai 反向领先的模块清单（分类 D: if2Ai 已有且更强）

| 模块 | if2Ai 优势 | uclaw-rs 差距 |
|---|---|---|
| **Memory Scope 隔离** | session/project/global 三级作用域 + promotion | uclaw-rs MemoryManager 无作用域概念 |
| **Memory HRR 代数推理** | HybridMemoryProvider (HRR + Vector) | uclaw-rs 无代数推理 |
| **Memory Audit + 前端推送** | 完整审计链 + Tauri 事件推送 | uclaw-rs 无前端事件推送 |
| **Skill 分发签名验证** | SkillDistributionEnvelope + SHA256 签名 | uclaw-rs 无技能安全治理 |
| **ToolContext 会话隔离** | dispatch_with_context 避免跨会话泄漏 | uclaw-rs WorkspaceGuard 较简单 |
| **LearningModule** | SelfModel + ReflectionEngine | uclaw-rs 无自学习模块 |
| **ActiveRetrievalManager** | 主动检索策略 | uclaw-rs 只有被动 recall |
| **Tauri 原生集成** | 浏览器控制、系统 tray、文件对话框 | uclaw-rs 纯 HTTP，无桌面能力 |
| **Onboarding 配置平台** | Phase 6G 完整 onboarding 流程 | uclaw-rs 无 onboarding |
| **预算配置文件** | budget.yaml 支持动态加载 | uclaw-rs 硬编码 |
| **ToolRegistry 安全治理** | requires_explicit_context 高风工具 gating | uclaw-rs 无内置高风工具 gating |

---

## 十一、如何避免迁移后出现"逻辑和事实真相双标"

### 11.1 识别 UClaw 历史产品语义 vs 通用平台能力

**产品语义特征**（不要迁移）:
- 人格绑定（Channel→Minami, Worker→Williams）
- IM 通道（微信桥、IRC、Nostr）
- 专属功能（ClawHub、UInvest、Oasis）
- 旧路径约定（`~/.uclaw/` 目录、`UCLAW_HOME` 环境变量）
- 侧车进程（sidecar LLM 子进程）
- mood/pulse/reflect 区块（uclaw-rs 人格化内省）
- CAPTURE 记忆协议（`[CAPTURE: ...]` / `[NOTE_WRITE: ...]` 格式）
- agent-notes 文件路径（`~/.uclaw/agent-notes/`）

**通用平台能力特征**（可以迁移）:
- 不依赖特定通道类型
- 不依赖特定目录/环境变量命名
- 不涉及人格/情绪
- 有清晰的 trait 边界
- 可被 if2Ai 现有模块直接消费

### 11.2 事实真相单一来源原则

| 事实 | if2Ai 单一来源 | 禁止 |
|---|---|---|
| 工具注册 | `ToolRegistry` | 不创建 `ToolDispatcher` |
| 会话状态 | `SessionManager` | 不引入 uclaw-rs 的 `ProcessId` |
| 记忆存储 | `MemoryProvider` trait | 不绕过 provider 直接操作底层存储 |
| Agent 循环 | `modules/runtime/` | 不引入 uclaw-rs 的 `ConversationActor` |
| 权限控制 | `modules/runtime/permissions.rs` | 不并行维护两套权限 |
| Prompt 构建 | `modules/runtime/prompt.rs` | 不创建 uclaw-rs 风格的 PromptBuilder 副本 |
| 事件系统 | `modules/harness/` | 不引入 uclaw-rs 的 `RuntimeEvent` 副本（迁移后统一） |

### 11.3 迁移时的重命名策略

迁移时必须重命名以避免命名冲突和概念混淆：
- `ToolDispatcher` → 不迁移，扩展 `ToolRegistry`
- `ProcessRunner` → `AgentProcessRunner`
- `ProcessControlRegistry` → `AgentProcessRegistry`
- `RoutingConfig` → `ModelRoutingConfig`
- `McpManager` → `McpConnectionManager`
- `MemoryManager` → `MemoryCoordinator`（避免与 if2Ai 的 MemoryProvider 混淆）
- `ChatTurnRequest` → `AgentTurnRequest`
- `ChatOrchestrator` → `AgentOrchestrator`

---

## 十二、推荐迁移优先级

### P0 — 立即迁移（独立、高价值、低侵入）

| 优先级 | 模块 | 工作量 | 依赖 |
|---|---|---|---|
| P0-1 | ApprovalGate | 0.5 天 | 无 |
| P0-2 | ToolAuditStore | 0.5 天 | ApprovalGate |
| P0-3 | ToolLoopGuard | 0.5 天 | 无 |
| P0-4 | ToolInvocationLifecycle | 0.5 天 | 无 |
| P0-5 | EventBus | 0.25 天 | 无 |
| P0-6 | ProviderCooldownTracker | 0.5 天 | 无 |
| P0-7 | 错误分类函数 | 0.25 天 | 无 |
| P0-8 | RRF 融合算法 | 0.5 天 | 无 |
| P0-9 | detect_recall_intent | 0.25 天 | 无 |
| P0-10 | is_near_duplicate | 0.25 天 | 无 |

### P1 — 第二阶段迁移（需适配、中等侵入）

| 优先级 | 模块 | 工作量 | 依赖 |
|---|---|---|---|
| P1-1 | RiskBasedPolicyEngine → permissions | 1 天 | P0-1, P0-2 |
| P1-2 | ToolDescriptor 增强 → ToolEntry | 0.5 天 | P0 模块 |
| P1-3 | DualLayerScorer + ModelRoutingConfig | 1.5 天 | 无 |
| P1-4 | MemoryCoordinator (prepare_context/after_turn) | 2 天 | MemoryProvider 稳定 |
| P1-5 | should_store_user_message → policy.rs | 0.5 天 | P1-4 |
| P1-6 | RetrievalIntent + 权重 → intent.rs | 0.5 天 | P0-8 |
| P1-7 | RuntimeEvent 枚举 → events.rs | 0.5 天 | EventBus |
| P1-8 | 比例 Token 预算 → budget.rs | 0.5 天 | 无 |
| P1-9 | Turn Debouncer + Canceller | 1 天 | 无 |
| P1-10 | Dynamic Introspection Policy | 0.5 天 | 无 |

### P2 — 长期评估（复杂、与架构定位相关）

| 优先级 | 模块 | 工作量 | 依赖 |
|---|---|---|---|
| P2-1 | ProcessRunner + 进程生命周期 | 3 天 | agent loop 重构 |
| P2-2 | SwarmRuntime | 5+ 天 | 需产品定位确认 |
| P2-3 | UnifiedExecutionPolicy | 2 天 | permissions 重构 |
| P2-4 | Context Compression Pipeline | 1 天 | compact 模块重构 |
| P2-5 | PromptBuilder 结构化注入（去除产品语义） | 2 天 | P1-4 |
| P2-6 | ModelCapabilityRegistry | 1 天 | P1-3 |

---

## 十三、给实现者的落地建议

### 13.1 迁移顺序（Cursor 实施路线图）

**Phase 1: 工具安全治理层 (P0-1 ~ P0-7)**

```
Step 1: 将 ApprovalGate (tools/approval.rs) 复制到 src-tauri/src/modules/tools/approval.rs
        - 修改：去掉 uclaw-rs 特定的 import 路径
Step 2: 将 ToolAuditStore (tools/audit.rs) 复制到 src-tauri/src/modules/tools/audit.rs
Step 3: 将 ToolLoopGuard (tools/loop_guard.rs) 复制到 src-tauri/src/modules/tools/loop_guard.rs
Step 4: 将 ToolInvocationLifecycle 复制到 src-tauri/src/modules/tools/invocation_state.rs
Step 5: 将 EventBus 复制到 src-tauri/src/modules/harness/event_bus.rs
Step 6: 将 ProviderCooldownTracker 和错误分类函数复制到 src-tauri/src/modules/provider/
Step 7: 在 ToolRegistry 中增加可选的 ApprovalGate + AuditStore 字段
        - 通过 builder 模式可选注入
Step 8: 在 dispatch 流程中串联：RiskCheck → ApprovalGate → Execute → Audit
```

**Phase 2: Memory 协调层 + 检索增强 (P0-8 ~ P0-10, P1-4 ~ P1-6)**

```
Step 1: 将 RRF 融合算法复制到 src-tauri/src/modules/memory/rrf.rs
Step 2: 将 detect_recall_intent 复制到 src-tauri/src/modules/memory/recall_intent.rs
Step 3: 将 is_near_duplicate 复制到 src-tauri/src/modules/memory/dedup.rs
Step 4: 将 RetrievalIntent + 权重复制到 src-tauri/src/modules/memory/intent.rs
Step 5: 将 MemoryManager 协调层复制到 src-tauri/src/modules/memory/manager.rs
        - 重命名为 MemoryCoordinator
        - 适配 prepare_context 到 if2Ai 的 MemoryProvider::recall_scoped
        - 适配 after_turn 到 if2Ai 的 MemoryProvider::store_scoped
Step 6: 将 should_store_user_message 合并到 src-tauri/src/modules/memory/policy.rs
Step 7: 在 agent turn 流程中接入 prepare_context / after_turn
```

**Phase 3: LLM 路由增强 (P1-3, P1-8)**

```
Step 1: 将 DualLayerScorer 复制到 src-tauri/src/modules/provider/scorer.rs
Step 2: 将 ProviderCooldownTracker 复制到 src-tauri/src/modules/provider/cooldown.rs
Step 3: 将错误分类函数复制到 src-tauri/src/modules/provider/errors.rs
Step 4: 将比例 Token 预算 + 瀑布重分配合并到 src-tauri/src/modules/runtime/budget.rs
Step 5: 在 provider 模块中增加 call_with_retry + fallback 链
```

**Phase 4: 事件系统 + 可观测性 (P0-5, P1-7)**

```
Step 1: 将 RuntimeEvent 枚举复制到 src-tauri/src/modules/harness/events.rs
        - 去除 UClaw 特定变体（如 ProcessType 相关）
Step 2: 将 EventBus 复制到 src-tauri/src/modules/harness/event_bus.rs
Step 3: 将 StreamEventEnvelope 复制到 src-tauri/src/modules/harness/envelope.rs
Step 4: 在 agent turn 流程中 emit 事件
```

### 13.2 代码迁移规范

1. **每个文件迁移时**:
   - 顶部加注释：`// Migrated from uclaw-rs/src/<原始路径>, adapted for if2Ai`
   - 删除所有 UClaw 特定引用（`crate::messaging::`, `crate::config::settings::`）
   - 替换为 if2Ai 等价路径（`crate::modules::`）
   - 删除人格绑定（process_persona 等）
   - 删除 mood/pulse/reflect 相关代码
   - 删除 `~/.uclaw/` 路径引用

2. **禁止行为**:
   - 不引入第二套 ToolRegistry / ToolDispatcher
   - 不引入第二套 Session truth source
   - 不复制 `~/.uclaw/` 路径约定
   - 不迁移任何 `persona` / `mood` / `process_persona` 代码
   - 不迁移 CAPTURE 记忆协议
   - 不迁移 agent-notes 文件路径

3. **测试要求**:
   - 每个迁移的模块必须有对应的单元测试
   - 复用 uclaw-rs 的测试用例，但改用 if2Ai 的依赖

---

## 十四、风险与回归点

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| ApprovalGate 与现有 respond_permission 命令冲突 | 中 | 先审计现有 permission 流程，确认兼容后再集成 |
| RiskBasedPolicyEngine 与 PermissionMode 重叠 | 高 | 设计合并方案，不要并行维护两套 |
| MemoryCoordinator 与现有 MemoryProvider 接口不兼容 | 高 | 先定义适配层，不直接修改 provider trait |
| RoutingConfig 与现有 provider config 冲突 | 中 | 作为增强字段加入现有配置，不替代 |
| McpManager 与现有 MCP client 重复 | 中 | McpManager 只做连接管理，client 做协议交互 |
| ProcessRunner 与 Tauri 事件循环不兼容 | 高 | 先用 sync adapter 包一层，再考虑原生 async |
| SwarmRuntime 过于复杂且定位不匹配 | 高 | 标记为 P2，等产品需要时再评估 |
| PromptBuilder 迁移引入产品语义 | 中 | 严格审查注入块，只保留通用块 |
| RRF 融合与现有 ActiveRetrievalManager 冲突 | 中 | RRF 作为独立模块，不替代现有检索 |

---

## 十五、最终结论

**uclaw-rs 是一个功能丰富的 Agent 后端，但其核心价值不在于"多通道 IM Agent"，而在于它在运行过程中积累的通用平台能力**。

**对 if2Ai 最有价值的迁移资产排序**:

1. **工具安全治理** (ApprovalGate + Audit + RiskPolicy + LoopGuard) — 填补 if2Ai 工具系统最大的空白
2. **LLM 路由** (难度感知 + fallback 链 + 冷却追踪) — 提升模型调用可靠性和成本效率
3. **Memory 协调层** (MemoryCoordinator prepare_context/after_turn/rerank/dedup) — 补齐 if2Ai 记忆系统缺失的流程控制
4. **RRF 融合 + RetrievalIntent** — 升级 if2Ai 的检索质量
5. **EventBus + Journal** — 轻量级事件总线和执行回放
6. **比例 Token 预算 + 瀑布重分配** — 优化上下文窗口利用率
7. **Dynamic Introspection Policy** — 智能调节 LLM 内省深度

**if2Ai 应坚守的优势阵地**:
- Memory scope 隔离 + promotion + HRR
- Skill 安全治理
- Tauri 原生集成
- LearningModule 自学习
- ToolRegistry 高风工具 gating

**迁移原则**: 渐进式、trait-first、绝不创建第二套 truth source。每个迁移模块都必须有清晰的接口契约和测试覆盖。

**四大 "优化后迁移" 方案总结**:
1. **统一编排器主链**：迁移架构模式（TurnRequestAssembler → PromptBuilder → ToolLoopCoordinator → RunProjector），不直接复制代码
2. **LLM 路由**：迁移独立组件（CooldownTracker + Scorer + Error Classification），适配到 if2Ai provider
3. **Memory Manager + Hybrid Retrieval**：保留 if2Ai provider 层，迁移 uclaw-rs 协调层（RRF + Intent + should_store + dedup）
4. **Run Snapshot / Recovery / Replay**：迁移事件体系和 journal，增加可观测性
