# GFR-T1-A: Split `modules/plugins/lib.rs` (2995 LOC)

## Status
- State: `cancelled`
- Charter: [../CHARTER.md](../CHARTER.md)
- Resolution: source files were dead orphan code; deleted in `chore(dead-code)` 2026-04-21
- Last Updated: `2026-04-21`

> ❌ CANCELLED — pack is moot.
>
> Investigation 2026-04-21 found that `modules/plugins/mod.rs` was a
> doc-only stub that never declared `pub mod lib;` or `pub mod hooks;`.
> The "2995 LOC god-file" was orphan code from a workspace migration
> ("Migrated from /rust/crates/plugins") and never compiled. The same
> applied to `modules/commands/lib.rs` (2667 LOC). Total ~6057 LOC of
> unreachable code was deleted in a single `chore(dead-code)` commit;
> no GFR slice was needed.
>
> If a plugin system is reintroduced later, write a fresh `FEAT-` pack
> rather than reviving the cancelled cluster — the deleted code drifted
> from the live module conventions established in M1 and is no longer a
> good starting point.


---

## Goal

按 GFR roadmap 第 T1-A 步：把 source 中列出的代码段以**纯位移**方式搬到 destination 模块；零业务变更。

---

## Source (candidate ranges — re-verify on activation)

- `src-tauri/src/modules/plugins/lib.rs` 全部
-   - manifest 解析
-   - registry（installed.json）
-   - settings.json
-   - marketplace（external/builtin/bundled）
-   - hooks（已 mod hooks）

> 行号会随前序 GFR 完成而漂移；激活时必须重新校准。

---

## Destination

- `modules/plugins/{manifest,registry,settings,marketplace}.rs`（hooks 已存在）
- `lib.rs` 留 `pub use` shim

---

## Files (scope)

- src-tauri/src/modules/plugins/lib.rs

---

## Out of Scope

- ❌ 不改 manifest schema 字段名
- ❌ 不改 marketplace 名常量（`EXTERNAL_MARKETPLACE` 等）

---

## Activation Trigger

1. `GFR-018（与 chat 主链解耦，可与 D 系列并行）` 状态变 `done`。
2. 重读 source god-file，校准 Source 行号。
3. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
4. 补写 `## Verify Whitelist`：列出 verify 报告中允许出现的 added 项（pub 符号 / 事件 / 测试）。
5. **校准 `## Files (scope)` 路径**为 repo-relative（如 `src-tauri/src/commands/agent.rs` 或 `src/components/ui/chat-ui.tsx`），含被搬源 + 所有 destination 文件。
6. 把 State 改为 `active`。

未完成上述 6 步的 stub，agent **禁止执行**。
