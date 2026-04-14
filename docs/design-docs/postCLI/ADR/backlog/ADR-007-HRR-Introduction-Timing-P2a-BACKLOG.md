# ADR-007 HRR Introduction Timing (P2a) — 详细实施 backlog

## 概述

**目标**: 在 `src-tauri/src/modules/memory/hrr/` 中实现 HRR 代数推理模块 (NEW SUBMODULE)
**ADR**: [ADR-007](./ADR-007-HRR-Introduction-Timing-P2a.md)
**依赖**: ADR-001 (SQLite P0), ADR-003 (LanceDB) — 先执行这两个
**优先级**: P2a
**预估工时**: 6-8 天
**现状**: `src-tauri/src/modules/memory/hrr/` 目录不存在，需要创建

---

## 子任务清单

### TASK-007-01: HRRVector 定义与操作

**目标**: 实现 HRR 向量基本操作

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/memory/hrr/mod.rs`
- [ ] 定义 `HRRVector` 结构体 (wraps `Vec<f32>`)
- [ ] 实现 `HRRVector::new(dimension: usize) -> Self`
- [ ] 实现 `HRRVector::from_text(text: &str, embedder: &FastEmbedProvider) -> Result<Self, HRRError>`
- [ ] 实现 `dimension(&self) -> usize`
- [ ] 实现 `into_inner(self) -> Vec<f32>`
- [ ] 实现 `Clone`, `Debug`, `Serialize`, `Deserialize`

**验收标准**:
- [ ] HRRVector 正确包装 384d 向量
- [ ] 可从文本创建 HRRVector
- [ ] Serde trait 正确实现

---

### TASK-007-02: bind (Circular Convolution)

**目标**: 实现 HRR 绑定操作

**具体任务**:
- [ ] 在 `src-tauri/src/modules/memory/hrr/operations.rs` 实现
- [ ] 实现 `pub fn bind(key: &HRRVector, value: &HRRVector) -> HRRVector`
  - 循环卷积: `result[i] = Σ a[j] * b[(i-j) mod dim]`
  - 归一化到单位长度
- [ ] 实现辅助函数 `fn circular_convolution(a: &[f32], b: &[f32]) -> Vec<f32>`
- [ ] 编写测试验证绑定正确性

**验收标准**:
- [ ] bind 操作返回归一化向量
- [ ] bind(unbind(bind(a,b), a)) ≈ b (近似相等)
- [ ] 测试覆盖

**测试标准**:
```rust
#[test]
fn bind_unbind_recovers_original() {
    let key = HRRVector::random(384);
    let value = HRRVector::random(384);

    let bound = bind(&key, &value);
    let recovered = unbind(&bound, &key);

    // Check cosine similarity > 0.95
    let sim = similarity(&recovered, &value);
    assert!(sim > 0.95);
}
```

---

### TASK-007-03: unbind (Inverse Convolution)

**目标**: 实现 HRR 解绑操作

**具体任务**:
- [ ] 实现 `pub fn unbind(composite: &HRRVector, key: &HRRVector) -> HRRVector`
  - 逆卷积 = 共轭 = 反转相位
  - 对于实数 HRR: `key_inv[j] = key[(dim - j) mod dim]`
- [ ] 实现辅助函数 `fn circular_convolution_inverse(a: &[f32], key: &[f32]) -> Vec<f32>`
- [ ] 编写测试

**验收标准**:
- [ ] unbind 能正确恢复 bind 前的值
- [ ] 测试覆盖边界情况（零向量等）

---

### TASK-007-04: bundle (Superposition)

**目标**: 实现 HRR 绑定操作

**具体任务**:
- [ ] 实现 `pub fn bundle(vectors: &[HRRVector], weights: Option<&[f32]>) -> HRRVector`
  - 加权求和
  - 归一化到单位长度
- [ ] 默认权重为 1.0
- [ ] 编写测试验证 bundle 的可组合性

**验收标准**:
- [ ] bundle(bind(a,x), bind(a,y)) ≈ bind(a, bundle(x,y))
- [ ] 支持权重
- [ ] 归一化正确

---

### TASK-007-05: similarity (Cosine)

**目标**: 实现 HRR 相似度计算

**具体任务**:
- [ ] 实现 `pub fn similarity(a: &HRRVector, b: &HRRVector) -> f32`
  - Cosine similarity: `dot(a,b) / (||a|| * ||b||)`
- [ ] 返回范围 [-1.0, 1.0]
- [ ] 编写测试

**验收标准**:
- [ ] 相同向量相似度 = 1.0
- [ ] 正交向量相似度 ≈ 0.0
- [ ] 相反向量相似度 = -1.0

---

### TASK-007-06: HolographicStore 实现

**目标**: 实现 HRR 存储

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/memory/hrr/store.rs`
- [ ] 定义 `HolographicStore` 结构体:
  ```rust
  pub struct HolographicStore {
      vectors: RwLock<HashMap<String, HRRVector>>,
      dimension: usize,
      max_capacity: usize,  // O(√dim) ≈ 700 for 384d
  }
  ```
