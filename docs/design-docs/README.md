# If2Ai 设计文档库

> **唯一真相来源**：所有架构决策都在这里。代码必须与文档保持一致。  
> **最后更新**: 2026-04-11 | **技术栈**: Tauri 2 + Rust + React + shadcn/ui

---

## 阅读路径

### 新人入门（30 分钟）

| 顺序 | 文档                                                                           | 时间   | 获得什么                                   |
| ---- | ------------------------------------------------------------------------------ | ------ | ------------------------------------------ |
| 1    | [system-architecture-framework.md](./system-architecture-framework.md)         | 15 min | 全局视角：9 个子系统、分阶段路线图、数据流 |
| 2    | [module-boundaries-and-integration.md](./module-boundaries-and-integration.md) | 10 min | 每个 Rust 模块的职责与边界                 |
| 3    | [entry-points-design.md](./entry-points-design.md)                             | 10 min | 三个入口点：Tauri / API Server / IDE       |

### 按功能领域选读

| 你要做的          | 首先读                                                   | 然后读                                             |
| ----------------- | -------------------------------------------------------- | -------------------------------------------------- |
| Agent 对话循环    | [agent-loop.md](./agent-loop.md)                         | [agent-orchestrator.md](./agent-orchestrator.md)   |
| 接入 / 切换 LLM   | [provider-resolution.md](./provider-resolution.md)       | [llm-routing.md](./llm-routing.md)                 |
| 构建 / 扩展工具   | [tool-system.md](./tool-system.md)                       | [error-handling.md](./error-handling.md)           |
| 提示词工程        | [prompt-builder.md](./prompt-builder.md)                 | [context-compression.md](./context-compression.md) |
| 会话与数据持久化  | [session-persistence.md](./session-persistence.md)       | [data-schema.md](./data-schema.md)                 |
| 多平台消息网关    | [messaging-gateway.md](./messaging-gateway.md)           | —                                                  |
| 记忆系统 / Honcho | [memory-system.md](./memory-system.md)                   | —                                                  |
| Agent RL 自我进化 | [agent-self-improvement.md](./agent-self-improvement.md) | —                                                  |
| 测试与评估        | [testing-strategy.md](./testing-strategy.md)             | [harness-testing.md](./harness-testing.md)         |

---

## 文档全览

### 第一层：系统架构（必读基础）

这三份文档构成整个系统的认知地图，**所有其他文档都基于这个框架**。

```
system-architecture-framework.md
  ├─ Hermes 9 子系统 → If2Ai 模块的完整映射
  ├─ Phase 1 → 2 → 3 的分阶段路线图
  └─ 系统全局数据流图

module-boundaries-and-integration.md
  ├─ 6 个 Rust 核心模块的边界和职责
  ├─ AppState 的共享状态设计
  └─ Tauri Commands 作为 IPC 网关

entry-points-design.md
  ├─ Phase 1：Tauri 桌面入口（当前实现）
  ├─ Phase 2：JSON-RPC API Server
  └─ Phase 3：IDE / LSP 集成
```

### 第二层：核心子系统（Phase 1）

| 文档                                               | 职责                         | 状态    | 对标 Hermes                |
| -------------------------------------------------- | ---------------------------- | ------- | -------------------------- |
| [agent-loop.md](./agent-loop.md)                   | 对话循环、工具调用、预算管理 | ✅ 完成 | run_agent.py (9.2K)        |
| [agent-orchestrator.md](./agent-orchestrator.md)   | Orchestrator 核心协调引擎    | ✅ 完成 | AIAgent 类 (3.6K)          |
| [provider-resolution.md](./provider-resolution.md) | LLM 提供商解析与路由         | ✅ 完成 | runtime_provider.py (1.2K) |
| [llm-routing.md](./llm-routing.md)                 | 多提供商故障转移与动态切换   | ✅ 完成 | 8+ providers               |
| [tool-system.md](./tool-system.md)                 | 工具注册、执行、沙箱         | ✅ 完成 | tool_registry.py (0.8K)    |
| [prompt-builder.md](./prompt-builder.md)           | 系统提示动态构建             | ✅ 完成 | prompt_builder.py (0.5K)   |
| [session-persistence.md](./session-persistence.md) | 会话历史持久化               | ✅ 完成 | hermes_state.py (0.6K)     |
| [data-schema.md](./data-schema.md)                 | 数据模型与数据库 Schema      | ✅ 完成 | —                          |
| [context-compression.md](./context-compression.md) | 长对话自适应压缩             | ✅ 完成 | compact.rs                 |
| [error-handling.md](./error-handling.md)           | 错误分类与恢复策略           | ✅ 完成 | —                          |

### 第三层：高级特性（Phase 2）

| 文档                                                     | 职责                                       | 状态        |
| -------------------------------------------------------- | ------------------------------------------ | ----------- |
| [messaging-gateway.md](./messaging-gateway.md)           | Telegram / Discord / Slack 等 15+ 平台接入 | ✅ 设计完成 |
| [memory-system.md](./memory-system.md)                   | Honcho AI 用户建模 + 8 种记忆提供商        | ✅ 设计完成 |
| [agent-self-improvement.md](./agent-self-improvement.md) | RL-Training / GRPO 自我进化                | ✅ 设计完成 |

### 第四层：测试与质量

| 文档                                         | 职责                               | 状态    |
| -------------------------------------------- | ---------------------------------- | ------- |
| [testing-strategy.md](./testing-strategy.md) | 单元 / 集成 / E2E / Harness 全策略 | ✅ 完成 |
| [harness-testing.md](./harness-testing.md)   | Harness 评估框架详细流程           | ✅ 完成 |

---

## 模块与文档对应关系

```
src-tauri/src/modules/
├── runtime/          ← agent-loop.md + agent-orchestrator.md
├── api/              ← provider-resolution.md + llm-routing.md
├── tools/            ← tool-system.md
├── commands/         ← entry-points-design.md（Tauri IPC 层）
└── plugins/          ← messaging-gateway.md（Phase 2）

src/ (React 前端)
└── App.tsx + components/  ← entry-points-design.md（Tauri UI 部分）
```

---

## 系统与 Hermes 的对标

| If2Ai 模块          | Hermes 组件           | 设计文档                             |
| ------------------- | --------------------- | ------------------------------------ |
| `modules/runtime/`  | `run_agent.py`        | agent-loop.md                        |
| `modules/api/`      | `runtime_provider.py` | provider-resolution.md               |
| `modules/tools/`    | `tool_registry.py`    | tool-system.md                       |
| `modules/commands/` | Tauri IPC 网关        | module-boundaries-and-integration.md |
| `src/` (React UI)   | 无（Hermes 无 GUI）   | entry-points-design.md               |
| 待建 API Server     | `server.py`           | entry-points-design.md（Phase 2）    |

---

## 文档规范

每份设计文档都应包含：

1. **版本号 + 最后更新时间** — 头部元信息
2. **一句话定位** — 说清楚这个模块解决什么问题
3. **核心数据结构** — 关键的 Rust struct / enum
4. **业务流程图** — ASCII 或 Mermaid 图
5. **与其他模块的接口** — 依赖什么、暴露什么
6. **与 Hermes 的对标说明** — 映射关系
