# Design Documents (设计文档库)

> If2Ai 项目的完整系统设计文档。
> 
> **最近更新** (2026-04-11): ✨ 完成了 3 份核心架构文档，实现了 Hermes 到 If2Ai 的完整对标！

---

## 🌟 最新：完整的架构框架

### ✨ 新增三份核心架构文档

我们刚刚完成了 **3 份关键架构文档**，将 Hermes 的设计完整地映射到 If2Ai：

#### 1. 📐 [system-architecture-framework.md](./system-architecture-framework.md)
Hermes 的 9 个子系统→If2Ai 的分阶段实现

```
Hermes 9 系统                If2Ai 模块         Phase 1 代码
─────────────────────────────────────────────────────────
Agent Loop           →       Agent Module       80% ✅
Prompt System        →       Prompt Builder     30% (待增强)
Provider Resolution  →       Provider Module    70% ✅
Tool Executor        →       Tools Module       90% ✅
Session Manager      →       Session Module     80% ✅
Messaging Gateway    →       Phase 3 (Future)   0%
Plugin System        →       Plugin Module      0%
Cron Scheduler       →       Phase 3 (Future)   0%
ACP/IDE              →       Phase 3 (Future)   0%
```

**阅读时间**: 15 分钟 | **难度**: 入门 ⭐ | **优先级**: 必读 ✅

---

#### 2. 🏗️ [module-boundaries-and-integration.md](./module-boundaries-and-integration.md)
Rust 模块的清晰边界、依赖和集成方式

6 个核心模块 + AppState 设计 + Tauri Commands 网关

```rust
pub struct AppState {
    pub agent_runtime: Arc<Mutex<ConversationRuntime>>,
    pub provider_manager: Arc<ProviderManager>,
    pub tool_registry: Arc<ToolRegistry>,
    pub session_manager: Arc<SessionManager>,
    // Phase 2
    pub memory_manager: Arc<MemoryManager>,
    pub plugin_manager: Arc<PluginManager>,
}
```

**阅读时间**: 20 分钟 | **难度**: 中级 ⭐⭐ | **优先级**: 必读 ✅

---

#### 3. 🚀 [entry-points-design.md](./entry-points-design.md)
三个入口点的完整设计（未来扩展路线）

```
Phase 1: Tauri 桌面应用
  ↓
Phase 2: JSON-RPC API Server
  ↓
Phase 3: IDE/LSP 集成 (VS Code, Zed, JetBrains)

共享的业务逻辑 (commands/ 模块) ✅ 代码复用
```

**阅读时间**: 20 分钟 | **难度**: 中级 ⭐⭐ | **优先级**: 必读 ✅

---

## 🗺️ 快速导航

### 按角色选择（推荐）

**👤 新加入的开发者** (30 分钟入门)
```
1. system-architecture-framework.md      (15 min)  ← 从这里开始
2. module-boundaries-and-integration.md  (10 min)
3. 选择你的专业领域继续深入具体文档
```

**👨‍💻 Agent 开发** 
→ [agent-loop.md](./agent-loop.md) + [provider-resolution.md](./provider-resolution.md)

**🔧 工具开发** 
→ [tool-system.md](./tool-system.md)

**💾 数据/会话开发** 
→ [session-persistence.md](./session-persistence.md)

**� 消息网关/多平台** 
→ [messaging-gateway.md](./messaging-gateway.md) ✨ NEW (Phase 2)

**�🖥️ 前端开发** 
→ [entry-points-design.md](./entry-points-design.md) (Tauri 部分)

**🎨 架构审查** 
→ [system-architecture-framework.md](./system-architecture-framework.md)

---

## 📚 完整设计文档列表

### Phase 1 核心系统（✅ 设计 100% 完成）

