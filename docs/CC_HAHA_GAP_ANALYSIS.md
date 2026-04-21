# cc-haha-main 与 If2Ai 完整差距分析报告

> **版本**: v1.0
> **日期**: 2026-04-21
> **范围**: cc-haha-main（TypeScript/Bun 全栈）vs If2Ai（Rust/React 全栈）
> **方法**: 基于双方代码库的架构对比、功能逐项对齐、已有 bs_gap 文档交叉验证

---

## 一、摘要

本报告对 **cc-haha-main**（纯 TypeScript 智能体应用）和 **If2Ai**（Rust + React 智能体桌面应用）进行全面差距分析。

### 核心发现

1. **架构哲学根本不同**：cc-haha 采用纯 TypeScript 全栈（CLI + WebSocket 服务器 + Tauri），代码统一但性能受限于 Bun 运行时；If2Ai 采用 Rust 后端 + React 前端，性能潜力大但跨语言集成复杂度高。

2. **Agent 循环成熟度差距显著**：cc-haha 的 QueryEngine.ts（1,296 行）实现了完整的 32 轮迭代工具循环、指数退避重试、成本追踪；If2Ai 存在两条 Agent 路径（`run_agent_turn` / `start_agent_stream`），流式路径完全缺失工具循环。

3. **安全是最大短板**：cc-haha 有完整的权限框架（4 种模式）、沙箱隔离、Keychain 凭证存储、符号链接逃逸检测；If2Ai 权限硬编码 `DangerFullAccess`，无沙箱，凭证以明文存储。

4. **If2Ai 在记忆系统和语音领域有独特优势**：14,000 行记忆模块（4 层降级链 + 编译记忆管线 + ThreatScanner）和本地 TTS/STT 是 cc-haha 完全不具备的能力。

5. **前端工程质量差距明显**：cc-haha 前端有 19 个 Zustand store、完整 i18n、26 个测试文件、TypeScript strict 模式；If2Ai 前端无 i18n、无测试、非 strict 模式、存在大量占位符 UI。

6. **基础设施是 If2Ai 的盲区**：cc-haha 有 CI/CD、自动发布脚本、VitePress 文档站、Issue 模板；If2Ai 无 CI/CD、仅有 macOS 发布脚本、文档分散。

---

## 二、对比维度表格

### 2.1 整体架构

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 架构模式 | CLI (Ink TUI) + WebSocket 服务器 + Tauri 桌面 | Tauri 2 桌面（Rust 后端 + React 前端） | 各有侧重 |
| 后端语言 | TypeScript（Bun 运行时） | Rust（Tokio 异步） | — |
| 前端框架 | React 18 + Zustand 5.0 | React 19 + 运行时投影 + UI 会话状态 | If2Ai 更新 |
| 代码规模 | ~59,561 行（产品 42,560 + 测试 17,001） | ~127,000 行 Rust + ~113 个 TS/TSX 文件 | If2Ai 更大 |
| 模块化 | 按功能目录分 9 个分类 | 16+ 有界上下文模块 | If2Ai 更严格 |
| 分层架构 | 功能导向分层 | 三层架构（入口→核心→基础设施） | If2Ai 更正式 |
| 进程模型 | CLI 进程 + WebSocket 服务器 + Tauri 渲染进程 | Tauri 主进程 + 渲染进程 | — |
| 多端支持 | CLI + 桌面 + WebSocket 远程 | 仅桌面 | cc-haha 更广 |

### 2.2 后端技术栈与实现

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 运行时 | Bun 1.x | Rust + Tokio | — |
| 包管理 | npm/pnpm | Cargo | — |
| 类型安全 | TypeScript strict | Rust 编译时保证 | — |
| 异步模型 | Bun 原生异步 | tokio 异步运行时 | — |
| 序列化 | JSON 原生 | serde_json | — |
| 并发安全 | 单线程事件循环 | Rust 所有权 + Arc<Mutex> | — |
| 热重载 | Bun 原生支持 | cargo-watch | — |
| 编译速度 | 即时 | 较慢（Rust 编译） | cc-haha 更快 |
| 运行时性能 | 受限于 V8/Bun | 原生性能 | If2Ai 更强 |

### 2.3 Agent 循环设计

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 核心引擎 | QueryEngine.ts（1,296 行） | ConversationRuntime（conversation.rs） | — |
| 迭代上限 | 32 轮 | 有 max_iterations 检查 | — |
| 流式支持 | 完整（SSE + WebSocket） | 双路径（run_agent_turn + start_agent_stream） | 🟠 If2Ai 双路径问题 |
| 工具循环 | 完整闭环 | 非流式路径可用，流式路径断裂 | 🔴 严重 |
| 系统提示 | 动态构建（instruction 文件 + git 上下文） | SystemPromptBuilder 存在但未使用，硬编码三行 | 🔴 严重 |
| 上下文压缩 | 内置 compaction | 代码存在但未接入 | 🟡 中 |
| 成本追踪 | CostTracker（Token 计数 + 美元成本 + 预算限制） | 无 | 🟠 缺失 |
| 中断能力 | 完整（取消/中断） | 部分（有 stop_agent_stream 但前端未接入） | 🟡 中 |
| 恢复能力 | /resume 加载历史会话 | 部分实现（session 恢复丢失 tool_call 上下文） | 🟡 中 |

### 2.4 工具系统

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 工具数量 | 57+ 内置工具 | 45+ 内置工具（两套系统共存 31 个独立工具） | — |
| 注册模式 | 统一 ToolSpec + match 分发 | 两套系统（ToolRegistry + GlobalToolRegistry） | 🔴 架构混乱 |
| 工具分类 | BashTool, FileEditTool, AgentTool, SkillTool, MCPTool, TaskTools, CronTools, LSPTool 等 | bash, file_read/write, glob_search, web_search/fetch, memory_store/recall, cron 等 | — |
| 异步执行 | 基于 Bun async | 基于 tokio async（ToolHandler trait） | — |
| 工具超时 | 全局超时 | per-tool timeout_secs | If2Ai 更精细 |
| 结果大小 | 全局限制 | per-tool max_result_size | If2Ai 更精细 |
| 动态注册 | 无（静态 match） | DashMap 动态注册 | If2Ai 更灵活 |
| SkillTool | ✅ 完整实现 | ❌ 完全缺失 | 🔴 缺失 |
| AgentTool | ✅ 嵌套 Agent 调用 | 代码存在但未注册到活跃路径 | 🟡 部分 |
| MCPTool | ✅ MCP 协议支持 | ❌ 完全缺失 | 🔴 缺失 |
| LSPTool | ✅ LSP 集成 | ❌ 缺失 | 🟠 缺失 |
| TaskTools | ✅ TodoWrite + TaskRead | TodoWrite 代码存在但未注册 | 🟡 部分 |
| CronTools | ✅ 定时任务 | ✅ 已实现 | — |
| ToolSearch | ✅ 工具搜索 | ❌ 未注册 | 🟡 缺失 |

