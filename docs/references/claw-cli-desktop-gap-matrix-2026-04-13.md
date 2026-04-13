# Claw-CLI → Desktop 功能对照清单（2026-04-13）

## 1. 目的与范围

本文用于把 `rust/`（Claw-CLI 备份能力）与当前桌面实现（`src-tauri` + `src`）做逐项对照，输出：

- 当前能力状态（已实现 / 部分实现 / 未实现）
- 关键证据路径（代码文件）
- 迁移优先级与粗略工时（人天）

---

## 2. 基线与对照对象

- CLI 基线入口：`rust/crates/claw-cli/src/main.rs`
- CLI 命令基线：`rust/crates/commands/src/lib.rs`
- CLI 工具基线：`rust/crates/tools/src/lib.rs`
- Desktop 入口：`src-tauri/src/main.rs`
- Desktop Agent 主链路：`src-tauri/src/commands/agent.rs`
- Desktop 工具注册：`src-tauri/src/modules/tools/mod.rs`
- Desktop slash：`src-tauri/src/commands/slash.rs`

---

## 3. 总体结论（TL;DR）

1. Desktop 主流程已打通（流式、工具、会话持久化、项目管理），可稳定承载日常对话。
2. 与 Claw-CLI 的主要差距在 **完整 slash 命令生态、插件深度集成、LSP 真能力、配置统一**。
3. 当前更像“核心可用 + 长尾收口阶段”，若目标是功能等价，建议按 4 个波次推进（见第 6 节）。

---

## 4. 能力完成度矩阵

## A. 核心运行链路

| 能力       | CLI                                       | Desktop                                                  | 状态     | 说明                             |
| ---------- | ----------------------------------------- | -------------------------------------------------------- | -------- | -------------------------------- |
| 对话主循环 | `rust/crates/runtime/src/conversation.rs` | `src-tauri/src/modules/runtime/conversation.rs`          | 部分实现 | 主循环已迁；细节策略仍需持续对齐 |
| 流式输出   | `rust/crates/claw-cli/src/main.rs`        | `src-tauri/src/commands/agent.rs`                        | 已实现   | 含 token/thinking/tool 状态事件  |
| 流式中断   | CLI 通过终端控制                          | `stop_agent_stream` in `src-tauri/src/commands/agent.rs` | 已实现   | 前端已接 Stop                    |
| 会话持久化 | `rust/crates/runtime/src/session.rs`      | `src-tauri/src/modules/session/manager.rs`               | 已实现   | 桌面会话恢复链路完整             |
| 权限策略   | `rust/crates/runtime/src/permissions.rs`  | `src-tauri/src/modules/runtime/permissions.rs`           | 部分实现 | 基础可用，仍需深度策略一致性核验 |

## B. Slash 命令（CLI 对照）

CLI 命令基线来自 `rust/crates/commands/src/lib.rs`（`SlashCommandSpec`）。

| 命令               | Desktop 状态 | 证据                              |
| ------------------ | ------------ | --------------------------------- |
| `/help`            | 已实现       | `src-tauri/src/commands/slash.rs` |
| `/clear`           | 已实现       | `src-tauri/src/commands/slash.rs` |
| `/skills`          | 已实现       | `src-tauri/src/commands/slash.rs` |
| `/agents`          | 已实现       | `src-tauri/src/commands/slash.rs` |
| `/status`          | 未实现       | CLI 有，Desktop slash 未注册      |
| `/compact`         | 未实现       | CLI 有，Desktop slash 未注册      |
| `/model`           | 未实现       | CLI 有，Desktop slash 未注册      |
| `/permissions`     | 未实现       | CLI 有，Desktop slash 未注册      |
| `/cost`            | 未实现       | CLI 有，Desktop slash 未注册      |
| `/resume`          | 未实现       | CLI 有，Desktop slash 未注册      |
| `/config`          | 未实现       | CLI 有，Desktop slash 未注册      |
| `/memory`          | 未实现       | CLI 有，Desktop slash 未注册      |
| `/init`            | 未实现       | CLI 有，Desktop slash 未注册      |
| `/diff`            | 未实现       | CLI 有，Desktop slash 未注册      |
| `/version`         | 未实现       | CLI 有，Desktop slash 未注册      |
| `/bughunter`       | 未实现       | CLI 有，Desktop slash 未注册      |
| `/branch`          | 未实现       | CLI 有，Desktop slash 未注册      |
| `/worktree`        | 未实现       | CLI 有，Desktop slash 未注册      |
| `/commit`          | 未实现       | CLI 有，Desktop slash 未注册      |
| `/commit-push-pr`  | 未实现       | CLI 有，Desktop slash 未注册      |
| `/pr`              | 未实现       | CLI 有，Desktop slash 未注册      |
| `/issue`           | 未实现       | CLI 有，Desktop slash 未注册      |
| `/ultraplan`       | 未实现       | CLI 有，Desktop slash 未注册      |
| `/teleport`        | 未实现       | CLI 有，Desktop slash 未注册      |
| `/debug-tool-call` | 未实现       | CLI 有，Desktop slash 未注册      |
| `/export`          | 未实现       | CLI 有，Desktop slash 未注册      |
| `/session`         | 未实现       | CLI 有，Desktop slash 未注册      |
| `/plugin`          | 未实现       | CLI 有，Desktop slash 未注册      |

