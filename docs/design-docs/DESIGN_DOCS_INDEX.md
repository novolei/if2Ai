# Design Docs Navigation & Overview

**版本**: 1.0  
**最后更新**: 2026-04-11  
**目的**: 统一导航和理解 If2Ai 设计文档中的所有模块和概念

---

## 📚 文档地图

### 基础文档
| 文档 | 用途 | 对标 | 完成度 |
|-----|------|------|------|
| [README.md](./README.md) | 设计文档概述 | - | ✅ |
| [index.md](./index.md) | 快速导航 | - | ✅ |

### 核心架构（必读）
| 文档 | 系统 | Hermes 行数 | If2Ai 文档 | 完成度 |
|-----|------|-----------|----------|------|
| [agent-loop.md](./agent-loop.md) | Agent 运行循环 | 9,200 | 430 | ✅ |
| [provider-resolution.md](./provider-resolution.md) | 提供商解析和路由 | 1,200 | 800 | ✅ |
| [session-persistence.md](./session-persistence.md) | 会话存储和历史 | 600 | 450 | ✅ |
| [tool-system.md](./tool-system.md) | 工具注册和执行 | 800 | 200+ | ✅ 存在 |
| [prompt-builder.md](./prompt-builder.md) | 提示构建和工程 | 500 | - | 📋 计划 |

### 高级特性
| 文档 | 系统 | Hermes 行数 | If2Ai 文档 | 完成度 |
|-----|------|-----------|----------|------|
| [messaging-gateway.md](./messaging-gateway.md) | 多平台消息网关 (Phase 2) | 1,200 | 800 | ✅ 设计完成 |
| [agent-self-improvement.md](./agent-self-improvement.md) | RL-Training 和自我进化 (Phase 2) | 2,500 | 1,200 | ✅ 设计完成 |
| [memory-system.md](./memory-system.md) | 记忆系统 + Honcho (Phase 2) | 平衡 | 1,400+ | ✅ 设计完成 |
| [context-compression.md](./context-compression.md) | 上下文压缩 | 400 | - | 📋 计划 |
| [plugin-architecture.md](./plugin-architecture.md) | 插件系统 | 300 | - | 📋 计划 |
| [mcp-integration.md](./mcp-integration.md) | Model Context Protocol | 400 | - | 📋 计划 |

### 支持文档
| 文档 | 用途 | 完成度 |
|-----|------|------|
| [error-handling.md](./error-handling.md) | 统一错误处理 | 📋 计划 |
| [testing-strategy.md](./testing-strategy.md) | Harness 集成测试 | 📋 计划 |
| [data-schema.md](./data-schema.md) | 数据结构和序列化 | 📋 计划 |
| [harness-testing.md](./harness-testing.md) | 评估框架集成 | 📋 计划 |

---

## ⚡ 源码基础完整包 (New!)

> **新增三份核心文档**: 定义 /rust → src-tauri 的源码迁移战略

| 文档 | 用途 | 优先级 |
|-----|------|------|
| [../CODE_FOUNDATION_COMPLETE_PACKAGE.md](../CODE_FOUNDATION_COMPLETE_PACKAGE.md) | 📦 **完整包总览** - 75 分钟快速入门 | **⭐ P0** |
| [../CODE_FOUNDATION_STRATEGY.md](../CODE_FOUNDATION_STRATEGY.md) | 🎯 源码战略定义 + 三角关系 | **⭐ P0** |
| [../CODE_MIGRATION_EXECUTION.md](../CODE_MIGRATION_EXECUTION.md) | 🚀 10 步迁移执行指南 (8-10 小时) | **⭐ P0** |
| [../CONSISTENCY_VERIFICATION_REPORT.md](../CONSISTENCY_VERIFICATION_REPORT.md) | ✅ 一致性验证 + 零冲突证明 | **⭐ P0** |

**推荐阅读顺序**:
1. CODE_FOUNDATION_COMPLETE_PACKAGE.md (75 min → 全景理解)
2. CODE_FOUNDATION_STRATEGY.md (15 min → 战略意图)
3. CONSISTENCY_VERIFICATION_REPORT.md (20 min → 验证确认)
4. CODE_MIGRATION_EXECUTION.md (30 min → 操作准备)
5. 然后开始 10 步迁移

---

## 🔄 文档依赖关系