### 2.5 记忆系统

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 存储方式 | 基于文件的 MEMORY.md | 4 层降级链（Hybrid → LanceDB → SQLite → InMemory） | ✅ If2Ai 领先 |
| 搜索能力 | Fuse.js 模糊搜索（25KB 上限） | 向量搜索 + 全文搜索 + 模糊搜索 | ✅ If2Ai 领先 |
| 向量数据库 | 无 | LanceDB | ✅ If2Ai 领先 |
| 记忆编译 | 无 | 编译记忆管线 | ✅ If2Ai 领先 |
| 安全扫描 | 无 | ThreatScanner（60+ 正则） | ✅ If2Ai 领先 |
| 代码规模 | ~200 行 | ~14,000 行 | ✅ If2Ai 远超 |
| 持久化 | 文件（MEMORY.md） | SQLite + LanceDB + JSON | ✅ If2Ai 更可靠 |
| 上下文窗口 | 25KB 上限 | 无硬性上限（由 context budget 控制） | ✅ If2Ai 更灵活 |

### 2.6 LLM 提供商支持

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 提供商数量 | 6 个（Anthropic, MiniMax, OpenRouter, AWS Bedrock, LiteLLM, Ollama） | 8+ 个（含设计文档定义） | — |
| 主提供商 | Anthropic | Anthropic | — |
| 故障转移 | 完整 FallbackChain | 代码存在，需验证 | 🟡 待验证 |
| 速率限制 | 内置 RateLimiter | 部分 | 🟡 部分 |
| 成本估算 | CostTracker 完整 | 无 | 🔴 缺失 |
| OAuth 支持 | 支持（Keychain 存储） | 仅 Bearer/API Key | 🟡 缺失 |
| 流式解析 | SseParser + Content-Length | 同 | — |
| 模型选择 | 运行时动态切换 | 存在但前端 UI 断开 | 🟡 UI 断开 |

### 2.7 安全与权限

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 权限模式 | 4 种（DangerFullAccess / RequireApproval / Allow / Deny） | 5 种（含 Prompt/ReadOnly/WorkspaceWrite）但硬编码 DangerFullAccess | 🔴 形同虚设 |
| 权限粒度 | 每工具独立 required_permission | 全局 permission_mode | 🔴 粒度不足 |
| 权限提示 | CliPermissionPrompter（终端交互） | Prompter 传 None，无交互 | 🔴 完全缺失 |
| 沙箱隔离 | SandboxConfig（Linux namespace + 文件系统隔离） | 无沙箱 | 🔴 完全缺失 |
| 凭证存储 | Keychain（macOS 安全存储） | 明文 JSON | 🔴 不安全 |
| 路径验证 | 符号链接逃逸检测 | 基础 workdir 检查 | 🟠 不足 |
| 前端权限 UI | 权限对话框（计算机自动化权限） | 权限对话框存在但功能不完整 | 🟡 部分 |

### 2.8 错误处理与容错

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 错误类型 | APIError 类体系 | RuntimeError 枚举 | — |
| 结果模式 | Result<T> 模式 | Result<T, E> 模式 | — |
| 重试策略 | 指数退避重试（可配置） | 代码存在，需验证 | 🟡 待验证 |
| 熔断器 | 有 | 设计文档定义 | 🟡 待验证 |
| 降级策略 | Provider 故障转移 | FallbackChain | 🟡 部分 |
| 错误日志 | 结构化日志 | tracing 框架 | — |
| 错误消息 | 英文（统一） | 混合中英文（硬编码中文） | 🟡 不统一 |

### 2.9 前端架构

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| React 版本 | React 18 | React 19 | If2Ai 更新 |
| 组件数量 | 9 个分类目录，34+ 聊天组件 | 7 个组件目录，7 个聊天组件 | cc-haha 更丰富 |
| 状态管理 | Zustand 5.0（19 个独立 store） | 双层架构（运行时投影 + UI 会话 slice） | 各有优劣 |
| 路由 | 状态驱动（TabBar 标签页管理） | 状态驱动（ProjectRail） | — |
| 组件设计 | 按功能分类（chat/permission/settings/cron/team/skill/automation） | 按功能分类（chat/browser/ds/memory/settings/theme/ui） | — |
| 代码组织 | 清晰的 store 分离 | App.tsx 超大文件（2,500+ 行） | 🔴 If2Ai 上帝文件 |

### 2.10 状态管理

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 方案 | Zustand 5.0 | 运行时投影（Reducer + useSyncExternalStore）+ conversation-slice + browser-slice | — |
| store 数量 | 19 个独立 store | 2 个 slice + 运行时投影 | — |
| 关注点分离 | 高（每 store 单一职责） | 中（App.tsx 集中管理大量状态） | 🟠 If2Ai 不足 |
| 持久化 | Zustand middleware | 无自动持久化 | 🟡 部分 |
| DevTools | Zustand DevTools | 无 | 🟡 缺失 |

### 2.11 前后端通信

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 通信方式 | HTTP REST (127.0.0.1:3456) + WebSocket | Tauri IPC（invoke + listen） | 架构差异 |
| 自动重连 | WebSocket 自动重连 + 心跳保活 | Tauri 事件自动管理 | — |
| 消息缓冲 | WebSocket 消息队列缓冲 | Tauri 事件队列 | — |
| 跨平台 | 支持 WebSocket 远程连接 | 仅本地 IPC | cc-haha 更灵活 |
| 类型安全 | 手动类型定义 | Tauri 命令自动推导 | — |

