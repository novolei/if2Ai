# Hermes → If2Ai 映射与简化指南

**目的**: 理解 Hermes 的哪些部分被采用、简化或推迟到 Phase 2  
**目标用户**: 架构师、高级开发者  
**更新**: 2025年1月  

---

## 📊 功能映射表

### Agent Loop（核心）

| 功能 | Hermes | If2Ai Phase 1 | 简化内容 | 推迟到 Phase 2 |
|------|--------|--------------|--------|--------------|
| 基础循环 | ✅ 9200 行 | ✅ ~700 行 | 单提供商，无压缩 | 多提供商，压缩 |
| 消息历史 | ✅ OpenAI 格式 | ✅ OpenAI 格式 | 无 | — |
| 工具执行 | ✅ 并发 + 重试 | ✅ 顺序执行 | 无并发，<1 次重试 | 完整重试逻辑 |
| 中断支持 | ✅ 信号处理 | ⏳ 基础 | 仅 Tauri 事件 | 信号处理 |
| 预算压力 | ✅ 90 次迭代警告 | ⏳ 简单计数 | 无警告反馈 | 完整压力系统 |

**决定**：Phase 1 专注于 **<600 行的核心循环**，保持简单，后续逐步增强。

---

### 提供商支持（Provider Resolution）

| 功能 | Hermes | If2Ai P1 | If2Ai P2 计划 | 推迟 |
|------|--------|----------|------------|------|
| OpenAI | ✅ 18+ 提供商 | ✅ OpenAI 仅 | ✅ + Anthropic | Ollama, Z.AI |
| OAuth 流程 | ✅ 7 种 | ❌ 无 | ✅ 简单 OAuth | — |
| 凭证池 | ✅ 复杂轮换 | ✅ 单 API key | ✅ 池管理 | — |
| 降级 | ✅ 自动切换 | ❌ 无 | ✅ 手动选择 | 自动 |

**决定**：Phase 1 **仅 OpenAI**（最快上市），Phase 2 添加 Anthropic 和通用适配器。

---

### 工具系统（Tool System）

| 工具 | Hermes 数量 | If2Ai P1 | If2Ai P2 | 推迟 |
|------|-----------|----------|----------|------|
| Terminal | 1 | ✅ 1 | ✅ (6 后端) | — |
| File Ops | 3 | ✅ 3 | ✅ (patch) | — |
| Web | 3 | ✅ 2 (search+extract) | ✅ (browser) | crawl, screenshot |
| Vision | 2 | ❌ | ⏳ P2 | — |
| Code | 1 | ✅ 1 | ✅ (multiple langs) | — |
| MCP | ✅ 动态 | ❌ | ⏳ P2 | — |
| 总数 | 47+ | **6-8** | **15+** | **30+** |

**决定**：Phase 1 的 **"最小可用集"** = Terminal + File Ops + Web Search + Code Exec

---

### 会话持久化（Session Persistence）

| 功能 | Hermes | If2Ai P1 | If2Ai P2 | 推迟 |
|------|--------|----------|----------|------|
| SQLite 存储 | ✅ FTS5 全文 | ✅ 基础表 | ✅ FTS5 搜索 | — |
| 会话血统 | ✅ 压缩时链接 | ❌ 无 | ✅ 简单链接 | — |
| 多会话 | ✅ 隔离 | ✅ UUID 键 | ✅ 改进 | — |
| 原子写入 | ✅ 争用处理 | ✅ 基础 | ✅ 完整 | — |

**决定**：Phase 1 使用 **SQLite + UUID**，无 FTS，无复杂事务。

---

### 记忆系统（Memory）

| 功能 | Hermes | If2Ai P1 | If2Ai P2 | 推迟 |
|------|--------|----------|----------|------|
| SOUL.md | ✅ 系统身份 | ❌ 硬编码 | ✅ 文件加载 | — |
| MEMORY.md | ✅ 持久记忆 | ❌ 无 | ✅ 简单文件 | FTS 搜索 |
| USER.md | ✅ 用户建模(Honcho) | ❌ 无 | ⏳ 可选集成 | — |
| 技能 | ✅ 创建/改进循环 | ❌ 无 | ⏳ 人工管理 | 自动学习 |

**决定**：Phase 1 **无记忆系统**，Phase 2 添加基础 MEMORY.md，后续考虑 Honcho。

---

### 消息网关（Messaging Gateway）

