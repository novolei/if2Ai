# 设计文档索引 (Design Docs Index)

本目录包含 If2Ai 的具体设计决策文档。每个文档聚焦于单一的架构设计或重要决策。

## 📋 当前设计文档

### 核心架构设计
- **agent-orchestrator.md** - Agent 协调引擎设计
- **tool-system.md** - 工具注册表和执行系统
- **llm-routing.md** - 多 LLM 提供商路由和故障转移
- **memory-system.md** - 记忆管理系统（双层架构）
- **context-compression.md** - 上下文压缩算法

### 数据和状态
- **data-schema.md** - 核心数据模型和数据库设计
- **state-management.md** - 状态管理和事件系统
- **serialization.md** - 数据序列化和反序列化

### 通信和集成
- **tauri-ipc.md** - Tauri IPC 命令和事件设计
- **agent-driven-workflow.md** - 人类-Agent 协作工作流
- **observability-stack.md** - 可观测性集成

### 开发流程
- **development-workflow.md** - 开发工作流和最佳实践
- **error-handling.md** - 错误分类和处理策略
- **testing-strategy.md** - 测试策略（单元、集成、E2E）

### 扩展和集成
- **extension-points.md** - 系统扩展点和插件机制
- **external-tools.md** - 外部工具集成指南

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

**新人** → 始终从 [agent-orchestrator.md](./agent-orchestrator.md) 开始
**前端开发** → [tauri-ipc.md](./tauri-ipc.md) + [state-management.md](./state-management.md)
**后端开发** → [tool-system.md](./tool-system.md) + [llm-routing.md](./llm-routing.md)
**测试** → [testing-strategy.md](./testing-strategy.md)

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11
