# ADR-004 Token Budget Allocation — 详细实施 backlog

## 概述

**目标**: 实现上下文预算分配和槽位管理
**ADR**: [ADR-004](./ADR-004-Token-Budget-Allocation.md)
**优先级**: P0
**预估工时**: 2-3 天

---

## 子任务清单

### TASK-004-01: ContextBudget 结构体

**目标**: 定义上下文预算配置结构

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/runtime/budget.rs`
- [ ] 定义 `ContextBudget` 结构体:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct ContextBudget {
      /// 总预算 (默认 4000)
      pub total: usize,
      /// System 百分比 (默认 10%)
      pub system_pct: f32,
      /// Episodic 百分比 (默认 20%)
      pub episodic_pct: f32,
      /// Semantic 百分比 (默认 30%)
      pub semantic_pct: f32,
      /// Working 百分比 (默认 40%)
      pub working_pct: f32,
  }
  ```
- [ ] 实现 `Default` for `ContextBudget` (标准 10/20/30/40)
- [ ] 实现 `validate(&self) -> Result<(), BudgetError>` 验证百分比和为 1.0
- [ ] 实现辅助方法: `system_tokens()`, `episodic_tokens()`, `semantic_tokens()`, `working_tokens()`
- [ ] 编写单元测试

**验收标准**:
- [ ] 默认值正确 (4000 tokens, 10/20/30/40%)
- [ ] `validate()` 正确检测无效百分比
- [ ] 每个 slot 的 token 数计算正确

**测试标准**:
```rust
#[test]
fn default_budget() {
    let budget = ContextBudget::default();
    assert_eq!(budget.total, 4000);
    assert!((budget.system_pct - 0.10).abs() < 0.001);
    assert!((budget.episodic_pct - 0.20).abs() < 0.001);
    assert!((budget.semantic_pct - 0.30).abs() < 0.001);
    assert!((budget.working_pct - 0.40).abs() < 0.001);
}

#[test]
fn validate_rejects_invalid_percentages() {
    let budget = ContextBudget { total: 4000, system_pct: 0.5, episodic_pct: 0.5, semantic_pct: 0.5, working_pct: 0.5 };
    assert!(budget.validate().is_err());
}
```

---

### TASK-004-02: ContextSlots 结构体

**目标**: 实现槽位使用跟踪

**具体任务**:
- [ ] 定义 `ContextSlots` 结构体:
  ```rust
  pub struct ContextSlots {
      pub system: Slot,
      pub episodic: Slot,
      pub semantic: Slot,
      pub working: Slot,
  }

  pub struct Slot {
      pub budget: usize,
      pub used: usize,
      pub entries: Vec<MemoryEntry>,
  }
  ```
- [ ] 实现 `ContextSlots::new(budget: ContextBudget) -> Self`
- [ ] 实现 `available(&self, slot_name: &str) -> usize`
- [ ] 实现 `total_available(&self) -> usize`
- [ ] 实现 `allocate(&mut self, slot_name: &str, tokens: usize, entry: MemoryEntry) -> Result<(), BudgetError>`
- [ ] 实现 `evict(&mut self, slot_name: &str, count: usize) -> Vec<MemoryEntry>`
- [ ] 编写测试

**验收标准**:
- [ ] `new()` 正确初始化所有槽位
- [ ] `available()` 返回正确的剩余 token 数
- [ ] 分配超过预算返回错误
- [ ] 驱逐正确移除最少重要的条目

**测试标准**:
```rust
#[test]
fn slot_available() {
    let budget = ContextBudget::default();
    let slots = ContextSlots::new(budget);
    assert_eq!(slots.available("system"), 400);  // 4000 * 0.10
    assert_eq!(slots.available("episodic"), 800); // 4000 * 0.20
}

#[test]
fn allocate_respects_budget() {
    let budget = ContextBudget::default();
    let mut slots = ContextSlots::new(budget);

    // 尝试分配超过预算
    let entry = create_test_entry(500);  // 500 tokens
    let result = slots.allocate("system", 500, entry);
    assert!(result.is_ok());

    // 再次分配 1 token 会失败（只剩 0 可用）
    let entry2 = create_test_entry(1);
    let result2 = slots.allocate("system", 1, entry2);
    assert!(result2.is_err());
}
```

---

### TASK-004-03: Token 计数实现

**目标**: 实现准确的 token 计数

**具体任务**:
- [ ] 引入 `tiktoken-rs` 依赖
- [ ] 实现 `fn estimate_tokens(text: &str) -> usize`
  - 使用 `cl100k_base` 编码器
  - 缓存编码器实例
- [ ] 实现 `fn estimate_entry_tokens(entry: &MemoryEntry) -> usize`
- [ ] 实现 `fn estimate_messages_tokens(messages: &[Message]) -> usize`
- [ ] 编写测试（使用已知 token 数的样本）

**验收标准**:
- [ ] token 计数与 TikToken 一致
- [ ] 编码器正确缓存
- [ ] 性能可接受（< 1ms/调用）

---

### TASK-004-04: WorkingMemory 滑动窗口