| 功能 | Hermes | If2Ai P1 | If2Ai P2 | 推迟 |
|------|--------|----------|----------|------|
| 14+ 平台 | ✅ CLI/Telegram/Discord/等 | ❌ 仅 UI | ✅ 准备架构 | 实际集成 |
| 统一路由 | ✅ 会话管理 | ✅ Tauri IPC | ✅ 消息总线 | — |
| slash 命令 | ✅ 全平台 | ❌ | ✅ 简单命令 | — |
| 后台维护 | ✅ cron 等 | ❌ | ⏳ 可选 | — |

**决定**：Phase 1 **仅 Tauri UI**，Phase 2 设计网关实现，P3+ 逐步集成平台。

---

### 插件系统（Plugins）

| 功能 | Hermes | If2Ai P1 | If2Ai P2 | 推迟 |
|------|--------|----------|----------|------|
| 工具注册 | ✅ 发现 + 注册 | ✅ 硬编码注册 | ✅ 动态发现 | — |
| 记忆提供商 | ✅ Honcho, 自定义 | ❌ | ⏳ 接口定义 | 实现 |
| Context Engine | ✅ 压缩等 | ❌ | ⏳ 接口定义 | 实现 |
| Hook 系统 | ✅ pre/post tool | ❌ | ⏳ 事件发射 | 完整 hook |

**决定**：Phase 1 **无插件系统**，Phase 2 定义接口，P3+ 实现动态加载。

---

## 🎯 Phase 1 简化决策

### 优先级 1：**必须做**（影响核心功能）
1. ✅ Agent Loop（基础）
2. ✅ OpenAI Provider
3. ✅ Tool Registry + 6-8 工具
4. ✅ SQLite Session Store
5. ✅ Tauri UI + IPC

### 优先级 2：**应该做**（提升品质）
6. ✅ Logging/Tracing
7. ✅ 单元测试 ≥70%
8. ✅ API 文档注释
9. ⏳ 错误处理改进
10. ⏳ 性能优化

### 优先级 3：**可以延迟**（Phase 2+）
- 多提供商
- 记忆系统
- 插件系统
- Gateway 网关
- 上下文压缩
- 完整测试

---

## 🔄 Hermes 特性适应过程

### 过程 1：**直接采用**（90% 兼容）

这些 Hermes 特性可以直接适用于 If2Ai：

- **消息格式**: OpenAI 标准格式（role/content/tool_calls）
  ```rust
  // 直接复用 Hermes 的 Message 结构
  pub struct Message {
      pub role: String,
      pub content: Option<String>,
      pub tool_calls: Option<Vec<ToolCall>>,
  }
  ```

- **Tool 接口**: 基于 trait 的工具系统
  ```rust
  #[async_trait]
  pub trait Tool {
      async fn execute(&self, args: serde_json::Value) -> Result<String>;
      fn schema(&self) -> serde_json::Value;
      fn name(&self) -> &str;
  }
  ```

- **Session 数据库**: SQLite 键值结构
  ```rust
  // 表: messages(session_id, role, content, created_at)
  // 表: sessions(session_id, created_at, updated_at)
  ```

### 过程 2：**简化接口**（50% 功能，80% 用途）

这些复杂的 Hermes 特性进行了简化：

**Hermes Provider 系统**（18+ 提供商，OAuth，凭证池）
```rust
// Hermes 版本：复杂的提供商路由 + 凭证管理
pub struct ProviderRuntime { ... }  // 数百行

// If2Ai 简化版本：单提供商
pub struct OpenAIProvider {
    client: OpenAIClient,
}
```

**Hermes Context Compression**（LLM 总结，FTS 搜索）
```rust
// Hermes：完整的上下文管理引擎
pub struct ContextEngine { ... }  // 几百行

// If2Ai Phase 1：无压缩（直到超过 token 限制）
// Phase 2：添加简单总结
```

**Hermes Memory System**（自主学习，用户建模）
```rust
// Hermes：Honcho + SOUL.md + MEMORY.md + USER.md
struct MemorySystem { ... }

// If2Ai Phase 1：无记忆（只有会话历史）
// Phase 2：+ MEMORY.md (手动)
// Phase 3：+ Honcho 可选
```

### 过程 3：**推迟实现**（保留接口，后续填充）

这些 Hermes 特性暂时不实现，但预留接口：

