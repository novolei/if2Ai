# If2Ai 设计原则 (DESIGN.md)

> 本文档阐述 If2Ai 的核心设计哲学、约束和品味规范。它指导每一个架构决策。

## 🎯 核心设计理念

If2Ai 采用 **智能体优先的工程** (Agent-First Engineering) 思想，受 OpenAI 的 Codex Harness 经验启发：

### 1. **代码库就是记录系统** 📚

- 所有知识必须在代码库中版本化
- 外部的 Docs、Slack、Notion 中的知识对 Agent 不可见，因此不存在
- 清晰的文档层级和导航，避免信息过载
- 自动化工具检查文档新鲜度和完整性

### 2. **为智能体的可读性优化** 🤖

- 代码、架构和工具应该对 AI 易于理解
- 明确的约束体系 > 微观管理
- 强制规范（通过 linter）> 温和建议
- 结构化、可预测的代码模式

### 3. **清晰的边界和可组合性** 🧩

- 严格的分层架构（Types → Config → Repo → Service → Runtime → UI）
- 单向依赖流（只能"向前"）
- 显式的 Provider 注入点处理横切关注点
- 最小化隐式依赖和神奇行为

### 4. **可观测和可评估** 📊

- 完整的可观测性栈（日志、指标、追踪）
- Harness 框架用于测试、评估、对比、复现
- Agent 行为可度量和可验证
- 结构化日志便于 Agent 和人类理解

## 🏛️ 架构约束体系

### 强制约束 (Must Have)

#### 1. **分层依赖架构**

每个业务域内的依赖流向必须严格遵循：

```
Types (纯数据定义)
  ↓
Config (配置和常量)
  ↓
Repo (数据访问层)
  ↓
Providers (注入管道)
  ↓
Service (业务逻辑)
  ↓
Runtime (应用运行时)
  ↓
UI (用户界面)
```

**规则**:

- ✅ Service 可以依赖 Types, Config, Repo, Providers
- ✅ UI 可以依赖下层的所有东西
- ❌ 不能反向依赖（下层不依赖上层）
- ❌ 不能跨层依赖（比如 Config → Service 直接调用 UI）

**强制方式**: 自定义 linter + 结构化测试在 CI 中检查

#### 2. **Provider 模式强制**

所有外部依赖（LLM、存储、工具等）必须通过 Provider 注入：

```rust
// ✅ GOOD
pub struct AgentOrchestrator {
    providers: Arc<Providers>,
}

// ❌ BAD
pub struct AgentOrchestrator {
    llm_client: OpenAIClient,  // 直接依赖，不可测试
}
```

**原因**: 便于测试（mock providers）、配置切换、故障转移

#### 3. **类型边界处理**

数据只在系统边界处验证和解析（Parse, Don't Validate）：

```rust
// ✅ GOOD - 边界处验证
pub async fn create_session(req: Value) -> Result<Session> {
    let session = parse_session(req)?;  // 边界处解析
    self.orchestrator.run(session).await
}

// ❌ BAD - 随意验证
pub async fn create_session(req: Value) -> Result<()> {
    let session = Session { ... };  // 假设数据有效
    // 后续代码充满防御性检查
}
```

#### 4. **结构化日志和错误**

所有日志必须结构化，所有错误必须分类：

```rust
// ✅ GOOD
info!(
    message = "LLM call started",
    tool = "search",
    model = "gpt-4",
    tokens_used = 150,
);

// ❌ BAD
println!("Calling GPT-4 for search tool");
```

#### 5. **异步优先**

所有 I/O 操作使用 async/await，不允许阻塞：

```rust
// ✅ GOOD
pub async fn execute_tool(&self, tool: Tool) -> Result<Value> {
    self.tool_registry.run_async(tool).await
}

// ❌ BAD
pub fn execute_tool(&self, tool: Tool) -> Result<Value> {
    std::thread::block_on(async { ... })
}
```

