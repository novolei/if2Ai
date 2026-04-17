# Phase 6B + 6BW + 6E Gap Audit Report — v2 (Backlog-Driven)

> **审计日期**: 2026-04-16
> **审计范围**: Phase 6B, 6BW, 6E
> **审计方法**: 12 个 backlog 文档逐 TASK checkbox 对照 + 代码文件级验证
> **忽略范围**: `harness/` Python 目录 (与真实 APP 无关)
> **版本**: v2 — 基于全部 backlog 文档的深度审计

---

## Executive Summary

| Phase | 声明状态 | 实际状态 | 达成率 |
|-------|---------|---------|--------|
| **6B** | `complete` (9/9) | **基本达成** | ~72% |
| **6BW** | `complete` (9/9) | **部分达成** | ~55% |
| **6E** | `draft` (0/7) | **未开始** | 0% |

**按 backlog TASK 统计 (全部 12 个 ADR)**:

| 状态 | 数量 | 百分比 |
|------|------|--------|
| ✅ COMPLETE | 54 | 49% |
| ⚠️ PARTIAL | 20 | 18% |
| ❌ NOT DONE | 36 | 33% |
| **总计** | **110** | **100%** |

---

## Part I: 按 ADR Backlog 逐 TASK 审计

### ADR-001: SQLite P0 Persistence (8 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 001-01 | 环境依赖 (sqlx/rusqlite) | ✅ | rusqlite 0.32 bundled in Cargo.toml |
| 001-02 | MemoryEntry 扩展字段 | ✅ | importance(f64), access_count(u64), trust_score(f64) |
| 001-03 | SqliteMemoryProvider 结构 | ✅ | `conn: Arc<Mutex<Connection>>`, auto-create table |
| 001-04 | MemoryProvider trait (store/recall/delete) | ✅ | INSERT OR REPLACE, fuzzy search, KeyNotFound |
| 001-05 | purge_category + export | ✅ | 完整实现, 33 测试覆盖 |
| 001-06 | 路径管理 get_memory_db_path() | ⚠️ | `default_memory_provider()` 路径构造正确但无独立辅助函数 |
| 001-07 | 模块集成 default_memory_provider | ✅ | 返回 `Arc<SqliteMemoryProvider>`, InMemory deprecated |
| 001-08 | 数据迁移 (JSON→SQLite) | ❌ | `migrate_from_json()` 不存在, legacy.json 导入功能缺失 |

**达成率**: 6/8 = 75%

---

### ADR-002: Active Retrieval vs Passive Invocation (8 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 002-P0-01 | MemoryProvider trait 5 方法 | ✅ | async trait 完整 |
| 002-P0-02 | 被动调用验证 | ⚠️ | LLM tool calling schema 未在代码中暴露 |
| 002-P1-01 | QueryIntent 枚举 (5 种) | ✅ | Code/Task/Fact/Person/General + weights |
| 002-P1-02 | Intent 分类器 (规则引擎) | ✅ | 13 测试, 关键词匹配正确 |
| 002-P1-03 | RetrievalWeights + From<QueryIntent> | ✅ | 完整实现 |
| 002-P1-04 | RRF Fusion (k=60) | ✅ | weighted_fusion + rrf_fusion, 12 测试 |
| 002-P1-05 | ActiveRetrievalManager (pre_llm_call) | ⚠️ | struct 存在, `retrieve()` + `retrieve_as_context()` 有, 但无 `pre_llm_call()` / `inject_memory()` 方法名 |
| 002-P1-06 | SessionManager 三层集成 | ❌ | `active_retrieval` 字段不在 SessionManager, 无 `active_retrieval_enabled` 配置 |

**达成率**: 4/8 = 50%

---