| 文档 | 行数 | Hermes 参考 | If2Ai 代码 | 状态 |
|------|------|-----------|-----------|------|
| [system-architecture-framework.md](./system-architecture-framework.md) | 750 | - | 全系统 | ✅ |
| [module-boundaries-and-integration.md](./module-boundaries-and-integration.md) | 700 | - | 全系统 | ✅ |
| [entry-points-design.md](./entry-points-design.md) | 650 | - | 全系统 | ✅ |
| [agent-loop.md](./agent-loop.md) | 430 | 9.2K | 80% | ✅ |
| [provider-resolution.md](./provider-resolution.md) | 800 | 1.2K | 70% | ✅ |
| [session-persistence.md](./session-persistence.md) | 450 | 0.6K | 80% | ✅ |
| [tool-system.md](./tool-system.md) | 已有 | 0.8K | 90% | ✅ |

### Phase 2 核心系统（✅ 开始设计）

| 文档 | 行数 | Hermes 参考 | 优先级 | 状态 |
|------|------|-----------|--------|------|
| [messaging-gateway.md](./messaging-gateway.md) | 800 | 1.2K | P1 | ✅ 设计完成 |
| [prompt-builder.md](./prompt-builder.md) | - | 0.5K | P1 | 📋 待设计 |
| [memory-system.md](./memory-system.md) | - | 1.0K | P2 | 📋 待设计 |
| [context-compression.md](./context-compression.md) | - | 0.4K | P2 | 📋 待设计 |
| [error-handling.md](./error-handling.md) | - | 0.3K | P1 | 📋 待设计 |
| [testing-strategy.md](./testing-strategy.md) | - | 0.5K | P1 | 📋 待设计 |

### Phase 3+ 扩展（🔮 未来计划）

- **plugin-architecture.md** - 插件系统架构
- **mcp-integration.md** - Model Context Protocol
- **cron-scheduler.md** - 定时任务调度

---

## 📊 进度统计

```
设计文档完成度 (Design)           代码实现进度 (Code)
════════════════════════════════════════════════════

Phase 1 设计:
  System Architecture  ██████████ 100%  ✨ NEW
  Module Boundaries    ██████████ 100%  ✨ NEW  
  Entry Points         ██████████ 100%  ✨ NEW
  Agent Loop           ██████████ 100%   Agent          ████████░ 80%
  Provider Sys         ██████████ 100%   Provider       ███████░░ 70%
  Tool System          ██████████ 100%   Tools          ██████████ 90%
  Session Persist      ██████████ 100%   Session        ████████░ 80%
  ────────────────────────────────────────────────────
  Phase 1 总体:        ██████████ 100%   Phase 1 总体   ████████░ 80%

Phase 2 计划: ░░░░░░░░░░  0%   Phase 2 计划  ░░░░░░░░░░  0%

整体项目:    ███████░░░ 64%   ✈️ 可开始编码
```

---

## 💡 学习建议

### 最短路径 (30 分钟)
```
1. system-architecture-framework.md     ← 理解全景
2. module-boundaries-and-integration.md ← 理解模块
3. 根据角色选择具体文档
```

### 标准路径 (2 小时)
```
上述 3 份 + 2-3 份专题文档 + 查看对应源代码
```

### 深度路径 (1 天)
```
所有设计文档 (6 小时) + 源代码深度阅读 (3 小时) + 运行 harness 测试 (1 小时)
```

---

## 🔗 常见问题

**Q: 我应该从哪里开始？**
A: 👉 阅读 [system-architecture-framework.md](./system-architecture-framework.md) (15 分钟)

**Q: Hermes 的哪个部分对应 If2Ai 的什么模块？**
A: 👉 见 system-architecture-framework.md 的对标表

**Q: 代码实现进度是多少？**
A: 👉 Phase 1: 80% | Phase 2: 0% | Phase 3: 0%

**Q: Phase 1 还缺什么？**
A: 👉 Prompt Builder 需增强 (30% → 100%) | 其他工作量小

---

## 📌 与 Hermes 的对标

如果你有 Hermes 代码库，可以这样对标：

| 组件 | Hermes | If2Ai | 映射文档 |
|------|--------|-------|---------|
| 架构全景 | `README.md` | 无 | system-architecture-framework.md |
| Agent 循环 | `run_agent.py` (9.2K) | `src-tauri/src/modules/agent/` (80%) | agent-loop.md |
| LLM 提供商 | `runtime_provider.py` (1.2K) | `src-tauri/src/modules/provider/` (70%) | provider-resolution.md |
| 工具系统 | `tool_registry.py` (0.8K) | `src-tauri/src/modules/tools/` (90%) | tool-system.md |
| 会话存储 | `hermes_state.py` (0.6K) | `src-tauri/src/modules/session/` (80%) | session-persistence.md |
| Desktop UI | 无 | `src/` (Svelte) | entry-points-design.md |
| API Server | 无 | 待创建 (Phase 2) | entry-points-design.md |
| IDE 集成 | LSP adapter | 待创建 (Phase 3) | entry-points-design.md |

