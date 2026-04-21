# 记忆模块（Memory）

> If2Ai 的长期记忆系统——向量搜索 + SQLite 持久化 + 编译记忆管线 + 威胁扫描

## 📚 文档目录

| 文件 | 说明 |
|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 面向用户的使用指南 |
| [02-implementation.md](./02-implementation.md) | 面向开发者的实现原理 |
| [03-compiled-memory.md](./03-compiled-memory.md) | 编译记忆管线深度解析 |

## 📍 核心概念速查表

| 概念 | 说明 |
|------|------|
| **MemoryProvider** | 记忆存储的核心 trait，定义 store/recall/delete/purge/export 等操作 |
| **VectorMemoryProvider** | 向量记忆提供商，FastEmbed 嵌入 + LanceDB 向量存储 + SQLite 双写 |
| **SqliteMemoryProvider** | SQLite 持久化提供商，scope 感知 + Weibull 重要性衰减 |
| **HybridMemoryProvider** | HRR 全息表示 + LanceDB 混合提供商（实验性） |
| **InMemoryMemoryProvider** | 纯内存提供商（已弃用，仅测试用） |
| **ThreatScanner** | 威胁扫描器，15+ 类别 / 60+ 正则模式，写入前自动检测 PII/密钥/凭证 |
| **MemoryCompiler** | 编译记忆编排器，驱动 today/week/longterm/facts/assemble 五步管线 |
| **MemoryTicker** | 轮次调度器，按 turn 计数触发滚动摘要和编译 |
| **RollingSummarizer** | 滚动摘要器，每 N 轮对对话历史生成压缩摘要 |
| **JobRunner** | 后台作业运行器，最多 3 次重试 + 3 并发 + 持久化状态 |
| **MemoryExecutionScope** | 三级作用域：Session / Project / Global |
| **PinnedStore** | 钉选记忆存储，将关键记忆注入系统提示词 |
| **降级链** | HybridMemory → VectorMemory(FastEmbed+LanceDB) → SqliteMemory → InMemory |

## 🏗️ 源码位置

| 组件 | 路径 |
|------|------|
| 模块入口 | `src-tauri/src/modules/memory/mod.rs` |
| 向量提供商 | `src-tauri/src/modules/memory/providers/vector_provider.rs` |
| SQLite 提供商 | `src-tauri/src/modules/memory/providers/sqlite_provider.rs` |
| LanceDB 存储 | `src-tauri/src/modules/memory/providers/lancedb.rs` |
| HRR 混合提供商 | `src-tauri/src/modules/memory/hrr/integration.rs` |
| 嵌入引擎 | `src-tauri/src/modules/memory/embedding/fastembed.rs` |
| 威胁扫描器 | `src-tauri/src/modules/memory/security.rs` |
| 编译器 | `src-tauri/src/modules/memory/compiler/mod.rs` |
| 轮次调度器 | `src-tauri/src/modules/memory/ticker.rs` |
| 作用域 | `src-tauri/src/modules/memory/scope.rs` |
| 作业运行器 | `src-tauri/src/modules/memory/job_runner.rs` |
| 钉选记忆 | `src-tauri/src/modules/memory/pinned/mod.rs` |
| 滚动摘要 | `src-tauri/src/modules/memory/summary/rolling.rs` |
| 审计 | `src-tauri/src/modules/memory/audit.rs` |
| 策略引擎 | `src-tauri/src/modules/memory/policy.rs` |

## 🔗 相关资源

- [记忆系统设计文档](../../design-docs/memory-system.md)
- [上下文压缩](../../design-docs/context-compression.md)
- [cc-haha 记忆增强参考](../../design-docs/postCLI/memory-enhancement-from-openhanako-v1.md)
- [编码规范](../../references/coding-style-and-lint-contract.md)
