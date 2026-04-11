# If2Ai 项目导航 (AGENTS.md)

> **设计原则**: 本文件作为内容目录和导航地图，而不是详细说明书。使用 100 行程度的简洁指引，将详细信息从代码库其他位置链接过来。

## 📍 快速开始导航

如果你是...
- **新人贡献者** → 查看 [ARCHITECTURE.md](./ARCHITECTURE.md) 了解系统全景，然后选择 [docs/exec-plans/active/](./docs/exec-plans/active/) 中的任务
- **功能开发者** → 前往 [docs/product-specs/](./docs/product-specs/) 找到你负责的模块
- **测试/质量** → 查看 [harness/](../harness/) 目录了解测试框架
- **架构师/决策者** → 阅读 [docs/design-docs/](./docs/design-docs/) 中的设计决策

## 🏗️ 项目核心

**If2Ai** 是一个 Tauri + Rust + Svelte 的智能体桌面应用，复刻 hermes-agent 框架。

核心理念：
- 🤖 Agent 优先的工程思维
- 📋 代码库作为记录系统
- 🔍 完整的可观测性和可评估性
- 🎯 清晰的架构约束和品味规范

## 📚 关键文档

| 文档 | 目的 | 受众 |
|------|------|------|
| [ARCHITECTURE.md](./ARCHITECTURE.md) | 系统架构全景图和模块关系 | 所有人 |
| [DESIGN.md](./DESIGN.md) | 设计原则和决策框架 | 架构师、决策者 |
| [docs/design-docs/](./docs/design-docs/) | 具体设计决策（分主题） | 实现者 |
| [docs/product-specs/](./docs/product-specs/) | 产品规范和需求 | PM、开发者 |
| [docs/exec-plans/](./docs/exec-plans/) | 执行计划和进度追踪 | 项目经理、开发者 |
| [harness/README.md](../harness/README.md) | 测试和评估框架 | 测试、QA |

## 🎯 当前执行计划

**活跃计划** (查看 [docs/exec-plans/active/](./docs/exec-plans/active/))：
1. Phase 1: 核心框架搭建
2. Phase 2: Agent 系统实现
3. Phase 3: 工具和命令系统

**完成的计划** (查看 [docs/exec-plans/completed/](./docs/exec-plans/completed/))：
- 项目初始化和目录结构

## 🔧 核心模块

```
if2Ai/
├── src/                    # Svelte 前端
├── src-tauri/             # Rust 后端
│   ├── modules/           # 核心业务逻辑
│   └── commands/          # Tauri IPC 命令处理
├── harness/               # 测试和评估框架
└── docs/                  # 知识库和规范
```

详见 [ARCHITECTURE.md](./ARCHITECTURE.md)

## 📖 文档维护规范

所有文档遵循以下规则：
- ✅ 使用结构化 Markdown，保持清晰层级
- ✅ 包含"最后更新"时间戳
- ✅ 定期运行文档检查（linter）
- ✅ 过时文档在 CI 中被自动标记
- ✅ 保持与实际代码同步

具体见 [DESIGN.md](./DESIGN.md) 的"文档即记录系统"部分。

## 🚀 如何开始

### 1. 理解整体架构
```bash
# 阅读这些文件，顺序很重要
→ ARCHITECTURE.md         # 5 分钟：了解全景
→ DESIGN.md               # 10 分钟：理解设计理念  
→ docs/design-docs/       # 15 分钟：深入具体设计
```

### 2. 查找你的任务
```bash
# 在活跃计划中找到相关任务
→ docs/exec-plans/active/
→ 选择符合你技能的任务：
   - 前端：Svelte/TypeScript 相关
   - 后端：Rust/Tokio 相关
   - 测试：harness 框架相关
```

### 3. 实施和验证
```bash
# 按照执行计划的步骤进行开发
# 使用 harness 框架验证你的改动
→ harness/README.md      # 了解如何运行测试
→ npm run test           # 运行整个测试套件
```

## 📋 关键约束和品味规范

If2Ai 工程遵循严格的架构约束（见 [DESIGN.md](./DESIGN.md)）：

1. **分层架构**：类型 → 配置 → 模型 → 服务 → 运行时 → UI
2. **依赖方向**：只能"向前"依赖
3. **命名约定**：符合 Rust/Svelte 官方指南
4. **测试覆盖率**：核心逻辑 ≥ 80%
5. **文档完备性**：公开 API 必须有文档和例子

违反这些规则的 PR 将被 CI 自动拒绝。

## 🔄 反馈循环和迭代

If2Ai 使用人类-智能体协作的开发模式：
- 📝 提出任务 → 智能体执行 → 人类审查
- 🔍 发现问题 → 转化为文档/约束 → 更新系统
- 📊 评估结果 → 使用 harness 框架度量 → 调整

详见 [docs/design-docs/agent-driven-workflow.md](./docs/design-docs/agent-driven-workflow.md)

## 💡 常见问题

**Q: 我想添加一个新功能，应该从哪里开始？**
A: 
1. 在 [docs/design-docs/](./docs/design-docs/) 中创建一个设计文档
2. 在 [docs/product-specs/](./docs/product-specs/) 中创建产品规范
3. 在 [docs/exec-plans/active/](./docs/exec-plans/active/) 中创建执行计划
4. 开始编码并使用 harness 验证

**Q: 如何理解当前的项目进度？**
A: 查看 `docs/QUALITY_SCORE.md` 和 `docs/exec-plans/` 中的各个计划。

**Q: 遇到架构决策问题怎么办？**
A: 查看 [DESIGN.md](./DESIGN.md) 和相应的设计文档，如无答案则创建新的设计讨论。

## 📞 维护和支持

- **代码质量问题** → 创建 issue 或 PR
- **文档过时** → "doc-gardening" 自动化将处理，或手动更新
- **架构问题** → 在 [docs/design-docs/](./docs/design-docs/) 中讨论

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11
