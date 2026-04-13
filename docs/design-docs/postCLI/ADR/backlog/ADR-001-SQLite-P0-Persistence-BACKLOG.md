# ADR-001 SQLite P0 Persistence — 详细实施 backlog

## 概述

**目标**: 将 if2Ai 记忆系统从进程内存 HashMap 迁移到 SQLite 持久化存储
**ADR**: [ADR-001](./ADR-001-SQLite-P0-Persistence.md)
**优先级**: P0 (Critical — 当前实现丢失数据)
**预估工时**: 3-4 天

---

## 子任务清单

### TASK-001-01: 环境准备与依赖添加

**目标**: 在 Rust 项目中引入 SQLite 支持

**具体任务**:
- [ ] 在 `src-tauri/Cargo.toml` 中添加依赖 `sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }`
- [ ] 添加 `rusqlite` 作为可选依赖用于 FTS5 支持
- [ ] 创建 `src-tauri/src/modules/memory/providers/` 目录
- [ ] 验证 SQLite 功能正常：编写测试 `#[test] fn sqlite_connection_works()`

**验收标准**:
- [ ] `cargo build --package if2ai` 成功无错误
- [ ] SQLite 连接测试通过

**测试标准**:
```rust
#[tokio::test]
async fn sqlite_connection_works() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let row: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&pool).unwrap();
    assert_eq!(row.0, 1);
}
```

---

### TASK-001-02: 数据库 Schema 设计

**目标**: 设计并实现 SQLite 表结构

**具体任务**:
- [ ] 设计 `memory_entries` 表 schema:
  - `key TEXT PRIMARY KEY` — 记忆唯一标识
  - `content TEXT NOT NULL` — 记忆内容
  - `category TEXT NOT NULL` — 分类 (Core/Daily/Conversation)
  - `created_at TEXT NOT NULL` — RFC3339 时间戳
  - `updated_at TEXT NOT NULL` — RFC3339 时间戳
  - `importance REAL DEFAULT 0.5` — 重要性评分
  - `access_count INTEGER DEFAULT 0` — 访问次数
  - `trust_score REAL DEFAULT 0.0` — 信任评分
- [ ] 设计索引:
  - `CREATE INDEX idx_memory_category ON memory_entries(category)`
  - `CREATE INDEX idx_memory_created_at ON memory_entries(created_at)`
- [ ] 编写 Schema 迁移 SQL 文件 `migrations/001_create_memory_entries.sql`
- [ ] 编写测试验证表创建成功

**验收标准**:
- [ ] Schema 定义在代码中可查看
- [ ] 索引创建语句存在
- [ ] 测试覆盖表创建和索引

**测试标准**:
```rust
#[tokio::test]
async fn create_memory_entries_table() {
    let pool = create_test_pool().await;
    pool.execute(include_str!("../../migrations/001_create_memory_entries.sql"), [])
        .await
        .unwrap();

    // 验证表存在
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='memory_entries')"
    ).fetch_one(&pool).await.unwrap();
    assert!(exists);
}
```

---

### TASK-001-03: SqliteMemoryProvider 结构体实现

**目标**: 实现 `SqliteMemoryProvider` 结构体

**具体任务**:
- [ ] 定义 `SqliteMemoryProvider` 结构体，包含 `pool: SqlitePool` 字段
- [ ] 实现 `new(db_path: PathBuf) -> Result<Self, MemoryError>` 构造函数
- [ ] 在构造函数中调用 `create_table_if_not_exists()`
- [ ] 实现连接池配置：最大连接数 5，超时 30 秒
- [ ] 编写测试验证 `SqliteMemoryProvider::new()` 可用

**验收标准**:
- [ ] 结构体定义包含 `SqlitePool` 字段
- [ ] 构造函数返回 `Result<Self, MemoryError>`
- [ ] 单元测试覆盖构造函数

