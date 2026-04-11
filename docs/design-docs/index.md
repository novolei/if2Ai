# 设计文档索引 (Design Docs Index)

本目录包含 If2Ai 的具体设计决策文档。每个文档聚焦于单一的架构设计或重要决策。

## 📋 设计文档概览（与 Hermes 完全对齐）

### 🌟 推荐阅读顺序

#### 对于新人 (30 分钟)
1. **[system-architecture-framework.md](./system-architecture-framework.md)** ✨ NEW
   - 了解 If2Ai 与 Hermes 的 9 个子系统
   - 分阶段的实现路线图
   - 系统数据流
   
2. **[module-boundaries-and-integration.md](./module-boundaries-and-integration.md)** ✨ NEW
   - 理解 Rust 模块的结构
   - 每个模块的职责
   
3. **[entry-points-design.md](./entry-points-design.md)** ✨ NEW
   - 三个入口点设计（Tauri, API, IDE）

#### 深度学习 (1-2 小时)

按职能选择核心设计文档：

- **Agent 开发** → [agent-loop.md](./agent-loop.md) + [provider-resolution.md](./provider-resolution.md)
- **工具开发** → [tool-system.md](./tool-system.md)
- **会话/数据** → [session-persistence.md](./session-persistence.md)
- **消息网关/多平台** → [messaging-gateway.md](./messaging-gateway.md) ✨ NEW
- **Agent 自我进化** → [agent-self-improvement.md](./agent-self-improvement.md) ✨ NEW
- **记忆系统 / Honcho** → [memory-system.md](./memory-system.md) ✨ NEW
- **Tauri 前端** → [entry-points-design.md](./entry-points-design.md) (Tauri 部分)
- **API 后端** → [entry-points-design.md](./entry-points-design.md) (API 部分)
- **IDE 集成** → [entry-points-design.md](./entry-points-design.md) (IDE 部分)

### 核心系统设计（Hermes 对标）✅

| 子系统 | 设计文档 | Hermes 行数 | If2Ai 进度 |
|-------|---------|-----------|----------|
| **系统架构** | [system-architecture-framework.md](./system-architecture-framework.md) | - | ✅ 100% |
| **模块组织** | [module-boundaries-and-integration.md](./module-boundaries-and-integration.md) | - | ✅ 100% |
| **入口点** | [entry-points-design.md](./entry-points-design.md) | - | ✅ 100% |
| **Agent Loop** | [agent-loop.md](./agent-loop.md) | 9,200 | ✅ 80% |
| **Provider** | [provider-resolution.md](./provider-resolution.md) | 1,200 | ✅ 70% |
| **Tools** | [tool-system.md](./tool-system.md) | 800 | ✅ 90% |
| **Session** | [session-persistence.md](./session-persistence.md) | 600 | ✅ 80% |

### 高级特性（Phase 2）🔄
- **[messaging-gateway.md](./messaging-gateway.md)** ✨ NEW - 15+ 平台集成, Hermes v0.8.0 对齐
- **[agent-self-improvement.md](./agent-self-improvement.md)** ✨ NEW - RL-Training, GRPO 算法, Hermes 完全对标
- **[memory-system.md](./memory-system.md)** ✨ NEW - Honcho AI-native 用户建模, 8 个提供商, Hermes 完全对标
- **prompt-builder.md** - 提示构建和系统提示工程
- **context-compression.md** - 上下文压缩算法
- **error-handling.md** - 统一错误处理
- **testing-strategy.md** - Harness 集成测试

### 扩展系统（Phase 3）📋
- **plugin-architecture.md** - 插件系统架构
- **mcp-integration.md** - Model Context Protocol
- **cron-scheduler.md** - 定时任务系统
- **smart-home-integration.md** - Home Assistant 和智能家居集成

## 📊 完成进度总结

