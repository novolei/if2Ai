# ADR-012 Phase 6B Wiring — 详细实施 backlog

## 概述

**目标**: 将 Phase 6B 所有"孤岛"模块接入生产管线，使记忆系统成为可用的端到端功能
**ADR**: [ADR-012](./ADR-012-Phase6B-Wiring.md)
**优先级**: P0 (Critical — 已实现的模块未被接入生产流程)
**预估工时**: 5-7 天

---

## Gap 总览

### G1-G12 Gap 清单（来自 Phase 6B 审查报告）

| Gap | 描述 | 严重程度 | 对应 TASK |
|-----|------|----------|-----------|
| G1 | VectorMemoryProvider/ActiveRetrievalManager/HybridMemoryProvider 全为 dead code | 高 | TASK-012-04, 05, 09 |
| G2 | LearningModule 无调用方 | 高 | TASK-012-07, 08 |
| G3 | TrajectoryManager 无 record 调用方 | 高 | TASK-012-06 |
| G4 | ContextBudget 不在 agent loop 中检查 | 高 | TASK-012-02 |
| G5 | WorkingMemory 不被 agent loop 使用 | 中 | TASK-012-05 |
| G6 | HRR tests 全部 #[ignore] | 中 | TASK-012-10 |
| G7 | Memory UI 完全不存在 | 高 | TASK-012-11~015 |
| G8 | Trajectory 导出无 UI 触发 | 中 | TASK-012-14 |
| G9 | 无 Memory 相关 Settings UI | 中 | TASK-012-13 |
| G10 | default_memory_provider() 硬编码 SQLite | 低 | TASK-012-09 |
| G11 | fastembed 测试需下载模型 | 中 | TASK-012-10 |
| G12 | QUALITY_SCORE.md 缺少前端条目 | 低 | TASK-012-15 |

---

## 子任务清单

### TASK-012-01: AppState 扩展 — 添加 Memory Infrastructure 字段

**目标**: 将 memory_provider, context_budget, trajectory_manager, learning_module 纳入 AppState

**具体任务**:

1. **修改 `src-tauri/src/commands/mod.rs`** (line 18-32):
   - 在 `AppState` struct 中添加字段:
     ```rust
     pub memory_provider: SharedMemoryProvider,
     pub context_budget: ContextBudget,
     pub trajectory_manager: Option<Arc<TrajectoryManager>>,
     pub learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
     pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
     ```
   - 更新 `AppState::new()` 签名接受这些参数

2. **修改 `src-tauri/src/main.rs`** (line 115-121):
   - 创建 context_budget: `ContextBudget::default()`
   - 创建 trajectory_manager: `TrajectoryManager::new(if2ai_dir.join("trajectories"))`
   - 创建 learning_module: `LearningModule::new(memory_provider.clone(), trajectory_manager.clone())`
   - 创建 active_retrieval_manager: `ActiveRetrievalManager::with_defaults()`
   - 将所有新字段传入 `AppState::new()`

3. **确保所有新字段为 `Option<>` 类型**，以便 graceful fallback

**验收标准**:
- [ ] AppState 编译通过
- [ ] main.rs 初始化所有新字段
- [ ] 如果 trajectory_manager 创建失败，AppState 仍正常启动
- [ ] 如果 learning_module 创建失败，AppState 仍正常启动
- [ ] `cargo clippy -D warnings` 通过

**相关文件**:
- `src-tauri/src/commands/mod.rs` — AppState struct
- `src-tauri/src/main.rs` — app initialization

---

### TASK-012-02: ContextBudget 接入 Agent Loop

**目标**: 用 `ContextBudget` 替代 `max_token_budget: Option<usize>` 简单检查

**具体任务**:

1. **修改 `src-tauri/src/modules/runtime/conversation.rs`** (line 132-142):
   - 替换 `max_token_budget: Option<usize>` 为 `context_budget: Option<ContextBudget>`
   - 替换 `with_max_token_budget()` 为 `with_context_budget()`

2. **修改 `src-tauri/src/modules/runtime/conversation.rs`** (line 239-247):
   - 替换简单预算检查为 ContextBudget 的 slot-based 验证:
     ```rust
     if let Some(ref budget) = self.context_budget {
         let system_tokens = estimate_tokens(&self.system_prompt);
         let working_tokens = estimate_tokens(&self.session.messages);
         // Check each slot: System 10%, Episodic 20%, Semantic 30%, Working 40%
         budget.validate_slots(system_tokens, working_tokens)?;
     }
     ```

