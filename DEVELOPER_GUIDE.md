# If2Ai 开发者指南

> 本指南帮助开发者快速上手 If2Ai 项目，理解项目结构、工作流程和最佳实践。

## 🚀 5 分钟快速开始

### 1. 克隆项目并了解结构

```bash
cd /Users/ryanliu/Documents/IfAI/if2Ai
tree -L 2 -I 'node_modules|target'
```

### 2. 理解项目架构

```bash
# 按这个顺序阅读文档（每个 5-10 分钟）
1. AGENTS.md          # 导航地图（这个文件的内容目录）
2. ARCHITECTURE.md    # 系统全景图
3. DESIGN.md          # 设计原则和约束
```

### 3. 查看你的第一个任务

```bash
# 在活跃执行计划中找到任务
cat docs/exec-plans/active/phase-1-foundation.md

# 选择一个你感兴趣的工作项
# 比如：添加新设计文档或实现工具
```

### 4. 开始开发

```bash
# 遵循 DESIGN.md 中的约束
# 使用 Harness 框架验证你的工作
python -m harness runner run --config test.yaml
```

## 📚 完整学习路径

### 对于后端开发者 (Rust/Tokio)

**第一天**: 架构和设计

```bash
# 阅读这些文件（顺序很重要）
→ ARCHITECTURE.md                          # 系统全景
→ DESIGN.md                                # 设计约束
→ docs/design-docs/                        # 具体设计
  - agent-orchestrator.md                 # Agent 设计
  - tool-system.md                        # 工具设计
```

**第二天**: 代码和实现

```bash
# 浏览现有代码
→ src-tauri/src/main.rs              # 主入口
→ src-tauri/src/modules/             # 核心模块

# 写你的第一个单元测试
→ src-tauri/src/modules/tools/registry_tests.rs
```

**第三天**: 测试

```bash
# 学习测试框架
→ docs/design-docs/testing-strategy.md
→ harness/README.md

# 运行第一个 Harness 测试
python -m harness runner run --config harness/tests/basic.yaml
```

### 对于前端开发者 (Svelte/TypeScript)

**第一天**: UI 架构

```bash
# 理解前端结构
→ ARCHITECTURE.md                    # 找到 UI 层
→ src/                               # 前端代码
→ index.html                         # HTML 入口
```

**第二天**: Tauri IPC

```bash
# 了解前后端通信
→ docs/design-docs/tauri-ipc.md     # IPC 设计
→ src-tauri/src/commands/           # 后端命令处理

# 写一个简单的 Tauri 命令
# 修改 src-tauri/src/main.rs，添加你的命令
```

**第三天**: 组件和状态

```bash
# 创建你的第一个 Svelte 组件
→ src/components/                   # 组件目录
→ src/App.svelte                    # 主应用

# 添加状态管理（stores）
→ src/lib/                          # 工具库
```

### 对于质量/测试部门

**第一天**: Harness 框架

```bash
# 完整理解 Harness
→ harness/README.md                 # Harness 概览
→ 运行示例测试
  python -m harness runner suite --suite basic
```

**第二天**: 编写评估器

```bash
# 创建自定义评估器
→ harness/evaluators/__init__.py    # 评估器实现
→ docs/design-docs/testing-strategy.md  # 测试策略

# 为某个功能创建新评估器
```

**第三天**: 测试套件

```bash
# 创建测试套件
→ harness/tests/
→ 创建 my-feature-tests.yaml
→ python -m harness runner suite --suite my-feature-tests
```

## 🏗️ 项目目录导航

```
if2Ai/
│
├── AGENTS.md                  # 📍 START HERE - 项目导航
├── ARCHITECTURE.md            # 系统全景图
├── DESIGN.md                  # 设计原则
├── README.md                  # 项目概述
│
├── src/                       # Svelte 前端
│   ├── App.svelte
│   ├── main.ts
│   ├── components/            # UI 组件
│   ├── lib/                   # 工具库
│   └── styles/                # 样式
│
├── src-tauri/                 # Rust 后端
│   ├── src/
│   │   ├── main.rs            # 主入口
│   │   ├── commands/          # Tauri 命令
│   │   └── modules/           # 核心业务模块
│   ├── Cargo.toml
│   └── build.rs
│
├── harness/                   # 🧪 Harness 测试框架
│   ├── README.md              # 框架文档
│   ├── __init__.py            # 核心类和接口
│   ├── evaluators/            # 评估器实现
│   ├── fixtures/              # 测试夹具
│   ├── runners/               # 运行器
│   └── tests/                 # 测试用例
│
├── docs/                      # 📚 知识库
│   ├── AGENTS.md              # 项目导航（复制到repo根）
│   ├── design-docs/           # 设计决策
│   │   ├── index.md           # 设计文档索引
│   │   ├── agent-orchestrator.md
│   │   ├── tool-system.md
│   │   ├── testing-strategy.md
│   │   └── ... 其他设计文档
│   ├── exec-plans/            # 执行计划
│   │   ├── active/            # 活跃计划
│   │   │   └── phase-1-foundation.md
│   │   └── completed/         # 已完成计划
│   ├── product-specs/         # 产品规范
│   ├── references/            # 参考资料
│   └── generated/             # 自动生成文档
│
├── package.json               # NPM 配置
├── Cargo.toml                 # Rust 工作空间
├── tauri.conf.json            # Tauri 配置
├── vite.config.ts             # Vite 配置
└── tsconfig.json              # TypeScript 配置
```