### 2.12 组件设计

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 聊天组件 | 34+ 组件（消息、输入、工具调用、思考展示等） | 7 个组件（ChatMessage 等） | 🔴 If2Ai 不足 |
| 权限组件 | 完整权限对话框 | 权限对话框存在但功能不完整 | 🟡 部分 |
| 设置组件 | 8 个标签页 | Settings 页面存在但内容有限 | 🟡 部分 |
| 记忆组件 | 无 | 8+ 组件（MemoryBrowser 等） | ✅ If2Ai 领先 |
| 浏览器组件 | 计算机自动化面板 | BrowserTab + 相关组件 | — |
| 工具调用展示 | streamingText + streamingToolInput | 无专门工具调用展示组件 | 🔴 缺失 |
| 项目导航 | 无专门组件 | ProjectRail（37.1KB） | ✅ If2Ai 领先 |

### 2.13 设计系统与样式

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| CSS 框架 | Tailwind CSS 4 | Tailwind v4 | — |
| 组件库 | 自定义组件 + Tailwind | ShadCN + Radix UI + Tailwind v4 | — |
| 设计系统 | CSS 变量 + Tailwind 集成 | Paico 设计系统（80+ CSS 令牌，翡翠薄雾色调） | — |
| 主题 | 深色/浅色主题 | 深色主题为主 | cc-haha 更完整 |
| 字体 | Inter/Manrope/JetBrains Mono | 系统字体 | cc-haha 更精致 |
| 图标 | Lucide React | Lucide React | — |

### 2.14 国际化

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| i18n 支持 | 完整（英文 + 中文） | 无 | 🔴 完全缺失 |
| API 风格 | 函数式（useTranslation / t()） | 无 | 🔴 缺失 |
| 语言切换 | 运行时切换 | 无 | 🔴 缺失 |
| 消息键 | 结构化命名空间 | 无 | 🔴 缺失 |

### 2.15 测试覆盖

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 前端测试 | 26 个测试文件（Vitest + React Testing Library） | 无前端测试 | 🔴 完全缺失 |
| 后端测试 | Vitest | 6 个集成测试 + 内联 #[test] | 🟠 不足 |
| 测试框架 | Vitest | Harness Python 框架 + Rust #[test] | — |
| E2E 测试 | 无 | 14 个 YAML 套件 | 🟡 各有侧重 |
| 测试工具 | React Testing Library | Harness gate/runner | — |
| 覆盖率 | chatStore 678 行测试 | 无系统化覆盖率 | 🔴 If2Ai 不足 |

### 2.16 文档体系

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 文档框架 | VitePress 1.6+（双语、搜索、Mermaid） | 多目录分散文档 | — |
| 文档数量 | 59 个 Markdown 文档 | 大量设计文档 + Pack 文档 | — |
| 搜索能力 | 本地搜索 | 无统一搜索 | 🟠 If2Ai 不足 |
| 图表支持 | Mermaid | 架构图 | — |
| 文档部署 | GitHub Pages 自动部署 | 无 | 🟡 缺失 |
| 开发流程 | 常规 | Pack 流水线（5 步） | If2Ai 更正式 |

### 2.17 CI/CD 与自动化

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| CI/CD | 3 个 GitHub Actions（Release Desktop, Dev Build, VitePress 部署） | 无 | 🔴 完全缺失 |
| 发布脚本 | scripts/release.ts（一键版本管理 + Git tag） | release-macos.sh | 🟠 不足 |
| 代码统计 | scripts/count-app-loc.ts | 无 | 🟡 缺失 |
| 架构 Lint | 无 | lint_architecture.py | ✅ If2Ai 领先 |
| Issue 模板 | Bug Report + Question | 无 | 🟡 缺失 |
| 多平台构建 | macOS ARM64/x64, Linux ARM64/x64, Windows x64 | macOS（仅 ARM64 确认） | 🟠 不足 |

### 2.18 多 Agent 协调

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 多 Agent 团队 | 完整的团队 UI 和协调 | Agent 工具存在但未注册 | 🟡 部分 |
| 嵌套 Agent | AgentTool 递归调用 | agent.rs 存在但深度限制为环境变量 | 🟡 部分 |
| Agent 发现 | /agents 命令列出可用 Agent | 无 | 🟡 缺失 |
| 并发 Agent | 支持 | 未验证 | 🟡 待验证 |

### 2.19 语音系统

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| TTS | 无 | MOSS-TTS-Nano ONNX | ✅ If2Ai 独有 |
| STT | 无 | SenseVoice ONNX | ✅ If2Ai 独有 |
| 本地推理 | 无 | ONNX Runtime | ✅ If2Ai 独有 |
| 语音代码规模 | 0 | ~15,000 行（tts + stt 模块） | ✅ If2Ai 远超 |

### 2.20 计算机自动化/浏览器

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| 浏览器自动化 | Computer Use（39.6KB 文档） | Chrome CDP 自动化 | — |
| 截图能力 | 有 | 有 | — |
| 鼠标/键盘控制 | 有 | 有 | — |
| 安全控制 | 计算机自动化权限对话框 | 部分 | 🟡 部分 |
| 代码规模 | 大型模块 | 9 个文件（browser 模块） | — |

### 2.21 代码质量工具

| 维度 | cc-haha-main | If2Ai | 差距评级 |
|------|-------------|-------|---------|
| TypeScript strict | 全部启用（noUnusedLocals, noUnusedParameters, noUncheckedIndexedAccess） | 非 strict 模式 | 🔴 不足 |
| Rust Lint | 无 | 编译时检查 + lint_architecture.py | ✅ If2Ai 更强 |
| 死代码检测 | TypeScript 编译器 | Rust 编译器 + 架构 Lint | — |
| 文件大小限制 | 无 | 800 行 god-file watchlist | ✅ If2Ai 更严格 |
| pub fn 文档 | 无强制 | 所有 pub fn 有 /// doc | ✅ If2Ai 更严格 |
| unwrap/expect | 无限制 | 非测试代码禁止 | ✅ If2Ai 更严格 |

---

## 三、差距分析（优先级排序）

### P0 — 紧急：严重影响产品可用性/可靠性