3. **修改 `src-tauri/src/commands/agent.rs`** (line 772+):
   - 从 `state.context_budget` 读取配置传入 `ConversationRuntime`

**验收标准**:
- [ ] ContextBudget 默认值 4000 tokens, 10/20/30/40%
- [ ] 超过预算时返回错误（或触发 compaction）
- [ ] 现有 `max_token_budget` 向后兼容（deprecated）
- [ ] `cargo test --workspace` 全部通过

**相关文件**:
- `src-tauri/src/modules/runtime/conversation.rs` — run_turn 中的预算检查
- `src-tauri/src/modules/runtime/budget.rs` — ContextBudget struct（已有实现，无需修改）
- `src-tauri/src/commands/agent.rs` — 传入 context_budget

---

### TASK-012-03: ActiveRetrievalManager 接入 Pre-LLM-Call

**目标**: 在 LLM 调用前自动检索记忆并注入 context

**具体任务**:

1. **修改 `src-tauri/src/commands/agent.rs`** (line 770-786):
   - 在创建 `ConversationRuntime` 之前:
     ```rust
     // 1. Intent 分类
     let intent = QueryIntent::classify(&user_message);

     // 2. 主动检索
     if let (Some(mem), Some(arm)) = (&state.memory_provider, state.active_retrieval_manager.as_ref()) {
         let context_entries = arm
             .pre_llm_call(mem, &user_message, intent)
             .await
             .unwrap_or_default();

         // 3. 注入 context 到 system prompt
         if !context_entries.is_empty() {
             system_prompt.push(format_relevant_context(&context_entries));
         }
     }
     ```

2. **修改 `src-tauri/src/modules/memory/retrieval.rs`**:
   - 确保 `ActiveRetrievalManager.pre_llm_call()` 返回 `Vec<MemoryEntry>`
   - 确保配置开关 `enabled: bool` 可控制是否启用

3. **添加辅助函数 `format_relevant_context()`**:
   ```rust
   fn format_relevant_context(entries: &[MemoryEntry]) -> String {
       let context_lines: Vec<String> = entries.iter()
           .map(|e| format!("- [{}] {}: {}", e.category.as_str(), e.key, e.content))
           .collect();
       format!("Relevant context:\n{}", context_lines.join("\n"))
   }
   ```

**验收标准**:
- [ ] pre_llm_call 在 LLM 调用前执行
- [ ] 检索结果注入 system prompt
- [ ] 配置开关可禁用主动检索
- [ ] Intent 分类正确识别 Code/Task/Fact/Person/General
- [ ] RRF Fusion (k=60) 融合多个来源结果

**相关文件**:
- `src-tauri/src/commands/agent.rs` — agent loop 接入点
- `src-tauri/src/modules/memory/retrieval.rs` — ActiveRetrievalManager
- `src-tauri/src/modules/memory/intent.rs` — QueryIntent

---

### TASK-012-04: VectorMemoryProvider 移除 dead_code 并接入

**目标**: 将 VectorMemoryProvider 从 dead code 升级为可调用的生产代码

**具体任务**:

1. **修改 `src-tauri/src/modules/memory/providers/vector_provider.rs`**:
   - 移除 `#![allow(dead_code)]` 模块级标注
   - 替换为函数级 `#[allow(dead_code)]` 用于尚未被消费的内部方法
   - 确保 `MemoryProvider` trait 实现的方法不被 allow

2. **修改 `src-tauri/src/modules/memory/providers/mod.rs`**:
   - 添加 feature-gated re-export:
     ```rust
     #[cfg(feature = "vector-search")]
     pub use vector_provider::VectorMemoryProvider;
     ```

3. **添加 mock embedding provider 用于测试**:
   ```rust
   #[cfg(test)]
   struct MockEmbedder;

   impl EmbeddingProvider for MockEmbedder {
       fn embed(&self, _text: &str) -> Result<Vec<f32>> {
           // Return deterministic dummy vector (384 zeros)
           Ok(vec![0.0; 384])
       }
   }
   ```

