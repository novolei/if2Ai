# ADR-001: If2Ai Phase 1 核心架构决策

**日期**: 2025-01-（待签署）  
**作者**: AI Assistant (初稿)  
**状态**: 待审核  
**影响**: 整个 Phase 1 架构

---

## 问题陈述

如何为 If2Ai 设计一个 **最小可用产品 (MVP)**，能在 **4-6 周内交付**，同时保留 **扩展到完整 Hermes 功能的能力**？

### 约束条件

- ⏱️ 时间：4-6 周
- 👥 团队：1-2 名开发者
- 🎯 目标：可工作的桌面应用
- 📚 参考：Hermes Agent (45K+ 行)

### 目标

- ✅ 至少 70% 的代码覆盖率
- ✅ <2 秒的平均响应时间
- ✅ Harness 评估通过 ≥80%
- ✅ 为 Phase 2 留下清晰的扩展点

---

## 决策

### ADR-1.1：单一 LLM 提供商（Phase 1 仅 OpenAI）

**背景**：

- Hermes 支持 18+ 提供商（OAuth, 凭证池, 降级）
- 这增加了 ~500 行基础设施代码
- 时间成本：1-2 周

**选择**：

```
阶段 1: 仅 OpenAI（硬编码 API key）
阶段 2: + 一个其他提供商（Anthropic）
阶段 3: 完整提供商框架
```

**原因**：

1. **快速上市**: 减少初期架构复杂度
2. **学习机会**: 理解单提供商后再概括
3. **清晰的重构点**: 多提供商是清晰的 Phase 2 功能

**折中**：

- ❌ Phase 1 不支持 Anthropic/OpenRouter 等
- ✅ 架构允许后续轻松添加

**实现计划**：

```rust
// Phase 1
pub struct OpenAIProvider { ... }
let client = OpenAIProvider::new(api_key);

// Phase 2
pub trait Provider: Send + Sync { ... }
pub struct AnthropicProvider { ... }
pub enum ProviderRuntime { ... }
```

---

### ADR-1.2：核心 Agent 循环 < 700 行

**背景**：

- Hermes `run_agent.py` = 9200 行
- 包含：压缩，插件，降级，中断，缓存等
- 大多数都是可选的 Phase 2+ 特性

**选择**：

```
Agent Loop = 5 个清晰的步骤
1. 添加用户消息
2. 构建系统提示
3. 调用 LLM
4. 解析响应
5. 如果有工具调用→执行→回到 3，否则返回
```

**原因**：

1. **可理解性**: <700 行 = 一个文件，易于修改
2. **测试友好**: 单步功能容易单元测试
3. **性能**: 直线流程，无复杂路由
4. **渐进式增强**: 后续逐步添加高级特性

**关键简化**：

- ❌ 无并发工具执行（顺序）
- ❌ 无预算压力警告
- ❌ 无上下文压缩
- ❌ 无中断处理（除 Tauri events）
- ✅ 清晰的扩展点用于后续

**代码轮廓**：

```rust
impl AIAgent {
    pub async fn run_conversation(
        &self,
        user_message: String,
        history: &mut ConversationHistory,
    ) -> Result<AgentResponse> {
        // Step 1: Add user message
        // Step 2: Build system prompt
        // Step 3: Call OpenAI
        // Step 4: Parse response
        // Step 5: Handle tool calls (loop)
        // Return response
    }
}
```

---

### ADR-1.3：SQLite 作为主要存储（无 FTS, 无复杂事务）

**背景**：

- Hermes 使用：SQLite + FTS5（全文搜索）+ Lightning DB（可选）
- FTS5 需要额外配置和查询优化
- 完整的 ACID 事务在本地应用中不关键

**选择**：

```
Phase 1: 简单 SQLite（3 个表）
  - sessions: session_id, created_at, updated_at
  - messages: id, session_id, role, content, created_at
  - tools (可选): session_id, tool_name, result, created_at

Phase 2: 添加 FTS5 用于会话搜索

Phase 3: 可选添加 Honcho 跨设备同步
```

**原因**：

1. **可用性**: SQLite 足以满足单机应用
2. **实现简单**: 标准 SQL，无特殊配置
3. **足够好的性能**: 对于 <10K 条消息有效
4. **便于测试**: 清晰的表结构，易于 mock

**折合**：

- ❌ Phase 1 无全文搜索（简单的日期过滤）
- ✅ FTS5 是 Phase 2 清晰的升级路径

---

### ADR-1.4：8 个核心工具（Terminal, Files, Web, Code）

**背景**：

- Hermes 有 47+ 工具，覆盖广泛用例
- 每个工具 = ~100-200 行代码 + 集成测试
- Phase 1 不需要全部工具，只需代表性样本