| 编号 | 差距 | 现状 | 影响 | 修复方案 | 预估工作量 |
|------|------|------|------|---------|----------|
| G-P0-1 | 流式路径工具循环断裂 | `start_agent_stream` 忽略 InputJsonDelta 和 ContentBlockStart(ToolUse)，无工具执行和 tool_result 回传 | 流式模式下 Agent 无法使用任何工具，产品核心功能失效 | 重写流式路径，实现完整工具循环（累积参数→提取 tool_use→权限检查→执行→回传结果→继续循环） | 5-8 人天 |
| G-P0-2 | 系统提示缺失/硬编码 | `start_agent_stream` 的 `system_prompt: None`，`run_agent_turn` 硬编码三行英文 prompt | 流式模式无行为约束，非流式模式无工作目录/git/instruction 上下文 | 两条路径统一使用 SystemPromptBuilder | 1-2 人天 |
| G-P0-3 | 权限系统形同虚设 | PermissionPolicy 硬编码 DangerFullAccess，Prompter 传 None | 所有工具调用无安全约束，用户敏感操作（如 rm -rf）无法阻止 | 实现权限模式配置 + TauriPermissionPrompter + 前端权限对话框 | 3-5 人天 |
| G-P0-4 | 两套工具系统共存 | ToolRegistry + GlobalToolRegistry 互不通信，11 个独有工具对主系统不可见 | 工具注册混乱，新增工具可能注册到错误系统 | 统一为单一 ToolRegistry，迁移 GlobalToolRegistry 的独有工具 | 5-8 人天 |

#### G-P0-1 详细分析：流式路径工具循环断裂

**代码位置**：`src-tauri/src/commands/agent.rs:686, 699-713`

**现状描述**：

```rust
// InputJsonDelta 被空处理
crate::modules::api::ContentBlockDelta::InputJsonDelta { .. } => {}  // line 686

// ContentBlockStart 只处理 Thinking，不处理 ToolUse
ApiStreamEvent::ContentBlockStart(start_event) => {
    if matches!(start_event.content_block, OutputContentBlock::Thinking { .. }) {
        // thinking_start 事件
    }
    // ToolUse 块完全没有处理！
}
```

**影响分析**：

- LLM 在流式模式下确实会收到工具定义（`tools: Some(tool_defs)` 在 agent.rs:593-619）
- 但当 LLM 返回 `tool_use` 时，`InputJsonDelta` 被忽略，工具参数丢失
- `ContentBlockStart` 中的 `ToolUse` 块被忽略，工具调用无法被识别
- 流式结束后只保存 `user_msg + assistant_msg`（纯文本），没有 `tool_result`
- **没有工具循环**——没有权限检查、没有工具执行、没有 `tool_result` 回传

**修复方案**（方案 A — 推荐短期）：

```
1. 在后台任务中累积 InputJsonDelta 到 tool_arguments 缓存
2. ContentBlockStop 时提取完整 tool_use
3. 执行 permission.authorize() + tool_execution.execute()
4. 将 tool_result 推入 session.messages
5. 继续发送下一轮 API 请求
6. 工具结果通过 SSE 推送给前端
```

**修复方案**（方案 B — 推荐长期）：

```
1. 废弃 run_agent_turn + start_agent_stream 双路径
2. 让 start_agent_stream 直接调用 ConversationRuntime::run_turn()
3. 通过回调机制将 tool_use/tool_result 事件推送给前端
4. 单一真相源，减少维护成本
```

---

#### G-P0-2 详细分析：系统提示缺失/硬编码

**代码位置**：`src-tauri/src/commands/agent.rs:361-365, 572`

**路径 A**（run_agent_turn）硬编码三行英文 prompt：

```rust
let system_prompt = vec![
    "You are If2Ai, a helpful AI assistant.".to_string(),
    "You have access to various tools to help the user.".to_string(),
    "Always be helpful, harmless, and honest.".to_string(),
];
```

**路径 B**（start_agent_stream）完全没有 system_prompt：

```rust
let api_request = ApiRequest {
    system_prompt: None,  // 🔴 无系统提示！
    messages: request_messages,
    tools: if tool_defs.is_empty() { None } else { Some(tool_defs) },
    stream: true,
};
```

**与 cc-haha 的差距**：

cc-haha 的 SystemPromptBuilder 会动态聚合：
1. 自定义 instruction 文件（`.codex/INSTRUCTION.md` 等）
2. git status 输出
3. git diff 输出
4. 工作目录信息
5. 工具使用指南

If2Ai 的 `SystemPromptBuilder` 已存在于 `modules/runtime/prompt.rs`，包含 `discover_instruction_files()`、`git_status()`、`git_diff()` 集成，但完全未被调用。

---

#### G-P0-3 详细分析：权限系统形同虚设

**代码位置**：`src-tauri/src/commands/agent.rs:358, 382`

**问题 1**：硬编码 DangerFullAccess

```rust
// agent.rs:358 — 🔴 问题
let permission_policy = PermissionPolicy::new(PermissionMode::DangerFullAccess);
```

所有工具调用都硬编码为最高权限，没有任何权限检查。

**问题 2**：Prompter 参数传 None

```rust
// agent.rs:382 — 🔴 问题
let result = runtime.run_turn(user_message.clone(), None);
//                                                 ↑ None！prompter 从未传入
```

由于 `prompter` 是 `None`，即使 `PermissionPolicy` 的算法要求 `Prompt`，也会静默降级。

**与 cc-haha 的对比**：

| 能力 | cc-haha | If2Ai |
|------|---------|-------|
| 权限模式 | 4 种（DangerFullAccess/RequireApproval/Allow/Deny） | 5 种但硬编码 DangerFullAccess |
| 工具级权限 | 每工具 required_permission | 无（全局 permission_mode） |
| 交互式确认 | CliPermissionPrompter（终端 y/n） | 无（Prompter=None） |
| 沙箱隔离 | SandboxConfig（Linux namespace） | 无 |
| 凭证存储 | Keychain（macOS 安全存储） | 明文 JSON |
| 符号链接检测 | 有 | 无 |

---

#### G-P0-4 详细分析：两套工具系统共存

**系统 A**（Phase 4 ToolRegistry）：
- `modules/tools/registry.rs` → `ToolRegistry`（DashMap 动态注册）
- `modules/tools/mod.rs` → `register_builtin_tools()` 注册 19 个工具
- async Handler 模式，支持 per-tool timeout/max_result_size/SharedToolContext

**系统 B**（GlobalToolRegistry — Baseline 副本）：
- `modules/tools/lib.rs` → `GlobalToolRegistry`
- 包含 `mvp_tool_specs()` 和 `execute_tool()` match 分发
- 包含 TodoWrite、Skill、Agent、ToolSearch 等 11 个独有工具
- **完全未被使用！** 没有任何代码调用 `GlobalToolRegistry`