**验收标准**:
- [ ] vector_provider.rs 不再有模块级 dead_code
- [ ] VectorMemoryProvider 可在代码中被调用
- [ ] Mock embedder 使测试不再需要下载模型
- [ ] `cargo test` 通过

**相关文件**:
- `src-tauri/src/modules/memory/providers/vector_provider.rs`
- `src-tauri/src/modules/memory/providers/mod.rs`
- `src-tauri/src/modules/memory/embedding/fastembed.rs`

---

### TASK-012-05: WorkingMemory 接入 Agent Loop

**目标**: 用 `WorkingMemory` 替代原始 `Vec<ConversationMessage>` 作为上下文窗口

**具体任务**:

1. **修改 `src-tauri/src/modules/runtime/conversation.rs`** (line 132-142):
   - 在 `ConversationRuntime` struct 中添加字段:
     ```rust
     pub working_memory: Option<WorkingMemory>,
     ```
   - 添加 `with_working_memory()` builder 方法

2. **修改 `src-tauri/src/modules/runtime/conversation.rs`** (line 220-253):
   - 在 `run_turn()` 中构建 `ApiRequest` 前，使用 WorkingMemory 限制上下文:
     ```rust
     // 使用 WorkingMemory 获取上下文窗口
     let context_messages = if let Some(ref mut wm) = self.working_memory {
         wm.get_context(&self.session.messages)
     } else {
         self.session.messages.clone()
     };

     let request = ApiRequest {
         system_prompt: self.system_prompt.clone(),
         messages: context_messages,  // 不再直接用 self.session.messages
         tools: Some(self.tool_executor.get_definitions()),
     };
     ```

3. **修改 `src-tauri/src/commands/agent.rs`** (line 772+):
   - 从 AppState 读取 working_memory 配置传入 ConversationRuntime

**验收标准**:
- [ ] WorkingMemory max_turns=8 默认值生效
- [ ] WorkingMemory max_tokens=1600 默认值生效
- [ ] 超过限制时最旧条目被驱逐
- [ ] Agent loop 行为等价（测试应通过）

**相关文件**:
- `src-tauri/src/modules/runtime/conversation.rs` — run_turn 中的上下文获取
- `src-tauri/src/modules/memory/working_memory.rs` — WorkingMemory（已有实现）

---

### TASK-012-06: TrajectoryManager 接入 Session Lifecycle

**目标**: 在每个 agent turn 完成后记录轨迹

**具体任务**:

1. **修改 `src-tauri/src/commands/agent.rs`** (line 868-878 之后):
   ```rust
   // 在 save_session 之后:
   if let Some(ref tm) = state.trajectory_manager {
       let model_id = "unknown"; // TODO: 从配置获取
       if let Ok(trajectory) = Trajectory::from_session(
           &updated_app_session,
           &system_prompt.join("\n"),
           model_id,
       ) {
           if let Err(e) = tm.record(&trajectory).await {
               tracing::warn!("[trajectory] Failed to record: {e}");
           }
       }
   }
   ```

2. **确保 `TrajectoryManager.record()` 是 async** 或在 tokio spawn 中执行

**验收标准**:
- [ ] 每个 turn 完成后 trajectory 被记录到 `~/.if2ai/trajectories/` 目录
- [ ] 文件按日期轮转 (trajectory_YYYY-MM-DD.jsonl)
- [ ] 100MB 文件轮转生效
- [ ] 隐私控制（min_session_length=3）生效
- [ ] 记录失败只 warn 不阻塞主流程

**相关文件**:
- `src-tauri/src/commands/agent.rs` — agent turn 完成后记录
- `src-tauri/src/modules/learning/trajectory.rs` — TrajectoryManager（已有实现）

---

### TASK-012-07: LearningModule 初始化与 Session Hook

**目标**: 初始化 LearningModule 并在 session 结束时触发反思

**具体任务**:

1. **修改 `src-tauri/src/modules/learning/mod.rs`**:
   - 添加 `init_learning()` 函数:
     ```rust
     pub fn init_learning(
         memory: SharedMemoryProvider,
         trajectory_manager: Arc<TrajectoryManager>,
     ) -> Result<LearningModule, LearningError> {
         let mut lm = LearningModule::new(memory);
         lm.set_trajectory_manager(trajectory_manager);
         Ok(lm)
     }
     ```