### ADR-003: FastEmbed + LanceDB Selection (8 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 003-01 | 依赖安装 | ✅ | fastembed 4, lancedb 0.18, arrow-* |
| 003-02 | FastEmbedProvider (384d) | ✅ | multilingual-e5-small, embed/embed_one |
| 003-03 | LanceDB 表 schema | ✅ | key/content/category/embedding(384d)/created_at |
| 003-04 | Vector Insert + Search | ⚠️ | insert/search/delete 有, 但 **update() 方法缺失** |
| 003-05 | Hybrid Search (FTS5 + Vector) | ⚠️ | hybrid_search 有 RRF, 但 FTS 是客户端过滤, 非真实 FTS5 索引 |
| 003-06 | Category + Pagination | ⚠️ | category filter + limit 有, **无 offset 分页, 无去重** |
| 003-07 | IVF-PQ 索引优化 | ❌ | 无 `create_index()` 调用, 无索引配置 |
| 003-08 | SQLite 双写集成 | ⚠️ | VectorMemoryProvider 实现 MemoryProvider trait, 但 **不向 SQLite 双写** |

**达成率**: 2/8 = 25%

---

### ADR-004: Token Budget Allocation (8 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 004-01 | ContextBudget 结构体 | ✅ | total=4000, 10/20/30/40%, validate() |
| 004-02 | ContextSlots 结构体 | ✅ | Slot, available(), slot_mut(), evict |
| 004-03 | Token 计数 | ⚠️ | 4:1 char/token 启发式, **无 tiktoken-rs, 无 estimate_messages_tokens()** |
| 004-04 | WorkingMemory 滑动窗口 | ✅ | max_turns=8, max_tokens=1600, 7 测试 |
| 004-05 | FrozenSnapshot | ✅ | capture + verify, 5 测试, DefaultHasher |
| 004-06 | EpisodicCompaction | ✅ | compact_episodic_entries, 按 importance 排序保留 |
| 004-07 | Weibull 衰减 | ✅ | lambda=7d, k=1.2, 6 测试 |
| 004-08 | YAML 配置加载 | ❌ | BudgetConfig + 文件解析不存在 |

**达成率**: 6/8 = 75%

---

### ADR-005: Upstream Claw-CLI 关系 (5 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 005-01 | MemoryEntry schema 对齐 | ✅ | 基础字段兼容, 扩展字段文档化 |
| 005-02 | Session schema 对齐 | ⚠️ | 部分匹配 — 缺 `project_id`, `title`, `created_at`, `updated_at`, `token_count`, `pinned` 字段 |
| 005-03 | compact.rs 溯源注释 | ✅ | "adapted from iClaw/hermes-agent" 完整 |
| 005-04 | claw-cli 导出工具 | ✅ | compat.rs: export_for_clawcli(), ClawCliMemoryEntry |
| 005-05 | UPSTREAM-RELATIONSHIP.md | ✅ | 文档存在, schema 兼容表格, 决策分歧说明 |

**达成率**: 4/5 = 80%

---

### ADR-006: Security Design (7 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 006-01 | 输入验证 | ✅ | validate_memory_entry, XSS/模板注入检测, 8 测试 |
| 006-02 | 路径验证 | ✅ | validate_safe_path, canonicalize, 防 `..` 逃逸 |
| 006-03 | 原子写入 | ✅ | atomic_write + sync_all + rename, 3 测试 |
| 006-04 | MemoryAccessContext | ✅ | for_session/for_project, can_read/can_write, 6 测试 |
| 006-05 | ThreatScanner (正则扫描) | ❌ | `security/scanner.rs` 不存在, 无 Regex-based 扫描 |
| 006-06 | FrozenSnapshot 安全验证 | ⚠️ | verify 方法存在但无警告日志, 无 SessionManager 集成 |
| 006-07 | 安全集成测试 | ❌ | `tests/security_integration.rs` 不存在 |

**达成率**: 4/7 = 57%

---

