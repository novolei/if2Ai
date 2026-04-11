# If2Ai Phase 1 快速开始指南

**最后更新**: 2025年1月  
**目标用户**: 开发者  
**时间投入**: 4-6 周

---

## 🚀 立即开始（30 分钟）

### 步骤 1：环境检查

```bash
# 进入项目目录
cd /Users/ryanliu/Documents/IfAI/if2Ai

# 检查 Rust 工具链
rustc --version  # 应该 ≥ 1.70
cargo --version

# 检查 Node.js
node --version   # 应该 ≥ 18
npm --version

# 测试编译
cd src-tauri
cargo build
cd ..
```

### 步骤 2：理解项目结构

```
if2Ai/
├── IMPLEMENTATION_PLAN.md  ← 详细 4 周计划（你在这里）
├── src-tauri/              ← Rust 后端
│   ├── src/
│   │   ├── main.rs
│   │   ├── modules/        ← 核心逻辑（待实现）
│   │   ├── commands/       ← Tauri IPC（待实现）
│   │   └── ...
│   └── Cargo.toml
├── src/                    ← Svelte 前端
│   ├── App.svelte          ← 主组件（待完全实现）
│   ├── components/         ← UI 组件
│   └── lib/
├── harness/                ← 测试框架
│   ├── README.md
│   └── ...
└── docs/                   ← 文档
    ├── design-docs/
    ├── exec-plans/
    └── ...
```

### 步骤 3：标记您的开发里程碑

**第 1 天（今天）**：完成环境设置，理解代码结构

```bash
# 后端编译测试
cd src-tauri && cargo build --release

# 前端编译测试
cd .. && npm install && npm run build

# 运行开发服务器（可选）
npm run tauri dev &
```

**第 2-10 天**：按照 `IMPLEMENTATION_PLAN.md` 的周计划进行

---

## 📊 关键决策（需要你确认）

### Q1: API 提供商选择

**现状**: 计划支持 OpenAI  
**可选**:

- [ ] 仅 OpenAI (简单，推荐)
- [ ] OpenAI + Anthropic (中等复杂)
- [ ] 3+ 提供商 (延迟到 Phase 2)

**建议**: 选择**仅 OpenAI**，Phase 2 时添加更多提供商

### Q2: 工具优先级

**计划的 8 个工具**（按优先级）:

1. ✅ Terminal (shell 执行)
2. ✅ Read File
3. ✅ Write File
4. ✅ Web Search
5. ✅ Web Extract
6. ✅ Code Execution (Python/Node)
7. ⭐ Browser Navigation (可选)
8. ⭐ Vision (图像分析，可选)

**建议**: 优先实现 1-6，浏览器和视觉可推迟

### Q3: UI 复杂度

**计划**：简单的聊天界面 (Svelte)  
**增强选项**:

- [ ] 保持简单（推荐）
- [ ] 添加工具执行面板
- [ ] 添加内存/历史视图

**建议**: Phase 1 保持简单，Phase 2 添加面板

### Q4: 测试覆盖目标

**计划**: ≥70% 单元测试  
**可选**:

- [ ] 70% (足够)
- [ ] 85% (严格)
- [ ] 95% (非常严格)

**建议**: 80% 作为折中

---

## 💡 核心架构概览（5 分钟理解）

```
用户输入 (Chat UI)
    ↓
[Tauri IPC 命令]
    ↓
AIAgent.run_conversation()
    ├─ 1. 添加用户消息到历史
    ├─ 2. 构建系统提示
    ├─ 3. 调用 OpenAI API
    ├─ 4. 解析响应
    └─ 5. 如果有工具调用 → 执行工具，回到 3
    ↓
[SQLite 保存会话]
    ↓
返回响应给 UI
```

**关键类型** (`src-tauri/src/modules/types.rs`):

```rust
Message          // OpenAI 格式消息
ConversationHistory  // 会话历史容器
ToolCall         // 工具调用请求
ToolResult       // 工具执行结果
AgentResponse    // 最终响应
```

**关键模块** (`src-tauri/src/modules/`):

```
agent.rs         // AIAgent 循环逻辑
providers.rs     // OpenAI API 集成
tools/           // 工具实现
tools/registry.rs    // 工具管理
memory.rs        // SQLite 持久化
```

---

## 🛑 常见陷阱 + 避免方案