2. **修改 `src-tauri/src/commands/agent.rs`** (line 878+):
   ```rust
   // 在 trajectory 记录之后:
   if let Some(ref lm) = state.learning_module {
       let mut learning = lm.lock().await;
       learning.record_turn(
           &updated_app_session,
           summary.usage.output_tokens,
           summary.iterations,
       );

       // 每 N 个 turn 触发反思（默认 5）
       if learning.should_reflect() {
           match learning.reflect_on_session(&updated_app_session).await {
               Ok(reflections) => learning.update_self_model(&reflections),
               Err(e) => tracing::warn!("[learning] Reflection failed: {e}"),
           }
       }
   }
   ```

**验收标准**:
- [ ] LearningModule 在 main.rs 中初始化
- [ ] record_turn 在每个 agent turn 后调用
- [ ] 反射每 5 个 turn 触发一次
- [ ] SelfModel 的 performance_metrics 被更新
- [ ] 失败不阻塞主流程

**相关文件**:
- `src-tauri/src/modules/learning/mod.rs` — LearningModule
- `src-tauri/src/commands/agent.rs` — agent turn 完成后触发学习
- `src-tauri/src/main.rs` — init_learning 调用

---

### TASK-012-08: FrozenSnapshot 接入 Session Init

**目标**: 在 session 开始时捕获系统提示快照

**具体任务**:

1. **修改 `src-tauri/src/commands/agent.rs`** (line 772+):
   ```rust
   // 在创建 runtime 之前:
   let snapshot = FrozenSnapshot::capture(&system_prompt);

   // ... 创建 runtime 并执行 turn ...

   // turn 完成后验证:
   if !snapshot.verify(&system_prompt) {
       tracing::warn!("[run_agent_turn] System prompt modified since session start");
   }
   ```

**验收标准**:
- [ ] FrozenSnapshot 在 session init 时捕获
- [ ] 修改 prompt 后 verify 返回 false
- [ ] 警告日志记录 prompt 修改

**相关文件**:
- `src-tauri/src/commands/agent.rs` — session init 和 turn 完成后
- `src-tauri/src/modules/runtime/snapshot.rs` — FrozenSnapshot（已有实现）

---

### TASK-012-09: WeibullDecay 接入 Compaction

**目标**: 在 session compaction 后触发 WeibullDecay

**具体任务**:

1. **修改 `src-tauri/src/commands/agent.rs`** (line 857-866):
   ```rust
   // 在 compaction 之后:
   if should_compact(&updated_runtime_session, compaction_config) {
       let compact_result = compact_session(&updated_runtime_session, compaction_config);
       let removed_count = updated_runtime_session.messages.len()
           - compact_result.compacted_session.messages.len();

       if removed_count > 0 {
           // 触发 WeibullDecay 调整 episodic memory 重要性
           let decay = WeibullDecay::default();
           let adjustments = decay.calculate_adjustments(removed_count);
           if let Err(e) = state.memory_provider.apply_importance_decay(&adjustments).await {
               tracing::warn!("[decay] Failed to apply Weibull decay: {e}");
           }
       }

       compact_result.compacted_session
   }
   ```

2. **扩展 `MemoryProvider` trait** (如果需要):
   - 添加 `apply_importance_decay(&self, adjustments: &[ImportanceAdjustment]) -> Result<(), MemoryError>`

3. **修改 `src-tauri/src/modules/memory/providers/sqlite_provider.rs`**:
   - 实现 `apply_importance_decay()` 方法

**验收标准**:
- [ ] compaction 后 WeibullDecay 被触发
- [ ] 旧条目的 importance 被衰减
- [ ] lambda=7d, k=1.2 参数生效
- [ ] 失败不阻塞主流程

**相关文件**:
- `src-tauri/src/commands/agent.rs` — compaction 之后
- `src-tauri/src/modules/runtime/episodic_compaction.rs` — WeibullDecay（已有实现）
- `src-tauri/src/modules/memory/mod.rs` — MemoryProvider trait
- `src-tauri/src/modules/memory/providers/sqlite_provider.rs` — 实现 decay

---

### TASK-012-10: HybridMemoryProvider 与 HRR 测试修复

**目标**: 修复 HRR 集成测试，添加 mock embedding

**具体任务**:

