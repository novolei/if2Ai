# ADR-002 Active Retrieval vs Passive Invocation — 详细实施 backlog

## 概述

**目标**: 实现 P0 被动调用 + P1 主动检索双模式记忆系统
**ADR**: [ADR-002](./ADR-002-Active-Retrieval-vs-Passive-Invocation.md)
**优先级**: P0/P1
**预估工时**: 2-3 天 (P0) + 3-4 天 (P1)

**现状**:
- ✅ MemoryProvider trait 存在 (5方法)
- ✅ 被动调用已实现 (LLM显式调用 memory.recall)
- ❌ 主动检索不存在 (需要新建)
- ❌ QueryIntent/Intent分类器不存在 (需要新建)
- ❌ RRF Fusion 不存在 (需要新建)

---

## P0 阶段：被动调用模式

### TASK-002-P0-01: 扩展 MemoryProvider Trait

**目标**: 在 trait 中添加支持被动调用的方法

**具体任务**:
- [ ] 在 `MemoryProvider` trait 中确认现有方法签名:
  - `store(key, content, category)` — 存储
  - `recall(query, category, limit)` — 召回
  - `delete(key)` — 删除
  - `purge_category(category)` — 清除分类
  - `export(category)` — 导出
- [ ] 现有 `recall` 签名无需修改，支持被动调用
- [ ] 添加 `#[async_trait]` 属性确保 async trait 正确编译

**验收标准**:
- [ ] Trait 定义清晰，文档注释完整
- [ ] `recall` 支持 `query=""` 返回全部（或 category 过滤）

---

### TASK-002-P0-02: 验证被动调用流程

**目标**: 确认 LLM 可以显式调用 `memory.recall()`

**具体任务**:
- [ ] 检查现有代码中 LLM 如何调用记忆工具
- [ ] 确认工具 schema 中 `memory.recall` 正确暴露给 LLM
- [ ] 编写集成测试模拟 LLM 调用 `memory.recall`
- [ ] 验证工具返回格式符合 LLM 输入要求

**验收标准**:
- [ ] LLM 可通过工具调用 `memory.recall`
- [ ] 返回格式为 `Vec<MemoryEntry>` 的 JSON 序列化
- [ ] 测试覆盖各类查询场景

---

## P1 阶段：主动检索模式

### TASK-002-P1-01: QueryIntent 枚举定义

**目标**: 定义查询意图分类

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/memory/intent.rs`
- [ ] 定义 `QueryIntent` 枚举:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum QueryIntent {
      /// 代码相关: semantic=0.5, episodic=0.3, working=0.2
      Code,
      /// 任务相关: semantic=0.4, episodic=0.4, working=0.2
      Task,
      /// 事实相关: semantic=0.6, episodic=0.2, working=0.2
      Fact,
      /// 人物相关: semantic=0.3, episodic=0.5, working=0.2
      Person,
      /// 一般查询: semantic=0.3, episodic=0.3, working=0.4
      General,
  }
  ```
- [ ] 实现 `QueryIntent::retrieval_weights(&self) -> (f32, f32, f32)`
- [ ] 编写单元测试验证权重计算

**验收标准**:
- [ ] 5 种意图全部定义
- [ ] 每种意图返回正确的三元组权重
- [ ] 测试覆盖所有意图

**测试标准**:
```rust
#[test]
fn code_intent_weights() {
    let (sem, epi, work) = QueryIntent::Code.retrieval_weights();
    assert!((sem - 0.5).abs() < 0.001);
    assert!((epi - 0.3).abs() < 0.001);
    assert!((work - 0.2).abs() < 0.001);
}
```

---

### TASK-002-P1-02: IntentClassification 实现

**目标**: 实现基于规则的意图分类器

**具体任务**:
- [ ] 实现 `fn classify_intent(query: &str) -> QueryIntent`
  - 规则引擎，关键词匹配
  - `fix`/`bug`/`how do i` → Code
  - `remember`/`did we` → Task
  - `who`/`person` → Person
  - `what is`/`fact` → Fact
  - 默认 → General
- [ ] 考虑大小写不敏感
- [ ] 考虑多关键词组合
- [ ] 编写测试覆盖各种模式

**验收标准**:
- [ ] 分类准确率在测试集上 > 90%
- [ ] 测试覆盖每种意图至少 5 个样本
- [ ] 未匹配到特定意图时返回 `General`

**测试标准**:
```rust
#[test]
fn classify_code_queries() {
    let queries = vec![
        "How do I fix the memory leak?",
        "There's a bug in the session manager",
        "fix: null pointer exception",
    ];
    for q in queries {
        assert_eq!(classify_intent(q), QueryIntent::Code);
    }
}

#[test]
fn classify_fact_queries() {
    let queries = vec![
        "What is the capital of France?",
        "Tell me a fact about quantum computing",
    ];
    for q in queries {
        assert_eq!(classify_intent(q), QueryIntent::Fact);
    }
}
```

