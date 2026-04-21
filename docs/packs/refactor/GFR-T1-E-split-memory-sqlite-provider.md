# GFR-T1-E: Split `modules/memory/providers/sqlite_provider.rs` (1579 LOC)

## Status
- State: `pending`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-T1-D` must be `done` first
- Last Updated: `2026-04-21`

> ⚠️ STUB — Agent 禁止执行。激活前必须按 CHARTER §4.1 补 `## Execute Plan` 与 `## Verify Whitelist` 两节，并把 State 改为 `active`。


---

## Goal

按 GFR roadmap 第 T1-E 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- schema + table DDL
- CRUD
- index（embedding / fts）
- scoring
- migration

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- `modules/memory/providers/sqlite/{schema,crud,index,scoring,migration,mod}.rs`
- 原 `sqlite_provider.rs` 退化为 shim

---

## Files (scope)

- TBD on activation

---

## Out of Scope

- ❌ 不改任何 sqlite 表名 / 列名（I4）
- ❌ 不改 migration 顺序
- ❌ 不改 scoring 算法

---

## Activation Trigger

1. `GFR-T1-D` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