**工具覆盖分析**：

| 分类 | 工具数 | 工具名 |
|------|--------|--------|
| 两者重叠 | 7 | bash, read_file, write_file, edit_file, glob_search, web_fetch/WebFetch, web_search/WebSearch |
| 仅 GlobalToolRegistry | 11 | grep_search, TodoWrite, Skill, Agent, ToolSearch, Sleep, SendUserMessage, Config, NotebookEdit, StructuredOutput, PowerShell |
| 仅 ToolRegistry (Phase 4) | 13 | cron_add/list/remove/run/runs, content_search, file_edit(disabled stub), http_request, memory_store/recall/forget/purge/export |
| **合计** | **31** | 7 去重 + 11 独有 + 13 独有 |

**迁移策略**：采用全量迁移方案，废弃 GlobalToolRegistry，将 11 个独有工具适配为 ToolHandler 模式注册到 ToolRegistry。

### P1 — 高优先级：重要功能缺失或明显落后

| 编号 | 差距 | 现状 | 影响 | 修复方案 | 预估工作量 |
|------|------|------|------|---------|-----------|
| G-P1-1 | Skill 系统完全缺失 | 无 Skill 工具注册、无发现路径、无 SKILL.md 解析 | 无法使用自定义技能，Agent 能力扩展受限 | 实现 SkillTool + discover_skill_roots + parse_skill_frontmatter | 3-5 人天 |
| G-P1-2 | SlashCommand 死代码 | 28 个命令代码完整但未暴露为 Tauri command | /help, /compact, /model 等核心命令无法使用 | 新建 commands/slash.rs，暴露 parse/list/suggest/execute | 2-3 人天 |
| G-P1-3 | 无成本追踪 | 无 Token 计数、无美元成本估算、无预算限制 | 用户无法了解 API 使用成本，可能产生高额账单 | 实现 CostTracker（参考 cc-haha 的 Token 计数 + 成本估算） | 3-5 人天 |
| G-P1-4 | 前端工具调用无可见性 | 无工具调用展示组件，用户无法看到 Agent 调用了哪些工具 | 用户体验差，无法理解 Agent 行为 | 实现渐进式工具调用展示（紧凑状态行→展开参数→完整 JSON） | 3-5 人天 |
| G-P1-5 | 上下文压缩未接入 | compact_session 代码存在但未在 Agent 路径调用 | 长对话导致 token 溢出，Agent 行为异常 | 在 run_turn 返回前调用 should_compact + compact_session | 0.5-1 人天 |
| G-P1-6 | 凭证明文存储 | API Key 等敏感信息以明文 JSON 存储 | 安全风险，凭证可能被恶意程序读取 | 使用 Keychain（macOS）或加密存储 | 2-3 人天 |
| G-P1-7 | 无 i18n 支持 | 所有 UI 文本硬编码中文 | 无法支持国际用户 | 实现 i18n 框架（参考 cc-haha 的 useTranslation / t() 模式） | 5-8 人天 |
| G-P1-8 | App.tsx 上帝文件 | 2,500+ 行，承担过多状态管理和 UI 逻辑 | 维护困难，容易引入 bug，违反单一职责 | 拆分为独立模块，状态提升到 store，UI 逻辑抽取到 hooks | 5-8 人天 |

### P2 — 中优先级：有价值的改进

| 编号 | 差距 | 现状 | 影响 | 修复方案 | 预估工作量 |
|------|------|------|------|---------|-----------|
| G-P2-1 | 无 CI/CD | 无 GitHub Actions，无自动构建/测试/发布 | 发布流程手动，质量无法自动保障 | 配置 GitHub Actions（build + test + release） | 2-3 人天 |
| G-P2-2 | 无前端测试 | 零个前端测试文件 | 前端代码无质量保障，重构风险高 | 引入 Vitest + React Testing Library，优先覆盖核心组件 | 5-8 人天 |
| G-P2-3 | TypeScript 非 strict | 未启用 strict 模式 | 类型安全不足，潜在运行时错误 | 启用 strict 模式 + 修复所有类型错误 | 3-5 人天 |
| G-P2-4 | 模型选择 UI 断开 | 前端有两个模型选择器但互不感知，后端从配置文件读取 | 用户看到的模型选择无效 | 提升为全局状态，前后端贯通 | 1-2 人天 |
| G-P2-5 | TodoWrite 未注册 | 代码存在于 GlobalToolRegistry 但未注册到活跃 ToolRegistry | Agent 无法管理任务列表，用户无法看到任务进度 | 迁移到 ToolRegistry + 实现 TodoPanel | 2-3 人天 |
| G-P2-6 | Session 恢复不完整 | 历史会话丢失 tool_call 上下文 | 用户看到不完整的对话历史 | 恢复时遍历所有 ContentBlock 类型 | 1-2 人天 |
| G-P2-7 | 无流式中断 | 用户无法取消正在进行的 Agent 响应 | 错误 prompt 导致长时间等待 | 实现 Stop 按钮 + 后端取消信号 | 1-2 人天 |
| G-P2-8 | 多平台构建不足 | 仅确认 macOS ARM64 | 无法覆盖 Windows/Linux 用户 | 配置跨平台 GitHub Actions | 3-5 人天 |
| G-P2-9 | 无 MCP 协议支持 | MCP 完全缺失 | 无法集成外部工具服务器 | 实现 MCP stdio 传输协议 | 8-12 人天 |
| G-P2-10 | 占位符 UI 过多 | Skills/Automation 图标、header 按钮等无功能 | 虚假交互比没有更糟糕 | 移除或 disable 无功能 UI 元素 | 0.5-1 人天 |

### P3 — 低优先级：锦上添花的改进