```
┌─────────────────────────────────────────────────┐
│  整体系统架构 (ARCHITECTURE.md + DESIGN.md)      │
└────────────────┬────────────────────────────────┘
                 │
        ┌────────┴────────┐
        ▼                 ▼
  ┌──────────────┐  ┌────────────────┐
  │ Agent Loop   │  │ Provider       │
  │ (430 lines)  │  │ Resolution     │
  │              │  │ (800 lines)    │
  └────┬─────────┘  └────────┬───────┘
       │                     │
       │   ┌────────────┐   │
       └──►│ Tool       ◄───┘
           │ System     │
           │(existing)  │
           └──┬──────────┘
              │
       ┌──────┴──────┬──────────┬──────────────┐
       ▼             ▼          ▼              ▼
  ┌─────────┐  ┌──────────┐ ┌──────────┐  ┌──────────────┐
  │Session  │  │Prompt    │ │Memory    │  │RL-Training & │
  │Persist  │  │Builder   │ │System    │  │ Self-Improve │
  │(450 ln) │  │(pending) │ │(pending) │  │ (1200 ln) ✨ │
  └────┬────┘  └────┬─────┘ └────┬─────┘  └──────┬───────┘
       │           │            │               │
       └───────────┬────────────┴───────────────┘
                   ▼
          ┌──────────────────┐
          │Error Handling &  │
          │Testing Strategy  │
          └──────────────────┘
               │          │
        ┌──────┘          └──────┐
        ▼                        ▼
   ┌─────────┐          ┌──────────────┐
   │Plugin   │          │Context       │
   │Arch     │          │Compression   │
   │(pending)│          │(pending)     │
   └─────────┘          └──────────────┘
          ▲                      ▲
          │                      │
          └──────────┬───────────┘
                     ▼
          ┌──────────────────┐
          │Messaging         │
          │Gateway           │
          │(800 ln) ✨       │
          └──────────────────┘
```

---

## 🎯 按用途快速导航

### 对我有什么帮助？

#### "我要理解整个系统"
1. 阅读 [ARCHITECTURE.md](../ARCHITECTURE.md) - 5分钟
2. 阅读 [DESIGN.md](../DESIGN.md) - 10分钟
3. 按顺序阅读：Agent Loop → Provider → Session Persistence

#### "我要修改 Agent 循环"
- 主文档：[agent-loop.md](./agent-loop.md)
- 代码位置：`crates/runtime/src/conversation.rs`
- 相关：[tool-system.md](./tool-system.md)、[prompt-builder.md](./prompt-builder.md)

#### "我要添加新的 LLM 提供商"
- 主文档：[provider-resolution.md](./provider-resolution.md)
- 代码位置：`crates/api/src/providers/`
- 例如：`openai.rs`、`anthropic.rs`

#### "我要修改工具系统"
- 主文档：[tool-system.md](./tool-system.md)
- 代码位置：`crates/tools/src/`

#### "我要优化成本或性能"
- 会话优化：[session-persistence.md](./session-persistence.md)
- 上下文优化：[context-compression.md](./context-compression.md)（待）
- 提示优化：[prompt-builder.md](./prompt-builder.md)（待）

#### "我要添加新特性（Memory、Plugin 等）"
- 记忆系统：[memory-system.md](./memory-system.md)（✅ 完成）- 8 个提供商 + Honcho AI-native 用户建模
- 自我进化：[agent-self-improvement.md](./agent-self-improvement.md)（✅ 完成）- RL-Training + GRPO
- 消息网关：[messaging-gateway.md](./messaging-gateway.md)（✅ 完成）- 15+ 平台
- 插件系统：[plugin-architecture.md](./plugin-architecture.md)（待）

---

## 📋 Hermes 功能映射表

这个表展示了 Hermes 的每个核心功能在 If2Ai 中的对应位置：