1. **修改 `src-tauri/src/modules/memory/hrr/integration.rs`**:
   - 移除 `#[ignore]` 标记的测试
   - 添加 mock embedding provider:
     ```rust
     #[cfg(test)]
     struct MockEmbedder;

     impl EmbeddingProvider for MockEmbedder {
         fn embed(&self, text: &str) -> Result<Vec<f32>> {
             // 生成确定性向量（基于文本 hash）
             let hash = fasthash::hash(text);
             let mut vec = vec![0.0f32; 384];
             let seed = hash as u32;
             for (i, v) in vec.iter_mut().enumerate() {
                 *v = ((seed.wrapping_mul(1103515245 + i as u32) >> 16) as f32) / 32768.0;
             }
             Ok(vec)
         }
     }
     ```

2. **修改 `src-tauri/src/modules/memory/embedding/fastembed.rs`**:
   - 在测试中使用 MockEmbedder 替代 FastEmbedProvider

3. **修改 `src-tauri/src/modules/memory/hrr/store.rs`**:
   - 确保 `HolographicStore` 测试不依赖外部模型

**验收标准**:
- [ ] HRR integration tests 不再 `#[ignore]`
- [ ] `cargo test memory::hrr` 全部通过
- [ ] `cargo test memory::embedding` 全部通过
- [ ] Mock embedder 生成确定性向量

**相关文件**:
- `src-tauri/src/modules/memory/hrr/integration.rs`
- `src-tauri/src/modules/memory/hrr/store.rs`
- `src-tauri/src/modules/memory/embedding/fastembed.rs`

---

### TASK-012-11: Memory Provider 升级 — Vector 优先回退 SQLite

**目标**: 将 `create_memory_provider()` 升级为 Vector 优先，SQLite fallback

**具体任务**:

1. **修改 `src-tauri/src/main.rs`** (line 41-61):
   ```rust
   fn create_memory_provider() -> SharedMemoryProvider {
       // 尝试 VectorMemoryProvider (FastEmbed + LanceDB)
       match VectorMemoryProvider::new(default_vector_config()).await {
           Ok(vector) => {
               tracing::info!("[memory] VectorMemoryProvider initialized");
               Arc::new(vector)
           }
           Err(e) => {
               tracing::warn!("[memory] VectorMemoryProvider failed: {e}, falling back to SQLite");
               create_sqlite_provider()
           }
       }
   }
   ```

2. **添加超时保护**:
   ```rust
   // FastEmbed 模型下载有超时保护
   let vector_result = tokio::time::timeout(
       std::time::Duration::from_secs(30),
       VectorMemoryProvider::new(config),
   ).await;
   ```

**验收标准**:
- [ ] VectorMemoryProvider 优先尝试
- [ ] 30 秒超时回退 SQLite
- [ ] 日志记录回退原因
- [ ] 离线环境下 SQLite 可用

**相关文件**:
- `src-tauri/src/main.rs` — create_memory_provider 函数

---

### TASK-012-12: HybridMemoryProvider 配置化启用

**目标**: 通过配置开关启用 HRR 代数推理

**具体任务**:

1. **添加配置结构**:
   ```rust
   #[derive(Debug, Clone, serde::Deserialize)]
   pub struct HybridMemoryConfig {
       pub hrr_enabled: bool,          // 默认 false
       pub hrr_capacity_multiplier: f64, // 默认 1.0
       pub vector_search_enabled: bool, // 默认 true
   }
   ```

2. **修改 `src-tauri/src/main.rs`**:
   ```rust
   if config.hybrid.hrr_enabled {
       let hybrid = HybridMemoryProvider::new(hybrid_config).await?;
       Arc::new(hybrid)
   } else {
       // 使用 VectorMemoryProvider 或 SQLite
   }
   ```

**验收标准**:
- [ ] hrr_enabled 默认 false
- [ ] HybridMemoryProvider 在启用时正常工作
- [ ] HRR contradict 检测可调用

**相关文件**:
- `src-tauri/src/main.rs` — 配置化初始化
- `src-tauri/src/modules/memory/hrr/integration.rs` — HybridMemoryProvider

---

## 前端 TASK 清单

### TASK-012-13: Memory Browser UI 组件

**目标**: 创建前端 Memory 浏览面板

**具体任务**:

