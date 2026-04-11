# Phase 1: 核心框架搭建 (Foundation)

## 概述 (Overview)

- **计划目标**: 建立 If2Ai 的核心基础设施和架构框架
- **预计时长**: 2-3 周
- **所有者**: @architects
- **关键结果**: 
  - ✅ 项目文档框架完成
  - ⏳ Harness 测试框架建立
  - ⏳ Agent Orchestrator 基础实现
  - ⏳ Tool System 基础实现

## 背景 (Context)

If2Ai 是 hermes-agent 的 Rust/Tauri 复刻版本。Phase 1 专注于建立开发基础，确保后续功能有坚实的架构基础。

前提条件：
- 项目初始化完成 ✅
- 项目结构创建完成 ✅
- 开发环境配置完成

## 工作项 (Work Items)

### Stage 1: 文档和框架 (Weeks 1-2)

#### Sprint 1.1: 项目文档架构
- [x] 创建 AGENTS.md 导航
- [x] 创建 ARCHITECTURE.md 全景图
- [x] 创建 DESIGN.md 原则文档
- [x] 创建 docs/ 目录结构
- [x] 创建设计文档索引
- [x] 创建产品规范索引
- [x] 创建执行计划索引
- [ ] 创建首个设计文档 (agent-orchestrator.md)
- [ ] 创建首个产品规范 (chat-interface.md)

**所有者**: @documentation-lead  
**预计**: 3 天  
**检查点**:
- 信息架构清晰，易于导航
- 所有关键部分有链接
- 文档格式一致

#### Sprint 1.2: Harness 测试框架基础
- [ ] 创建 harness/ 目录结构
- [ ] 实现 TestRunner 基础类
- [ ] 实现 Agent Fixture 系统
- [ ] 创建基础评估器接口
- [ ] 编写 Harness 文档 (README.md)

**所有者**: @testing-lead  
**预计**: 3 天  
**检查点**:
- Harness 框架可以运行第一个测试
- TestRunner 支持基本的 setup/teardown
- 文档清晰开发者可以快速上手

### Stage 2: Agent 核心实现 (Week 2-3)

#### Sprint 2.1: Agent Orchestrator 基础
- [ ] 实现 Agent 结构和生命周期
- [ ] 实现基础的对话循环逻辑
- [ ] 实现 LLM 调用基础
- [ ] 实现 Token 计算
- [ ] 编写单元测试 (≥ 80% 覆盖)

**所有者**: @backend-lead  
**预计**: 4 天  
**检查点**:
- Agent 可以初始化和运行简单对话
- LLM 调用成功
- 测试覆盖率 ≥ 80%

#### Sprint 2.2: Tool System 基础
- [ ] 实现 Tool 结构定义
- [ ] 实现 ToolRegistry
- [ ] 实现基础的 Tool 执行逻辑
- [ ] 实现 3-5 个示例工具
- [ ] 编写单元和集成测试

**所有者**: @tool-system-lead  
**预计**: 4 天  
**检查点**:
- ToolRegistry 可以注册和检索工具
- 工具可以被正确执行
- 示例工具演示了好的实现模式

### Stage 3: 集成和验证 (Week 3)

#### Sprint 3.1: Agent-Tool 集成
- [ ] 连接 Agent 和 Tool System
- [ ] 实现工具调用和结果处理
- [ ] 编写集成测试

**所有者**: @integration-lead  
**预计**: 2 天  
**检查点**:
- Agent 可以识别和调用工具
- 工具结果被正确整合到对话中

#### Sprint 3.2: UI 骨架和 IPC
- [ ] 实现基础 Tauri Command 处理
- [ ] 创建 Svelte 前端骨架
- [ ] 实现前后端通信

**所有者**: @frontend-lead  
**预计**: 2 天  
**检查点**:
- 前后端可以通信
- 可以在 UI 上看到 Agent 响应

#### Sprint 3.3: 文档完善和发布
- [ ] 完成 Phase 1 设计文档
- [ ] 编写开发者快速开始指南
- [ ] 创建第一套 Harness 测试
- [ ] 更新所有计划和文档

**所有者**: @documentation-lead  
**预计**: 1 天

## 进度追踪 (Progress)

| 日期 | 完成度 | 关键事件 | 更新者 | 备注 |
|------|--------|---------|-------|------|
| 2026-04-11 | 20% | Phase 1 计划创建 | @init | 文档框架完成 |
| 2026-04-12 | 20% | 等待实现开始 | @init | 准备进入 Sprint 1.1 |

## 决策日志 (Decision Log)

### 决策 1: 使用 OpenAI 推荐的文档架构
- **背景**: 需要清晰的知识库结构，使 Agent 能够理解和导航
- **选项**: 
  - A: 单一大型 AGENTS.md（OpenAI 不推荐的方式）
  - B: 分层文档结构（AGENTS.md + docs/ 子目录）✅ 选中
  - C: Wiki 式文档（难以版本控制）
- **决定**: 选择 B
- **原因**: 更容易维护、自动检查、避免信息过载
- **日期**: 2026-04-11
- **所有者**: @architects

### 决策 2: Harness 框架优先于单元测试
- **背景**: 测试框架需要支持 Agent 行为的评估和复现
- **选项**:
  - A: 仅使用标准单元测试框架
  - B: 建立完整的 Harness 框架，然后单元测试 ✅ 选中
- **决定**: 选择 B
- **原因**: 1) 符合 OpenAI 推荐实践，2) Agent 行为需要特殊评估，3) 便于 Agent 测试 Agent
- **日期**: 2026-04-11
- **所有者**: @testing-lead

## 风险和缓解 (Risks)

| 风险 | 可能性 | 影响 | 缓解策略 |
|------|--------|------|--------|
| LLM API 额度不足 | Medium | High | 建立本地模型回退、成本追踪 |
| 架构理解偏差 | Medium | High | 定期架构审查、文档同步 |
| 工具实现复杂 | Medium | Medium | 从简单工具开始、可复用模板 |
| 团队协作障碍 | Low | Medium | 清晰的分工、日常同步 |

## 关键检查点 (Milestones)

- [x] 文档框架完成 (2026-04-11)
- [ ] Harness 基础框架完成 (2026-04-15)
- [ ] Agent Orchestrator 基础完成 (2026-04-18)
- [ ] Phase 1 全部完成，可进入 Phase 2 (2026-04-25)

## 成功标准 (Definition of Done)

Phase 1 完成的标准：
- ✅ 项目文档架构完成且清晰
- ✅ Harness 框架可运行基础测试
- ✅ Agent 可以执行简单对话
- ✅ Tool System 可以加载和执行工具
- ✅ 前后端基础通信工作
- ✅ 文档覆盖 ≥ 80%
- ✅ 无关键 Bugs
- ✅ 所有工作项有明确的所有者

## 相关文档

- [AGENTS.md](../../AGENTS.md) - 项目导航
- [ARCHITECTURE.md](../../ARCHITECTURE.md) - 系统架构
- [DESIGN.md](../../DESIGN.md) - 设计原则
- [harness/README.md](../../harness/README.md) - Harness 框架文档

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**下一步**: Phase 2 - Agent 系统完善和工具扩展