### 规范但灵活的约束 (Should Have)

#### 1. **命名约定**

按照官方指南：

- **Rust**: `snake_case` 函数/变量，`PascalCase` 类型
- **Svelte**: `kebab-case` 文件名，`PascalCase` 组件
- **前缀约定**:
  - `new_` 用于构造函数
  - `with_` 用于 builder pattern
  - `handle_` 用于事件处理
  - `try_` 用于可能失败的操作

#### 2. **代码大小限制**

- 单个文件：< 500 行代码（包括注释）
- 单个函数：< 50 行（不包括文档）
- 单个模块：< 5 个责任

超过这些限制时，自动触发 CI 警告，提示进行重构。

#### 3. **测试覆盖率**

- **核心逻辑**: ≥ 80% 代码覆盖率
- **公开 API**: 100% 有 doc 注释和使用例
- **工具和命令**: 每个都需要至少 1 个集成测试

#### 4. **文档完备性**

每个公开函数/结构必须有：

- ✅ doc 注释
- ✅ 最少 1 个使用例
- ✅ 错误情况说明

## 📏 品味规范 (Taste Invariants)

这些是约束但也留有余地的品味规则：

### 1. **共享工具优于重实现**

```rust
// ✅ GOOD - 使用标准库或共享工具包
use our_utils::concurrent_map;

// ⚠️ 有时候重实现反而更好
// 如果库行为不透明或与我们的架构不匹配
// 但必须有清晰的理由注释
```

### 2. **显式优于隐式**

```rust
// ✅ GOOD
let future = agent.run_with_timeout(Duration::from_secs(30))?;

// ❌ AVOID
let future = agent.run()?;  // 暗含超时？
```

### 3. **值对象优于可变状态**

```rust
// ✅ GOOD - 不可变值流
let state1 = State::new();
let state2 = state1.with_message(msg);

// ⚠️ AVOID 当可能时
let mut state = State::new();
state.add_message(msg);
```

### 4. **单向数据流**

```rust
// ✅ GOOD - 清晰的数据流
Message → Agent → Tools → Results → Formatter → UI

// ❌ AVOID - 循环和多向流
Agent ←→ Tools ←→ Memory ←→ Agent
```

## 🔨 强制机制

### CI/CD 检查 (Automated)

| 检查项     | 工具          | 失败后果          |
| ---------- | ------------- | ----------------- |
| 分层依赖   | 自定义 linter | 阻止合并          |
| 文件大小   | cloc + 脚本   | 警告 + 阻止       |
| 覆盖率     | tarpaulin     | 如果 < 目标则阻止 |
| 代码质量   | clippy        | 警告              |
| 文档完备   | 自定义工具    | 如未文档则阻止    |
| 结构化日志 | 代码检查      | 警告              |

### 人工审查 (Manual)

PR 审查检查项：

- ✅ 是否遵循架构约束？
- ✅ 是否增加了可读性还是降低了？
- ✅ 有新的"神奇行为"吗？
- ✅ 测试是否一致和完整？

## 📋 设计决策框架

当面临架构选择时，使用这个框架：

### 1. **Agent 可读性优先**

> "这对 AI 来说是否易于理解？"

代码对 AI 的透明性甚至可能超过对人类开发者的美观性。

### 2. **可组合内胜于功能强大**

> "这是否可以组合成其他概念？"

选择小的、可组合的构建块，而不是大的、功能强大的但难以理解的抽象。

### 3. **显式边界内的自由**

> "边界清晰吗？边界内是否足够灵活？"

在系统层级强制执行边界，在本地层级允许自治。

### 4. **可测试性胜于聪慧**

> "我们能否轻松地为此编写测试？"

如果测试变得复杂，这通常表示设计有问题。

## 🧠 不同的人类品味规则

我们承认代码生成器的输出可能与人类品味不完全一致。这是可接受的，只要：