1. **创建 `src/components/memory/MemoryBrowser.tsx`**:
   - 记忆列表（分页）
   - 搜索框（调用 memory_recall Tauri command）
   - 删除按钮（调用 memory_forget）
   - 分类标签导航（Core / Daily / Conversation / Custom）

2. **创建 `src/components/memory/MemoryCard.tsx`**:
   - 显示 key, content, category, created_at
   - 显示 importance (0.0-1.0) 进度条
   - 显示 trust_score (-1.0~1.0) 颜色渐变
   - 显示 access_count

3. **创建 `src/components/memory/MemoryCategoryNav.tsx`**:
   - 分类标签切换
   - 创建自定义分类

4. **在 App Router 中添加 `/memory` 路由**

**验收标准**:
- [ ] 可查看/搜索/删除记忆
- [ ] 分类筛选生效
- [ ] UI 风格匹配设计文档（暖色调纸张感）
- [ ] trust_score 颜色渐变（红色负分 → 绿色正分）

**相关文件**:
- `src/components/memory/` — 新目录
- `src/App.tsx` — 添加路由
- `src/api/tauri.ts` — 添加 memory 调用封装

---

### TASK-012-14: Token Budget Settings UI

**目标**: 创建 Token Budget 配置界面

**具体任务**:

1. **创建 `src/components/settings/MemorySettings.tsx`**:
   - Total tokens 输入框（默认 4000）
   - 四个槽位的百分比滑块（System/Episodic/Semantic/Working）
   - 百分比总和必须 = 100%
   - 保存按钮

2. **创建 Tauri command 读取/写入配置**:
   ```rust
   #[tauri::command]
   async fn get_memory_config() -> Result<MemoryConfig, String> { ... }

   #[tauri::command]
   async fn set_memory_config(config: MemoryConfig) -> Result<(), String> { ... }
   ```

**验收标准**:
- [ ] 可调整四个槽位百分比
- [ ] 总和必须 100%
- [ ] 保存后配置持久化
- [ ] 配置在 app 重启后生效

**相关文件**:
- `src/components/settings/MemorySettings.tsx` — 新文件
- `src-tauri/src/commands/settings.rs` — 配置命令

---

### TASK-012-15: Trajectory Export UI

**目标**: 创建轨迹导出按钮

**具体任务**:

1. **在 `src/components/settings/MemorySettings.tsx` 中添加**:
   - "Export Trajectories" 按钮
   - 调用 `export_trajectories` Tauri command
   - 显示导出结果（成功/失败/文件路径）
   - 显示当前轨迹数量

2. **创建 `src/components/settings/TrajectoryInfo.tsx`** (可选):
   - 显示轨迹文件路径
   - 显示最后导出时间

**验收标准**:
- [ ] 点击按钮可导出 ShareGPT JSONL
- [ ] 显示文件路径
- [ ] 显示当前轨迹数量

**相关文件**:
- `src/components/settings/MemorySettings.tsx` — 添加导出按钮
- `src-tauri/src/modules/commands/trajectory.rs` — export_trajectories 命令（已有）

---

## 验收标准总览

### 编译与 Lint
- [ ] `cargo fmt --all` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [ ] `cargo test --workspace` 全部通过（573+ tests）
- [ ] 无 `#![allow(dead_code)]` 模块级标注（改用函数级）

### 功能验证
- [ ] AppState 包含所有新字段
- [ ] ContextBudget 在 agent loop 中检查
- [ ] ActiveRetrievalManager 在 LLM call 前检索
- [ ] TrajectoryManager 在 turn 完成后记录
- [ ] LearningModule 记录 turn 并周期性反射
- [ ] WorkingMemory 替代 Vec 作为上下文窗口
- [ ] FrozenSnapshot 在 session init 时捕获
- [ ] WeibullDecay 在 compaction 后触发
- [ ] HRR tests 不再 `#[ignore]`
- [ ] VectorMemoryProvider 可被调用

### 前端验证
- [ ] Memory Browser 可查看/搜索/删除记忆
- [ ] Settings 页可配置 Token Budget
- [ ] 可导出 Trajectory 到文件

### 文档更新
- [ ] `docs/generated/QUALITY_SCORE.md` 追加 Phase 6BW 完成记录
- [ ] `docs/exec-plans/active/phase-6bw-memory-wiring.yaml` 更新 dashboard
