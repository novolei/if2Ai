# ADR-013 Critical Gaps — 详细实施 backlog

## 概述

**目标**: 修复 Phase 6B/6BW 中最严重的 5 个 Critical gaps，这些 gaps 导致核心生产管线不完整或失效。
**ADR**: [ADR-013](./ADR-013-Phase-Remediation-Design.md)
**优先级**: P0 (最高 — 必须在其他层级之前修复)
**预估工时**: 2-3 天
**基于审计**: [phase-6b-6bw-6e-gap-audit-report.md](../../../../generated/phase-6b-6bw-6e-gap-audit-report.md) v2

**为什么单独拆分**: C1-C5 涉及核心 agent loop 代码，每个必须独立可测试。修复后才能安全进行 High/Medium/Phase 6E 修复。

---

## 依赖关系

```
C2: AppState extension ──┬── C4: ReflectionEngine wiring (依赖 C2 提供持久化实例)
                          └── H1: SessionManager active retrieval (依赖 C2)
C1: WorkingMemory ──────── 独立，可并行
C5: WeibullDecay ───────── 需要扩展 MemoryProvider trait
C4: ReflectionEngine ──── 依赖 C2
```

---

## TASK-013-C1: WorkingMemory Not Integrated into ConversationRuntime

**对应 TASK**: 012-05
**对应 ADR**: ADR-004 (Token Budget Allocation), ADR-012 (Wiring)

### 目的

`WorkingMemory` (max_turns=8, max_tokens=1600 sliding window) 是 4-slot ContextBudget 架构的核心组件。没有它接入 `ConversationRuntime`，agent 会向每次 LLM 调用发送**全部** session messages，导致：
- 上下文随 session 长度无限增长
- 40% 的 ContextBudget（Working slot）完全浪费 — 滑动窗口未生效
- API 成本随 session 长度线性增长而非有界

### 当前状态

- `WorkingMemory` struct 存在于 `src-tauri/src/modules/memory/working_memory.rs` — 完整实现，7 个测试通过
- `ConversationRuntime` 在 `src-tauri/src/modules/runtime/conversation.rs:133-143` **没有** `working_memory` 字段
- `run_turn()` 在 `conversation.rs:272-274` 发送 `self.session.messages.clone()` — 完整消息列表
- `agent.rs:999-1019` 创建了一个**局部** `WorkingMemory::default()` 仅用于日志 — 不影响实际 API 调用

### 实施计划

**Step 1**: 添加 `working_memory` 字段到 `ConversationRuntime`

File: `src-tauri/src/modules/runtime/conversation.rs:133-143`

```rust
use super::working_memory::WorkingMemory;

pub struct ConversationRuntime<C, T> {
    session: Session,
    api_client: C,
    tool_executor: T,
    permission_policy: PermissionPolicy,
    system_prompt: Vec<String>,
    max_iterations: usize,
    context_budget: Option<ContextBudget>,
    usage_tracker: UsageTracker,
    hook_runner: HookRunner,
    working_memory: Option<WorkingMemory>,  // NEW: sliding window for LLM context
}
```

**Step 2**: 添加 builder 方法

File: `conversation.rs:191-205` — 在 `with_context_budget` 之后

```rust
/// Set the working memory sliding window.
///
/// When Some, only the most recent N turns (default: 8) within
/// max_tokens (default: 1600) are sent to the LLM. Older messages
/// remain in the session for continuity but are excluded from the API call.
#[must_use]
pub fn with_working_memory(mut self, working_memory: WorkingMemory) -> Self {
    self.working_memory = Some(working_memory);
    self
}
```

**Step 3**: 在构造函数中初始化为 `None`

File: `conversation.rs:178-188`

```rust
Self {
    // ... existing fields ...
    working_memory: None,
}
```

**Step 4**: 在 `run_turn()` API 调用中应用 WorkingMemory

File: `conversation.rs:272-276`

```rust
// BEFORE:
let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: self.session.messages.clone(),
    tools: Some(self.tool_executor.get_definitions()),
};

// AFTER:
let context_messages = match &self.working_memory {
    Some(wm) => wm.get_context(&self.session.messages),
    None => self.session.messages.clone(),
};

let request = ApiRequest {
    system_prompt: self.system_prompt.clone(),
    messages: context_messages,
    tools: Some(self.tool_executor.get_definitions()),
};
```

**Step 5**: 在 agent.rs `run_agent_turn()` 中接入

File: `src-tauri/src/commands/agent.rs` — 构造 `ConversationRuntime` 的位置

