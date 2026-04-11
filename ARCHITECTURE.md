# If2Ai 架构全景 (ARCHITECTURE.md)

> **架构就像城市规划**: 清晰的区域划分、明确的交通规则、易于导航。本文档提供俯视图。详细设计见 `docs/design-docs/`。

## 🏗️ 系统架构概览

```
┌─────────────────────────────────────────────────────┐
│                      User Interface (Svelte)        │
│           Chat UI | Agent Dashboard | Settings       │
└────────────────────┬────────────────────────────────┘
                     │ Tauri IPC Commands
                     ▼
┌─────────────────────────────────────────────────────┐
│              Application Server (Rust/Tokio)        │
│  ┌──────────────────────────────────────────────┐   │
│  │   Agent Orchestrator (核心协调引擎)           │   │
│  └──────────────────────────────────────────────┘   │
│                     │                                │
│    ┌────┬─────────┼─────────┬────────┬────────┐   │
│    ▼    ▼         ▼         ▼        ▼        ▼   │
│  Tool Prompt Context  Memory  LLM  Budget    │   │
│  System Builder Compressor Storage Manager Tracker │
└─────────────────────────────────────────────────────┘
        │              │              │
        ▼              ▼              ▼
┌──────────────┐ ┌────────────┐ ┌──────────┐
│ Tool/Skill   │ │LLM Providers│ │ Storage  │
│ Registry     │ │(OpenAI,etc)│ │(SQLite,  │
│              │ │             │ │ Vector DB)
└──────────────┘ └────────────┘ └──────────┘
```

## 📦 核心模块详解

### 1. **Agent Orchestrator** (核心)
负责整个 Agent 生命周期：
- 任务分解和规划
- 工具调用和执行
- 对话循环管理
- 状态和进度跟踪

**位置**: `src-tauri/src/agent/orchestrator.rs`

### 2. **Tool System** (工具系统)
动态工具加载和执行：
- 工具注册表（Tool Registry）
- 工具分类和索引
- 并行执行管理
- 工具依赖解析

**位置**: `src-tauri/src/modules/tools/`

### 3. **Prompt Builder** (提示词构建)
动态生成优化的提示词：
- 系统提示构建
- 上下文打包
- Token 预计算
- 版本管理

**位置**: `src-tauri/src/modules/prompt/`

### 4. **Context Compressor** (上下文压缩)
自动处理上下文窗口：
- 触发器管理（在 50% 时触发）
- 智能摘要而非简单截断
- 保护头部和尾部信息
- 增量更新支持

**位置**: `src-tauri/src/modules/context/`

### 5. **Memory System** (记忆系统)
双层记忆管理：
- Built-in 内存（文件系统）
- 可选外部记忆（如 Honcho）
- 会话管理
- 消息历史

**位置**: `src-tauri/src/modules/memory/`

### 6. **LLM Provider Router** (LLM 路由)
多提供商支持与故障转移：
- OpenAI / Claude / 开源模型
- API 模式（完成 / 流）
- 动态提供商切换
- 超时和重试策略

**位置**: `src-tauri/src/modules/llm/`

### 7. **Budget Tracker** (预算追踪)
分层资源管理：
- 迭代预算（最大迭代次数）
- 上下文预算（最大 tokens）
- 令牌预算（最大成本控制）
- 实时监控和告警

**位置**: `src-tauri/src/modules/budget/`

### 8. **Tauri Command Handler** (UI 桥接)
前后端通信：
- 命令定义和路由
- 错误序列化
- 事件流式传输
- 会话管理

**位置**: `src-tauri/src/commands/`

## 🔄 数据流和工作流

### Agent 对话循环 (Conversation Loop)

```
1. 接收用户输入
   │
2. 构建提示词 → 包含工具定义、上下文、历史
   │  
3. 调用 LLM
   │  ├─ 流式接收响应
   │  └─ 实时发送到前端
   │
4. 解析 LLM 输出
   ├─ 工具调用？ → 执行工具 → 获取结果
   ├─ 思考？ → 添加到对话
   └─ 完成？ → 返回最终答案
   │
5. 管理上下文
   ├─ 上下文 > 50%？ → 触发压缩
   └─ 更新会话记忆
   │
6. 重复（直到完成或达到迭代限制）
```

### 工具执行流 (Tool Execution)

```
1. 识别所需工具 (从 LLM 输出)
   │
2. 类型验证和参数处理
   │
3. 依赖检查 (工具间顺序)
   │
4. 并行执行 (最多 8 个 worker)
   │
5. 结果聚合和格式化
   │
6. 错误处理和重试
```