**目标**: 实现工作记忆的滑动窗口

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/memory/working_memory.rs`
- [ ] 定义 `WorkingMemory` 结构体:
  ```rust
  pub struct WorkingMemory {
      pub turns: Vec<ConversationTurn>,
      pub max_turns: usize,   // 默认 8
      pub max_tokens: usize, // 默认 1600
  }
  ```
- [ ] 实现 `fn push(&mut self, turn: ConversationTurn)`
  - 添加新 turn
  - 超过 `max_turns` 时移除最旧的
  - 超过 `max_tokens` 时移除最旧的
- [ ] 实现 `fn token_count(&self) -> usize`
- [ ] 实现 `fn iter(&self) -> Iter<ConversationTurn>`
- [ ] 编写测试

**验收标准**:
- [ ] `max_turns` 限制正确
- [ ] `max_tokens` 限制正确
- [ ] 迭代器正确遍历

**测试标准**:
```rust
#[test]
fn sliding_window_respects_max_turns() {
    let mut wm = WorkingMemory::new(3, 10000);  // max_turns = 3

    for i in 0..5 {
        wm.push(ConversationTurn { content: format!("turn {}", i), tokens: 10 });
    }

    assert_eq!(wm.turns.len(), 3);
    assert_eq!(wm.turns[0].content, "turn 2");
    assert_eq!(wm.turns[2].content, "turn 4");
}

#[test]
fn sliding_window_respects_max_tokens() {
    let mut wm = WorkingMemory::new(10, 30);  // max_tokens = 30

    for i in 0..5 {
        wm.push(ConversationTurn { content: format!("turn {}", i), tokens: 10 });
    }

    // 5 * 10 = 50 > 30，应该只剩 3 个
    assert!(wm.turns.len() <= 3);
}
```

---

### TASK-004-05: FrozenSnapshot 实现

**目标**: 实现系统提示快照

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/runtime/snapshot.rs`
- [ ] 定义 `FrozenSnapshot` 结构体:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct FrozenSnapshot {
      pub prompt: String,
      pub prompt_hash: String,
      pub created_at: DateTime<Utc>,
      pub version: String,
  }
  ```
- [ ] 实现 `FrozenSnapshot::capture(prompt: &str) -> Self`
  - 计算 prompt 的 SHA256 哈希
  - 记录创建时间和版本
- [ ] 实现 `FrozenSnapshot::verify(&self, current_prompt: &str) -> bool`
- [ ] 实现 `fn compute_hash(s: &str) -> String`
- [ ] 编写测试

**验收标准**:
- [ ] 快照正确捕获 prompt
- [ ] 哈希计算确定性（相同输入 → 相同哈希）
- [ ] 验证正确检测变更

---

### TASK-004-06: EpisodicCompaction 实现

**目标**: 实现情景记忆压缩

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/runtime/compaction.rs`
- [ ] 实现 `async fn compact_episodic_if_needed(session: &mut Session, budget_tokens: usize) -> Result<bool>`
  - 检查当前使用量是否超过预算
  - 按重要性排序条目
  - 保留 top-N 条目
  - 对被移除的条目生成 LLM 摘要
  - 插入摘要作为新条目
- [ ] 实现 `fn calculate_importance(entry: &MemoryEntry, now: DateTime<Utc>) -> f32`
  - 使用 Weibull 衰减
  - 结合 access_count 和 trust_score
- [ ] 编写测试（mock LLM 调用）

**验收标准**:
- [ ] 压缩在超过预算时触发
- [ ] 摘要正确生成
- [ ] 压缩后总 token 数 < 预算

---

### TASK-004-07: Weibull 衰减实现

**目标**: 实现记忆重要性衰减

**具体任务**:
- [ ] 实现 `pub fn weibull_decay(age_hours: f32, initial_importance: f32) -> f32`
  - lambda = 24 * 7 (7 天尺度)
  - k = 1.2 (形状参数)
- [ ] 实现 `pub fn compute_memory_importance(entry: &MemoryEntry) -> f32`
  - base = importance + access_count * 0.01
  - trust_boost = (trust_score + 1.0) * 0.05
  - decay = weibull_decay(age_hours, 1.0)
  - final = (base + trust_boost) * decay
- [ ] 编写测试验证衰减曲线

**验收标准**:
- [ ] 衰减符合 Weibull 分布
- [ ] 7 天后重要性显著下降
- [ ] 高 trust_score 提升重要性

---

### TASK-004-08: 配置加载与验证

**目标**: 支持配置文件覆盖

**具体任务**:
- [ ] 定义 `BudgetConfig` 结构体支持 YAML 配置
- [ ] 实现 `impl From<&ConfigFile> for ContextBudget`
- [ ] 实现验证：百分比和为 1.0
- [ ] 实现默认值回退
- [ ] 编写集成测试

**验收标准**:
- [ ] 配置文件正确解析
- [ ] 无效配置报错
- [ ] 默认值正确回退

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-004-01 | 基础结构 |
| P0 | TASK-004-02 | 槽位管理核心 |
| P0 | TASK-004-03 | Token 计数基础 |
| P0 | TASK-004-04 | Working Memory 核心 |
| P1 | TASK-004-05 | Frozen Snapshot |
| P1 | TASK-004-06 | 压缩逻辑 |
| P1 | TASK-004-07 | Weibull 衰减 |
| P2 | TASK-004-08 | 配置加载 |

---

## 验收总览

- [ ] ContextBudget 默认值正确 (4000, 10/20/30/40%)
- [ ] ContextSlots 正确跟踪使用量
- [ ] Token 计数准确
- [ ] WorkingMemory 滑动窗口正确
- [ ] FrozenSnapshot 正确捕获和验证
- [ ] Episodic 压缩正确触发
- [ ] Weibull 衰减曲线正确
- [ ] 配置可从文件加载
