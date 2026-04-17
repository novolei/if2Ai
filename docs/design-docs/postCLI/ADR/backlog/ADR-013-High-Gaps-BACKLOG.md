# ADR-013 High Gaps — 详细实施 backlog

## 概述

**目标**: 修复 Phase 6B/6BW 中的 6 个 High severity gaps，这些 gaps 导致功能部分失效。
**ADR**: [ADR-013](./ADR-013-Phase-Remediation-Design.md)
**优先级**: P1 (在 Critical gaps 之后修复)
**预估工时**: 2-3 天
**基于审计**: [phase-6b-6bw-6e-gap-audit-report.md](../../../../generated/phase-6b-6bw-6e-gap-audit-report.md) v2

**依赖**: H4（dead_code 清理）依赖 C1-C5 先完成（必须先有调用方才能清理 dead_code）。H1 依赖 C2（ActiveRetrievalManager 在 AppState 中可用）。

---

## 子任务清单

### TASK-013-H1: SessionManager Has No Active Retrieval Integration

**对应 TASK**: 002-P1-06
**对应 ADR**: ADR-002 (Active Retrieval vs Passive Invocation)

#### 目的

`ActiveRetrievalManager` 存在但从未集成到 `SessionManager`。session 生命周期应该为每个新 session 支持主动检索。

#### 当前状态

- `ActiveRetrievalManager` 存在于 `src-tauri/src/modules/memory/retrieval.rs`
- `SessionManager` 中没有 `active_retrieval` 字段
- 没有 `active_retrieval_enabled` 配置

#### 实施计划

**Step 1**: 添加 `active_retrieval` 字段到 `SessionManager`

File: `src-tauri/src/modules/session/mod.rs`

```rust
use crate::modules::memory::retrieval::ActiveRetrievalManager;

pub struct SessionManager {
    // ... existing fields ...
    active_retrieval: Option<Arc<ActiveRetrievalManager>>,
}
```

**Step 2**: 添加 builder 方法和构造函数参数

```rust
impl SessionManager {
    pub fn with_active_retrieval(mut self, arm: Arc<ActiveRetrievalManager>) -> Self {
        self.active_retrieval = Some(arm);
        self
    }
}
```

**Step 3**: 在 session 创建期间注入检索上下文

在 `create_session()` 中，如果主动检索已启用：

```rust
if let Some(ref arm) = self.active_retrieval {
    if let Ok(context) = arm.retrieve_as_context(&user_message).await {
        // inject into initial system prompt
    }
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/session/mod.rs` | 添加 `active_retrieval` 字段 |
| `src-tauri/src/commands/session.rs` | 接入主动检索到 session 创建 |

#### 验收标准

- [ ] `SessionManager` 拥有可选的 `active_retrieval` 字段
- [ ] builder 方法 `with_active_retrieval()` 存在
- [ ] session 创建在可用时使用主动检索

---

### TASK-013-H2: No IVF-PQ Index Configuration for LanceDB

**对应 TASK**: 003-07
**对应 ADR**: ADR-003 (FastEmbed + LanceDB Selection)

#### 目的

没有 IVF-PQ 索引，LanceDB 执行暴力向量搜索。对于 >10K 条目的数据集，每次查询是 O(n) — 不可接受的慢。

#### 实施计划

在 `VectorMemoryProvider::new()` 中添加索引创建：

File: `src-tauri/src/modules/memory/providers/vector_provider.rs`

```rust
// After creating/opening the LanceDB table:
use lancedb::index::Index;
use lancedb::index::vector::IvfPqIndexBuilder;

table
    .create_index(
        &["embedding"],
        Index::IvfPq(IvfPqIndexBuilder::default()
            .num_partitions(32)
            .num_sub_vectors(24)),
    )
    .await?;
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/memory/providers/lancedb.rs` | 添加 IVF-PQ 索引创建 |
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | 在 `new()` 中调用索引创建 |

#### 验收标准

- [ ] IVF-PQ 索引在表初始化期间创建
- [ ] 索引参数可配置（num_partitions, num_sub_vectors）
- [ ] 索引创建失败不 panic（记录 warning，无索引继续）

---

### TASK-013-H3: VectorMemoryProvider Doesn't Dual-Write to SQLite

**对应 TASK**: 003-08
**对应 ADR**: ADR-003 (FastEmbed + LanceDB Selection)

#### 目的

`VectorMemoryProvider.store()` 只写入 LanceDB。如果 app 在没有 FastEmbed 重新初始化的情况下重启，向量条目会丢失。SQLite 应该是持久化后备。

#### 实施计划

**Step 1**: 添加 `sqlite_provider` 字段到 `VectorMemoryProvider`

File: `src-tauri/src/modules/memory/providers/vector_provider.rs`

```rust
pub struct VectorMemoryProvider {
    lancedb: LanceDBMemory,
    embedder: Arc<FastEmbedProvider>,
    config: VectorProviderConfig,
    sqlite: Option<Arc<SqliteMemoryProvider>>, // NEW: dual-write backing store
}
```

**Step 2**: 在 `store()` 中双写

