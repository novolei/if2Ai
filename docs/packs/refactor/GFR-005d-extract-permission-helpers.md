# GFR-005d: Move permission helpers into existing `permission_service.rs`

## Status
- State: `done`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-005c` (done 2026-04-21)
- Last Updated: `2026-04-21`

---

## Goal

GFR-005 的第四刀。把 `commands/agent.rs` 的两个 `pub(crate)` permission helper（`parse_permission_mode` 字符串→枚举映射、`build_permission_policy` 工具→所需权限的 builder 表）以**纯位移**搬到 GFR-003 已建立的 `application/permission_service.rs`。

**关键差异（与 005a/b/c 相比）**：本 pack 是首个**对 `pack verify` 自然 GREEN** 的 GFR-005x 切片——
agent.rs 删 2 个 `pub(crate) fn` + 留 1 个 `pub(crate) use` 再导出 shim，permission_service.rs 加 2 个同名 `pub(crate) fn`。
脚本 aggregator 跨 scope 文件 union → 加减相抵 → I1 set-equal，预计 `pub_symbols added=[]` `removed=[]`。

---

## Source

`src-tauri/src/commands/agent.rs`：

| 段 | 行号 | 内容 |
|---|---|---|
| `fn parse_permission_mode` | 164–178 | 5 个字符串别名 → `PermissionMode` 枚举（含 `None` → DangerFullAccess 默认） |
| `fn build_permission_policy` | 180–233 | 41 个 `with_tool_requirement` 调用，按 ReadOnly / WorkspaceWrite / DangerFullAccess 分层 |

调用点（5 处 in agent.rs + 1 处 in commands/tools.rs）：
- `agent.rs:297, 367, 864, 1095, 1101` — 通过本文件内私有引用调用
- `commands/tools.rs:144–145` — `super::agent::parse_permission_mode(...)` / `super::agent::build_permission_policy(...)`

> ⚠️ tools.rs 通过 `super::agent::*` 调用——本 pack 用 `pub(crate) use` shim 维持该 import 路径不变（CHARTER I5）。

---

## Destination

- **扩展** `src-tauri/src/modules/application/permission_service.rs`（GFR-003 已建，86 → ~158 LOC）
- 该文件 `use` 头加：
  ```rust
  use crate::modules::runtime::permissions::{PermissionMode, PermissionPolicy};
  ```
  （已有 `PermissionPromptDecision/PermissionPrompter/PermissionRequest`，本 pack 追加上面 2 个）
- agent.rs 保留 shim：
  ```rust
  pub(crate) use crate::modules::application::permission_service::{
      build_permission_policy, parse_permission_mode,
  };
  ```
  以便 `commands/tools.rs` 的 `super::agent::parse_permission_mode(...)` 调用路径一字不动（I5）。
- agent.rs 顶部已存在的 `use crate::modules::application::{... TauriPermissionPrompter ...}` 不需改动（仍走 application::mod 重导出）。
- `application/mod.rs` **不动**（mod.rs 已对外稳定，新增重导出非必要——agent.rs 的 shim 已覆盖唯一外部调用方 `commands/tools.rs`）。

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/application/permission_service.rs

---

## Contract（CHARTER 不变量映射）

- **I1 pub 符号集合**：set-equal（agent.rs 减 2 个 `pub(crate) fn`；permission_service.rs 加 2 个同名 `pub(crate) fn`；agent.rs 的 `pub(crate) use` shim 不被 RUST_PUB_RE 捕获）
- **I2 测试**：不增不减
- **I3 事件名**：本 cluster 无 emit/listen/invoke 调用 — 0 影响
- **I4 持久化 key**：5 个字符串别名 (`readOnly`/`read-only`/`read_only`/`workspaceWrite`/...) byte-identical
- **I5 shim**：保留 `pub(crate) use ::permission_service::{...}` 在 agent.rs；`commands/tools.rs` 的 `super::agent::*` import 路径不变
- **I6 函数体一行不改**：byte-identical move
- **I7 lint**：cargo fmt + clippy `--workspace --all-targets -D warnings` 必须 GREEN

---

## Out of Scope

- ❌ 不改 5 个字符串别名（`readOnly` / `workspaceWrite` / `dangerFullAccess` 等）
- ❌ 不改 41 个工具→权限 mapping 中任意一项（包括字符串大小写如 `WebFetch` vs `web_fetch`）
- ❌ 不动 `commands/tools.rs`（保持 import 路径 `super::agent::parse_permission_mode`）
- ❌ 不动 `application/mod.rs` 重导出
- ❌ 不动 `permission_service.rs` 已有 `TauriPermissionPrompter`
- ❌ 不顺手抽 `app_session_to_runtime` / `log_context_fingerprint` / `make_turn_service`（留 005e+）

---

## Execute Plan

1. 在 `src-tauri/src/modules/application/permission_service.rs` 中：
   - `use` 头追加：`PermissionMode, PermissionPolicy`（已有 import 行内合并）
   - 文件末尾追加 2 段 `pub(crate) fn`（byte-identical 复制 agent.rs 164–233，包括 `///` doc 注释）
2. 在 `agent.rs` 中：
   - 删除 line 164–233 的 2 段 fn 定义（连同各自的 `///` doc 注释）
   - 在删除位置留一行注释：`// parse_permission_mode + build_permission_policy moved to application::permission_service (GFR-005d).`
   - 紧接着追加 shim：
     ```rust
     pub(crate) use crate::modules::application::permission_service::{
         build_permission_policy, parse_permission_mode,
     };
     ```
3. `agent.rs` 顶部 `use crate::modules::application::{...}` 不需追加（应用于直接 import 而非这两个 fn 的本地名解析）。
4. `cargo build --manifest-path src-tauri/Cargo.toml` PASS。
5. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 后再 build。
6. `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings` GREEN。
7. `cargo test --manifest-path src-tauri/Cargo.toml --no-run` PASS。

---

## Verify Whitelist

`./scripts/pack run GFR-005d` 预期 added/removed 均为空集合（首个真正的 set-equal 切片）：

- `pub_symbols` added: `[]`
- `pub_symbols` removed: `[]`
- `event_strings` added/removed: `[]`
- `test_names` added/removed: `[]`
- LOC drift 容忍 ≤ 5%

如出现非空 added，必须**且只能**是以下白名单（脚本不读 whitelist，用于人审）：
- 无（本 pack 设计为零白名单）

如出现非空 removed → STOP，本 pack 实现错误（破坏 I5）。

---

## Done Criteria

- `cargo build` / `cargo test --no-run` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` GREEN
- `./scripts/pack run GFR-005d` 报告 `pub_symbols: <N> (unchanged)` 且 exit 0
- `agent.rs` LOC 下降 ≈ 65（2803 → ~2738）
- `application/permission_service.rs` ≈ 158 LOC
- 1 PR、1 commit，message 以 `refactor(GFR-005d):` 起头
- `docs/packs/REGISTRY.md` 中 `GFR-005d` 状态为 `done`
