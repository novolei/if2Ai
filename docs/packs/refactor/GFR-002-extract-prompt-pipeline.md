# GFR-002: Extract prompt assembly / sanitize / governor from `agent.rs`

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-001` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 002 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `commands/agent.rs` (≈3258–3905)
-   - `RequestPreflightStats` / `ContextGovernor` / `apply_request_preflight_limits`
-   - token & char count estimators / message summarization
-   - `SanitizationStats` / `sanitize_messages_for_provider` / `remove_tool_use_blocks`
-   - `extend_sample_ids` / `parse_tool_input_json`

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 扩展现有 `application/prompt_planner.rs`：拆出 `prompt_planner/preflight.rs`、`prompt_planner/sanitize.rs`、`prompt_planner/governor.rs`
- 原 god-file 留 `pub use crate::modules::application::prompt_planner::*;` shim

---

## Files (scope)

- commands/agent.rs

---

## Out of Scope

- ❌ 不改 token 估算口径（I6：函数体一行不动）
- ❌ 不动 `MAX_REQUEST_*` 常量值
- ❌ 不合并 sanitize 与 preflight 的逻辑边界

---

## Activation Trigger

1. `GFR-001` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