```rust
// BEFORE: existing construction
let mut runtime = ConversationRuntime::new_with_features(
    runtime_session, api_client, tool_executor, permission_policy, system_prompt, feature_config,
);

// AFTER: add WorkingMemory builder
let mut runtime = ConversationRuntime::new_with_features(
    runtime_session, api_client, tool_executor, permission_policy, system_prompt, feature_config,
)
.with_working_memory(WorkingMemory::default());
```

**Step 6**: 移除冗余的 post-turn WorkingMemory 检查

File: `agent.rs:999-1019` — 移除局部 `WorkingMemory::default()` 创建和日志块。Working memory 现在在 API 调用级别强制执行，此 post-turn 检查变得冗余。

### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/runtime/conversation.rs` | 添加 `working_memory` 字段 + builder + `run_turn` 中应用 |
| `src-tauri/src/commands/agent.rs` | 接入 WorkingMemory 到 runtime 构造，移除冗余 post-turn 检查 |

### 风险

- **上下文丢失**: 如果 WorkingMemory 排除了 LLM 需要的消息。缓解：`WorkingMemory::get_context()` 设计上保留 system message 和最近 N turns。
- **测试断裂**: `conversation.rs` 现有测试未设置 WorkingMemory。缓解：`Option<WorkingMemory>` 默认为 `None` — 现有测试不变。

### 验收标准

- [ ] `ConversationRuntime` 拥有 `working_memory: Option<WorkingMemory>` 字段
- [ ] `with_working_memory()` builder 存在且带有 doc 注释
- [ ] `run_turn()` 仅在 `Some` 时应用 WorkingMemory，`None` 路径保留给测试
- [ ] `agent.rs` 中冗余的 post-turn 检查已移除
- [ ] 所有现有测试无需修改即可通过
- [ ] 没有对 `self.working_memory` 的 `unwrap()`

---

## TASK-013-C2: AppState Missing 3 Critical Fields

**对应 TASK**: 012-01
**对应 ADR**: ADR-012 (Wiring Layer 1)

### 目的

`AppState` 当前只持有 `memory_provider` 和 `context_budget` 两个字段，缺少 5 个计划字段中的 3 个。缺失的 `trajectory_manager`、`learning_module` 和 `active_retrieval_manager` 导致：
- **TrajectoryManager** 每次 turn 重新创建（agent.rs:715-735 创建局部实例）
- **LearningModule** 每次 turn 重新创建（agent.rs:971 创建局部实例）— 状态无法跨 turn 积累
- **ActiveRetrievalManager** 在 AppState 层不可访问 — 预 LLM 检索无法使用共享 memory provider

### 当前状态

File: `src-tauri/src/commands/mod.rs:21-48`

```rust
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub project_manager: Arc<ProjectManager>,
    pub permission_senders: ...,
    pub permission_overrides: ...,
    pub stream_cancel_senders: ...,
    pub memory_provider: SharedMemoryProvider,       // exists
    pub context_budget: ContextBudget,               // exists
    // MISSING:
    // pub trajectory_manager: Option<Arc<TrajectoryManager>>,
    // pub learning_module: Option<Arc<Mutex<LearningModule>>>,
    // pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
}
```

### 实施计划

**Step 1**: 扩展 `AppState` struct

File: `src-tauri/src/commands/mod.rs:21-48`

```rust
use crate::modules::learning::LearningModule;
use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::memory::retrieval::ActiveRetrievalManager;

pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub project_manager: Arc<ProjectManager>,
    pub permission_senders: Arc<Mutex<HashMap<String, Sender<PermissionPromptDecision>>>>,
    pub permission_overrides: Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
    pub stream_cancel_senders: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    pub memory_provider: SharedMemoryProvider,
    pub context_budget: ContextBudget,
    // NEW: Persistent memory infrastructure (Phase 6BW wiring)
    pub trajectory_manager: Option<Arc<TrajectoryManager>>,
    pub learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
    pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
}
```

**Step 2**: 更新 `AppState::new()` 签名

File: `src-tauri/src/commands/mod.rs:50-71`

```rust
pub fn new(
    session_manager: SessionManager,
    tool_registry: ToolRegistry,
    project_manager: ProjectManager,
    memory_provider: SharedMemoryProvider,
    context_budget: ContextBudget,
    trajectory_manager: Option<Arc<TrajectoryManager>>,     // NEW
    learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>, // NEW
    active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,     // NEW
) -> Self {
    Self {
        session_manager: Arc::new(session_manager),
        tool_registry: Arc::new(tool_registry),
        project_manager: Arc::new(project_manager),
        permission_senders: Arc::new(Mutex::new(HashMap::new())),
        permission_overrides: Arc::new(Mutex::new(HashMap::new())),
        stream_cancel_senders: Arc::new(Mutex::new(HashMap::new())),
        memory_provider,
        context_budget,
        trajectory_manager,     // NEW
        learning_module,        // NEW
        active_retrieval_manager, // NEW
    }
}
```