| 编号 | 差距 | 现状 | 影响 | 修复方案 | 预估工作量 |
|------|------|------|------|---------|-----------|
| G-P3-1 | 无沙箱隔离 | 仅靠 cd 限制 workdir | 高级安全隔离缺失 | 实现 SandboxConfig（Linux namespace + 文件系统隔离） | 8-12 人天 |
| G-P3-2 | 无 OAuth 登录 | 仅支持 Bearer/API Key | 限制了需要 OAuth 的提供商集成 | 实现 OAuth 流程 + Keychain token 存储 | 3-5 人天 |
| G-P3-3 | 无插件系统 | 完全缺失 | 第三方扩展能力受限 | 实现插件发现/加载/生命周期管理 | 10-15 人天 |
| G-P3-4 | 无 VitePress 文档站 | 文档分散在多目录 | 外部用户难以查阅 | 搭建 VitePress 文档站 + 自动部署 | 3-5 人天 |
| G-P3-5 | 错误消息国际化 | 混合中英文 | 不符合国际化标准 | 后端统一英文 + 前端本地化映射 | 1-2 人天 |
| G-P3-6 | 无 Issue 模板 | 无标准化 Issue 报告格式 | Bug 报告质量参差不齐 | 添加 Bug Report + Feature Request 模板 | 0.5 人天 |
| G-P3-7 | 无自动发布脚本 | 仅 release-macos.sh | 版本管理手动，容易遗漏 | 实现 TypeScript/Rust 一键发布脚本 | 2-3 人天 |
| G-P3-8 | 无 LSP 集成 | LSPTool 缺失 | 无法提供代码智能感知 | 实现 LSPTool（LSP 客户端 + 请求代理） | 5-8 人天 |
| G-P3-9 | 无深度/浅色主题切换 | 仅深色主题 | 用户偏好受限 | 实现浅色主题 + 切换功能 | 2-3 人天 |
| G-P3-10 | Session 与 Project 强绑定 | 所有 Session 必须关联 Project | 架构灵活性受限 | 支持全局/临时会话 | 1-2 人天 |

---

## 四、If2Ai 优势领域

以下方面 If2Ai 已经超越 cc-haha-main，应继续巩固和扩大优势：

### 4.1 记忆系统 🏆

If2Ai 的记忆系统是其核心竞争力，远超 cc-haha 的基于文件的 MEMORY.md 方案：

| 能力 | cc-haha | If2Ai |
|------|---------|-------|
| 存储层级 | 单层文件 | 4 层降级链 |
| 向量搜索 | 无 | LanceDB |
| 全文搜索 | 无 | SQLite FTS |
| 记忆编译 | 无 | 编译记忆管线 |
| 安全扫描 | 无 | ThreatScanner（60+ 正则） |
| 代码规模 | ~200 行 | ~14,000 行 |

**建议**：将记忆系统封装为可复用的 Rust crate，作为独立产品能力推广。

### 4.2 语音系统 🏆

If2Ai 是唯一具备本地语音能力的项目：

| 能力 | cc-haha | If2Ai |
|------|---------|-------|
| TTS | 无 | MOSS-TTS-Nano ONNX |
| STT | 无 | SenseVoice ONNX |
| 本地推理 | 无 | ONNX Runtime |
| 隐私保护 | 依赖云端 | 全本地推理 |

**建议**：结合 STT 实现语音输入，结合 TTS 实现语音播报，形成差异化体验。

### 4.3 架构严格性 🏆

If2Ai 在架构治理方面更成熟：

| 能力 | cc-haha | If2Ai |
|------|---------|-------|
| 有界上下文 | 功能目录 | 16+ 有界上下文模块 |
| 依赖方向 | 无强制约束 | 严格分层依赖 |
| 文件大小限制 | 无 | 800 行 god-file watchlist |
| pub fn 文档 | 无强制 | 所有 pub fn 有 /// doc |
| unwrap/expect | 无限制 | 非测试代码禁止 |
| 架构 Lint | 无 | lint_architecture.py |
| 开发流程 | 常规 | Pack 流水线（5 步） |

**建议**：将架构 Lint 和 Pack 流水线标准化，作为项目工程实践输出。

### 4.4 项目导航 🏆

If2Ai 的 ProjectRail（37.1KB）提供了独特的项目空间管理：

- 项目/会话树形导航
- 项目级配置和上下文
- 多项目并行管理

**建议**：继续深化项目空间概念，增加项目级记忆和技能继承。

### 4.5 浏览器自动化 🏆

If2Ai 的浏览器模块（9 个文件）与 cc-haha 的 Computer Use 能力相当，但深度集成在 Rust 后端：

- Chrome CDP 自动化
- 截图和页面操作
- 与 Agent 循环深度集成

**建议**：增加浏览器操作的安全提示和权限控制。

---

## 五、行动计划

### Phase 0：紧急修复（1-2 周）

| 行动 | 差距编号 | 负责领域 | 优先级 |
|------|---------|---------|--------|
| 修复流式路径工具循环 | G-P0-1 | 后端 | 最高 |
| 接入 SystemPromptBuilder | G-P0-2 | 后端 | 最高 |
| 实现权限模式配置 | G-P0-3 | 全栈 | 最高 |
| 统一工具注册系统 | G-P0-4 | 后端 | 最高 |

### Phase 1：核心功能补全（2-4 周）

| 行动 | 差距编号 | 负责领域 | 优先级 |
|------|---------|---------|--------|
| 实现 Skill 系统 | G-P1-1 | 全栈 | 高 |
| 暴露 SlashCommand | G-P1-2 | 全栈 | 高 |
| 实现成本追踪 | G-P1-3 | 后端 | 高 |
| 实现工具调用可见性 | G-P1-4 | 前端 | 高 |
| 接入上下文压缩 | G-P1-5 | 后端 | 高 |
| 实现凭证安全存储 | G-P1-6 | 后端 | 高 |

### Phase 2：工程质量提升（4-6 周）

| 行动 | 差距编号 | 负责领域 | 优先级 |
|------|---------|---------|--------|
| 拆分 App.tsx 上帝文件 | G-P1-8 | 前端 | 高 |
| 实现 i18n 框架 | G-P1-7 | 前端 | 高 |
| 配置 CI/CD | G-P2-1 | 基础设施 | 中 |
| 引入前端测试 | G-P2-2 | 前端 | 中 |
| 启用 TypeScript strict | G-P2-3 | 前端 | 中 |
| 修复模型选择 UI | G-P2-4 | 前端 | 中 |
| 注册 TodoWrite + TodoPanel | G-P2-5 | 全栈 | 中 |
| 修复 Session 恢复 | G-P2-6 | 前端 | 中 |
| 实现流式中断 | G-P2-7 | 全栈 | 中 |
| 清理占位符 UI | G-P2-10 | 前端 | 中 |

### Phase 3：生态扩展（6-12 周）