### ADR-007: HRR Introduction Timing (11 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 007-01 | HRRVector 定义 | ✅ | 384d, new/dimension/into_inner |
| 007-02 | bind (循环卷积) | ✅ | circular_convolution + 归一化 |
| 007-03 | unbind (逆卷积) | ✅ | circular_convolution_inverse |
| 007-04 | bundle (叠加) | ✅ | 加权叠加 + 归一化 |
| 007-05 | similarity (余弦) | ✅ | cosine [-1, 1], 4 测试 |
| 007-06 | HolographicStore | ✅ | O(√dim)×100 容量, store 方法 |
| 007-07 | probe (检索) | ✅ | 排序返回 Vec<(String, f32)> |
| 007-08 | reason (推理) | ✅ | premises/conclusions 相关性矩阵 |
| 007-09 | contradict (矛盾) | ✅ | similarity < -0.8 检测 |
| 007-10 | LRU 驱逐 | ✅ | evict_one 最低 importance 优先 |
| 007-11 | HybridMemoryProvider | ✅ | LanceDB 主存储 + HRR 补充, MemoryProvider impl |

**达成率**: 11/11 = 100% ✅ **唯一全达成的 ADR**

---

### ADR-008: Self-Learning Modules Independence (8 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 008-01 | Learning Module 结构 | ✅ | mod.rs 4 子模块 + 导出 |
| 008-02 | SelfModel 定义 | ✅ | capabilities/limitations/learned_patterns/performance |
| 008-03 | ReflectionEngine trait | ✅ | analyze_session/update_self_model/get_self_model |
| 008-04 | StandardReflectionEngine | ✅ | tool_sequences + outcomes + topics 分析 |
| 008-05 | Tool Sequence Analysis | ✅ | 工具对计数, ≥3 次触发反射 |
| 008-06 | TrustTracker | ✅ | +0.05/-0.10/0.0, clamp [-1,1] |
| 008-07 | 模块初始化 (LearningModule::new) | ✅ | 懒加载 self_model + trust_tracker + reflection_engine |
| 008-08 | Runtime 集成 (turn 结束时调用) | ⚠️ | `record_turn()` 被调用, 但 **无 `learning_enabled` 配置**, **`analyze_session()` 和 `update_self_model()` 未被调用**, 每 turn 新建实例 |

**达成率**: 7/8 = 88% (但 008-08 的运行时集成是核心 Gap)

---

### ADR-009: Trajectory Learning Timing (6 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 009-01 | Trajectory 数据结构 | ✅ | ShareGPT 格式, from_session, to_jsonl |
| 009-02 | TrajectoryManager | ✅ | record/export_all, base_path |
| 009-03 | 文件轮转 | ✅ | trajectory_YYYY-MM-DD.jsonl, 100MB rotate |
| 009-04 | 隐私控制 | ✅ | TrajectoryPrivacy, min_session_length=3 |
| 009-05 | Compressor | ✅ | 过滤低质量 + 截断超长 |
| 009-06 | Tauri Commands | ⚠️ | export_trajectories + get_trajectory_count 存在, **但未在 commands/mod.rs 导出为 IPC 命令** |

**达成率**: 5/6 = 83%

---

### ADR-010: Skills Hub Not Introduced (2 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 010-01 | 文档确认 | ⚠️ | ADR-010 文件存在但 ClawHub 安全事件引用未验证 |
| 010-02 | ToolRegistry 强化确认 | ⚠️ | ToolRegistry 注册/发现/执行完整, 但 **无 `register_from_dir()` 本地目录加载**, 无正式能力矩阵文档 |

**达成率**: 0/2 = 0% (但这是 "不做什么" 的确认性 TASK, 影响小)

---

### ADR-011: Agent Loop Harness Framework (7 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 011-01 | EventBus 核心 | ❌ | `src-tauri/src/modules/harness/` 目录不存在 |
| 011-02 | TelemetryCollector | ❌ | 无 telemetry.rs, 无 TokenStats/ToolStats |
| 011-03 | AgentLoopIntegration | ❌ | agent.rs 零 event_bus.emit 调用 |
| 011-04 | SessionRecorder | ❌ | 无 recorder.rs, 无 harness SQLite 表 |
| 011-05 | HarnessControl IPC | ❌ | 无 commands/harness.rs, 5 个 IPC 命令均不存在 |
| 011-06 | harness-cli 二进制 | ❌ | 无 harness-cli/ 目录, 不在 workspace |
| 011-07 | 集成测试 | ❌ | 无 harness_integration_test.rs |