- ✅ 输出是正确的
- ✅ 输出是可维护的
- ✅ 对未来的 Agent 运行而言清晰易读
- ✅ 遵循所有强制约束

人类品味反馈通过以下方式进入系统：

1. PR 审查评论 → 编码到工具或文档
2. 重构 PR → 自动运行和应用
3. Bug 发现 → 更新规范和约束

## 📚 代码库作为记录系统

If2Ai 将代码库分为多个知识层，防止信息过载：

### 导航层 (Navigation)

- `AGENTS.md` - 100 行的内容目录
- `ARCHITECTURE.md` - 系统全景图
- `DESIGN.md` - 本文件（原则和决策）

### 设计层 (Design)

- `docs/design-docs/` - 具体设计决策（分主题）
- `docs/design-docs/index.md` - 设计文档导航
- 每个设计文档 ≤ 300 行，聚焦于单一决策

### 规范层 (Specification)

- `docs/product-specs/` - 功能规范和需求
- `docs/product-specs/index.md` - 功能导航

### 执行层 (Execution)

- `docs/exec-plans/active/` - 当前工作
- `docs/exec-plans/completed/` - 已完成工作
- 每个计划包含进度、决策日志、关键检查点

### 参考层 (Reference)

- `docs/references/` - LLM 参考资料
- `src/` 中的代码注释 - 实现细节
- 测试代码 - 使用例

### 生成层 (Generated)

- `docs/generated/` - 由工具自动生成的文档
- `README.md` - 自动生成的快速开始

## 🔄 文档维护周期

### 自动维护

- **doc-gardening bot**: 每周扫描并标记过时文档
- **CI 检查**: 验证交叉链接和完整性
- **linter**: 检查标记、结构和新鲜度

### 半自动维护

- **PR 检查**: 修改代码时要求更新相关文档
- **失败提示**: CI 失败时自动建议修复

### 手动维护

- **定期审查**: 团队每月审查一次关键文档
- **反馈融合**: 将评论反馈编码到文档中

## 🎓 新成员入职流程

1. **第一天**: 从 AGENTS.md 开始，浏览导航层
2. **第二天**: 阅读 ARCHITECTURE.md 和相关设计文档
3. **第三天**: 接手第一个 exec-plan 中的小任务
4. **第一周**: 完成一个完整的功能实现并通过 harness 测试

这个流程确保新成员能够快速（但不仓促地）理解系统。

## 📊 质量度量

### 代码质量

| 指标     | 目标       | 工具      |
| -------- | ---------- | --------- |
| 覆盖率   | ≥ 80%      | tarpaulin |
| 复杂度   | < 15       | clippy    |
| 重复代码 | < 3%       | dupli     |
| 代码异味 | 0 critical | rslint    |

### 文档质量

| 指标     | 目标                | 工具         |
| -------- | ------------------- | ------------ |
| 新鲜度   | < 2 周过期          | doc-gardener |
| 完整性   | 100% 公开 API       | 自定义       |
| 可链接性 | 0 断链              | 自定义       |
| 可读性   | Flesch-Kincaid ≤ 16 | readability  |

### Agent 行为质量

见 [harness/README.md](../harness/README.md)

## 🚀 未来演进方向

### 短期 (Phase 1-2)

- 建立基础架构和约束体系
- 实现 Harness 框架
- 自动化文档和质量检查

### 中期 (Phase 3-4)

- 扩展 Agent 自主性
- 实现完整的可观测性
- 建立更高级的评估指标

### 长期

- 支持多个 Agent 协作
- 学习和改进循环自动化
- 完全自主的代码库维护

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11

相关文档：

- [AGENTS.md](./AGENTS.md) - 项目导航
- [ARCHITECTURE.md](./ARCHITECTURE.md) - 系统架构
- [docs/design-docs/](./docs/design-docs/) - 具体设计
- [harness/README.md](../harness/README.md) - Harness 框架