---

## 🎯 当前工作重点

**✅ 完成** (2026-04-11)
- 系统架构框架完整设计
- 模块边界明确定义
- 端点入口设计完成
- Phase 1 代码 80% 完成

**⏳ 进行中**
- Prompt Builder 设计增强
- Phase 1 代码最后 20%

**🔮 计划中**
- Error Handling 设计 (Phase 2)
- Testing Strategy 设计 (Phase 2)
- Memory System 设计 (Phase 2)

---

## 📖 使用说明

### 添加新设计文档

1. 使用 `kebab-case.md` 命名
2. 包含标准头部：
   ```markdown
   # 文档标题
   
   **版本**: 1.0 | **最后更新**: YYYY-MM-DD | **状态**: Draft/Stable
   ```
3. 控制长度在 200-800 行
4. 更新 index.md 和 DESIGN_DOCS_INDEX.md

### 保持文档最新

- 每当实现代码时，更新对应的"代码进度"指标
- 发现设计问题时，创建 issue + 更新文档
- 每周一更新 进度统计表

## 📚 文档组织

设计文档按照系统的逻辑层级组织：

```
┌─────────────────────────────────────┐
│ Agent Orchestrator (中枢)            │  agent-orchestrator.md
│ 对话循环、预算、协调                  │
└────────────┬────────────────────────┘
             │
    ┌────────┼────────┬──────────┬──────────────┐
    │        │        │          │              │
    ▼        ▼        ▼          ▼              ▼
┌────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐ ┌──────────┐
│ Tool   │ │LLM      │ │Prompt   │ │Context    │ │Memory   │
│System │ │Routing  │ │Builder  │ │Compression│ │System   │
└────────┘ └──────────┘ └──────────┘ └────────────┘ └──────────┘
    │
┌─────────────────────────────────────┐
│ Data Schema (数据)                   │
└─────────────────────────────────────┘
    │
┌─────────────────────────────────────┐
│ Error Handling (容错)                │
└─────────────────────────────────────┘
    │
┌─────────────────────────────────────┐
│ Harness Testing (评估)               │
└─────────────────────────────────────┘
```

## 📖 核心设计文档

### 1️⃣ Agent 系统核心

#### [Agent Orchestrator](./agent-orchestrator.md) ⭐⭐⭐⭐⭐
**450+ 行 | 复杂度: 极高**

Agent 系统的大脑。负责对话循环、工具编排、LLM 交互和预算管理。

主要内容：
- 对话循环的完整流程（用户输入 → LLM → 工具 → 上下文压缩 → 保存）
- 三层预算系统（迭代、上下文、成本）
- LLM 生命周期管理和动态模型切换
- 工具执行管理（依赖解析、并行执行）
- 状态管理和持久化

**关键代码**:
```rust
pub async fn run_conversation(...) -> ConversationResult
pub struct BudgetTracker
pub async fn execute_tools(...)
```

### 2️⃣ 工具和LLM

#### [Tool System](./tool-system.md) ⭐⭐⭐⭐
**400+ 行 | 复杂度: 高**

工具的完整管理框架。支持 40+ 预定义工具和无限扩展。

主要内容：
- ToolRegistry 单例模式和动态注册
- 9 大工具分类（Web、Files、Terminal、Vision、Browser、Code、Memory、Delegation、Utility）
- 工具执行器（并行/顺序、依赖解析、安全检查）
- 工具开发指南
- 安全性和速率限制

**支持的工具类别**:
- Web: web_search, web_extract
- Files: read_file, write_file, patch, search_files
- Terminal: 6 个后端（local, docker, ssh, modal, daytona, singularity）
- Vision: analyze, perception
- 等等...