---

### TASK-002-P1-03: RetrievalWeights 结构体

**目标**: 支持动态权重配置

**具体任务**:
- [ ] 创建 `RetrievalWeights` 结构体:
  ```rust
  #[derive(Debug, Clone)]
  pub struct RetrievalWeights {
      pub semantic: f32,
      pub episodic: f32,
      pub working: f32,
  }
  ```
- [ ] 实现 `From<QueryIntent> for RetrievalWeights`
- [ ] 支持从配置文件加载自定义权重
- [ ] 实现默认值和验证（和为 1.0）

**验收标准**:
- [ ] 结构体可从 Intent 转换
- [ ] 验证权重和为 1.0
- [ ] 支持配置文件覆盖

---

### TASK-002-P1-04: Weighted Fusion 实现

**目标**: 实现 RRF 融合算法

**具体任务**:
- [ ] 实现 `fn weighted_fusion(results: Vec<Vec<ScoredMemory>>, k: u32) -> Vec<ScoredMemory>`
  - 使用 Reciprocal Rank Fusion
  - k=60 (标准值)
- [ ] 实现 `fn rrf_score(rank: u32, weight: f32, k: u32) -> f32`
- [ ] 编写测试验证融合正确性

**验收标准**:
- [ ] RRF 分数计算正确
- [ ] 结果按分数降序排列
- [ ] 去重逻辑正确

**测试标准**:
```rust
#[test]
fn rrf_fusion_combines_ranks() {
    let list1 = vec![
        ScoredMemory { key: "a".to_string(), score: 1.0 },
        ScoredMemory { key: "b".to_string(), score: 0.9 },
    ];
    let list2 = vec![
        ScoredMemory { key: "b".to_string(), score: 0.9 },
        ScoredMemory { key: "c".to_string(), score: 0.8 },
    ];

    let fused = weighted_fusion(vec![list1, list2], 60);

    // "b" 出现在两个列表中，分数应该最高
    let b_score = fused.iter().find(|m| m.key == "b").unwrap().score;
    assert!(b_score > fused.iter().find(|m| m.key == "a").unwrap().score);
}
```

---

### TASK-002-P1-05: ActiveRetrievalManager 实现

**目标**: 实现主动检索管理器

**具体任务**:
- [ ] 创建 `ActiveRetrievalManager` 结构体
- [ ] 实现 `async fn pre_llm_call(&self, context: &ChatContext) -> Result<Vec<MemoryEntry>>`
  - 1. 提取当前查询
  - 2. 分类意图
  - 3. 获取权重
  - 4. 并行查询三个记忆层 (semantic/episodic/working)
  - 5. RRF 融合
  - 6. 返回融合结果
- [ ] 实现 `async fn inject_memory(&self, context: &mut ChatContext, memories: Vec<MemoryEntry>)`
  - 将记忆注入 context
  - 遵守 token 预算

**验收标准**:
- [ ] 预检索在 LLM 调用前触发
- [ ] 结果正确融合三个记忆层
- [ ] Token 预算不超标

---

### TASK-002-P1-06: 三层记忆查询集成

**目标**: 与 SessionManager 集成实现主动检索

**具体任务**:
- [ ] 在 `SessionManager` 中添加 `active_retrieval` 字段
- [ ] 在 `add_message` 后触发主动检索（可选）
- [ ] 实现配置开关 `active_retrieval_enabled: bool`
- [ ] 编写集成测试

**验收标准**:
- [ ] 可通过配置启用/禁用主动检索
- [ ] 启用后记忆自动注入 context
- [ ] 集成测试覆盖完整流程

---

## 优先级排序

| 阶段 | Task | 优先级 | 理由 |
|------|------|--------|------|
| P0 | TASK-002-P0-01 | P0 | P0 基础 |
| P0 | TASK-002-P0-02 | P0 | 验证 LLM 集成 |
| P1 | TASK-002-P1-01 | P1 | Intent 定义 |
| P1 | TASK-002-P1-02 | P1 | 分类器 |
| P1 | TASK-002-P1-03 | P1 | 权重结构 |
| P1 | TASK-002-P1-04 | P1 | RRF 融合 |
| P1 | TASK-002-P1-05 | P1 | 检索管理器 |
| P1 | TASK-002-P1-06 | P2 | 三层集成 |

---

## 验收总览

### P0
- [ ] `recall()` 方法支持被动调用
- [ ] LLM 可显式调用记忆召回

### P1
- [ ] 5 种 QueryIntent 全部定义
- [ ] 分类器准确率 > 90%
- [ ] RRF 融合正确
- [ ] 主动检索可配置启用
- [ ] 三层记忆正确查询和融合
