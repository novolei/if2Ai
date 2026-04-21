# 记忆模块使用指南

> 面向用户的记忆操作手册——存储、召回、遗忘、钉选与安全防护

## 📍 三级作用域

If2Ai 记忆系统支持三级隔离，防止跨会话/跨项目的记忆污染：

| 作用域 | 说明 | 可见范围 |
|--------|------|----------|
| **Session** | 会话级 | 仅当前会话内可见 |
| **Project** | 项目级 | 同一 project_id 下所有会话可见 |
| **Global** | 全局级 | 跨所有会话和项目可见 |

作用域通过 `MemoryExecutionScope` 自动解析：

```mermaid
graph TD
    A[用户调用记忆工具] --> B{有 session_id?}
    B -->|是| C[Session 作用域]
    B -->|否| D{有 project_id?}
    D -->|是| E[Project 作用域]
    D -->|否| F[Global 作用域]
    C --> G[recall 时过滤: session_id 匹配]
    E --> H[recall 时过滤: project_id 匹配]
    F --> I[recall 时无过滤: 全量返回]
```

> 源码参考：`src-tauri/src/modules/memory/scope.rs` — `MemoryScopeResolver::resolve()`

## 📝 记忆操作

### 📥 存储记忆（memory_store）

将信息持久化到长期记忆，支持自动向量嵌入和语义搜索。

```
工具: memory_store
参数:
  - key (string, 必需): 记忆键名，如 "project-arch-decision"
  - content (string, 必需): 记忆内容
  - category (string, 可选): 分类标签，如 "decision" / "fact" / "preference"
```

**存储流程：**

```mermaid
sequenceDiagram
    participant User as 用户/Agent
    participant Tool as memory_store
    participant Scanner as ThreatScanner
    participant Vector as VectorMemoryProvider
    participant SQLite as SqliteMemoryProvider
    participant Lance as LanceDB

    User->>Tool: store(key, content, category, scope)
    Tool->>Scanner: scan(key, content)
    alt 检测到威胁
        Scanner-->>Tool: ThreatReport{flagged=true}
        Tool->>Tool: 记录审计事件 + 扫描并脱敏
    end
    Tool->>SQLite: store_scoped(key, cleaned, category, scope)
    SQLite-->>Tool: OK (同步写入)
    Tool->>Vector: embed(content) → 384维向量
    Vector->>Lance: insert(entry, embedding)
    Lance-->>Tool: OK (异步写入)
    Tool-->>User: 存储成功
```

### 🔍 召回记忆（memory_recall）

通过语义搜索或关键词匹配召回相关记忆。

```
工具: memory_recall
参数:
  - query (string, 必需): 搜索查询
  - category (string, 可选): 按分类过滤
  - limit (number, 可选): 返回条数上限
```

召回优先使用向量搜索（LanceDB ANN），回退到 SQLite 全文搜索。两者结果通过 **Reciprocal Rank Fusion (RRF)** 融合排序。

### 🗑️ 遗忘记忆（memory_forget）

删除指定键的记忆条目。

```
工具: memory_forget
参数:
  - key (string, 必需): 要删除的记忆键
```

同时从 SQLite 和 LanceDB 中删除。

### 🧹 清除记忆（memory_purge）

按条件批量清除记忆。

```
工具: memory_purge
参数:
  - category (string, 可选): 按分类清除
  - scope (string, 可选): 按作用域清除
```

### 📤 导出记忆（memory_export）

将记忆条目导出为结构化数据。

```
工具: memory_export
参数:
  - category (string, 可选): 按分类导出
```

## 🛡️ 安全防护：ThreatScanner

每次 `memory_store` 调用都会自动通过 ThreatScanner 扫描内容，检测并拦截敏感信息：

| PII 类别 | 说明 | 示例模式 |
|----------|------|----------|
| **ApiKey** | API 密钥 | `sk-...`、`sk-ant-...`、`AKIA...`、`ghp_...` |
| **InlineSecret** | 内联密码 | `password=...`、`passwd=...` |
| **PrivateKey** | PEM 私钥 | `-----BEGIN PRIVATE KEY-----` |
| **CreditCard** | 信用卡号 | 13-19 位数字 + 分隔符 |
| **IdCard** | 身份证号 | 18 位中国身份证号 |
| **Ssn** | 社保号码 | `xxx-xx-xxxx` 格式 |

