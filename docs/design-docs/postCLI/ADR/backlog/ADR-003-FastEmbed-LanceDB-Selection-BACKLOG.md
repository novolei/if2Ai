# ADR-003 FastEmbed + LanceDB Selection — 详细实施 backlog

## 概述

**目标**: 实现离线向量搜索，支持语义记忆检索
**ADR**: [ADR-003](./ADR-003-FastEmbed-LanceDB-Selection.md)
**依赖**: ADR-001 (SQLite P0)
**优先级**: P1
**预估工时**: 5-7 天

---

## 子任务清单

### TASK-003-01: 依赖安装与环境配置

**目标**: 引入 FastEmbed 和 LanceDB 依赖

**具体任务**:
- [ ] 在 `src-tauri/Cargo.toml` 添加:
  ```toml
  fastembed = "3.0"
  lancedb = "0.9"
  ```
- [ ] 添加 tokenizers 依赖: `tokenizers = "0.20"`
- [ ] 验证编译: `cargo build --package if2ai`
- [ ] 编写测试验证向量维度 (384d)

**验收标准**:
- [ ] 所有依赖正确引入
- [ ] `cargo build` 无错误
- [ ] 向量维度确认为 384

---

### TASK-003-02: FastEmbedProvider 实现

**目标**: 实现 FastEmbed 向量化提供者

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/memory/embedding/fastembed.rs`
- [ ] 定义 `FastEmbedProvider` 结构体:
  ```rust
  pub struct FastEmbedProvider {
      model: FastEmbed,
      dimension: usize,  // 384
  }
  ```
- [ ] 实现 `new() -> Result<Self, EmbeddingError>` 构造函数
  - 模型: `multilingual-e5-small`
  - 自动下载并缓存模型文件
- [ ] 实现 `fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>`
- [ ] 实现 `fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>`
- [ ] 编写单元测试（需要首次下载模型）

**验收标准**:
- [ ] 模型正确下载和加载
- [ ] 单文本嵌入返回 384 维向量
- [ ] 批量嵌入正确处理
- [ ] 模型缓存到本地

**测试标准**:
```rust
#[tokio::test]
async fn embed_single_text() {
    let provider = FastEmbedProvider::new().unwrap();
    let embedding = provider.embed("Hello, world!").unwrap();
    assert_eq!(embedding.len(), 384);
}

#[tokio::test]
async fn embed_batch() {
    let provider = FastEmbedProvider::new().unwrap();
    let texts = vec!["Hello", "World", "Test"];
    let embeddings = provider.embed_batch(&texts).unwrap();
    assert_eq!(embeddings.len(), 3);
    assert_eq!(embeddings[0].len(), 384);
}
```

---

### TASK-003-03: LanceDBMemory 表设计

**目标**: 设计 LanceDB 向量表 schema

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/memory/providers/lancedb.rs`
- [ ] 定义 schema:
  ```rust
  Schema::new([
      field("key", DataType::Utf8),
      field("content", DataType::Utf8),
      field("category", DataType::Utf8),
      field("embedding", DataType::FixedSizeList(Box::new(DataType::Float32), 384)),
      field("created_at", DataType::Timestamp),
  ])
  ```
- [ ] 实现 `LanceDBMemory::new(db_path: PathBuf) -> Result<Self, LanceDBError>`
- [ ] 实现表创建或打开逻辑
- [ ] 编写测试验证表创建

**验收标准**:
- [ ] Schema 定义正确
- [ ] 表自动创建（如果不存在）
- [ ] 重复打开不报错

---

### TASK-003-04: 向量插入与检索

**目标**: 实现 LanceDB 的 CRUD 操作

**具体任务**:
- [ ] 实现 `async fn insert(&self, entry: &MemoryEntry, embedding: &[f32]) -> Result<(), LanceDBError>`
- [ ] 实现 `async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<ScoredMemory>, LanceDBError>`
- [ ] 实现 `async fn delete(&self, key: &str) -> Result<(), LanceDBError>`
- [ ] 实现 `async fn update(&self, entry: &MemoryEntry, embedding: &[f32]) -> Result<(), LanceDBError>`
- [ ] 编写测试覆盖 CRUD

**验收标准**:
- [ ] 插入后可通过 key 检索
- [ ] 搜索返回最相似的 top-k 结果
- [ ] 删除后搜索不到