```rust
// 在 Cargo.toml 中注释这些依赖
// [dependencies]
// async-openai = "0.14"     // ✅ Phase 1
// # anthropic = "0.17"       // ⏳ Phase 2
// # honcho = "0.1"           // ⏳ Phase 2
// # modal-client = "0.1"     // ⏳ Phase 3

// 在 providers 模块中定义接口
pub struct OpenAIProvider { ... }      // ✅ 实现
// pub struct AnthropicProvider { ... } // ⏳ TODO
// pub struct OllamaProvider { ... }    // ⏳ TODO

// 在 tools 模块中预留
pub trait Tool { ... }  // ✅ 实现
// pub mod mcp { ... }   // ⏳ MCP 工具
```

---

## 📈 功能实现时间线

```
Week 1-2: Core Agent Loop + Types
    ✅ AIAgent::run_conversation()
    ✅ Message types
    ✅ Basic OpenAI integration stub

Week 3-4: Full Integration
    ✅ OpenAI API complete
    ✅ 6-8 tools
    ✅ SQLite persistence
    ✅ Svelte UI

Week 4-5: Polish
    ✅ Testing
    ✅ Documentation
    ✅ Performance tuning

---

Phase 2 (Week 6-13): Advanced Features
    ✅ Anthropic provider
    ✅ Context compression
    ✅ MEMORY.md system
    ✅ Gateway architecture

Phase 3 (Week 14-20): Production Ready
    ✅ Plugin system
    ✅ Multi-platform gateway
    ✅ Training/evaluation
    ✅ Full test coverage
```

---

## 🎓 代码对标

### Hermes `run_agent.py` (~9200 行) vs If2Ai `agent.rs`

**Hermes 包含**：
- Provider resolution (OAuth, 凭证轮换)
- Context compression (LLM 总结，FTS)
- Plugin hooks (pre/post tool)
- Budget tracking (压力警告)
- Session lineage (压缩链接)
- Fallback models (多级降级)
- Prompt caching (Anthropic)
- Tool concurrent execution
- Interrupt handling (信号)

**If2Ai Phase 1 包含**：
- Basic Agent loop
- Single provider call
- Tool execution (1 个 at a time)
- Session storage (简单)
- Basic error handling

**代码量对比**：
```
Hermes:     9200 行
If2Ai P1:    ~700 行  (7% 的复杂度)
If2Ai P2:   ~1500 行  (16% 的复杂度)
If2Ai P3:   ~3000 行  (33% 的复杂度)

→ If2Ai 目标：简单但可扩展，而非立即完全功能
```

---

## ✅ 验证清单：Hermes 对齐

在提交代码前检查：

### 消息格式对齐
- [ ] Message 使用 role/content/tool_calls 格式
- [ ] Tool calls 支持 function.name + function.arguments
- [ ] Tool results 有 tool_call_id + content

### Tool 系统对齐
- [ ] 所有工具实现 async fn execute()
- [ ] 所有工具提供 JSON schema()
- [ ] Tool registry 支持动态注册

### Session 存储对齐
- [ ] SQLite 用于持久化
- [ ] Session by UUID（可重新加载）
- [ ] 消息按序保存和恢复

### Provider 接口对齐
- [ ] Provider::chat_completion() 返回 ChatCompletionResponse
- [ ] Response 包含 content + optional tool_calls
- [ ] 支持读取 OPENAI_API_KEY

### Error Handling 对齐
- [ ] 工具失败不会完全中止（返回错误结果）
- [ ] Provider 失败有清晰错误消息
- [ ] 所有错误可序列化为 String（返回给 UI）

---

## 🚀 成功标志

**Phase 1 完成后，你应该能够**：

1. 在 UI 中输入消息
2. Agent 调用 OpenAI API
3. 获取文本响应 **或** 工具调用
4. 如果有工具调用，执行工具并循环直到文本回应
5. 保存会话到 SQLite
6. 恢复会话并继续对话

**不需要能做的事**（Phase 2+）：
- ❌ 支持多个 LLM 提供商
- ❌ 上下文压缩
- ❌ 用户在多个平台（Telegram 等）交互
- ❌ 自主学习新技能
- ❌ 高级提示优化

---

## 💬 决策公开以供反馈

如果你认为这些简化/推迟的选择有问题：

1. **推迟太多？** → 可以提前实现部分 Phase 2 特性
2. **简化太过？** → 可以增加更多工具或特性
3. **顺序错误？** → 可以重新优先级排序

**告诉我调整**！目标是 **最有效地利用时间和资源**。

---

**下一步**：按照 `IMPLEMENTATION_PLAN.md` 开始第 1 周的实现！