#### [LLM Routing](./llm-routing.md) ⭐⭐⭐⭐
**380+ 行 | 复杂度: 高**

多提供商 LLM 支持的完整设计。

主要内容：
- 8+ 提供商支持（OpenAI、Anthropic、Claude、OpenRouter、GitHub Copilot、Kimi、MiniMax、DashScope、Deepseek）
- 提供商检测和优先级路由
- 故障转移链和动态切换
- 速率限制和指数退避
- 多模型同时支持

**核心特性**:
```rust
pub struct ProviderRouter
pub async fn route_request(...)
pub async fn switch_provider(...)
pub struct RateLimitManager
```

### 3️⃣ 计算和优化

#### [Prompt Builder](./prompt-builder.md) ⭐⭐⭐
**350+ 行 | 复杂度: 中等**

动态提示词构建框架。从系统配置和会话状态生成质量高的提示。

主要内容：
- Agent 定义构建（角色、能力、通信风格）
- 工具定义构建（OpenAI 和 Claude 格式支持）
- 会话上下文构建
- Few-shot 示例构建
- 输出格式规定

**组件**:
```rust
pub struct AgentDefinitionBuilder
pub struct ToolDefinitionBuilder
pub struct ConversationContextBuilder
pub struct ExampleBuilder
pub struct OutputFormatSpec
```

#### [Context Compression](./context-compression.md) ⭐⭐⭐⭐
**420+ 行 | 复杂度: 高**

自适应上下文压缩算法。在有限的 LLM 窗口内处理无限长对话。

主要内容：
- 上下文窗口管理和利用率追踪
- 三步压缩流程：修剪 → 保护头尾 → 总结中间
- 消息重要性评分
- LLM 总结生成
- 双层内存系统（内存 + 向量数据库）
- 压缩指标收集

**算法**:
修剪不重要消息 → 保留头部（用户意图）和尾部（最近） → 中间转为摘要

### 4️⃣ 数据和持久化

#### [Data Schema](./data-schema.md) ⭐⭐⭐
**320+ 行 | 复杂度: 中等**

完整的数据模型和数据库设计。

主要内容：
- 数据库表设计（Sessions、Messages、ToolCalls、CompressionEvents、Metrics）
- Rust 类型定义和 ORM 映射
- 数据访问层 (Repository 模式)
- 存储选项（PostgreSQL、Redis、Elasticsearch、S3、Weaviate）
- 数据库迁移策略

**核心表**:
- sessions: 会话信息、预算、性能指标
- messages: 消息历史
- tool_calls: 工具执行记录
- compression_events: 压缩事件日志
- execution_metrics: 执行指标（LLM 性能、工具耗时等）

### 5️⃣ 容错和恢复

#### [Error Handling](./error-handling.md) ⭐⭐⭐⭐
**380+ 行 | 复杂度: 高**

完整的错误分类、恢复策略和弹性设计。

主要内容：
- 四层错误分类（LLM、工具、上下文、系统）
- 3 类错误类型（可恢复、可转移、致命）
- 4 种恢复策略（重试、提供商转移、上下文压缩、模型降级）
- 熔断器模式
- 优雅降级
- 错误日志和分析

**恢复策略**:
1. **重试** - 指数退避（Fixed、Linear、Exponential、Jittered）
2. **提供商转移** - 尝试备用 LLM 提供商
3. **压缩** - 释放上下文空间
4. **降级** - 切换到更轻量的模型

### 6️⃣ 测试和评估

#### [Testing Strategy](./testing-strategy.md) ⭐⭐⭐⭐
**500+ 行 | 复杂度: 高**

完整的测试理论和实践指南。

主要内容：
- 单元、集成、E2E 测试分层
- Harness 框架设计原理
- 测试金字塔
- 代码与测试的协演

#### [Harness Testing](./harness-testing.md) ⭐⭐⭐⭐⭐
**520+ 行 | 复杂度: 极高**

完整的 Harness 测试框架实现。基于 OpenAI harness-engineering 最佳实践。

主要内容：
- Test Cases 定义（预定义套件：foundational, advanced, edge-cases）
- Runners（Local、Docker、Remote）
- 4 维度评估器（Correctness、Behavior、Performance、Reliability）
- A/B Testing & Comparison
- 报告生成（HTML、JSON）
- CI/CD 集成示例