| Hermes 系统 | 关键功能 | Hermes 行数 | If2Ai 设计文档 | 代码位置 | 完成度 |
|-----------|---------|----------|-------------|--------|------|
| **Agent Loop** | turn 执行 | 9,200 | agent-loop.md | runtime/conversation.rs | ✅ 80% |
| | 消息格式 | | agent-loop.md | api/src/types.rs | ✅ 100% |
| | 工具调用 | | agent-loop.md + tool-system.md | tools/src/ | ✅ 90% |
| | 会话管理 | | agent-loop.md + session-persistence.md | runtime/session.rs | ✅ 85% |
| **Provider System** | 多提供商 | 1,200 | provider-resolution.md | api/src/client.rs | ✅ 70% |
| | 证书管理 | | provider-resolution.md | api/src/ | ✅ 75% |
| | 模型路由 | | provider-resolution.md | api/src/ | ✅ 60% |
| | 回退策略 | | provider-resolution.md | api/src/ | ⏳ 40% |
| **Tool System** | 工具注册 | 800 | tool-system.md | tools/src/ | ✅ 90% |
| | 工具执行 | | tool-system.md | tools/src/ | ✅ 85% |
| | 权限控制 | | tool-system.md | tools/src/ | ✅ 70% |
| | MCP 集成 | | tool-system.md + mcp-integration.md | runtime/mcp.rs | ✅ 70% |
| **Session Storage** | 持久化 | 600 | session-persistence.md | runtime/session.rs | ✅ 80% |
| | 消息历史 | | session-persistence.md | runtime/session.rs | ✅ 85% |
| | 成本追踪 | | session-persistence.md | runtime/session.rs | ✅ 70% |
| | 会话压缩 | | context-compression.md | - | ❌ 0% |
| **Prompt System** | 系统提示 | 500 | prompt-builder.md | runtime/prompt.rs | ✅ 50% |
| | 用户建模 | | memory-system.md | - | ❌ 0% |
| | 上下文管理 | | context-compression.md | - | ❌ 0% |
| **Plugin System** | 发现 | 300 | plugin-architecture.md | plugins/src/ | ✅ 80% |
| | 加载 | | plugin-architecture.md | plugins/src/ | ✅ 85% |
| | 执行 | | plugin-architecture.md | plugins/src/ | ✅ 75% |
| **Memory System** | SOUL.md | 平衡 | memory-system.md | - | ❌ 0% |
| | MEMORY.md | | memory-system.md | - | ❌ 0% |
| | USER.md | | memory-system.md | - | ❌ 0% |
| **Message Gateway** | 多平台 | 1,200 | messaging-gateway.md | - | ❌ 0% |
| | 适配器 | | messaging-gateway.md | - | ❌ 0% |
| **Cron Scheduler** | 定时任务 | 200 | - | - | ❌ 0% |
| **Context Compression** | LLM 总结 | 400 | context-compression.md | - | ❌ 0% |
| | 向量搜索 | | session-persistence.md | - | ❌ 0% |

---

## 🔗 核心概念交叉引用

### Agent Loop 涉及的其他系统
- **Provider Resolution** → 选择使用哪个 LLM
- **Tool System** → 执行工具调用
- **Session Persistence** → 保存对话历史
- **Prompt Builder** → 构建系统提示
- **Error Handling** → 处理执行错误

### Provider System 涉及的其他系统
- **Agent Loop** → 在循环中调用
- **Error Handling** → 处理 API 错误
- **Cost Tracking** → 记录成本

### Tool System 涉及的其他系统
- **Agent Loop** → 工具调用和执行
- **Session Persistence** → 记录工具执行历史
- **Plugin Architecture** → 加载插件工具
- **MCP Integration** → 加载 MCP 工具

### Session Persistence 涉及的其他系统
- **Memory System** → 存储 SOUL/MEMORY/USER 数据
- **Agent Loop** → 保存对话历史
- **Context Compression** → 压缩旧历史
- **Tool System** → 记录工具执行日志

### Memory System 涉及的其他系统
- **Agent Loop** → 自动注入记忆上下文、执行 Honcho 工具
- **Session Persistence** → 了存储本地 MEMORY.md / USER.md
- **Provider Resolution** → 使用不同提供商的 API 密钥
- **Error Handling** → 处理记忆提供商的连接错误
- **Context Compression** → 自动从记忆中提取事实前再压缩

### RL-Training (Agent Self-Improvement) 涉及的其他系统
- **Agent Loop** → 收集训练数据（轨迹）
- **Session Persistence** → 存储训练运行历史
- **Provider Resolution** → 推理采样使用的 LLM 提供商
- **Error Handling** → Tinker/Atropos 服务错误处理

---

## 📊 完成进度总结

```
架构和设计文档 (Design):
├─ System Architecture Framework  ██████████ 100%  ✨ NEW
├─ Module Boundaries & Integration ██████████ 100%  ✨ NEW
├─ Entry Points Design            ██████████ 100%  ✨ NEW
├─ Agent Loop Design              ██████████ 100%
├─ Provider Resolution Design     ██████████ 100%
├─ Session Persistence Design     ██████████ 100%
├─ Tool System Design             ██████████ 100% (existing)
├─ Messaging Gateway Design       ██████████ 100%  ✨ NEW
├─ Agent Self-Improvement Design  ██████████ 100%  ✨ NEW
└─ 设计文档覆盖度                ██████████ 100%

## 📊 项目进度统计 (2026-04-11 更新)

```
源码基础战略 (NEW!) ⭐
├─ CODE_FOUNDATION_STRATEGY         ██████████ 100% ✅
├─ CODE_MIGRATION_EXECUTION         ██████████ 100% ✅
├─ CONSISTENCY_VERIFICATION         ██████████ 100% ✅
└─ CODE_FOUNDATION_COMPLETE_PACKAGE ██████████ 100% ✨ 本周新