```
设计文档完成度        编码实现进度          整体项目
════════════════════════════════════════════════════════════

Phase 1 核心系统:
├─ System Architecture  ██████████ 100%  ✨ NEW
├─ Module Boundaries    ██████████ 100%  ✨ NEW  
├─ Entry Points         ██████████ 100%  ✨ NEW
├─ Agent Loop           ██████████ 100%   Agent Loop      ████████░ 80%
├─ Provider System      ██████████ 100%   Provider Syst   ███████░░ 70%
├─ Tool System          ██████████ 100%   Tool System     ██████████ 90%
└─ Session Persist      ██████████ 100%   Session Persis  ████████░ 80%

Phase 1 设计文档总体:  ██████████ 100%  ✅ 完成
Phase 1 代码实现总体:  ████████░░ 80%   ✅ 大部分完成
────────────────────────────────────────────────────────────

Phase 2 高级特性（现在进行中）:
├─ Messaging Gateway    ██████████ 100%  ✅ 设计完成 ✨ NEW
├─ RL-Training & Self-  ██████████ 100%  ✅ 设计完成 ✨ NEW
│  Improvement
├─ Memory System        ██████████ 100%  ✅ 设计完成 ✨ NEW
├─ Error Handling       ░░░░░░░░░░  0%   (计划中)
├─ Testing Strategy     ░░░░░░░░░░  0%   (计划中)
├─ Prompt Builder       ░░░░░░░░░░  0%   (代码 30% → 设计待完善)
└─ Context Compress     ░░░░░░░░░░  0%   (计划中)

Phase 2 总体:          ██████░░░░ 43%  ✨ 3/7 设计完成，编码准备中
────────────────────────────────────────────────────────────

Phase 3+ 扩展（未来）:
├─ Plugin Architecture  ░░░░░░░░░░  0%
├─ MCP Integration      ░░░░░░░░░░  0%
└─ Cron Scheduler       ░░░░░░░░░░  0%

Phase 3 总体:          ░░░░░░░░░░  0%  🔮 未来计划
════════════════════════════════════════════════════════════

▶ 整体项目进度        ███████░░░ 71%   ✨ Phase 2 架构全部完成，编码准备中
```

### 🎯 下一步行动计划

**本周** (优先级高)
- [ ] 完成 Prompt Builder 的设计增强
- [ ] 创建 Error Handling 设计文档
- [ ] 创建 Testing Strategy 设计文档
- [ ] 验证代码实现与设计文档的对齐

**下周** (优先级中)
- [ ] 开始 Memory System 设计
- [ ] 完成 Phase 1 代码实现的最后 20%
- [ ] 启动 Phase 2 工作

**未来** (优先级低)
- [ ] 开始 Plugin Architecture 设计
- [ ] 规划 Phase 3 (IDE 集成)

## 📖 文档添加指南

添加新设计文档时：

1. **命名**: 使用 `kebab-case` 且描述性强
2. **大小**: 控制在 200-300 行以内
3. **结构**:
   ```markdown
   # 设计文档标题
   
   > 一句话概述设计决策
   
   ## 问题陈述
   我们要解决什么问题？
   
   ## 提议的方案
   设计的详细说明
   
   ## 实现细节
   关键的实现步骤
   
   ## 权衡和考虑
   为什么做这个决定？有哪些备选方案？
   
   ## 参考和相关文档
   ```

4. **链接**: 从其他文档（如 ARCHITECTURE.md）链接到这个设计文档
5. **版本**: 包含"最后更新"时间戳

## 🔗 快速导航

### 我想要...

**理解系统全景** 
→ [DESIGN_DOCS_INDEX.md](./DESIGN_DOCS_INDEX.md) (综合导航和依赖图)

**修改 Agent 循环**
→ [agent-loop.md](./agent-loop.md) + `crates/runtime/src/conversation.rs`

**添加新的 LLM 提供商**
→ [provider-resolution.md](./provider-resolution.md) + `crates/api/src/providers/`

**修改工具系统**
→ [tool-system.md](./tool-system.md) + `crates/tools/src/`

**优化成本/性能**
→ [session-persistence.md](./session-persistence.md) + [context-compression.md](./context-compression.md)（待）

**添加记忆功能**
→ [memory-system.md](./memory-system.md)（待）

**编写测试**
→ [testing-strategy.md](./testing-strategy.md)（待）+ [harness-testing.md](./harness-testing.md)（待）

---

## 📚 与 Hermes 对标

每个设计文档都标注了对应的 Hermes 功能和代码行数。见 [DESIGN_DOCS_INDEX.md](./DESIGN_DOCS_INDEX.md) 中的详细映射表。

---

**版本**: 0.2.0 | **最后更新**: 2026-04-11