**选择**：

```
优先级 1（必要）：
  1. terminal - shell 执行
  2. read_file - 文件读取
  3. write_file - 文件写入

优先级 2（推荐）：
  4. web_search - web 搜索
  5. web_extract - 提取网页内容
  6. execute_code - Python/Node.js

可选（如时间允许）：
  7. browse - 浏览器自动化
  8. vision - 图像分析
```

**原因**：

1. **代表性**: 涵盖不同类型的工具（命令、文件、Web、代码）
2. **学习效果**: 实现 3-4 个工具后，其他就明显了
3. **足够强大**: 处理 ~80% 的常见用例
4. **时间预算**: 8 个工具 = 3-4 天实现

**架构**：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(&self, args: serde_json::Value) -> Result<String>;
    fn schema(&self) -> serde_json::Value;  // OpenAI JSON Schema
    fn name(&self) -> &str;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}
```

**折合**：

- ❌ Phase 1 无浏览器工具（复杂，需 Playwright/Chromium）
- ❌ Phase 1 无视觉工具（需要多模态 API）
- ✅ 实现新工具是 ~30 分钟的机械任务

---

### ADR-1.5：Tauri + Svelte UI（无 Web 部署）

**背景**：

- 目标：桌面应用（Windows/Mac/Linux）
- 虚拟选项：Web UI, Electron, Flutter
- Tauri = 最小占用 + 原生性能

**选择**：

```
Phase 1: Tauri 2.0 + Svelte（本地 UI 仅）
  - 130MB 应用大小（vs Electron 300MB+）
  - 零依赖分发（原生 OS 框架）

Phase 2: 添加基础 Web 服务端（可选）

Phase 3: Gateway 多平台（Telegram, Discord 等）
```

**原因**：

1. **性能**: Rust 后端 + 原生 UI = 快速响应
2. **大小**: Tauri 很轻（相比 Electron）
3. **安全**: 不涉及网络通信（本地仅）
4. **学习**: Rust + Svelte = 现代 stack

**IPC 设计**：

```
UI (Svelte) --[Tauri Commands]--> Backend (Rust)
send_message(text) -> AgentResponse { messages, ... }
```

**折合**：

- ❌ Phase 1 无 Web UI（后续可添加）
- ❌ Phase 1 无移动应用（超出范围）

---

### ADR-1.6：Harness 框架进行评估，而非单元测试至上

**背景**：

- 传统的单元测试 ≥90% 覆盖对 AI 应用不太有意义
- AI 系统需要验证：**行为正确性**，不仅**函数逻辑**
- Hermes 使用自定义评估框架

**选择**：

```
Phase 1: Harness 评估 (4 个维度)
  1. 正确性: Agent 给出合理的回答
  2. 行为: Tool calls 正确执行
  3. 性能: 响应 <2 秒
  4. 可靠性: 相同输入给出一致结果

目标: ≥80% 通过所有维度

单元测试: ≥70% 代码覆盖
```

**原因**：

1. **相关性**: "代码的 95% pass" 不如 "能实际工作"
2. **学习**: 构建 Harness 框架本身很有教育意义
3. **参考**: 遵循 Hermes/OpenAI 的最佳实践
4. **完整性**: 单元 + Harness = 充分的信心

**示例（Harness 测试）**：

```python
def test_agent_correctness():
    """Agent 能回答数学问题"""
    prompt = "What is 15 + 27?"
    response = agent.run(prompt)

    assert "42" in response.text
    assert response.iterations <= 3  # 不应超过 1-3 个循环