**防护行为：**

1. **扫描阶段**：`ThreatScanner::scan()` 检测是否包含敏感模式
2. **脱敏阶段**：`ThreatScanner::scan_and_redact()` 将敏感内容替换为 `[REDACTED:<类别>]`
3. **审计阶段**：`MemoryAuditEmitter::memory_pii_redacted()` 发出审计事件
4. **策略阶段**：`MemoryPolicyEngine` 根据威胁类别决定是否阻断写入

> 源码参考：`src-tauri/src/modules/memory/security.rs` — `ThreatScanner` / `PiiKind`

## 📌 钉选记忆（Pinned Memory）

钉选记忆将关键信息固定注入系统提示词，确保每次对话都能访问：

```
工具: pin_memory    — 钉选一条记忆
工具: unpin_memory  — 取消钉选
```

**限制：**
- 每个作用域最多 `MAX_PINS_PER_SCOPE` 条钉选
- 单条内容不超过 `MAX_PIN_CONTENT_CHARS` 字符
- 钉选项通过 `PinnedStore::list_all_for_prompt()` 注入系统提示词

> 源码参考：`src-tauri/src/modules/memory/pinned/mod.rs` / `store.rs` / `types.rs`

## 📝 摘要记忆

RollingSummarizer 每 N 轮用户对话（默认 6 轮）自动生成滚动摘要：

- 摘要以原子文件写入 + SQLite 双写方式持久化
- 摘要作为编译记忆管线的输入源
- 会话结束时强制触发最终摘要

> 源码参考：`src-tauri/src/modules/memory/summary/rolling.rs`

## ⚙️ 配置说明

记忆系统通过 `memory_config.json` 和 `TickerConfig` 控制行为：

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `turns_per_summary` | 6 | 每隔多少轮生成一次滚动摘要 |
| `daily_check_interval_secs` | 3600 | 日常编译检查间隔（秒） |
| `experience_enabled` | true | 会话结束时是否提取经验 |
| `vector_search_enabled` | true | 是否启用向量搜索 |
| `sqlite_path` | `~/.if2ai/memory/memory.db` | SQLite 双写路径 |
| `IF2AI_HRR_ENABLED` | false | 是否启用 HRR 混合提供商 |

## ⚠️ 与 cc-haha 差距分析

### If2Ai 优势

| 特性 | If2Ai | cc-haha |
|------|-------|---------|
| **搜索方式** | 向量搜索 (LanceDB ANN + RRF) | 文本搜索 (Fuse.js) |
| **嵌入维度** | 384 维 FastEmbed ONNX | 无嵌入 |
| **安全扫描** | ThreatScanner 60+ 正则规则 | 基础 PII 检测 |
| **编译记忆** | 完整 5 步管线 (today→week→longterm→facts→assemble) | 无等价机制 |
| **作用域隔离** | 三级 (Session/Project/Global) + SQL 过滤 | 项目级隔离 |
| **降级链** | 4 层自动降级 | 单层 |

### If2Ai 劣势

| 特性 | If2Ai | cc-haha |
|------|-------|---------|
| **AutoDream** | 无等价机制 | 周期性自动整合记忆 |
| **团队记忆同步** | 无 | Pull/Push API 支持团队共享 |
| **记忆新鲜度** | 仅 Weibull 衰减 | 完整的新鲜度管理策略 |
| **记忆压缩** | 仅编译管线 | AutoDream 智能压缩 |

## 🎯 增强计划

1. **实现 AutoDream 等价机制**：在 MemoryTicker 的日常循环中添加周期性记忆整合步骤，自动发现并合并相关记忆
2. **团队记忆同步 API**：添加 `memory_pull` / `memory_push` 工具，支持跨设备/跨用户的记忆同步
3. **完善记忆过期策略**：基于 Weibull 衰减 + 访问频率的复合过期机制，替代单一衰减模型
4. **记忆质量评分**：引入记忆质量打分机制，自动降级低质量/过时记忆