**测试标准**:
```rust
#[tokio::test]
async fn sqlite_provider_creation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let provider = SqliteMemoryProvider::new(db_path).await;
    assert!(provider.is_ok());
}
```

---

### TASK-001-04: MemoryProvider Trait 实现 (store/recall/delete)

**目标**: 为 `SqliteMemoryProvider` 实现 `MemoryProvider` trait 的核心方法

**具体任务**:
- [ ] 实现 `async fn store(&self, key: &str, content: &str, category: MemoryCategory) -> Result<(), MemoryError>`
  - 使用 `INSERT OR REPLACE INTO` 实现 upsert
  - 自动设置 `created_at`（首次插入时）和 `updated_at`
- [ ] 实现 `async fn recall(&self, query: &str, category: Option<&str>, limit: usize) -> Result<Vec<MemoryEntry>, MemoryError>`
  - 支持模糊搜索（key 或 content 包含 query）
  - 支持 category 过滤
  - 支持 limit 限制
- [ ] 实现 `async fn delete(&self, key: &str) -> Result<(), MemoryError>`
  - 使用 `DELETE FROM memory_entries WHERE key = $1`
  - 找不到时返回 `MemoryError::KeyNotFound(key)`
- [ ] 为每个方法编写单元测试，覆盖正常路径和异常路径

**验收标准**:
- [ ] `store()` 正确处理插入和更新
- [ ] `recall()` 正确过滤 query 和 category
- [ ] `delete()` 找不到时返回 `KeyNotFound` 错误
- [ ] 所有方法线程安全（使用 `&self`）

**测试标准**:
```rust
#[tokio::test]
async fn store_and_recall() {
    let pool = create_test_pool().await;
    let provider = SqliteMemoryProvider { pool };

    provider.store("key1", "content1", MemoryCategory::Core).await.unwrap();

    let results = provider.recall("content1", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "key1");
}

#[tokio::test]
async fn recall_with_category_filter() {
    let pool = create_test_pool().await;
    let provider = SqliteMemoryProvider { pool };

    provider.store("k1", "test", MemoryCategory::Core).await.unwrap();
    provider.store("k2", "test", MemoryCategory::Daily).await.unwrap();

    let results = provider.recall("test", Some("core"), 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].category, MemoryCategory::Core);
}

#[tokio::test]
async fn delete_nonexistent_returns_error() {
    let pool = create_test_pool().await;
    let provider = SqliteMemoryProvider { pool };

    let result = provider.delete("nonexistent").await;
    assert!(matches!(result, Err(MemoryError::KeyNotFound(_))));
}
```

---

### TASK-001-05: purge_category 和 export 方法实现

**目标**: 完成 `MemoryProvider` trait 的剩余方法

**具体任务**:
- [ ] 实现 `async fn purge_category(&self, category: &str) -> Result<(), MemoryError>`
  - 删除指定分类下所有记忆
  - 返回删除数量或错误
- [ ] 实现 `async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError>`
  - 导出所有记忆或指定分类的记忆
  - 按 `created_at` 降序排序
- [ ] 为两个方法编写测试

**验收标准**:
- [ ] `purge_category()` 删除所有匹配分类的记录
- [ ] `export()` 返回正确排序的记录列表
- [ ] 测试覆盖空结果、单个结果、多个结果

**测试标准**:
```rust
#[tokio::test]
async fn purge_category() {
    let pool = create_test_pool().await;
    let provider = SqliteMemoryProvider { pool };

    provider.store("k1", "test", MemoryCategory::Core).await.unwrap();
    provider.store("k2", "test", MemoryCategory::Daily).await.unwrap();

    provider.purge_category("core").await.unwrap();

    let results = provider.export(None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].category, MemoryCategory::Daily);
}

#[tokio::test]
async fn export_sorted_by_created_at_desc() {
    // 验证导出按时间降序排列
}
```

---

### TASK-001-06: 路径管理与目录初始化