| 陷阱            | 症状                         | 解决方案                                  |
| --------------- | ---------------------------- | ----------------------------------------- |
| Tauri 版本不对  | `cargo build` 失败，版本冲突 | 使用 Cargo.toml 中指定的版本              |
| 缺少 OpenAI key | 运行时 401 错误              | 在 UI 中设置 API key，不要硬编码          |
| Tool 注册遗漏   | Agent 找不到工具             | 确保在 main.rs 中注册所有工具             |
| 消息格式错误    | Tool calls 不执行            | 验证消息格式与 OpenAI spec 一致           |
| IPC 序列化失败  | Serde 错误                   | 所有类型都要 derive Serialize/Deserialize |

---

## ✅ 该周完成清单

**周一早晨**：

- [ ] Git checkout 到新分支 (`git checkout -b feature/agent-core`)
- [ ] 环境验证 (编译通过)
- [ ] 阅读 `IMPLEMENTATION_PLAN.md` 第 1 周部分
- [ ] 开始 Cargo.toml 更新

**周五晚间**：

- [ ] modules/ 目录结构完成
- [ ] types.rs 所有类型定义
- [ ] agent.rs 循环框架（不完整但编译）
- [ ] 提交 PR

**验收标准**：

```bash
# 这个命令应该成功
cd src-tauri
cargo build --release
echo "✅ Phase 1 Week 1 Complete"
```

---

## 📞 遇到问题？

### 编译错误

1. 清理缓存: `cargo clean && cargo build`
2. 更新依赖: `cargo update`
3. 检查 Rust 版本: `rustc --version`

### Tauri 问题

1. 检查 tauri.conf.json 是否有效
2. 删除 src-tauri/target 并重建
3. 参考官方 [Tauri 文档](https://tauri.app)

### OpenAI API 问题

1. 验证 API key 有效（在 playground 中测试）
2. 检查 quota 限制
3. 参考 [OpenAI 文档](https://platform.openai.com/docs)

### 其他问题

1. 查看 [hermes-agent](https://github.com/NousResearch/hermes-agent) 源码参考设计
2. 查阅项目 README 和设计文档
3. 参考 `docs/design-docs/` 中的架构决策说明

---

## 🎓 学习路径

**如果你刚开始**（>2 小时）：

1. 阅读 ARCHITECTURE.md （5 分钟理解全景）
2. 阅读 DESIGN.md （8 分钟了解约束）
3. 快速扫一遍 IMPLEMENTATION_PLAN.md（这个文件）
4. 查看 Hermes Agent 源码结构（参考 `~/Documents/IfAI/hermes-agent-main`）

**如果你是 Rust 初学者**（>5 小时）：

1. 快速 Rust 教程：https://doc.rust-lang.org/book/
2. 学习 async/await：https://tokio.rs/tokio/tutorial
3. 学习 serde：https://serde.rs/

**如果你是 Tauri 初学者**（>3 小时）：

1. https://tauri.app/docs
2. 官方示例：https://github.com/tauri-apps/
3. 查看 `src-tauri/src/main.rs` 和 `src-tauri/src/commands/`

**如果你是 OpenAI API 初学者**（>2 小时）：

1. https://platform.openai.com/docs
2. 使用 Postman 测试 API
3. 查看 `async_openai` crate 文档

---

## 🚢 成功发布清单（第 4 周）

在提交 Phase 1 完成前检查：

### 代码品质

- [ ] `cargo clippy` 无警告
- [ ] `cargo fmt` 格式化
- [ ] `cargo test` ≥70% 通过
- [ ] Harness 测试通过 ≥80%

### 文档

- [ ] README.md 更新（包括构建说明）
- [ ] API 文档注释 (rustdoc)
- [ ] 新增设计决策文档

### 功能

- [ ] 完整 Agent 循环工作
- [ ] 最少 6 个工具实现
- [ ] 会话持久化工作
- [ ] UI 能聊天

### 性能

- [ ] 平均响应 <2 秒 (本地)
- [ ] 内存使用 <500MB
- [ ] 没有导致 OOM 的内存泄漏

---

## 🎯 Phase 2 预告

如果 Phase 1 顺利完成，Phase 2 将包括：

- [ ] 多提供商支持 (Anthropic, OpenRouter, etc.)
- [ ] Prompt 缓存 (Anthropic)
- [ ] 上下文压缩 (Gemini 作为汇总服务)
- [ ] Gateway 多平台支持 (Discord, Telegram, etc.)
- [ ] 记忆系统 (Honcho 集成可选)
- [ ] 技能系统

**预计时间**: 6-8 周

---

## 💬 反馈和改进

这个计划不是固定的。如果你发现：

- ❌ 某个部分不可行
- ❌ 时间估计太乐观
- ❌ 优先级不对

请告诉我调整！

---

**准备开始？**

👉 打开 `IMPLEMENTATION_PLAN.md` 的**第 1 周**部分，按照指导逐步进行。

祝好运！🚀
