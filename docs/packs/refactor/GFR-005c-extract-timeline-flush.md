# GFR-005c: Extract timeline-flush cluster from `agent.rs`

## Status
- State: `done`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-005b` (done 2026-04-21)
- Last Updated: `2026-04-21`

---

## Goal

GFR-005 的第三刀。把 `commands/agent.rs` 的 timeline-flush 微集群——`PersistedTurnOutcome` 结构 + `flush_assistant_timeline_segment` 函数——以**纯位移**搬到 `runtime/timeline_flush.rs`。两者紧耦合：fn 唯一参数化的 outcome 类型即该 struct，且 struct 只在该 fn 与 1 处实例化点使用。

---

## Source

`src-tauri/src/commands/agent.rs`：

| 段 | 行号 | 内容 |
|---|---|---|
| `struct PersistedTurnOutcome` | 86–93 | 5 字段 (`task_outcome`/`degraded_reason`/`resume_available`/`resume_cursor`/`request_id`) |
| `fn flush_assistant_timeline_segment` | 124–154 | 把累积的 text + thinking flush 成一条 `ConversationMessage` push 到 timeline 向量 |

调用点（agent.rs 内仅 1 + 2 处）：
- `agent.rs:2130` `PersistedTurnOutcome { ... }` 实例化
- `agent.rs:1797 / 2153` `flush_assistant_timeline_segment(...)` 调用

---

## Destination

- **新建** `src-tauri/src/modules/runtime/timeline_flush.rs`，承载 1 struct + 1 fn
- `src-tauri/src/modules/runtime/mod.rs` 加一行 `pub mod timeline_flush;`
- `agent.rs` 顶部 `use` 列表加：
  ```rust
  use crate::modules::runtime::timeline_flush::{
      flush_assistant_timeline_segment, PersistedTurnOutcome,
  };
  ```

> 选择 `runtime/`：fn 操作的全部类型（`ConversationMessage` / `ContentBlock` / `MessageRole`）都在 `runtime/session.rs`；struct 只含 String/bool/Option<String>，零跨层依赖。

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/runtime/mod.rs
- src-tauri/src/modules/runtime/timeline_flush.rs

---

## Contract（CHARTER 不变量映射）

- **I1 pub 符号集合**：见下方 Verify Whitelist（白名单内 added 项允许）
- **I2 测试**：不增不减
- **I3 事件名**：本集群无 emit/listen/invoke 调用 — 0 影响
- **I4 持久化 key**：`PersistedTurnOutcome` 的 5 个字段名是 ConversationMessage 的镜像；本 pack 字段名不变（I6）
- **I5 shim**：2 个符号都是 file-private，无外部调用方；不需 re-export shim
- **I6 函数体一行不改**：byte-identical move
- **I7 lint**：cargo fmt + clippy `--workspace --all-targets -D warnings` 必须 GREEN

---

## Out of Scope

- ❌ 不改 `PersistedTurnOutcome` 字段名 / 顺序 / 类型
- ❌ 不改 `flush_assistant_timeline_segment` 行为（包括 `mem::take` 的 ownership 语义）
- ❌ 不顺手改任何调用点（agent.rs 三处只允许改 import；调用本身一字不动）
- ❌ 不抽 `app_session_to_runtime` / `log_context_fingerprint`（更小，留给后续 005d 或更晚）
- ❌ 不动 turn-outcome 计算逻辑（属 turn 主体，留 GFR-005d/e）

---

## Execute Plan

1. 创建 `src-tauri/src/modules/runtime/timeline_flush.rs`，文件头：
   ```rust
   //! Timeline flush helper: collapse accumulated text/thinking into a
   //! single assistant ConversationMessage and push it onto the timeline.
   //!
   //! Extracted from `commands/agent.rs` in GFR-005c (pure structural
   //! move; struct fields and function body byte-identical).
   ```
   依赖 `use`：
   ```rust
   use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};
   ```
2. 把 `struct PersistedTurnOutcome` + `fn flush_assistant_timeline_segment` 原样粘入。
   - 函数体内 fully-qualified path `crate::modules::runtime::session::ConversationMessage` / `MessageRole::Assistant` 保持不变（CHARTER I6 — 一字不改），即使新文件已 `use` 了短名亦可（不强制收短）。
3. 可见性由 file-private 提升为 `pub(crate)`（白名单见下）。
4. 在 `runtime/mod.rs` 中按字母序插入 `pub mod timeline_flush;`（`stream_error_reason` 之后、`usage` 之前）。
5. 在 `agent.rs` 中：
   - 删除 line 86–93 `struct PersistedTurnOutcome` 定义
   - 删除 line 124–154 `fn flush_assistant_timeline_segment` 定义
   - 在删除位置各留一行注释：`// timeline-flush cluster moved to runtime::timeline_flush (GFR-005c).`
   - 顶部 `use` 列表加新 import（紧跟 `runtime::stream_error_reason::{...}` 之后）
6. `cargo build --manifest-path src-tauri/Cargo.toml` PASS。
7. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 后再 build。
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings` GREEN。
9. `cargo test --manifest-path src-tauri/Cargo.toml --no-run` PASS。

---

## Verify Whitelist

`./scripts/pack run GFR-005c` 中允许出现的 added pub_symbols（共 3 条）：

- `mod timeline_flush`                          （`runtime/mod.rs` 新增 `pub mod ...`）
- `struct PersistedTurnOutcome`                 （`timeline_flush.rs`，file-private → `pub(crate)`）
- `fn flush_assistant_timeline_segment`         （同上）

`pub_symbols` removed: 必须为 0。
`event_strings` added/removed: 必须为 0。
`test_names` added/removed: 必须为 0。

---

## Done Criteria

- `cargo build` / `cargo test --no-run` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` GREEN
- `./scripts/pack run GFR-005c` added pub_symbols 全在白名单内
- `agent.rs` LOC 下降 ≈ 40（2837 → ~2797）
- `runtime/timeline_flush.rs` ≈ 50 LOC
- 1 PR、1 commit，message 以 `refactor(GFR-005c):` 起头
- `docs/packs/REGISTRY.md` 中 `GFR-005c` 状态为 `done`