```rust
async fn store(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
    // Write to SQLite first (source of truth)
    if let Some(ref sqlite) = self.sqlite {
        sqlite.store(entry.clone()).await?;
    }

    // Write to LanceDB (vector index)
    let embedding = self.embedder.embed(&entry.content).await?;
    self.lancedb.insert(&entry, &embedding).await?;

    Ok(())
}
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | 添加 SQLite 双写 |

#### 验收标准

- [ ] `store()` 先写 SQLite 再写 LanceDB
- [ ] SQLite 是可选的（`Option<>`）— 优雅降级
- [ ] `recall()` 可以从任一后端读取

---

### TASK-013-H4: VectorProvider dead_code Suppression Not Cleaned

**对应 TASK**: 012-04
**对应 ADR**: ADR-012 (Wiring Layer 1)

#### 目的

模块级 `#![allow(dead_code)]` 在 `vector_provider.rs:11` 掩盖了模块已有真实调用方的事实。在 C1-C5 wiring 完成后，大部分函数应该有调用方。

#### 实施计划

将模块级 `#![allow(dead_code)]` 替换为针对性的 `#[allow(dead_code)]`：

File: `src-tauri/src/modules/memory/providers/vector_provider.rs`

```rust
// Remove line 11: #![allow(dead_code)]
// Add #[allow(dead_code)] only to functions that remain uncalled
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | 将模块级替换为函数级 `#[allow(dead_code)]` |

#### 验收标准

- [ ] 模块级 `#![allow(dead_code)]` 已移除
- [ ] 仅真正未调用的函数有 `#[allow(dead_code)]`
- [ ] `cargo clippy -- -D warnings` 无 dead_code 警告

---

### TASK-013-H5: HRR Tests Still `#[ignore]` — No MockEmbedder

**对应 TASK**: 012-10
**对应 ADR**: ADR-007 (HRR), ADR-012 (Wiring)

#### 目的

HRR 集成测试被忽略因为它们需要运行时下载 FastEmbed 模型。没有 mock embedder，HRR 代数推理（bind/unbind/bundle）零测试覆盖。

#### 实施计划

**Step 1**: 创建 `MockEmbedder`

File: `src-tauri/src/modules/memory/embedding/mock.rs`

```rust
/// Deterministic mock embedder for testing HRR algebraic operations.
///
/// Produces a reproducible 384-d vector based on input text hash.
/// Does NOT require model download — instant execution.
pub struct MockEmbedder;

impl MockEmbedder {
    pub fn embed_sync(&self, text: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; 384];
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        let hash_value = hasher.finish();

        // Seed the vector deterministically from the hash
        for i in 0..384 {
            let bit = (hash_value >> (i % 64)) & 1;
            vec[i] = if bit == 1 { 1.0 } else { -1.0 };
        }

        // Normalize
        let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }

        vec
    }
}
```

**Step 2**: 在 `hrr/integration.rs` 中使用 MockEmbedder 替换 `#[ignore]` 测试

File: `src-tauri/src/modules/memory/hrr/integration.rs`

创建使用 `MockEmbedder` 而非 `FastEmbedProvider` 的测试。

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/modules/memory/embedding/mock.rs` | **创建** — MockEmbedder |
| `src-tauri/src/modules/memory/embedding/mod.rs` | 导出 MockEmbedder |
| `src-tauri/src/modules/memory/hrr/integration.rs` | 用 MockEmbedder 替换 `#[ignore]` 测试 |

#### 验收标准

- [ ] MockEmbedder 产生确定性的 384-d 向量
- [ ] HRR 集成测试不再 `#[ignore]`
- [ ] `cargo test` 包含 HRR 测试全部通过

---

### TASK-013-H6: Trajectory Tauri Commands Not Exported

**对应 TASK**: 009-06
**对应 ADR**: ADR-009 (Trajectory Learning)

#### 目的

`export_trajectories` 作为 Tauri 命令存在，但 `get_trajectory_count` 未在 `commands/mod.rs` 的 invoke handler 列表中导出。

#### 当前状态

- `export_trajectories` 已在 `main.rs:295` 的 `invoke_handler!` 中
- `get_trajectory_count` 未导出

#### 实施计划

在 invoke handler 中添加 `get_trajectory_count`：

File: `src-tauri/src/main.rs:228-296`

```rust
// Add to invoke_handler:
get_trajectory_count,  // NEW
```

并在 `commands/mod.rs:104` 中导出：

```rust
pub use settings::{
    export_trajectories, get_memory_config, set_memory_config,
    get_trajectory_count,  // NEW
    MemoryConfig, MemoryConfigInput,
};
```

#### 目标文件

| 文件 | 变更 |
|------|------|
| `src-tauri/src/main.rs` | 添加 `get_trajectory_count` 到 invoke_handler |
| `src-tauri/src/commands/mod.rs` | 导出 `get_trajectory_count` |

#### 验收标准

- [ ] `get_trajectory_count` 在 `invoke_handler!` 中注册
- [ ] `get_trajectory_count` 从 `commands/mod.rs` 导出
- [ ] 前端可通过 IPC 调用该命令

---

## 验收总览

- [ ] H1: SessionManager 拥有 active_retrieval 字段和 builder
- [ ] H2: IVF-PQ 索引在 LanceDB 表初始化时创建
- [ ] H3: VectorMemoryProvider store() 双写到 SQLite
- [ ] H4: vector_provider.rs 模块级 dead_code 替换为函数级
- [ ] H5: MockEmbedder 存在，HRR 测试不再 #[ignore]
- [ ] H6: get_trajectory_count 在 invoke_handler 中注册
- [ ] cargo fmt + clippy + test 全部通过