**核心流程**:
```
Test Cases → Run (Local/Docker) → Evaluate (4 dimensions) → Compare (A/B) → Report
```

**4 维度评估**:
1. **Correctness** - 输出正确性（使用 LLM 评分）
2. **Behavior** - 工具使用是否符合预期
3. **Performance** - 执行时间、token 使用
4. **Reliability** - 是否无错误完成

## 💡 快速导航

### 按场景

| 我想... | 查看... |
|--------|--------|
| 理解 Agent 如何工作 | [Agent Orchestrator](./agent-orchestrator.md) |
| 添加新工具 | [Tool System](./tool-system.md) |
| 支持新 LLM 提供商 | [LLM Routing](./llm-routing.md) |
| 优化上下文使用 | [Context Compression](./context-compression.md) |
| 改进提示质量 | [Prompt Builder](./prompt-builder.md) |
| 编写测试 | [Harness Testing](./harness-testing.md) |
| 理解数据流 | [Data Schema](./data-schema.md) |
| 处理错误 | [Error Handling](./error-handling.md) |

### 按角色

**🏗️ 架构师**: [Agent Orchestrator](./agent-orchestrator.md) → [DESIGN.md](../../DESIGN.md)

**💻 后端开发**: [Tool System](./tool-system.md) → [Data Schema](./data-schema.md) → [Error Handling](./error-handling.md)

**🔬 LLM 工程师**: [Prompt Builder](./prompt-builder.md) → [LLM Routing](./llm-routing.md) → [Context Compression](./context-compression.md)

**✅ 测试/QA**: [Harness Testing](./harness-testing.md) → [Testing Strategy](./testing-strategy.md)

## 📊 文档统计

| 文档 | 行数 | 复杂度 | 关键指标 |
|------|------|--------|---------|
| agent-orchestrator.md | 450+ | ⭐⭐⭐⭐⭐ | 对话循环、三层预算、工具编排 |
| tool-system.md | 400+ | ⭐⭐⭐⭐ | 40+ 工具、9 大类、注册表 |
| llm-routing.md | 380+ | ⭐⭐⭐⭐ | 8+ 提供商、故障转移、速率限制 |
| context-compression.md | 420+ | ⭐⭐⭐⭐ | 自适应压缩、修剪-保护-总结、双层内存 |
| prompt-builder.md | 350+ | ⭐⭐⭐ | 模块化构建、多格式支持 |
| data-schema.md | 320+ | ⭐⭐⭐ | 5 核心表、ORM 映射、迁移 |
| error-handling.md | 380+ | ⭐⭐⭐⭐ | 4 层分类、3 异常类、4 恢复策略 |
| testing-strategy.md | 500+ | ⭐⭐⭐⭐ | 金字塔、原理、实践 |
| harness-testing.md | 520+ | ⭐⭐⭐⭐⭐ | Cases、Runners、评估器、A/B、CI |

**总计**: **3,700+ 行** 详细设计文档

## 🔗 关键文档链接

- 参考资料：[hermes-agent-analysis.md](../../references/hermes-agent-analysis.md) (14,000+ 字)
- 概念基础：[DESIGN.md](../../DESIGN.md)
- 快速开始：[DEVELOPER_GUIDE.md](../../DEVELOPER_GUIDE.md)
- 架构概览：[ARCHITECTURE.md](../../ARCHITECTURE.md)

## ✍️ 文档约定

所有设计文档遵循：

1. **开头概要** - 3 句话说明设计决策
2. **视觉图** - ASCII 架构图或 Mermaid 关系图
3. **Rust 代码** - 完整的实现参考
4. **权衡分析** - Why not X, why Y?
5. **Harness 集成** - 每个设计都包含测试示例
6. **超链接** - 相关文档互相引用

---

**版本**: 0.2.0 (Modularized) | **最后更新**: 2026-04-11  
**完整性**: 100% (所有核心模块已覆盖)  
**参考**: [docs/references/](../../references/) | [ARCHITECTURE.md](../../ARCHITECTURE.md)