**目标**: 实现安全的数据库路径管理

**具体任务**:
- [ ] 实现 `fn get_memory_db_path() -> Result<PathBuf, MemoryError>`
  - 使用 `dirs::data_local_dir()` 获取基础路径
  - 拼接 `.if2ai/memory/memory.db`
  - 验证路径在安全范围内（防止路径遍历）
- [ ] 创建目录结构：`~/.if2ai/memory/`
- [ ] 编写 `atomic_write` 辅助函数用于未来扩展
- [ ] 编写测试验证目录创建

**验收标准**:
- [ ] 路径解析正确（`~/.if2ai/memory/memory.db`）
- [ ] 目录不存在时自动创建
- [ ] 路径遍历攻击被阻止

**测试标准**:
```rust
#[test]
fn memory_db_path_is_under_expected_directory() {
    let path = get_memory_db_path().unwrap();
    assert!(path.to_string_lossy().contains(".if2ai/memory"));
}
```

---

### TASK-001-07: 与现有模块集成

**目标**: 将 `SqliteMemoryProvider` 集成到 if2Ai 现有模块架构

**具体任务**:
- [ ] 修改 `modules/memory/mod.rs` 中的 `default_memory_provider()` 工厂函数
  - 返回 `Arc<SqliteMemoryProvider>` 而非 `Arc<InMemoryMemoryProvider>`
- [ ] 保留 `InMemoryMemoryProvider` 但标记为 `#[deprecated]`
- [ ] 更新模块导出，将 `SqliteMemoryProvider` 添加到 public API
- [ ] 编写集成测试验证完整流程

**验收标准**:
- [ ] `default_memory_provider()` 返回 SQLite 实现
- [ ] 现有代码无需修改即可使用新 provider
- [ ] 集成测试覆盖 store/recall 完整流程

**测试标准**:
```rust
#[tokio::test]
async fn integration_store_and_recall() {
    let provider = default_memory_provider().await;
    provider.store("integration_key", "integration_content", MemoryCategory::Core).await.unwrap();
    let results = provider.recall("integration_content", None, 10).await.unwrap();
    assert!(!results.is_empty());
}
```

---

### TASK-001-08: 数据迁移脚本 (可选)

**目标**: 支持从旧 HashMap 格式迁移数据

**具体任务**:
- [ ] 创建迁移工具 `migrate_from_json()`
- [ ] 支持从 `~/.if2ai/memory/legacy.json` 导入
- [ ] 验证迁移后数据完整性
- [ ] 编写迁移文档

**验收标准**:
- [ ] 迁移工具可独立运行
- [ ] 迁移后 recall 返回相同结果
- [ ] 文档说明迁移步骤

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-001-01 | 依赖所有后续任务 |
| P0 | TASK-001-02 | Schema 是基础 |
| P0 | TASK-001-03 | Provider 结构体 |
| P0 | TASK-001-04 | 核心 CRUD 操作 |
| P0 | TASK-001-05 | 剩余 trait 方法 |
| P1 | TASK-001-06 | 路径管理 |
| P1 | TASK-001-07 | 模块集成 |
| P2 | TASK-001-08 | 数据迁移（可选） |

---

## 依赖关系图

```
TASK-001-01 (依赖)
     ↓
TASK-001-02 (依赖 01)
     ↓
TASK-001-03 (依赖 02)
     ↓
TASK-001-04 (依赖 03) ─────┐
     ↓                      │
TASK-001-05 ───────────────┤ (可并行)
     ↓                      │
TASK-001-06 ────────────────┤
     ↓                      │
TASK-001-07 ───────────────┘
```

---

## 验收总览

- [ ] SQLite 持久化正常工作
- [ ] `store/recall/delete/export/purge_category` 全部通过测试
- [ ] 进程重启后数据不丢失
- [ ] 与现有模块无缝集成
- [ ] 所有测试通过: `cargo test --package if2ai memory`