- [ ] 实现 `new(dimension: usize) -> Self`
- [ ] 实现 `store(&self, key: &str, value: &HRRVector) -> Result<(), HRRError>`
- [ ] 实现容量检查
- [ ] 编写测试

**验收标准**:
- [ ] 容量限制为 O(√dim)
- [ ] 存储和检索正确
- [ ] 容量超限返回错误

---

### TASK-007-07: probe (相似检索)

**目标**: 实现 HRR 探查操作

**具体任务**:
- [ ] 在 `HolographicStore` 实现 `pub async fn probe(&self, query: &HRRVector) -> Vec<(String, f32)>`
  - 计算 query 与所有存储向量的相似度
  - 按相似度降序排列
  - 返回 top-k
- [ ] 编写测试

**验收标准**:
- [ ] 返回正确排序的结果
- [ ] 性能可接受（< 100ms for 700 items）

---

### TASK-007-08: reason (关系推理)

**目标**: 实现 HRR 推理操作

**具体任务**:
- [ ] 实现 `pub async fn reason(&self, premises: &[HRRVector], conclusions: &[HRRVector]) -> Vec<(usize, usize, f32)>`
  - 检查 premise-conclusion 对的关联度
  - premise.bind(conclusion) ≈ identity → 相关
  - 返回 (premise_idx, conclusion_idx, relatedness)
- [ ] 编写测试

**验收标准**:
- [ ] 正确识别相关 premise-conclusion 对
- [ ] 阈值可配置

---

### TASK-007-09: contradict (矛盾检测)

**目标**: 实现矛盾检测

**具体任务**:
- [ ] 实现 `pub async fn contradict(&self, a: &HRRVector, b: &HRRVector) -> bool`
  - similarity(a, b) < -0.8 → 矛盾
- [ ] 编写测试

**验收标准**:
- [ ] 正确检测明显矛盾的向量
- [ ] 正确排除不矛盾的向量

---

### TASK-007-10: 容量驱逐策略

**目标**: 实现 LRU 驱逐

**具体任务**:
- [ ] 实现 `store_with_eviction(&self, key: &str, value: &HRRVector, importance: f32)`
  - 容量超限时驱逐最低重要性的条目
- [ ] 在 `HolographicStore` 添加 `importance` 字段
- [ ] 编写测试

**验收标准**:
- [ ] 容量超限时正确驱逐
- [ ] 高重要性条目优先保留

---

### TASK-007-11: HRR 与 LanceDB 集成

**目标**: 实现双存储架构

**具体任务**:
- [ ] 创建 `HybridMemoryProvider` 同时管理 LanceDB 和 HRR
- [ ] 实现 `MemoryProvider` trait:
  - `store` → 写入 LanceDB + HRR（可选）
  - `recall` → 从 LanceDB 检索（主要）+ HRR（代数推理）
- [ ] 配置开关: `hrr_enabled: bool`
- [ ] 编写集成测试

**验收标准**:
- [ ] LanceDB 作为主存储
- [ ] HRR 作为代数推理补充
- [ ] 可通过配置切换 HRR 开/关

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-007-01 | HRRVector 基础 |
| P0 | TASK-007-02 | bind 操作 |
| P0 | TASK-007-03 | unbind 操作 |
| P0 | TASK-007-04 | bundle 操作 |
| P0 | TASK-007-05 | similarity |
| P1 | TASK-007-06 | Store 结构 |
| P1 | TASK-007-07 | probe |
| P1 | TASK-007-08 | reason |
| P1 | TASK-007-09 | contradict |
| P2 | TASK-007-10 | 驱逐策略 |
| P2 | TASK-007-11 | 集成 |

---

## 依赖关系

```
TASK-007-01 ──┬── TASK-007-02 ──┬── TASK-007-03 ──┬── TASK-007-04
               │                │                │
               └────────────────┴────────────────┴── TASK-007-05
                                                         │
TASK-007-06 ── TASK-007-07 ── TASK-007-08 ── TASK-007-09
     │                                                      
     └────────────────── TASK-007-10 ── TASK-007-11
```

---

## 验收总览

- [ ] HRRVector 正确实现
- [ ] bind/unbind 正确配对
- [ ] bundle 正确叠加
- [ ] similarity 返回 [-1, 1]
- [ ] HolographicStore 容量为 O(√dim)
- [ ] probe 返回 top-k 相似向量
- [ ] reason 正确推理关系
- [ ] contradict 检测矛盾
- [ ] LRU 驱逐正确工作
- [ ] HybridMemoryProvider 正确集成