## 🗄️ 数据模型

### 核心实体

**Session** (会话)
```rust
pub struct Session {
    id: String,
    user_id: String,
    created_at: DateTime,
    messages: Vec<Message>,  // 完整历史
    state: SessionState,     // Active, Paused, Completed
}
```

**Message** (消息)
```rust
pub struct Message {
    id: String,
    session_id: String,
    role: MessageRole,  // User, Assistant, System
    content: String,
    tools_used: Vec<String>,
    tokens: TokenCount,
}
```

**Tool** (工具)
```rust
pub struct Tool {
    name: String,
    description: String,
    input_schema: JsonSchema,
    output_schema: JsonSchema,
    dependencies: Vec<String>,
    handlers: ToolHandler,
}
```

详见 [docs/design-docs/data-schema.md](./docs/design-docs/data-schema.md)

## 🎯 架构设计原则

### 1. **分层依赖**
```
Types (数据定义) ↓
Config (配置管理) ↓  
Repo (数据访问) ↓
Providers (注入点) → Service (业务逻辑) ↓
Runtime (运行时) ↓
UI (用户界面)
```

箭头方向明确，不允许逆向引用或跨层绕过。

### 2. **Provider 模式**
```rust
pub struct Providers {
    llm: Box<dyn LLMProvider>,
    memory: Box<dyn MemoryProvider>,
    tools: Arc<ToolRegistry>,
    config: ConfigManager,
}
```

所有外部依赖通过 `Providers` 注入，确保可测试性。

### 3. **类型安全**
- 使用 Rust 类型系统防止不合适的值
- JSON 在边界处解析和验证
- 结构化错误类型

### 4. **异步优先**
- 所有 I/O 使用 async/await（Tokio）
- 流式处理 LLM 输出
- 并行工具执行

## 📊 模块依赖图

```
UI (Svelte)
  │
  └─ Commands Handler
      │
      ├─ Agent Orchestrator (核心)
      │  ├─ Prompt Builder
      │  ├─ Tool System
      │  ├─ LLM Router
      │  ├─ Context Compressor
      │  ├─ Memory Manager
      │  └─ Budget Tracker
      │
      ├─ Config Manager
      └─ Observability Stack
```

**重要**: 依赖必须向下流动，不允许跨级调用。

## 🔌 扩展点

### 添加新的 LLM 提供商
1. 实现 `LLMProvider` trait
2. 在 `src-tauri/src/modules/llm/providers/` 中创建新文件
3. 注册到 `LLMRouter`
4. 添加配置选项

### 添加新工具
1. 实现 `Tool` 结构
2. 在 `src-tauri/src/modules/tools/` 中创建
3. 将其添加到 `ToolRegistry`
4. 定义 JSON schema

### 添加新的存储后端
1. 实现 `MemoryProvider` trait
2. 在 `src-tauri/src/modules/memory/providers/` 中创建
3. 注册到 `MemoryManager`

详见 [docs/design-docs/extension-points.md](./docs/design-docs/extension-points.md)

## 🧪 测试架构

完整的测试框架（Harness）位于 `harness/` 目录：
- **Unit tests**: 单个模块测试
- **Integration tests**: 跨模块交互
- **E2E tests**: 完整工作流
- **Evaluation tests**: Agent 行为评估

详见 [harness/README.md](../harness/README.md)

## 📈 性能特性

| 特性 | 目标 | 当前 |
|------|------|------|
| 首次响应延迟 | <100ms | - |
| 工具执行 | 并行 8 个 | - |
| 上下文压缩 | 50% 触发 | - |
| 内存占用 | <500MB | - |
| 吞吐量 | 100+ 并发 | - |

## 🚨 错误处理策略

系统采用分类错误处理：

```
RateLimitError → 使用备用提供商 → 重试
ContextWindowError → 触发上下文压缩 → 重试
ToolExecutionError → 记录错误 → 继续或让用户决定
ValidationError → 修复数据 → 重试
```

详见 [docs/design-docs/error-handling.md](./docs/design-docs/error-handling.md)

## 📚 相关文档

- [DESIGN.md](./DESIGN.md) - 设计原则和哲学
- [docs/design-docs/](./docs/design-docs/) - 具体设计决策
- [docs/product-specs/](./docs/product-specs/) - 功能规范
- [harness/README.md](../harness/README.md) - 测试框架

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11