## 💡 核心概念快速参考

### Agent Orchestrator

Agent 的核心引擎，负责：

- 对话循环管理
- 工具调用和结果处理
- 上下文管理
- 预算追踪

**位置**: `src-tauri/src/modules/agents/`

### Tool System

动态工具加载和执行：

- Tool Registry（工具注册表）
- 工具依赖解析
- 并行执行

**位置**: `src-tauri/src/modules/tools/`

### Harness Framework

测试和评估框架：

- Runner（运行测试）
- Evaluator（评估结果）
- Fixture（测试数据）

**位置**: `harness/`

## 🔧 常用命令

### 项目设置

```bash
# 安装依赖
npm install
cargo build

# Rust 工具链检查
rustup update
```

### 开发

```bash
# 启动开发服务器
npm run tauri dev

# 编译/检查
cargo build
cargo check
npm run build
```

### 测试

```bash
# 后端单元测试
cargo test --all

# 前端测试（当有时）
npm run test

# Harness 测试
python -m harness runner suite --suite default
```

### 文档

```bash
# 预览 Markdown
# 使用 VS Code 的 Markdown Preview

# 生成目录（如需要）
# 使用 markdown-toc 工具
```

## 📝 工作流程

### 新功能开发流程

```
1. 创建设计文档
   → docs/design-docs/my-feature.md
   → 包含问题陈述、方案、权衡

2. 创建产品规范
   → docs/product-specs/my-feature.md
   → 定义用户故事、需求、UI 草图

3. 创建执行计划
   → docs/exec-plans/active/my-feature-plan.md
   → 列出工作项、时间线、风险

4. 编码和测试
   → 遵循 DESIGN.md 中的约束
   → 编写单元、集成、Harness 测试

5. PR 和审查
   → 确保所有检查通过
   → 代码审查检查质量和约束

6. 合并和发布
   → 更新文档
   → 将完成计划移到 completed/ 目录
```

### 日常开发清单

- [ ] 修改代码前读现有文档
- [ ] 遵循 DESIGN.md 中的约束
- [ ] 编写/更新对应的测试
- [ ] 运行 `cargo test` 和 `pytest` 验证
- [ ] 更新相关文档
- [ ] 提交 PR（自动 CI 检查）

## 🎯 关键约束记住

1. **分层架构**: Types → Config → Repo → Service → Runtime → UI
2. **Provider 模式**: 所有外部依赖通过 Provider 注入
3. **类型安全**: 边界处验证，不要假设数据有效
4. **异步优先**: 所有 I/O 使用 async/await
5. **可测试性**: 好的设计能很容易被测试

详见 [DESIGN.md](./DESIGN.md)

## 🚨 遇到问题？

### 常见问题

**Q: 我想添加一个新工具，应该怎么做？**
A:

1. 在 `src-tauri/src/modules/tools/` 中创建工具实现
2. 在 ToolRegistry 中注册
3. 添加单元测试
4. 在 Harness 中添加集成测试

**Q: 前后端如何通信？**
A:

1. 查看 `src-tauri/src/commands/` 中的示例
2. 使用 `#[tauri::command]` 宏定义命令
3. 从 Svelte 中使用 `invoke()` 调用
4. 参考 `docs/design-docs/tauri-ipc.md`

**Q: 我对架构决策有疑问？**
A:

1. 查看 `docs/design-docs/` 中的相关文档
2. 检查决策日志中是否有记录
3. 创建 GitHub Discussion
4. 提出新的设计文档

### 获取帮助

- 📖 **文档**: 从 AGENTS.md 开始导航
- 💬 **讨论**: 创建 GitHub Issue 或 Discussion
- 👥 **团队**: 联系项目所有者或架构师

## 📚 继续学习

### 深入学习路径

- [ ] 完整阅读 ARCHITECTURE.md
- [ ] 研究 3 个设计文档
- [ ] 实现 1 个小功能
- [ ] 创建 1 个 Harness 测试
- [ ] 参与 1 次代码审查

### 推荐阅读

1. [DESIGN.md](./DESIGN.md) - 理解设计哲学
2. [harness/README.md](./harness/README.md) - 掌握测试框架
3. [docs/design-docs/](./docs/design-docs/) - 深入特定设计
4. [Rust 官方书籍](https://doc.rust-lang.org/book/) - 语言学习
5. [Svelte 文档](https://svelte.dev/docs) - 前端框架学习

## 🎓 最佳实践总结

### 代码质量

- ✅ 编写易读的代码（对 AI 友好）
- ✅ 遵循命名约定
- ✅ 保持函数简洁 (< 50 行)
- ✅ 添加关键逻辑的注释

### 测试

- ✅ 编写单元、集成、Harness 测试
- ✅ 目标覆盖率 ≥ 80%
- ✅ 使用 Mock 确保确定性
- ✅ 清晰的测试名称

### 文档

- ✅ 更新对应设计文档
- ✅ 包含示例代码
- ✅ 保持文档新鲜
- ✅ 添加链接便于导航

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11

下一步：选择一个任务开始贡献！