结论：CLI 28 个核心 slash 中，Desktop 当前仅实现 4 个，命令生态差距最明显。

## C. 工具系统

| 维度                                            | CLI                  | Desktop        | 状态     | 备注                                           |
| ----------------------------------------------- | -------------------- | -------------- | -------- | ---------------------------------------------- |
| 工具注册中心                                    | `GlobalToolRegistry` | `ToolRegistry` | 部分实现 | 架构重构后并非同一实现，需行为对齐             |
| 常用工具族（bash/file/glob/web/todo/skill）     | 有                   | 有             | 已实现   | 已在 `src-tauri/src/modules/tools/mod.rs` 注册 |
| 长尾工具（Agent/ToolSearch/REPL/PowerShell 等） | 有                   | 有             | 已实现   | 已见对应 builtin 注册                          |
| 工具语义一致性（参数/错误/边界）                | 基线                 | 待逐项核验     | 部分实现 | 建议做工具级回归矩阵                           |

## D. 插件、LSP、服务入口

| 能力                   | 状态             | 证据                                                                  | 判断             |
| ---------------------- | ---------------- | --------------------------------------------------------------------- | ---------------- |
| 插件体系（CLI 完整版） | Desktop 部分接入 | `rust/crates/plugins/src/lib.rs` vs `src-tauri/src/modules/plugins/*` | 需继续收口       |
| LSP                    | Desktop stub     | `src-tauri/src/modules/runtime/lsp.rs`                                | 明确未完成       |
| API Server 入口        | 与 Desktop 分离  | `rust/crates/server/src/lib.rs`                                       | 未并入桌面主路径 |

## E. 配置与体验一致性

| 项目                   | 状态     | 证据                                                             |
| ---------------------- | -------- | ---------------------------------------------------------------- |
| 后端模型配置源         | 已固定   | `src-tauri/src/commands/agent.rs` 读取 `~/.claude/settings.json` |
| 设置 UI 与后端配置联动 | 部分实现 | `src/modules/settings/*` 以前端展示为主，需补写回链路            |

---

## 5. 优先级排序（落地建议）

1. **P0：Slash 命令生态补齐**（先做高频命令）
2. **P0：插件体系完整接入与可观测**
3. **P1：LSP 真集成（替换 stub）**
4. **P1：设置 UI 与后端配置统一**
5. **P2：文档回收，消除架构漂移**
6. **P2：CLI↔Desktop 自动化等价回归**

---

## 6. 分阶段执行计划（建议版）

## Wave 1（高优先，约 6-9 人天）

- 补齐高频 slash：`/status` `/model` `/permissions` `/cost` `/config` `/memory` `/diff` `/version`
- 为每个命令定义 Desktop 行为契约（输入、输出、错误码）
- 增加前端命令面板与结果渲染规范

## Wave 2（高优先，约 5-8 人天）

- 插件模块编译链路收口：保证 `modules/plugins` 的真实运行路径明确
- 对齐 plugin lifecycle/hooks/tool command 的桌面调用路径
- 增加插件安装/启停/故障诊断最小闭环

## Wave 3（中优先，约 4-6 人天）

- 用 `rust/crates/lsp` 能力替换 `src-tauri/src/modules/runtime/lsp.rs` stub
- 打通 diagnostics / symbol / prompt enrichment 的可用链路

## Wave 4（中优先，约 3-5 人天）

- 设置页改造：把 UI 配置与 `~/.claude/settings.json` 或统一配置层联动
- 补齐读写校验、错误提示、热更新策略
- 更新架构文档，建立“文档与代码一致性检查”

> 总体粗估：**18-28 人天**（单人串行），并行可缩短自然日。

---

## 7. 验收标准（建议）

1. 命令等价：CLI 核心 slash 覆盖率 >= 90%
2. 插件等价：核心插件生命周期和工具注入路径可复现
3. LSP 可用：诊断与符号查询可在桌面路径完成闭环
4. 配置一致：设置 UI 修改后能被后端真实调用读取
5. 回归通过：新增“CLI vs Desktop 等价 suite”全部通过

---

## 8. 备注

- 本清单聚焦“能力等价收口”，不替代 `exec-plan` 的 slice 级执行定义。
- 建议将本清单拆分成下一阶段 Phase 的 slice 输入，并与 `harness/suites` 绑定验收。