**Step 3**: 更新 `main.rs` 初始化

File: `src-tauri/src/main.rs:213-223`

```rust
// BEFORE:
let app_state = AppState::new(
    session_manager,
    tool_registry,
    project_manager,
    memory_provider,
    context_budget,
);

// AFTER:
// Initialize TrajectoryManager
let trajectory_base = if2ai_dir.join("trajectories");
let trajectory_manager = TrajectoryManager::new(trajectory_base)
    .ok()
    .map(Arc::new);

// Initialize LearningModule (depends on memory_provider + trajectory_manager)
let learning_module = match trajectory_manager.clone() {
    Some(tm) => {
        let memory_for_learning = memory_provider.clone();
        match tokio::runtime::Runtime::new().unwrap().block_on(
            LearningModule::new(memory_for_learning)
        ) {
            Ok(lm) => Some(Arc::new(tokio::sync::Mutex::new(lm))),
            Err(e) => {
                tracing::warn!("[memory] LearningModule failed to init: {e}");
                None
            }
        }
    }
    None => {
        tracing::info!("[memory] TrajectoryManager not available, skipping LearningModule");
        None
    }
};

// Initialize ActiveRetrievalManager (non-blocking, best-effort)
let active_retrieval_manager = Some(Arc::new(ActiveRetrievalManager::with_defaults()));

let app_state = AppState::new(
    session_manager,
    tool_registry,
    project_manager,
    memory_provider,
    context_budget,
    trajectory_manager,
    learning_module,
    active_retrieval_manager,
);
```

**Step 4**: 重构 `agent.rs` 使用 AppState 级别的实例

File: `agent.rs:715-735` — `record_trajectory_if_possible`

```rust
// BEFORE: creates local TrajectoryManager
async fn record_trajectory_if_possible(session: &RuntimeSession, system_prompt: &[String]) {
    match TrajectoryManager::new(/* base_path */) { ... }
}

// AFTER: use AppState-level instance
async fn record_trajectory_if_possible(
    state: &AppState,
    session: &RuntimeSession,
    system_prompt: &[String],
) {
    if let Some(ref tm) = state.trajectory_manager {
        // use tm.record(...)
    }
}
```

File: `agent.rs:971-981` — LearningModule usage

```rust
// BEFORE: creates local LearningModule every turn
if let Ok(mut learning) = LearningModule::new(state.memory_provider.clone()).await {
    learning.self_model_mut().record_turn(...);
}

// AFTER: use AppState-level instance
if let Some(ref lm) = state.learning_module {
    let mut learning = lm.lock().await;
    learning.self_model_mut().record_turn(/* success= */ true, /* response_time_ms= */ 0.0);
}
```

### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/commands/mod.rs` | 添加 3 个字段到 AppState + 更新 `new()` 签名 |
| `src-tauri/src/main.rs` | 初始化 3 个新字段 + 更新 `AppState::new()` 调用 |
| `src-tauri/src/commands/agent.rs` | 重构为使用 AppState 级别实例，而非每 turn 创建 |

### 风险

- **tokio runtime in main.rs**: `LearningModule::new` 是 async 但 `main()` 是 sync。解决：使用临时 `tokio::runtime::Runtime` 进行初始化（与 `create_memory_provider` 相同模式）。
- **初始化顺序**: `LearningModule` 依赖 `memory_provider` — 按正确顺序初始化。

### 验收标准

- [ ] AppState 拥有 3 个新的 `Option<>` 字段，支持优雅降级
- [ ] `AppState::new()` 接受 3 个新参数
- [ ] `main.rs` 初始化所有 3 个字段，失败时使用 `tracing::warn!`
- [ ] `agent.rs` 不再每次 turn 创建局部 TrajectoryManager/LearningModule
- [ ] wiring 代码中没有 `unwrap()`

---

## TASK-013-C4: ReflectionEngine Has Zero Callers

**对应 TASK**: 008-08
**对应 ADR**: ADR-008 (Self-Learning Modules)

### 目的

