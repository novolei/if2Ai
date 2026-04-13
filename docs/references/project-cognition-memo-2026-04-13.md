# If2Ai 项目认知备忘录（2026-04-13）

## 1. 目标与当前核心任务

If2Ai 当前最关键目标是：将 `rust/` 中 Claw-CLI 的核心能力完整迁移到桌面形态（`src-tauri` + `src`），实现“CLI 等价能力 + 可视化交互 + 多项目会话管理”的 Desktop App。

当前代码库已经具备可运行的桌面主路径，但仍保留 CLI 备份实现与部分未完全对齐模块，处于“可用 + 持续对齐”的阶段。

---

## 2. 我对项目整体架构的认知

### 2.1 三条主线

1. `src/`：React 前端（聊天 UI、项目与会话管理、设置窗口）
2. `src-tauri/`：Rust 后端（Tauri 命令网关 + runtime/api/tools/session/projects）
3. `rust/`：CLI 备份源代码（workspace 多 crate，包含 `claw-cli`、`runtime`、`tools`、`commands`、`plugins`、`lsp`、`server`）

### 2.2 当前运行入口

- 桌面 App 真正执行入口：`src-tauri/src/main.rs`
- 前端主入口：`src/main.tsx` + `src/App.tsx`
- CLI 入口（备份）：`rust/crates/claw-cli/src/main.rs`

结论：当前“生产主路径”是桌面端，CLI 作为对照与能力来源仍非常重要。

---

## 3. 桌面端主干模块职责（已形成）

### 3.1 Tauri 命令层

`src-tauri/src/commands/` 提供前后端 IPC：

- `agent.rs`：`run_agent_turn`、`start_agent_stream`、`stop_agent_stream`、权限回调
- `session.rs`：会话 CRUD / 查询
- `project.rs`：项目 CRUD、Finder 打开、worktree 创建
- `tools.rs`：工具执行与列表
- `slash.rs`：桌面端简化版 slash 命令
- `window.rs`：设置窗口开关

### 3.2 Runtime + API + Tools

- `modules/runtime/`：会话对话循环、上下文压缩、权限策略、MCP、沙箱等
- `modules/api/`：模型 provider 客户端与流式事件
- `modules/tools/`：`ToolRegistry` + builtin 工具集合
- `modules/session/manager.rs`：会话持久化
- `modules/projects/manager.rs`：项目与会话绑定

### 3.3 前端能力

`src/App.tsx` + `src/lib/tauri.ts` 已打通：

- 流式对话事件监听（含 token/thinking/tool 状态）
- Stop 中断流式任务
- 项目/会话列表与恢复
- ToolResult 消息渲染
- 设置窗口打开与分窗口渲染

---

## 4. 业务流程（端到端）

1. 用户在前端发送消息（`App.tsx`）
2. 前端通过 `invoke('start_agent_stream')` 调后端（`src/lib/tauri.ts`）
3. 后端在 `commands/agent.rs` 恢复 session、构造模型请求、注入工具定义
4. 模型流式返回事件，后端逐步 `emit("agent-token", payload)`
5. 若模型触发 tool call，后端通过 `ToolRegistry::dispatch` 执行并回注结果
6. 前端实时更新 assistant/tool 消息
7. 后端周期性 + 结束时保存 session
8. 用户可调用 `stop_agent_stream` 取消进行中的流式任务

---

## 5. CLI 功能到桌面的迁移状态

## 已基本对齐

- 对话运行时主路径（session + stream + tools + persistence）
- ToolRegistry 与大量 builtin 工具
- 项目/会话管理
- 流式中断（Stop）
- 基本 slash 能力（`/help`、`/clear`、`/skills`、`/agents`）

## 部分对齐（能力缩水）

- `commands`：CLI 版命令生态更完整，桌面版 slash 当前为轻量子集
- `plugins`：CLI 版插件体系完整，桌面侧框架存在但未完全接入主编译路径

## 尚未完全迁移

- LSP：桌面端 `runtime/lsp.rs` 仍是 stub；完整实现仍在 `rust/crates/lsp`
- API Server 入口：`rust/crates/server` 尚未并入桌面后端依赖链

---

## 6. 当前“实现状态”判断

### 6.1 工程阶段判断

- `docs/exec-plans/index.md` 显示 Phase 5A~5E 已完成
- `phase-5e-long-tail-enhancement.yaml` 的 slices 均为 `done`
- dashboard 标注等待 human review

### 6.2 关键风险/技术债

1. 文档与代码存在漂移（例如 `ARCHITECTURE.md` 仍含旧路径/旧前端描述）
2. 设置页配置与后端真实 LLM 配置源（`~/.claude/settings.json`）可能存在脱节
3. `src-tauri/modules/commands/lib.rs`、`plugins/lib.rs` 存在“文件在但未完整挂载”风险，容易造成误判“已迁移”

---

## 7. 对“第一要务（还原 Claw-CLI 到桌面）”的执行建议

建议按以下顺序推进，避免“看似完成，实际不等价”：

1. **命令体系对齐**：以 `rust/crates/commands` 为基准做命令矩阵，补齐桌面 slash 能力
2. **插件体系对齐**：明确 `src-tauri/modules/plugins` 的编译接入策略，逐项验证 plugin lifecycle/hooks/tool
3. **LSP 正式迁移**：替换 stub，接入 `lsp` 真能力（诊断、符号、上下文增强）
4. **配置统一**：打通设置 UI 与后端真实配置读写，消除“双配置源”
5. **文档回收对齐**：更新 `ARCHITECTURE.md` 和入口点文档，确保“文档即真实状态”
6. **回归基线**：建立“CLI vs Desktop 功能等价清单 + 自动化 suite”作为长期 gate

---

## 8. 我对项目当前成熟度的结论

If2Ai 已从“纯 CLI 迁移期”进入“桌面可用期”，核心交互和主链路已落地；但若目标是“完整还原 Claw-CLI 功能到桌面”，目前仍处于 70%~85% 区间，主要差距集中在 **完整命令生态、插件深度、LSP 真集成、配置统一与文档一致性**。

一句话总结：**主干已成，长尾和等价性收口是下一阶段主战场。**