```

**折合**：

- ❌ Phase 1 不追求 95% 单元测试覆盖
- ✅ Phase 1 追求 80% Harness 通过率

---

### ADR-1.7：分层文档（AGENTS.md + docs/）

**背景**：

- Hermes 文档很好但也很长（50+ 页）
- If2Ai 项目新人需要快速定向
- 过多信息 = 信息超载

**选择**：

```
级别 0: README.md（1 页快速开始）
级别 1: AGENTS.md（导航 + 概览，100 行）
级别 2: ARCHITECTURE.md（全景，200 行）
级别 3: docs/design-docs/（详细设计，每个 2-3 KB）
级别 4: docs/product-specs/（功能规范）
级别 5: 代码注释 + Rust doc
```

**原因**：

1. **渐进式**: 新人可以从 AGENTS.md 开始，逐步深入
2. **维护**: 分散的文档最新速度快
3. **可导航**: 明确的跳转路径，非线性阅读
4. **可索引**: 每个文档有明确的用途和受众

**对标 Hermes**:

- ✅ 类似的 GitHub-based 文档方式
- ✅ 异步优先（非实时沟通）

---

### ADR-1.8：Rust + async/await（不是纯 Python）

**背景**：

- Hermes 是纯 Python（9200 行）
- If2Ai 选择 Rust（按照项目规范）
- 权衡：性能 vs 学习曲线

**选择**：

```
后端: Rust + Tokio（异步）
前端: Svelte + TypeScript
IPC: Tauri 命令（中间层）
```

**原因**：

1. **项目约定**: 项目团队已经精通 Rust
2. **性能**: Rust 在 API 调用和文件 I/O 上快得多
3. **安全**: 编译时内存安全 vs 运行时检查
4. **可部署**: 无需 Python 运行时

**折合**：

- ❌ 开发速度较慢（vs Python）
- ✅ 生成的二进制快很多
- ✅ 无依赖地狱

**异步设计**：

```rust
pub async fn run_conversation(&self, ...) -> Result<AgentResponse> {
    // 所有 I/O 是非阻塞的
    // API 调用，文件读取，工具执行都是 async
    // Tokio 运行时管理任务
}
```

---

## 推翻假设（如果它们被证明错误）

以下假设支持此设计。如果其中任何一个被推翻，ADR 可能需要修订：

| 假设                      | 如果错误的后果           | 复查条件                             |
| ------------------------- | ------------------------ | ------------------------------------ |
| OpenAI API 足以用于 P1    | 需要更早地添加 Anthropic | 如果用户不能使用 OpenAI（费用/访问） |
| <700 行循环足够           | 需要更复杂的设计         | 如果循环>1000 行导致维护问题         |
| SQLite 足够               | 需要更高级的存储         | 如果处理 >100K 条消息时性能下降      |
| 8 个工具代表              | 用户需要特定的工具       | 如果 >50% 请求失败因无工具           |
| Tauri UI 足够             | 需要 Web 前端            | 如果用户要求多平台访问               |
| Harness ≥80% vs Unit ≥95% | 需要更严格的测试         | 如果生产中出现出乎意料的 bug         |

---

## 替代方案考虑

### 替代 A：Hermes 的完全克隆（Rust）

**pros**：完全功能，参考代码清晰  
**cons**：30-40 周时间，过度设计，维护负担  
**为什么不用**：不现实的时间表

### 替代 B：完全简化（仅聊天，无工具）

**pros**：4 周可完成  
**cons**：无法演示 Agent 的核心价值（工具使用）  
**为什么不用**：无法展示 Agent 全力量

### 替代 C：Python 后端 + Tauri 前端（"混合")

**pros**：开发快速（Python）+ 原生 UI  
**cons**：打包复杂，Python 运行时依赖  
**为什么不用**：团队已深谙 Rust，这避免了额外学习

### 选定方案：**精简 Rust + 清晰扩展点** ✅

在 **可行性** 和 **完整性** 之间找到甜点

---

## 成功标准

此 ADR 成功，如果在第 4 周末我们有：

1. ✅ 完整的 Agent 循环（<700 行）
2. ✅ 6+ 个工作工具
3. ✅ SQLite 持久化
4. ✅ Tauri UI 可聊天
5. ✅ Harness 评估 ≥80%
6. ✅ 清晰的 Phase 2 扩展路径
7. ✅ 完整的代码文档
8. ✅ CLI 和 UI 都能运行

---

## 反馈和修订

**待审核方**:

- [ ] @project-lead：时间表现实吗？
- [ ] @rust-expert：设计模式好吗？
- [ ] @product-owner：功能集充足吗？
- [ ] @qa：测试策略合理吗？

**反馈截止**: [TBD]  
**期望状态变化**: Approved / In Review / Needs Changes

---

## 历史记录

| 日期       | 状态      | 变更               | 作者         |
| ---------- | --------- | ------------------ | ------------ |
| 2025-01-XX | Draft     | Initial creation   | AI Assistant |
| [TBD]      | In Review | Team feedback      | —            |
| [TBD]      | Approved  | Ready to implement | Team         |

---

**结论**：
这个 ADR 提出了一个**激进但现实的** Phase 1 项目范围。通过**有意的简化**和**清晰的扩展点**，我们可以在 4-6 周内交付一个有效的最小可用产品，同时为完整的 Hermes 克隆投资未来。

---

## 附录：相关文件

- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) - 周级细节
- [HERMES_MAPPING.md](./HERMES_MAPPING.md) - Hermes 特性适应
- [QUICKSTART.md](./QUICKSTART.md) - 开发者快速开始
- [ARCHITECTURE.md](./ARCHITECTURE.md) - 系统全景
- [DESIGN.md](./DESIGN.md) - 设计原则