| 行动 | 差距编号 | 负责领域 | 优先级 |
|------|---------|---------|--------|
| 多平台构建 | G-P2-8 | 基础设施 | 中 |
| MCP 协议支持 | G-P2-9 | 后端 | 中 |
| 沙箱隔离 | G-P3-1 | 后端 | 低 |
| OAuth 登录 | G-P3-2 | 全栈 | 低 |
| 插件系统 | G-P3-3 | 全栈 | 低 |
| VitePress 文档站 | G-P3-4 | 文档 | 低 |
| LSP 集成 | G-P3-8 | 后端 | 低 |
| 浅色主题 | G-P3-9 | 前端 | 低 |

### 工作量总估算

| 阶段 | 预估工作量 | 核心目标 |
|------|-----------|----------|
| Phase 0 | 14-23 人天 | 产品可用性 |
| Phase 1 | 17-28 人天 | 核心功能 |
| Phase 2 | 23-36 人天 | 工程质量 |
| Phase 3 | 32-50 人天 | 生态扩展 |
| **总计** | **86-137 人天** | — |

---

### 关键路径风险矩阵

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 用户发送敏感指令（如 `rm -rf /`）时 bash 无权限阻止 | 高 | 严重 | Phase 0 修复 G-P0-3 |
| 流式模式下 Agent 无法使用工具 | 高 | 严重 | Phase 0 修复 G-P0-1 |
| 长对话导致 token 溢出崩溃 | 中 | 严重 | Phase 1 修复 G-P1-5 |
| 两套工具系统导致注册混乱 | 低 | 中 | Phase 0 修复 G-P0-4 |
| 流式模式无系统提示导致低质量回复 | 高 | 中 | Phase 0 修复 G-P0-2 |
| 窗口关闭导致 session 数据丢失 | 中 | 中 | Phase 2 修复 G-P2-6 |
| 无 CI/CD 导致发布质量无法保障 | 中 | 中 | Phase 2 修复 G-P2-1 |
| 凭证明文存储被恶意程序读取 | 低 | 严重 | Phase 1 修复 G-P1-6 |

---

### 与已有 bs_gap 文档的交叉引用

本报告与项目已有的 `docs/bs_gap/` 系列文档高度相关。以下是映射关系：

| 本报告编号 | bs_gap 对应文档 | bs_gap 编号 | 一致性 |
|-----------|----------------|------------|--------|
| G-P0-1 | 02-agent-loop-gap.md | F2 | ✅ 完全一致 |
| G-P0-2 | 10-comprehensive-audit.md | N5, N1 | ✅ 完全一致 |
| G-P0-3 | 04-permissions-gap.md | F3, F4 | ✅ 完全一致 |
| G-P0-4 | 10-comprehensive-audit.md | N3 | ✅ 完全一致 |
| G-P1-1 | 09-skills-commands-gap.md | F6 | ✅ 完全一致 |
| G-P1-2 | 09-skills-commands-gap.md | F5 | ✅ 完全一致 |
| G-P1-4 | 06-frontend-gap.md | F7 | ✅ 完全一致 |
| G-P1-5 | 05-session-gap.md | F9 | ✅ 完全一致 |
| G-P2-5 | 10-comprehensive-audit.md | UI-8 | ✅ 完全一致 |
| G-P2-6 | 10-comprehensive-audit.md | UI-6 | ✅ 完全一致 |
| G-P2-7 | 10-comprehensive-audit.md | UI-7 | ✅ 完全一致 |
| G-P2-9 | 07-mcp-plugin-gap.md | F20 | ✅ 完全一致 |

**说明**：本报告的差距分析与 bs_gap 系列文档的核心发现完全一致，但视角更广——bs_gap 以 CLAW-CLI 为基准，本报告以 cc-haha-main（包含前端和基础设施）为基准，因此新增了前端架构、国际化、CI/CD、语音系统等维度的对比。

---

### 工具覆盖详细对比

以下是 cc-haha 和 If2Ai 工具覆盖的完整对比：

| 工具类别 | cc-haha 工具 | If2Ai 工具 | 差距 |
|---------|------------|-----------|------|
| **文件操作** | read_file, write_file, edit_file, NotebookEdit | file_read, file_write, file_edit(disabled) | If2Ai 缺 NotebookEdit |
| **搜索** | glob_search, grep_search, ToolSearch, SkillSearch | glob_search, content_search | If2Ai 缺 grep_search, ToolSearch, SkillSearch |
| **Shell** | bash, PowerShell | bash | If2Ai 缺 PowerShell |
| **网络** | WebFetch, WebSearch | web_fetch, web_search, http_request | If2Ai 多 http_request |
| **Agent** | AgentTool | agent(未注册) | If2Ai 需注册 |
| **技能** | SkillTool | 无 | 🔴 If2Ai 完全缺失 |
| **任务** | TodoWrite, TaskRead | 无(代码存在未注册) | 🟡 If2Ai 需注册 |
| **定时** | CronTools | cron_add/list/remove/run/runs | ✅ 对等 |
| **记忆** | 无 | memory_store/recall/forget/purge/export | ✅ If2Ai 独有 |
| **MCP** | MCPTool | 无 | 🔴 If2Ai 完全缺失 |
| **LSP** | LSPTool | 无 | 🟠 If2Ai 缺失 |
| **通信** | SendUserMessage | 无 | 🟡 If2Ai 缺失 |
| **配置** | Config | config | ✅ 对等 |
| **输出** | StructuredOutput | 无 | 🟡 If2Ai 缺失 |
| **等待** | Sleep | 无 | 🟡 If2Ai 缺失 |

---

### 前端组件覆盖详细对比

| 功能领域 | cc-haha 组件数 | If2Ai 组件数 | 差距描述 |
|---------|-------------|------------|----------|
| 聊天界面 | 34+ | 7 | If2Ai 缺少工具调用展示、流式渲染、思考展示等组件 |
| 设置页面 | 8 个标签页 | 2 个 | If2Ai 设置页功能不完整 |
| 权限对话框 | 完整（含计算机自动化） | 部分 | If2Ai 权限对话框存在但功能不完整 |
| 记忆浏览 | 无 | 8+ | ✅ If2Ai 独有 |
| 项目导航 | 无 | ProjectRail(37.1KB) | ✅ If2Ai 独有 |
| 浏览器自动化 | 计算机自动化面板 | BrowserTab | 基本对等 |
| 定时任务 | Cron UI | 无 UI | 🟡 If2Ai 缺 UI |
| 多 Agent | 团队 UI | 无 | 🟡 If2Ai 缺失 |
| 技能管理 | SkillsSettingsPage | 占位符 | 🟡 If2Ai 不完整 |
| 全局搜索 | 无 | GlobalSearch(9.3KB) | ✅ If2Ai 独有 |