**达成率**: 0/7 = 0% ❌

---

### ADR-012: Phase 6B Wiring (15 TASKs)

| TASK | 描述 | 状态 | 代码验证 |
|------|------|------|---------|
| 012-01 | AppState 扩展 (5 新字段) | ❌ | 只有 memory_provider + context_budget, **缺 trajectory_manager, learning_module, active_retrieval_manager** |
| 012-02 | ContextBudget 接入 Agent Loop | ✅ | ConversationRuntime 使用 context_budget, builder 方法 |
| 012-03 | ActiveRetrievalManager 接入 | ✅ | retrieve_memory_context() 在 LLM 前调用, 结果注入 system_prompt |
| 012-04 | VectorProvider dead_code 清理 | ❌ | `#![allow(dead_code)]` 仍在模块级 (line 11) |
| 012-05 | WorkingMemory 接入 Agent Loop | ❌ | **ConversationRuntime 无 working_memory 字段**, 无 builder, run_turn 使用全部 session.messages |
| 012-06 | TrajectoryManager 接入 Session | ✅ | record_trajectory_if_possible() 在 save_session 后调用 |
| 012-07 | LearningModule 初始化 + Hook | ⚠️ | record_turn 被调用但每 turn 新建实例, **ReflectionEngine 零调用** |
| 012-08 | FrozenSnapshot 接入 | ✅ | agent.rs capture + verify 完整 |
| 012-09 | WeibullDecay 接入 Compaction | ❌ | decay_factor 被计算但 **apply_importance_decay() 从未调用** |
| 012-10 | HRR 测试修复 + MockEmbedder | ❌ | `#[ignore]` 仍在, 无 MockEmbedder |
| 012-11 | Memory Provider 升级 (Vector 优先) | ✅ | create_memory_provider: Hybrid > Vector > SQLite, 30s timeout |
| 012-12 | Hybrid 配置化 | ✅ | IF2AI_HRR_ENABLED env var, 默认 false |
| 012-13 | Memory Browser UI | ❌ | 所有前端组件不存在 |
| 012-14 | Token Budget Settings UI | ⚠️ | 后端命令有, 前端组件无 |
| 012-15 | Trajectory Export UI | ⚠️ | 后端 export_trajectories 有, 前端无 |

**达成率**: 5/15 = 33%

---

## Part II: 综合 TASK 统计

### 按 ADR 汇总

| ADR | 标题 | ✅ | ⚠️ | ❌ | 总数 | 达成率 |
|-----|------|----|----|----|------|--------|
| 001 | SQLite P0 | 6 | 1 | 1 | 8 | 75% |
| 002 | Active Retrieval | 4 | 2 | 2 | 8 | 50% |
| 003 | FastEmbed+LanceDB | 2 | 4 | 2 | 8 | 25% |
| 004 | Token Budget | 6 | 1 | 1 | 8 | 75% |
| 005 | Upstream Claw | 4 | 1 | 0 | 5 | 80% |
| 006 | Security | 4 | 1 | 2 | 7 | 57% |
| 007 | HRR | **11** | 0 | 0 | **11** | **100%** ✅ |
| 008 | Self-Learning | 7 | 1 | 0 | 8 | 88% |
| 009 | Trajectory | 5 | 1 | 0 | 6 | 83% |
| 010 | Skills Hub | 0 | 2 | 0 | 2 | 0% |
| 011 | Harness | 0 | 0 | 7 | 7 | 0% ❌ |
| 012 | Wiring | 5 | 3 | 7 | 15 | 33% |
| **TOTAL** | | **54** | **17** | **22** | **93** | **58%** |

### 按 Phase 分组

| Phase | 相关 ADR | ✅ | ⚠️ | ❌ | 总 TASKs | 达成率 |
|-------|---------|----|----|----|----------|--------|
| **6B** | 001-009 | 49 | 12 | 6 | 67 | 73% |
| **6BW** | 012 | 5 | 3 | 7 | 15 | 33% |
| **6E** | 011 | 0 | 0 | 7 | 7 | 0% |
| **All** | 001-012 | **54** | **15** | **20** | **89** | **61%** |

