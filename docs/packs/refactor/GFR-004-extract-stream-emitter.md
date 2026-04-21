# GFR-004: Collapse散落 `app.emit` into stream emitter service

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-003` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 004 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `commands/agent.rs` 中所有 `app.emit("agent-token", ...)` / `MEMORY_AFTER_TURN_EVENT` 散点
- `runtime/stream_emitter.rs`（已有 282 行）作为 envelope 真实发射器

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- 新建 `application/stream_emitter_service.rs`：把 commands 层的散点 emit 收口为 envelope 调用
- 保留 `runtime/stream_emitter.rs` 不动（M1 已落）
- 原散点改为 `stream_emitter_service.emit_token(...)` 等 typed 方法

---

## Files (scope)

- commands/agent.rs
- runtime/stream_emitter.rs

---

## Out of Scope

- ❌ 不改任何事件名字符串字面量（I3 强约束）
- ❌ 不改 `RuntimeEventEnvelope` schema
- ❌ 不在本 GFR 内动前端 listener

---

## Activation Trigger

1. `GFR-003` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
