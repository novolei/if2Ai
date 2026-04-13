# ADR-008 Self-Learning Modules Independence — 详细实施 backlog

## 概述

**目标**: 实现与记忆存储分离的自学习模块
**ADR**: [ADR-008](./ADR-008-Self-Learning-Modules-Independence.md)
**优先级**: P2b
**预估工时**: 5-6 天

---

## 子任务清单

### TASK-008-01: Learning 模块结构

**目标**: 创建独立的学习模块

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/learning/` 目录
- [ ] 创建 `src-tauri/src/modules/learning/mod.rs`
- [ ] 定义模块结构:
  ```rust
  pub mod self_model;
  pub mod reflection;
  pub mod trust_tracker;
  pub mod trajectory;

  pub use self_model::{SelfModel, Capability, LearnedPattern, PerformanceMetrics};
  pub use reflection::{ReflectionEngine, Reflection};
  pub use trust_tracker::{TrustTracker, TrustFeedback};
  pub use trajectory::{TrajectoryManager, Trajectory, TurnMetadata};
  ```
- [ ] 实现 `LearningModule` 入口结构
- [ ] 实现 `init_learning(memory: Arc<dyn MemoryProvider>) -> Result<LearningModule>`

**验收标准**:
- [ ] 模块结构清晰
- [ ] 子模块正确导出
- [ ] 可独立初始化

---

### TASK-008-02: SelfModel 定义

**目标**: 定义自我模型结构

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/learning/self_model.rs`
- [ ] 定义核心结构:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct SelfModel {
      pub capabilities: Vec<Capability>,
      pub limitations: Vec<Limitation>,
      pub learned_patterns: Vec<LearnedPattern>,
      pub performance: PerformanceMetrics,
      pub updated_at: DateTime<Utc>,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct Capability {
      pub id: String,
      pub name: String,
      pub description: String,
      pub confidence: f32,  // 0.0-1.0
      pub last_used: DateTime<Utc>,
      pub use_count: u32,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct LearnedPattern {
      pub id: String,
      pub trigger: String,  // Context pattern
      pub action: String,  // What was done
      pub success_rate: f32,
      pub sample_count: u32,
      pub last_applied: DateTime<Utc>,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct PerformanceMetrics {
      pub total_turns: u64,
      pub successful_turns: u64,
      pub failed_turns: u64,
      pub average_response_time_ms: f64,
      pub tool_usage_stats: HashMap<String, u32>,
  }
  ```
- [ ] 实现 `Default` for `SelfModel`
- [ ] 编写测试

**验收标准**:
- [ ] 结构体定义完整
- [ ] Serde 正确实现
- [ ] Default 正确

---

### TASK-008-03: ReflectionEngine Trait

**目标**: 定义反思引擎接口

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/learning/reflection.rs`
- [ ] 定义 `ReflectionEngine` trait:
  ```rust
  #[async_trait]
  pub trait ReflectionEngine: Send + Sync {
      async fn analyze_session(&self, session: &Session) -> Result<Vec<Reflection>>;
      async fn update_self_model(&self, reflections: Vec<Reflection>) -> Result<()>;
      async fn get_self_model(&self) -> Result<SelfModel>;
  }

  pub struct Reflection {
      pub pattern: String,
      pub insight: String,
      pub confidence: f32,
      pub source_session: String,
      pub timestamp: DateTime<Utc>,
  }
  ```
- [ ] 编写测试

**验收标准**:
- [ ] Trait 定义完整
- [ ] Async trait 正确编译

---

### TASK-008-04: StandardReflectionEngine

**目标**: 实现标准反思引擎

**具体任务**:
- [ ] 实现 `StandardReflectionEngine`:
  ```rust
  pub struct StandardReflectionEngine {
      self_model: RwLock<SelfModel>,
      memory: Arc<dyn MemoryProvider>,
  }
  ```
- [ ] 实现 `analyze_session()`:
  - 分析工具序列模式
  - 分析成功/失败模式
  - 分析话题聚类
- [ ] 实现 `update_self_model()`:
  - 更新 learned_patterns
  - 更新 performance metrics
- [ ] 实现 `get_self_model()`
- [ ] 编写测试

**验收标准**:
- [ ] 工具序列分析正确
- [ ] 模式更新正确
- [ ] 线程安全

---

### TASK-008-05: 工具序列分析

**目标**: 实现工具使用模式分析

**具体任务**:
- [ ] 实现 `fn analyze_tool_sequences(&self, session: &Session) -> Result<Vec<Reflection>>`
  - 统计工具对 (tool_A → tool_B) 出现频率
  - 频率 ≥ 3 的作为反射
- [ ] 编写测试

**验收标准**:
- [ ] 正确统计工具对
- [ ] 正确生成 Reflection

---

### TASK-008-06: TrustTracker 实现

**目标**: 实现信任追踪

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/learning/trust_tracker.rs`
- [ ] 定义 `TrustFeedback` 枚举:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum TrustFeedback {
      Helpful,
      Unhelpful,
      Neutral,
  }
  ```
- [ ] 定义 `TrustTracker`:
  ```rust
  pub struct TrustTracker {
      scores: RwLock<HashMap<String, f32>>,
  }
  ```
- [ ] 实现 `adjust(&self, key: &str, feedback: TrustFeedback)`
  - Helpful: +0.05
  - Unhelpful: -0.10
  - Neutral: +0.0
  - Clamp to [-1.0, 1.0]
- [ ] 实现 `get(&self, key: &str) -> f32`
- [ ] 编写测试

**验收标准**:
- [ ] Helpful 反馈增加分数
- [ ] Unhelpful 反馈减少分数
- [ ] 分数被 clamp 到 [-1, 1]

---

### TASK-008-07: 自学习模块初始化

**目标**: 实现学习模块初始化

**具体任务**:
- [ ] 实现 `LearningModule::new(memory: Arc<dyn MemoryProvider>) -> Self`
- [ ] 实现懒加载自我模型
- [ ] 实现模块序列化/反序列化
- [ ] 实现自我模型持久化（JSON 文件）
- [ ] 编写集成测试

**验收标准**:
- [ ] 模块正确初始化
- [ ] 自我模型可持久化
- [ ] 集成测试通过

---

### TASK-008-08: 与 Runtime 集成

**目标**: 将自学习集成到运行时

**具体任务**:
- [ ] 在 `Runtime` 中添加 `learning: Option<Arc<LearningModule>>`
- [ ] 在每次 turn 结束时调用 `reflection_engine.analyze_session()`
- [ ] 实现开关配置 `learning_enabled: bool`
- [ ] 编写集成测试

**验收标准**:
- [ ] Turn 结束时触发反思
- [ ] 可通过配置禁用
- [ ] 不影响正常功能

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-008-01 | 模块结构 |
| P0 | TASK-008-02 | SelfModel 定义 |
| P0 | TASK-008-03 | ReflectionEngine trait |
| P0 | TASK-008-04 | StandardReflectionEngine |
| P1 | TASK-008-05 | 工具序列分析 |
| P0 | TASK-008-06 | TrustTracker |
| P1 | TASK-008-07 | 模块初始化 |
| P2 | TASK-008-08 | Runtime 集成 |

---

## 验收总览

- [ ] Learning 模块结构清晰
- [ ] SelfModel 完整定义
- [ ] ReflectionEngine trait 正确
- [ ] StandardReflectionEngine 实现完整
- [ ] 工具序列分析正确
- [ ] TrustTracker 正确追踪
- [ ] 模块可初始化和持久化
- [ ] Runtime 正确集成