`ReflectionEngine` 是自学习系统的核心。它分析工具序列、结果和主题，生成 `Reflection` 对象来更新 `SelfModel`。没有调用方意味着：
- Agent 永远不会反思自己的行为模式
- `SelfModel` 只跟踪 turn 数量，从不学习能力/限制
- 信任跟踪是单向的（仅工具结果），从不从反思更新

### 当前状态

- `ReflectionEngine` trait + `StandardReflectionEngine` 存在于 `src-tauri/src/modules/learning/reflection.rs`
- `LearningModule` 在内部创建 `StandardReflectionEngine`（`learning/mod.rs:64`）
- **零调用方**: 没有代码调用 `learning.reflection_engine.analyze_session()` 或 `learning.reflect()`
- `agent.rs:971-981` 调用 `learning.self_model_mut().record_turn()` 但从不触发反思

### 实施计划

**Step 1**: 添加反思触发到 post-turn 序列

File: `agent.rs:970-981` — 在现有 `record_turn` 调用之后

```rust
// AFTER: add reflection trigger
if let Some(ref lm) = state.learning_module {
    let mut learning = lm.lock().await;

    // Record turn outcome
    learning.self_model_mut().record_turn(
        /* success= */ true, /* response_time_ms= */ 0.0
    );

    // Trigger reflection every N turns (configurable, default: 5)
    let reflect_interval = 5;
    if learning.self_model().turn_count > 0
        && learning.self_model().turn_count % reflect_interval == 0
    {
        match learning.reflection_engine.analyze_session(&trajectory_session).await {
            Ok(reflections) => {
                learning.self_model.update_from_reflections(&reflections);
                tracing::info!(
                    "[run_agent_turn] Reflection triggered: {} insights",
                    reflections.len()
                );
            }
            Err(e) => {
                tracing::warn!("[run_agent_turn] Reflection failed: {e}");
            }
        }
    }
}
```

**Step 2**: 确保 `SelfModel` 拥有 `update_from_reflections` 方法

File: `src-tauri/src/modules/learning/self_model.rs`

如果不存在，添加：

```rust
impl SelfModel {
    pub fn update_from_reflections(&mut self, reflections: &[Reflection]) {
        for reflection in reflections {
            if let Some(pattern) = reflection.learned_pattern.as_ref() {
                self.learned_patterns.push(pattern.clone());
            }
            if let Some(cap) = reflection.new_capability.as_ref() {
                self.capabilities.push(cap.clone());
            }
            if let Some(limit) = reflection.limitation.as_ref() {
                self.limitations.push(limit.clone());
            }
        }
    }
}
```

**Step 3**: 移除 learning module 的 `#![allow(dead_code)]`

File: `learning/mod.rs:10`

```rust
// REMOVE: #![allow(dead_code)]
// The module now has callers in the agent loop.
```

### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/commands/agent.rs` | 添加 turn 记录后的反射触发 |
| `src-tauri/src/modules/learning/self_model.rs` | 添加 `update_from_reflections()`（如缺失） |
| `src-tauri/src/modules/learning/mod.rs` | 移除 `#![allow(dead_code)]` |

### 风险

- **反射延迟**: `analyze_session` 可能很慢（可能涉及 LLM）。缓解：异步运行，不阻塞用户响应。
- **反射间隔**: 每 5 turns 是合理的默认值但应该可配置。

### 验收标准

- [ ] Reflection 在可配置的间隔触发（默认：每 5 turns）
- [ ] Reflection 错误记录为 `warn!`，不是 `error!`
- [ ] `SelfModel::update_from_reflections` 存在且处理 reflections
- [ ] `learning/mod.rs` 不再有模块级 `#![allow(dead_code)]`
- [ ] Reflection 不阻塞面向用户的响应

---

## TASK-013-C5: WeibullDecay Results Never Applied to MemoryProvider

**对应 TASK**: 012-09
**对应 ADR**: ADR-012 (Wiring Layer 3)

### 目的

`WeibullDecay` 计算压缩条目的重要性衰减因子，但结果**从未应用回** `MemoryProvider`。这意味着：
- 情景条目永久保留其原始重要性
- 记忆检索返回重要性被高估的过时条目
- `compute_importance()` 方法存在但从未在压缩后的实时数据上调用

### 当前状态

- `WeibullDecay` 存在于 `episodic_compaction.rs:16-72`
- `agent.rs:1021-1031` 创建 `WeibullDecay::default()` 并计算 `decay_factor()` 但**仅用于日志**
- `MemoryProvider` trait 没有 `apply_importance_decay()` 方法 — 需要添加
- `compact_episodic_entries()` 在 `episodic_compaction.rs:113-173` 移除低重要性条目但不更新剩余条目

