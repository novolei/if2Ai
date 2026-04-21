# GFR-005e: Extract session-bridge helpers from `agent.rs`

## Status
- State: `done`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-005d` (done 2026-04-21)
- Last Updated: `2026-04-21`

---

## Goal

GFR-005 的第五刀（preamble 部分收尾）。把 `commands/agent.rs` 仅剩的两个 file-private 桥接 helper——
`app_session_to_runtime` (AppSession → RuntimeSession 适配) 与
`log_context_fingerprint` (SessionExecutionContext 调试信息打印)——以**纯位移**搬到新建的
`control_plane/session_bridge.rs`。沿用 GFR-005d 验证过的 `pub(crate) use` shim 模式 → `pack run` 自然 GREEN。

执行后 agent.rs 的 preamble (lines 1–157) 仅保留 `make_turn_service` + `resolve_session_execution_context`
(两者都依赖 `&AppState` 类型，绑定 commands 层，不能进 application/control_plane) +
`flush_assistant_timeline_segment` 残留注释 +常量声明，preamble god-file 责任正式收口。

---

## Source

`src-tauri/src/commands/agent.rs`：

| 段 | 行号 | 内容 |
|---|---|---|
| `fn app_session_to_runtime` | 120–129 | 6 LOC fn body + 4 LOC `///` doc。从 `&AppSession.messages` 克隆出 `RuntimeSession { version: 1, messages }` |
| `fn log_context_fingerprint` | 145–156 | 12 LOC。计算 `tools::context::context_fingerprint` 并 `tracing::info!` 打印 5 字段 |

调用点（agent.rs 内 5 处，全部 file-private）：
- `agent.rs:236, 800, 1042` — `log_context_fingerprint(caller, &execution_context)`
- `agent.rs:239, 820` — `let runtime_session = app_session_to_runtime(&app_session);`

> 0 跨 module 调用方。本 pack 用 shim 是为了 set-equal 验证、不是为了 import 路径稳定。

---

## Destination

- **新建** `src-tauri/src/modules/control_plane/session_bridge.rs`，承载 2 段
- 该文件 `use` 头：
  ```rust
  use crate::modules::control_plane::session_context::SessionExecutionContext;
  use crate::modules::runtime::session::Session as RuntimeSession;
  use crate::modules::session::Session as AppSession;
  ```
- `src-tauri/src/modules/control_plane/mod.rs` 加 `pub mod session_bridge;`
- agent.rs 留 shim：
  ```rust
  pub(crate) use crate::modules::control_plane::session_bridge::{
      app_session_to_runtime, log_context_fingerprint,
  };
  ```
- `application/mod.rs` / `control_plane/mod.rs` 不加 `pub use` 重导出（外部无调用方）。

> 选择 `control_plane/`：
> - `log_context_fingerprint` 操作 `SessionExecutionContext`（control_plane）+ 跨调 `tools::context`，与 `session_context.rs` 同语义层
> - `app_session_to_runtime` 是 `AppSession ↔ RuntimeSession` bridge，control_plane 已是 session 桥接层（`SessionContextResolver` 同样桥接 AppSession → SessionExecutionContext）
> - 替代方案：runtime/ 落（如 005a/c），但会再加 `runtime → control_plane` + `runtime → session` + `runtime → tools` 三条新跨层。control_plane 已自然依赖 session/runtime/tools，不增新边界

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/control_plane/mod.rs
- src-tauri/src/modules/control_plane/session_bridge.rs

---

## Contract（CHARTER 不变量映射）

- **I1 pub 符号集合**：set-equal（agent.rs 减 0 个 pub_symbols；本来就 file-private，不会被 RUST_PUB_RE 捕获）。session_bridge.rs 加 2 个 `pub(crate) fn`。control_plane/mod.rs 加 1 个 `pub mod session_bridge`。所以 added=3, removed=0 — **本 pack 走白名单路线**（与 005a/b/c 同），不像 005d 那样 set-equal。
  - 解释：005d 之所以 set-equal 是因为被搬符号已经是 `pub(crate)` 而被 RUST_PUB_RE 捕获了；005e 的两个 fn 现在是裸 `fn`（file-private），regex 不会捕获 → agent.rs snapshot 端无 removed。提升可见性到 `pub(crate)` 后 destination 端有 added。无法 set-equal。
