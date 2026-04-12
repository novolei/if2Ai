# If2Ai 桌面端 vs CLAW-CLI Baseline 完整差距审计报告

> 审计日期：2026-04-12
> 审计范围：前后端完整代码库 vs CLAW-CLI 基准实现
> 报告路径：`docs/bs_gap/`

---

## 执行摘要

本次审计对 If2Ai 桌面端（Tauri + React）与 CLAW-CLI（Rust CLI）进行了 1:1 全流程节点对比。共发现 **5 个阻塞级问题（🔴）**、**9 个严重偏差（🟠）**、**7 个功能缺失（🟡）**，覆盖 Agent Loop、工具调用、权限系统、会话管理、流式处理、UI 功能、Skills/Commands 系统等核心模块。

**最核心的发现**：当前桌面端的 Agent 功能**同时存在两个互补的致命缺陷**：
1. 非流式路径（`run_agent_turn`）的工具调用循环**从未将工具定义发给 LLM**（`tools: None`）
2. 流式路径（`start_agent_stream`）虽然**把工具定义发给了 LLM**，但在流式响应中**完全忽略了所有 tool_use 事件**，工具从未被执行

**第二个核心发现**：SlashCommand 系统（28 个命令）的完整实现在 `src-tauri/src/modules/commands/lib.rs` 中已存在（代码级复制），但从未被 Tauri command 暴露给前端，前端也完全绕过了这套系统。SkillsSettingsPage 是纯占位符 UI。

这意味着：无论用户使用哪种路径，**Agent 都无法真正调用工具**，且 CLI 的全部交互命令能力在桌面端完全缺失。

---

## 报告索引

| 文件 | 内容 |
|------|------|
| [00-index.md](00-index.md) | 本索引 + 执行摘要 |
| [01-architecture-overview.md](01-architecture-overview.md) | 整体架构对比 |
| [02-agent-loop-gap.md](02-agent-loop-gap.md) | Agent Loop 核心流程偏差 |
| [03-tool-system-gap.md](03-tool-system-gap.md) | 工具注册/分发/执行偏差 |
| [04-permissions-gap.md](04-permissions-gap.md) | 权限系统偏差 |
| [05-session-gap.md](05-session-gap.md) | 会话管理偏差 |
| [06-frontend-gap.md](06-frontend-gap.md) | 前端 UI 功能缺失 |
| [07-mcp-plugin-gap.md](07-mcp-plugin-gap.md) | MCP / 插件系统偏差 |
| [08-critical-fix-priority.md](08-critical-fix-priority.md) | 修复优先级与实施路线图 |
| [09-skills-commands-gap.md](09-skills-commands-gap.md) | Skills/Commands 系统差距（SlashCommand 28 命令、Skill 工具） |
| [10-comprehensive-audit.md](10-comprehensive-audit.md) | 全流程细粒度审计：逐行扫描 + 1:1 节点对比 + 二次审查 + 12 项新发现 |