> 注: ADR-010 不计入 Phase (确认性文档)。总 TASK 数 89 不含 ADR-010 的 2 个确认性 TASK。

---

## Part III: 严重 Gap 清单

### 🔴 Critical (影响核心功能)

| Gap ID | 对应 TASK | 描述 | 影响 |
|--------|----------|------|------|
| **C1** | 012-05 | **WorkingMemory 未接入 ConversationRuntime** | 40% 预算的 sliding window (8 turns) 完全无效, 上下文无限制增长 |
| **C2** | 012-01 | **AppState 缺 3 个关键字段** | trajectory/learning/retrieval 每 turn 重建, 状态不跨 turn 积累 |
| **C3** | 011-01~07 | **Phase 6E 完全不存在** | EventBus/Telemetry/Recorder/CLI 零实现, 运行时观测与控制框架缺失 |
| **C4** | 008-08 | **ReflectionEngine 零调用方** | 自学习核心组件不工作, SelfModel 不更新 |
| **C5** | 012-09 | **WeibullDecay 结果未应用** | `apply_importance_decay()` 从未调用, compaction 后语义记忆重要性不衰减 |

### 🟡 High (功能部分失效)

| Gap ID | 对应 TASK | 描述 | 影响 |
|--------|----------|------|------|
| **H1** | 002-P1-06 | SessionManager 无主动检索集成 | ActiveRetrieval 不在 SessionManager 层工作 |
| **H2** | 003-07 | 无 IVF-PQ 索引 | LanceDB 向量搜索无索引优化, 大数据量性能差 |
| **H3** | 003-08 | 无 SQLite↔LanceDB 双写 | VectorMemoryProvider 不向 SQLite 写, 数据不持久 |
| **H4** | 012-04 | VectorProvider dead_code 未清理 | 模块级 `#![allow(dead_code)]` 掩盖真实调用关系 |
| **H5** | 012-10 | HRR 测试仍 `#[ignore]` | 无 MockEmbedder, 代数推理零验证 |
| **H6** | 009-06 | Trajectory Tauri 命令未导出 | export_trajectories 存在但前端无法通过 IPC 调用 |

### 🟢 Medium (质量改进)

| Gap ID | 对应 TASK | 描述 |
|--------|----------|------|
| M1 | 001-08 | 无 JSON→SQLite 迁移工具 |
| M2 | 004-03 | Token 计数用 4:1 启发式, 无 tiktoken-rs |
| M3 | 004-08 | 无 YAML 配置加载 |
| M4 | 005-02 | Session schema 部分不兼容 claw-cli |
| M5 | 006-05 | ThreatScanner 模块不存在 |
| M6 | 006-06 | FrozenSnapshot 无警告日志 + SessionManager 集成 |
| M7 | 006-07 | 安全集成测试缺失 |

---

## Part IV: 前端审计

| 要求 | 对应 TASK | 状态 |
|------|----------|------|
| MemoryBrowser 组件 | 012-13 | ❌ 不存在 |
| MemoryCard 组件 | 012-13 | ❌ 不存在 |
| MemoryCategoryNav 组件 | 012-13 | ❌ 不存在 |
| /memory 路由 | 012-13 | ❌ 不存在 |
| MemorySettings 组件 | 012-14 | ❌ 不存在 |
| get_memory_config / set_memory_config | 012-14 | ✅ 后端存在 |
| Export Trajectories 按钮 | 012-15 | ❌ 前端不存在 |
| export_trajectories 命令 | 012-15 | ✅ 后端存在 |

**前端达成率**: 2/9 = 22%

---

## Part V: 质量指标

| 指标 | 目标 | 实际 | 差距 |
|------|------|------|------|
| Backlog TASK 达成率 | 100% | **61%** (54/89) | -39% |
| 测试通过率 | 100% | 573/581 (98.6%) | 8 ignored |
| Clippy clean | ✅ | ✅ | 无差距 |
| 模块接入率 | 100% | ~55% | -45% |
| 前端覆盖率 (6BW) | 100% | ~22% | -78% |
| Harness 覆盖率 (6E) | 100% | 0% | -100% |

