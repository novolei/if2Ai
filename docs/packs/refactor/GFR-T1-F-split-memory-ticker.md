# GFR-T1-F: Split `modules/memory/ticker.rs` (1363 LOC)

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-T1-E` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 T1-F 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- per-turn scheduler（`turns_per_summary`）
- session-end flush（`flush_session`）
- daily compile（rolling / week / longterm / facts）
- startup recovery（catch up rolling summaries）

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- `modules/memory/scheduler/{turn,session,daily,recovery,mod}.rs`
- 保留 `MemoryTicker` 顶层 API 不变

---

## Files (scope)

- TBD on activation

---

## Out of Scope

- ❌ 不改 8B.7 / 8B.8 / 8B.9 触发逻辑
- ❌ 不改 audit emit 调用模式
- ❌ 不改 TurnHook trait

---

## Activation Trigger

1. `GFR-T1-E` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