### 实施计划

**Step 1**: 添加 `apply_importance_decay` 到 `MemoryProvider` trait

File: `src-tauri/src/modules/memory/mod.rs`

```rust
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    // ... existing methods ...

    /// Apply importance decay to all episodic entries based on their age.
    /// Called after compaction to adjust remaining entries' importance scores.
    async fn apply_importance_decay(&self, decay: &WeibullDecay) -> Result<usize, MemoryError>;
}
```

**Step 2**: 为 `SqliteMemoryProvider` 实现

File: `src-tauri/src/modules/memory/providers/sqlite_provider.rs`

```rust
async fn apply_importance_decay(&self, decay: &WeibullDecay) -> Result<usize, MemoryError> {
    let conn = self.conn.lock().map_err(|_| MemoryError::Generic("lock poisoned".into()))?;

    // Get all episodic entries
    let mut stmt = conn.prepare(
        "SELECT key, importance, created_at FROM memory_entries WHERE category = 'episodic'"
    ).map_err(|e| MemoryError::Generic(format!("query failed: {e}")))?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?, row.get::<_, String>(2)?))
    })?;

    let mut updated = 0;
    for row in rows {
        let (key, _old_importance, created_at_str) = row?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| MemoryError::Generic(format!("parse date: {e}")))?;

        let age_hours = (chrono::Utc::now() - created_at).num_hours() as f32;
        let decay_factor = decay.decay_factor(age_hours);

        // Update importance = importance * decay_factor
        conn.execute(
            "UPDATE memory_entries SET importance = importance * ?1 WHERE key = ?2",
            params![decay_factor, key],
        )?;
        updated += 1;
    }

    Ok(updated)
}
```

**Step 3**: 为 `VectorMemoryProvider` 实现

File: `src-tauri/src/modules/memory/providers/vector_provider.rs`

类似模式 — 遍历 LanceDB 条目，应用衰减，更新。

**Step 4**: 在 agent.rs post-turn 中应用衰减

File: `agent.rs:1021-1031`

```rust
// BEFORE: only logs decay factor
let decay = WeibullDecay::default();
let removed_count = pre_compact_message_count.saturating_sub(post_compact_message_count);
if removed_count > 0 {
    let decay_factor = decay.decay_factor(24.0);
    tracing::info!(...);
}

// AFTER: actually apply decay
let decay = WeibullDecay::default();
let removed_count = pre_compact_message_count.saturating_sub(post_compact_message_count);
if removed_count > 0 {
    match state.memory_provider.apply_importance_decay(&decay).await {
        Ok(updated) => {
            tracing::info!(
                "[run_agent_turn] WeibullDecay applied: {updated} entries updated"
            );
        }
        Err(e) => {
            tracing::warn!("[run_agent_turn] WeibullDecay failed: {e}");
        }
    }
}
```

### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/memory/mod.rs` | 添加 `apply_importance_decay()` 到 `MemoryProvider` trait |
| `src-tauri/src/modules/memory/providers/sqlite_provider.rs` | 实现 `apply_importance_decay` |
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | 实现 `apply_importance_decay` |
| `src-tauri/src/commands/agent.rs` | 压缩后调用 `apply_importance_decay` |

### 风险

- **SQLite 写入竞争**: 衰减更新发生时其他操作可能在读取。缓解：`Mutex<Connection>` 已保护写入。
- **LanceDB 更新成本**: 向量 DB 更新可能很慢。缓解：批量更新，后台运行。

### 验收标准

- [ ] `MemoryProvider` trait 拥有 `apply_importance_decay()` 方法
- [ ] `SqliteMemoryProvider` 使用正确的 SQL UPDATE 实现它
- [ ] `VectorMemoryProvider` 实现它（可以是 no-op + `tracing::info`）
- [ ] `agent.rs` 压缩后调用它，带有错误处理
- [ ] 衰减应用代码中没有 `unwrap()`

---

## 验收总览

- [ ] C1: WorkingMemory 接入 ConversationRuntime，run_turn 应用滑动窗口
- [ ] C2: AppState 扩展 3 字段，main.rs 初始化，agent.rs 使用持久实例
- [ ] C4: ReflectionEngine 每 5 turns 触发，SelfModel 处理 reflections
- [ ] C5: MemoryProvider trait 扩展，WeibullDecay 实际应用于压缩后条目
- [ ] cargo fmt + clippy + test 全部通过
- [ ] learning/mod.rs 移除 #![allow(dead_code)]
- [ ] 所有 wiring 代码使用 `tracing::warn!` 而非 panic