---

## Part VI: 推荐行动计划

### Phase 1: 修复 Phase 6BW Critical Gaps (约 300 行)

| 步骤 | Gap | 行动 | 预估行数 |
|------|-----|------|---------|
| 1 | C1 (012-05) | ConversationRuntime 添加 `working_memory: Option<WorkingMemory>` 字段 + builder + run_turn 使用 | ~80 |
| 2 | C2 (012-01) | AppState 添加 3 字段 + main.rs 初始化 + AppState::new() 签名 | ~50 |
| 3 | C4 (008-08) | agent.rs turn 后调用 `reflect_on_session()` + `update_self_model()` | ~30 |
| 4 | C5 (012-09) | 在 compaction 后调用 `memory_provider.apply_importance_decay()` | ~20 |
| 5 | H4 (012-04) | 模块级 `#![allow(dead_code)]` → 函数级 | ~10 |
| 6 | H5 (012-10) | 实现 MockEmbedder + 移除 #[ignore] | ~50 |
| 7 | H6 (009-06) | commands/mod.rs 导出 trajectory 命令 | ~5 |
| 8 | M6 (006-06) | FrozenSnapshot verify 失败时加 warning 日志 | ~5 |

### Phase 2: 修复 Phase 6BW Remaining (约 200 行)

| 步骤 | Gap | 行动 |
|------|-----|------|
| 9 | H1 (002-P1-06) | SessionManager 添加 active_retrieval 字段 |
| 10 | H2 (003-07) | LanceDB 添加 IVF-PQ 索引配置 |
| 11 | H3 (003-08) | VectorMemoryProvider 添加 SQLite 双写 |
| 12 | M1 (001-08) | migrate_from_json() 工具 |
| 13 | M2 (004-03) | 添加 tiktoken-rs 依赖 + estimate_messages_tokens() |
| 14 | M5 (006-05) | ThreatScanner 模块 (Regex-based 扫描) |

### Phase 3: 启动 Phase 6E (约 1500 行, 7 slices)

| Slice | 标题 | 预估行数 | 依赖 |
|-------|------|---------|------|
| 6e.1 | EventBus Core | ~200 | 无 |
| 6e.2 | TelemetryCollector | ~200 | 6e.1 |
| 6e.3 | AgentLoopIntegration | ~50 | 6e.1 |
| 6e.4 | SessionRecorder | ~300 | 6e.1 |
| 6e.5 | HarnessControl IPC | ~150 | 6e.2, 6e.4 |
| 6e.6 | harness-cli | ~400 | 6e.5 |
| 6e.7 | Integration Tests | ~200 | 6e.1-06 |

### Phase 4: 前端 (约 500 行)

| 要求 | 预估行数 |
|------|---------|
| MemoryBrowser + MemoryCard + CategoryNav | ~250 |
| MemorySettings (Token Budget + Trajectory Export) | ~150 |
| /memory 路由 + App.tsx 集成 | ~50 |
| trust_score 颜色渐变 + importance 进度条 | ~50 |

---

## 结论

**Phase 6B**: 作为能力模块库已基本完成 (73%)。ADR-007 (HRR) 是唯一 100% 达成的 ADR。主要缺失集中在 ADR-003 (Vector 搜索, 25%) 和 ADR-002 (主动检索, 50%)。

**Phase 6BW**: 声明 9/9 done 但实际仅 33% (5/15)。6 个 Critical/High gaps 需要优先修复, 特别是 **WorkingMemory 未接入** 和 **AppState 字段缺失** 导致 Phase 6B 的大部分高级功能不生效。

**Phase 6E**: 0% 达成, 7 slices 全部需要从零实现。

**关键路径**: Phase 1 (修复 6BW Critical, ~300 行) → Phase 3 (Phase 6E 从零, ~1500 行) → Phase 4 (前端, ~500 行) → Phase 2 (其余改进, ~200 行)。

> 整体完成度: **61%** (54/89 TASKs done)
