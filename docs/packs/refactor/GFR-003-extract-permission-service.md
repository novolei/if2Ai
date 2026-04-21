# GFR-003: Extract `TauriPermissionPrompter` from `agent.rs`

## Status
- State: `active`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: GFR-002 series done
- Last Updated: `2026-04-21`

---

## Goal

把 `commands/agent.rs` 中 `TauriPermissionPrompter`（struct + 2 impls，~75 LOC）整体迁到 `application/permission_service.rs`。`respond_permission` Tauri 命令暂留 `commands/agent.rs`（它是 IPC 入口；future pack 决定是否搬到 commands/permission.rs）。

> 范围比 GFR-003 stub 小（不动 AppState.permission_senders / permission_overrides 所有权迁移），降低风险并保持稳定。AppState 迁移留给后续 pack。

---

## Source

`src-tauri/src/commands/agent.rs` (post-002b/c, agent.rs 现 3381 LOC)：

| 段 | 行号 | 内容 |
|---|---|---|
| `pub struct TauriPermissionPrompter` | ~3240–3244 | 3 字段 |
| `impl TauriPermissionPrompter { pub fn new }` | ~3246–3260 | inherent constructor |
| `impl PermissionPrompter for TauriPermissionPrompter` | ~3262–3305 | trait impl, emits `permission-request` event |

调用点（必须保持外部可访问）：
- `agent.rs:2199` — `TauriPermissionPrompter::new(...)`

---

## Destination

- 新建 `application/permission_service.rs` 承载 3 段
- `application/mod.rs` 加 `pub mod permission_service; pub(crate) use permission_service::TauriPermissionPrompter;`
- `agent.rs` 改 `use crate::modules::application::TauriPermissionPrompter;`

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/application/mod.rs
- src-tauri/src/modules/application/permission_service.rs

---

## Contract

- `TauriPermissionPrompter` 已经是 `pub`（commands/agent.rs 的 inherent pub 用于跨模块）。本 pack 把它从 `pub` 收紧为 `pub(crate)` 因 application/ 内的 service 不应跨 crate 暴露。该可见性收紧不影响 binary 调用者（仅 main.rs/commands/agent.rs/start_agent_stream 调用，全在 same crate）。
- `pub fn new` → `pub(crate) fn new` 同理
- `decide` impl method 保持当前可见性（trait impl，按 trait 签名）
- 事件名 `permission-request` 字面量必须一字不变（CHARTER I3）
- `respond_permission` Tauri 命令保留 `commands/agent.rs`（IPC 入口）；不在本 pack 范围

---

## Out of Scope

- ❌ 不动 `AppState.permission_senders` / `permission_overrides` 所有权（更大改动留给未来 pack）
- ❌ 不搬 `respond_permission` Tauri 命令
- ❌ 不改 `permission-request` 事件名（I3）
- ❌ 不改 `decide()` 实现（I6）

---

## Execute Plan

1. 创建 `application/permission_service.rs` 文件头：
   ```rust
   //! Permission service — bridges sync `PermissionPrompter` trait with
   //! async Tauri IPC by emitting `permission-request` events to the
   //! frontend and blocking on an mpsc channel for the response.
   //!
   //! Extracted from `commands/agent.rs` in GFR-003 (pure structural move).
   ```
2. 粘入 3 段，可见性按 Contract 调整
3. `use` 顶部：
   ```rust
   use crate::modules::runtime::permissions::{
       PermissionPrompter, PermissionPromptDecision, PermissionRequest,
   };
   use tauri::Emitter;
   ```
4. `application/mod.rs` 加 mod + re-export
5. `agent.rs`:
   - 删除 3 段（line ~3234–3305）
   - 留注释：`// TauriPermissionPrompter moved to application::permission_service (GFR-003).`
   - 顶部 use 加 `TauriPermissionPrompter`
6. cargo build PASS

---

## Verify Whitelist

允许 added pub_symbols（共 3 条）：
- `mod permission_service`
- `struct TauriPermissionPrompter`
- `fn new`（inherent constructor，可能被 set 去重）

注：原 `pub struct TauriPermissionPrompter` 在 agent.rs 是 `pub`；移到 application 后变 `pub(crate)`。我们的 regex 都会捕获它们为 `struct TauriPermissionPrompter`（同一字符串），set 去重后 added=0 removed=0 — 完美的"位置变化、符号集合不变"案例。

`pub_symbols` removed: 0 (TauriPermissionPrompter 仍在 set，路径不影响 set 内容)
`event_strings` change: 0 — `permission-request` literal 跟随移动
`test_names` change: 0

---

## Done Criteria

- cargo build + cargo test --no-run PASS
- agent.rs LOC 下降 ≈ 75
- permission_service.rs ≈ 90 LOC
- REGISTRY GFR-003 → done