**测试标准**:
```rust
#[tokio::test]
async fn insert_and_search() {
    let temp_dir = TempDir::new().unwrap();
    let db = LanceDBMemory::new(temp_dir.path().join("test.db")).await.unwrap();

    let entry = MemoryEntry {
        key: "test_key".to_string(),
        content: "Hello world".to_string(),
        category: MemoryCategory::Core,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let embedding = vec![0.1; 384];

    db.insert(&entry, &embedding).await.unwrap();

    let results = db.search(&embedding, 10).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].key, "test_key");
}
```

---

### TASK-003-05: 混合检索 (FTS5 + Vector)

**目标**: 实现 RRF 融合的混合搜索

**具体任务**:
- [ ] 实现 `async fn hybrid_search(&self, query: &str, category: Option<&str>, limit: usize) -> Result<Vec<ScoredMemory>>`
  - 1. 获取 query embedding
  - 2. 并行执行 FTS5 搜索和向量搜索
  - 3. RRF 融合 (k=60)
- [ ] 实现 `async fn fts_search(&self, query: &str, category: Option<&str>, limit: usize) -> Result<Vec<ScoredMemory>>`
- [ ] 实现辅助函数 `fn rrf_fusion(results: Vec<Vec<ScoredMemory>>, k: u32) -> Vec<ScoredMemory>`
- [ ] 编写测试验证融合正确性

**验收标准**:
- [ ] FTS5 和向量搜索并行执行
- [ ] RRF 融合正确
- [ ] Category 过滤同时作用于两种搜索

---

### TASK-003-06: 类别过滤与分页

**目标**: 支持类别过滤和结果分页

**具体任务**:
- [ ] 修改 `search` 和 `hybrid_search` 支持 `category: Option<&str>`
- [ ] 实现 `limit` 和 `offset` 参数支持
- [ ] 实现结果去重
- [ ] 编写测试覆盖各种过滤组合

**验收标准**:
- [ ] category 过滤正确
- [ ] limit/offset 分页正确
- [ ] 空结果处理正确

---

### TASK-003-07: 索引优化

**目标**: 优化向量检索性能

**具体任务**:
- [ ] 配置 IVF-PQ 索引:
  ```rust
  .create_index()
  .vector("embedding")
  .index_type(IndexType::IvfPq)
  .num_partitions(128)
  .build()
  ```
- [ ] 实现索引构建触发逻辑（数据量 > 1000 时）
- [ ] 测量优化效果并记录

**验收标准**:
- [ ] 1000+ 数据时索引自动构建
- [ ] 搜索性能提升 > 50%
- [ ] 索引构建不阻塞主线程

---

### TASK-003-08: 与 SqliteMemoryProvider 集成

**目标**: 实现 MemoryProvider trait

**具体任务**:
- [ ] 创建 `VectorMemoryProvider` 包装 `LanceDBMemory` + `FastEmbedProvider`
- [ ] 实现 `MemoryProvider` trait:
  - `store` → 嵌入 + 插入 LanceDB + 插入 SQLite (元数据)
  - `recall` → 混合搜索
  - `delete` → 两边都删除
- [ ] 实现向后兼容：仅在启用向量搜索时使用
- [ ] 编写集成测试

**验收标准**:
- [ ] `VectorMemoryProvider` 实现 `MemoryProvider` trait
- [ ] `store` 同时写入 LanceDB 和 SQLite
- [ ] 可通过配置切换 `SqliteMemoryProvider` / `VectorMemoryProvider`

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-003-01 | 依赖所有后续 |
| P0 | TASK-003-02 | FastEmbed 是向量来源 |
| P0 | TASK-003-03 | LanceDB 表设计 |
| P0 | TASK-003-04 | 核心 CRUD |
| P1 | TASK-003-05 | 混合检索核心 |
| P1 | TASK-003-06 | 过滤分页 |
| P2 | TASK-003-07 | 性能优化 |
| P1 | TASK-003-08 | 集成 |

---

## 依赖关系

```
TASK-003-01
     ↓
TASK-003-02 → TASK-003-03
     ↓              ↓
     └──────→ TASK-003-04
                   ↓
              TASK-003-05 → TASK-003-06
                   ↓              ↓
              TASK-003-07    TASK-003-08
```

---

## 验收总览

- [ ] FastEmbed 正确加载 multilingual-e5-small
- [ ] LanceDB 表创建成功
- [ ] 向量插入和搜索正确
- [ ] 混合搜索融合 FTS5 + Vector
- [ ] Category 过滤正确
- [ ] IVF-PQ 索引优化生效
- [ ] MemoryProvider trait 正确实现