---

## 六、结论与建议

### 6.1 战略定位

If2Ai 和 cc-haha 虽然都是智能体桌面应用，但战略定位不同：

- **cc-haha** 追求 **广度**：纯 TypeScript 全栈、多端支持（CLI + 桌面 + WebSocket）、丰富工具集（57+）、完整生态（CI/CD + 文档站 + Issue 模板）。
- **If2Ai** 追求 **深度**：Rust 高性能后端、高级记忆系统、本地语音能力、严格架构治理。

### 6.2 关键建议

1. **先修核心再追广度**：当前最紧迫的是修复 Agent 循环（G-P0-1, G-P0-2）和权限系统（G-P0-3），这些直接影响产品可用性和安全性。功能再多，核心循环断了都是零。

2. **发挥 Rust 优势**：记忆系统、语音系统、浏览器自动化是 If2Ai 的差异化能力。应继续深化这些领域，而不是简单追赶 cc-haha 的工具数量。

3. **前端需要系统性投资**：App.tsx 上帝文件、零测试、无 i18n、TypeScript 非 strict——这些不是小问题，而是前端工程化缺失的系统性表现。建议至少投入 20 人天进行前端重构。

4. **基础设施不是可选项**：CI/CD 不是锦上添花，而是质量保障的底线。没有自动构建和测试，每次发布都是赌博。建议 Phase 1 就配置基础 CI。

5. **安全是信任基石**：权限硬编码、凭证明文、无沙箱——这些安全缺陷如果不修，企业用户不会采用。建议在 Phase 0 至少修复权限系统。

6. **避免两线作战**：不要同时追 cc-haha 的广度和深化 If2Ai 的深度。建议按 Phase 0→1→2→3 顺序推进，每个阶段聚焦一个目标。

### 6.3 差距量化总结

| 领域 | cc-haha 领先 | If2Ai 领先 | 持平 |
|------|-------------|-----------|------|
| Agent 循环 | ✅ | | |
| 工具系统 | ✅ | | |
| 记忆系统 | | ✅ | |
| LLM 提供商 | 🟡 | | 🟡 |
| 安全与权限 | ✅ | | |
| 错误处理 | 🟡 | | 🟡 |
| 前端架构 | ✅ | | |
| 状态管理 | 🟡 | | 🟡 |
| 组件设计 | ✅ | 🟡 | |
| 设计系统 | 🟡 | 🟡 | |
| 国际化 | ✅ | | |
| 测试覆盖 | ✅ | | |
| 文档体系 | 🟡 | 🟡 | |
| CI/CD | ✅ | | |
| 语音系统 | | ✅ | |
| 浏览器自动化 | | | ✅ |
| 代码质量 | 🟡 | ✅ | |
| 多 Agent | 🟡 | | |

**cc-haha 领先 8 项，If2Ai 领先 3 项，持平/各有所长 7 项。**

If2Ai 当前处于“深度强、广度弱”的状态。建议优先修复核心缺陷（P0），补全关键功能（P1），然后在工程质量（P2）和生态扩展（P3）上逐步追赶。

### 6.4 技术债务热力图

根据差距的严重程度和修复紧迫性，以下是 If2Ai 各领域的技术债务热力图：

```
高热（需立即修复）:
  ├── Agent 循环（流式路径断裂）
  ├── 权限系统（形同虚设）
  └── 工具注册（两套系统共存）

中热（需尽快处理）:
  ├── 系统提示（硬编码/缺失）
  ├── 前端工程化（上帝文件/零测试/非strict）
  ├── 成本追踪（完全缺失）
  └── 凭证安全（明文存储）

低热（可计划排期）:
  ├── i18n（完全缺失）
  ├── CI/CD（完全缺失）
  ├── MCP/插件（完全缺失）
  └── 沙箱隔离（完全缺失）
```

### 6.5 与 cc-haha 的差异化路线建议

If2Ai 不应简单复制 cc-haha 的所有功能，而应在以下方向建立差异化壁垒：

1. **本地优先 (Local-First)**：
   - 利用 Rust 的性能优势，实现完全本地的 TTS/STT/记忆/推理
   - cc-haha 依赖云端 API，If2Ai 可以承诺“零数据外泄”
   - 目标：企业级隐私保障

2. **深度记忆**：
   - 将 14,000 行的记忆系统打造为核心卖点
   - 实现跨会话、跨项目的记忆继承
   - 目标：Agent 越用越懂用户

3. **严格安全**：
   - 修复权限系统后，成为“最安全的桌面 Agent”
   - 沙箱隔离 + Keychain + 路径验证 + ThreatScanner
   - 目标：企业合规首选

4. **项目空间**：
   - ProjectRail 是独特能力，cc-haha 没有项目概念
   - 深化项目级配置、记忆、技能继承
   - 目标：多项目开发者的最佳选择

### 6.6 长期架构演进建议

基于本差距分析的发现，以下是对 If2Ai 长期架构演进的思考：

1. **统一 Agent 路径**：当前的双路径（run_agent_turn + start_agent_stream）是最大的架构债务。长期应统一为单一路径，通过回调机制将事件推送给前端。

2. **前端状态架构重构**：App.tsx 上帝文件必须拆分。建议参考 cc-haha 的 Zustand store 模式，每个关注点一个独立 store，通过中间件实现持久化和 DevTools。

3. **插件化工具系统**：统一 ToolRegistry 后，应设计插件化接口，让第三方工具可以通过标准协议（如 MCP）注册，而非修改核心代码。

4. **事件驱动架构**：前后端通信应从“命令式”向“事件驱动”演进。Tauri 的 listen/emit 已经提供了基础，需要设计标准化的领域事件协议。

5. **可观测性栈**：当前仅有 tracing 日志。应增加指标收集（token 使用、工具调用频率、错误率）和追踪（请求链路追踪），为自学习系统提供数据基础。

---

> 本报告基于 2026-04-21 的代码状态编写，随着双方项目的演进，部分结论可能需要更新。