Phase 1 设计文档 (核心系统)
├─ system-architecture-framework    ██████████ 100% ✅
├─ module-boundaries-integration    ██████████ 100% ✅
├─ entry-points-design              ██████████ 100% ✅
├─ agent-loop                       ██████████ 100% ✅
├─ provider-resolution              ██████████ 100% ✅
└─ session-persistence              ██████████ 100% ✅
    Phase 1 总体: ██████████ 100% ✅ 设计完成

Phase 2 设计文档 (高级特性)
├─ messaging-gateway                ██████████ 100% ✅ 新增
├─ agent-self-improvement           ██████████ 100% ✅ 新增
├─ memory-system                    ██████████ 100% ✅ 新增
├─ error-handling                   ░░░░░░░░░░  0% ⏳
├─ testing-strategy                 ░░░░░░░░░░  0% ⏳
├─ prompt-builder                   ░░░░░░░░░░  0% ⏳
└─ context-compression              ░░░░░░░░░░  0% ⏳
    Phase 2 总体: ████░░░░░░ 43% ✅ (3/7 设计完成)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
设计文档整体:      ███████░░░ 71% ✅ Phase 1 100% + Phase 2 43%
源码战略完成:      ██████████ 100% ✅ 新增！三角关系明确
项目架构就绪:      ███████░░░ 71% 🚀 立即开始源码迁移！
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

下一步: 执行 CODE_MIGRATION_EXECUTION.md (8-10 小时)
```

Phase 1 核心系统实现 (Code):
├─ Agent Loop          ████████░░ 80%  (code 80%)
├─ Provider System     ███████░░░ 70%  (code 70%)
├─ Tool System         ██████████ 90%  (code 90%)
├─ Session Persist     ████████░░ 80%  (code 80%)
└─ Prompt Builder      ███░░░░░░░ 30%  (code 30%)
    Phase 1 总体: ████████░░ 82%  (设计 100% + 代码 65%)

Phase 2 高级特性
├─ Messaging Gateway   ██████████ 100% (设计完成) ✅
├─ RL-Training         ██████████ 100% (设计完成) ✅
├─ Memory System       ██████████ 100% (设计完成) ✅
├─ Error Handling      ░░░░░░░░░░  0%  ⏳
├─ Testing Strategy    ░░░░░░░░░░  0%  ⏳
├─ Prompt Builder      ░░░░░░░░░░  0%  ⏳
└─ Context Compress    ░░░░░░░░░░  0%  ⏳
    Phase 2 总体: ████░░░░░░ 43%  (3/7 设计完成)

Phase 3+ 扩展
├─ MCP Integration     ░░░░░░░░░░  0%
├─ Plugin Arch         ░░░░░░░░░░  0%
└─ Context Compress    ░░░░░░░░░░  0%
    Phase 3 总体: ░░░░░░░░░░  0%

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
设计文档整体:      ███████░░░ 71%  ✅ Phase 1 100% + Phase 2 43%
代码实现总体:      ████████░░ 65%  ✅ Phase 1 大部分
整体项目进度:      ███████░░░ 71%  🚀 架构完善，立即迁移
```

---

## 🚀 按阶段的阅读顺序

### 源码基础战略准备 (今天！- 75 分钟)
**推荐**, 在开始任何开发前务必完整阅读:
1. `CODE_FOUNDATION_COMPLETE_PACKAGE.md` - 📦 完整包总览 (15 min)
2. `CODE_FOUNDATION_STRATEGY.md` - 战略定义 (15 min)
3. `CONSISTENCY_VERIFICATION_REPORT.md` - 验证无冲突 (20 min)
4. `CODE_MIGRATION_EXECUTION.md` - 操作指南 (25 min)

然后: **立即执行 10 步源码迁移 (8-10 小时)**

### Phase 1: 理解核心系统 (迁移完成后)
1. `agent-loop.md` - Agent 如何运行
2. `provider-resolution.md` - 如何选择和使用 LLM
3. `tool-system.md` - 工具如何工作
4. `session-persistence.md` - 如何保存状态