- **I2 测试**：不增不减
- **I3 事件名**：本 cluster 无 emit/listen/invoke 调用 — 0 影响
- **I4 持久化 key**：`tracing::info!` log 字符串字面量保持 byte-identical（包括 `[{}] context fingerprint='{}', session_id='{}', workdir='{}', permission_mode='{}'` 模板）
- **I5 shim**：保留 `pub(crate) use ::session_bridge::{...}` 在 agent.rs；agent.rs 内 5 处调用站点表达式一字不动
- **I6 函数体一行不改**：byte-identical move
- **I7 lint**：cargo fmt + clippy `--workspace --all-targets -D warnings` 必须 GREEN

---

## Out of Scope

- ❌ 不改 `RuntimeSession { version: 1, ... }` 字面常量 1
- ❌ 不改 `app_session.messages.clone()` 克隆策略（性能优化留 PERF pack）
- ❌ 不改 `tracing::info!` 模板字符串（含字段名 / 单引号 / 顺序 — I3 协议）
- ❌ 不顺手抽 `make_turn_service` / `resolve_session_execution_context`（两者依赖 `&AppState`，application/control_plane 都禁止 import commands::AppState — 必须留 agent.rs 或重构为 trait 对象，超出本 pack 范围）
- ❌ 不动 `application/mod.rs` 重导出
- ❌ 不动 `control_plane` 现有 5 个文件

---

## Execute Plan

1. 创建 `src-tauri/src/modules/control_plane/session_bridge.rs`，文件头：
   ```rust
   //! Session bridge helpers: convert AppSession ↔ RuntimeSession and
   //! log context fingerprints for debugging.
   //!
   //! Extracted from `commands/agent.rs` in GFR-005e (pure structural
   //! move; function bodies byte-identical).
   ```
   依赖 `use`：
   ```rust
   use crate::modules::control_plane::session_context::SessionExecutionContext;
   use crate::modules::runtime::session::Session as RuntimeSession;
   use crate::modules::session::Session as AppSession;
   ```
2. 把 2 段 fn 原样粘入（包括 `///` doc 注释）。可见性由 file-private 提升为 `pub(crate)`。
3. 在 `control_plane/mod.rs` 中按字母序插入 `pub mod session_bridge;`（在 `prepare_step_execution` 之后、`session_context` 之前）。
4. 在 `agent.rs` 中：
   - 删除 line 120–129 (`app_session_to_runtime` + 其 doc)
   - 删除 line 145–156 (`log_context_fingerprint`)
   - 在删除位置（合并相邻区段）留一行注释：`// session-bridge helpers (app_session_to_runtime + log_context_fingerprint) moved to control_plane::session_bridge (GFR-005e).`
   - 紧接着追加 shim：
     ```rust
     pub(crate) use crate::modules::control_plane::session_bridge::{
         app_session_to_runtime, log_context_fingerprint,
     };
     ```
5. agent.rs 顶部 `use` 不需追加（shim 已覆盖本地名解析）。
6. `cargo build --manifest-path src-tauri/Cargo.toml` PASS。
7. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 后再 build。
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings` GREEN。
9. `cargo test --manifest-path src-tauri/Cargo.toml --no-run` PASS。

---

## Verify Whitelist

`./scripts/pack run GFR-005e` 中允许出现的 added pub_symbols（共 3 条，仍是软 FAIL 路线）：

- `mod session_bridge`                         （`control_plane/mod.rs` 新增 `pub mod ...`）
- `fn app_session_to_runtime`                  （`session_bridge.rs`，file-private → `pub(crate)`）
- `fn log_context_fingerprint`                 （同上）

`pub_symbols` removed: 必须为 0。
`event_strings` added/removed: 必须为 0。
`test_names` added/removed: 必须为 0。

---

## Done Criteria

- `cargo build` / `cargo test --no-run` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` GREEN
- `./scripts/pack run GFR-005e` added pub_symbols 全在白名单内
- `agent.rs` LOC 下降 ≈ 18（2736 → ~2718）
- `control_plane/session_bridge.rs` ≈ 30 LOC
- 1 PR、1 commit，message 以 `refactor(GFR-005e):` 起头
- `docs/packs/REGISTRY.md` 中 `GFR-005e` 状态为 `done`
