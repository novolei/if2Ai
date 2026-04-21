# GFR-006: Extract `ToolRegistryExecutor` + control-plane switches + trajectory writer

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-005` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 006 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `commands/agent.rs` (≈293–327) `ControlPlaneRuntimeSwitches` + `load_control_plane_switches`
- `commands/agent.rs` (≈483–706) `ToolRegistryExecutor`
- `commands/agent.rs` (≈707–805) tool 启发式（`is_mutating_tool_success` 等）
- `commands/agent.rs` (≈904–943) `record_trajectory_if_possible`

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 新建 `application/tool_executor.rs`：吸收 `ToolRegistryExecutor` + switches + 启发式
- 新建 `application/trajectory_service.rs`：吸收 trajectory writer
- `agent.rs` 留 shim use

---

## Files (scope)

- commands/agent.rs

---

## Out of Scope

- ❌ 不改 `ToolError` / `ToolExecutor` trait 定义
- ❌ 不改 trajectory JSONL 文件格式
- ❌ 不动 control-plane env var 名（`IF2AI_CONTROL_PLANE_V2_ENABLED` 等）

---

## Activation Trigger

1. `GFR-005` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