### Phase 2: 理解高级特性（下周）
5. `prompt-builder.md` - 如何构建提示
6. `error-handling.md` - 如何处理错误
7. `memory-system.md` - Honcho 和 8 个记忆提供商 ✨ NEW
8. `agent-self-improvement.md` - Agent 自我进化和 RL 训练 ✨ NEW
9. `messaging-gateway.md` - 多平台支持 ✨ NEW
10. `plugin-architecture.md` - 如何扩展功能

### Phase 3: 理解扩展系统（后续）
11. `context-compression.md` - 成本优化
12. `mcp-integration.md` - MCP 协议

---

## 🔍 按概念快速查找

### 消息和通信
- OpenAI 兼容的消息格式 → [agent-loop.md](./agent-loop.md)
- 多平台消息网关 → [messaging-gateway.md](./messaging-gateway.md)
- MCP 协议集成 → [mcp-integration.md](./mcp-integration.md)

### LLM 和模型
- 提供商选择 → [provider-resolution.md](./provider-resolution.md)
- 模型路由 → [provider-resolution.md](./provider-resolution.md)
- 成本跟踪 → [session-persistence.md](./session-persistence.md)

### 执行和工具
- 工具注册 → [tool-system.md](./tool-system.md)
- 工具执行 → [tool-system.md](./tool-system.md) + [agent-loop.md](./agent-loop.md)
- 工具并发 → [tool-system.md](./tool-system.md)

### 存储和记忆
- 会话历史 → [session-persistence.md](./session-persistence.md)
- 用户记忆 → [memory-system.md](./memory-system.md)
- 向量搜索 → [session-persistence.md](./session-persistence.md)

### 优化和性能
- 上下文压缩 → [context-compression.md](./context-compression.md)
- 成本优化 → [session-persistence.md](./session-persistence.md)
- Token 计数 → [session-persistence.md](./session-persistence.md)

### 开发和测试
- 单元测试 → [testing-strategy.md](./testing-strategy.md)
- 集成测试 → [harness-testing.md](./harness-testing.md)
- 错误处理 → [error-handling.md](./error-handling.md)

### Agent 自我进化和学习
- RL 训练管道 → [agent-self-improvement.md](./agent-self-improvement.md) ✨ NEW
- 环境定义和评分 → [agent-self-improvement.md](./agent-self-improvement.md) ✨ NEW
- GRPO 算法和优化 → [agent-self-improvement.md](./agent-self-improvement.md) ✨ NEW
- 轨迹收集和 WandB 指标 → [agent-self-improvement.md](./agent-self-improvement.md) ✨ NEW
- LoRA 微调集成 → [agent-self-improvement.md](./agent-self-improvement.md) ✨ NEW

### 记忆系统和用户建模
- Honcho AI-native 用户建模 → [memory-system.md](./memory-system.md) ✨ NEW
- 8 个外部记忆提供商 → [memory-system.md](./memory-system.md) ✨ NEW
- 内置记忆 (MEMORY.md / USER.md) → [memory-system.md](./memory-system.md) ✨ NEW
- 跨会话学习和回忆 → [memory-system.md](./memory-system.md) ✨ NEW
- 多档案支持 → [memory-system.md](./memory-system.md) ✨ NEW

### 扩展和集成
- 插件系统 → [plugin-architecture.md](./plugin-architecture.md)
- 数据结构 → [data-schema.md](./data-schema.md)
- MCP 集成 → [mcp-integration.md](./mcp-integration.md)

---

## 📝 贡献指南

### 创建新设计文档
1. 在本目录创建 `new-feature.md`
2. 按照已有文档的结构：
   - 系统概览
   - 架构设计
   - 实现细节
   - 与 Hermes 对齐
   - 集成检查清单
3. 在此索引中添加引用
4. 更新 `DESIGN_DOCS_INDEX.md` 的依赖关系图

### 更新现有文档
1. 保持与实际代码同步
2. 更新完成度指标
3. 如果有重大变化，更新依赖关系图
4. 更新本索引

---

## 相关资源

### 官方项目文档
- [ARCHITECTURE.md](../ARCHITECTURE.md) - 整体架构
- [DESIGN.md](../DESIGN.md) - 设计原则
- [AGENTS.md](../AGENTS.md) - 项目导航
- [DEVELOPER_GUIDE.md](../DEVELOPER_GUIDE.md) - 开发指南

### 参考资源
- [Hermes Agent 分析](../references/hermes-agent-analysis.md)
- [Hermes 设计模式](../references/hermes-agent-patterns.md)

### 支持文档
- [Harness 测试框架](../../harness/README.md)
- [执行计划](../exec-plans/)

---

**下一步**: 选择你感兴趣的文档开始阅读，或访问 [index.md](./index.md) 获取快速导航。

