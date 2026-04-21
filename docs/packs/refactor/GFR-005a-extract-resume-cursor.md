# GFR-005a: Extract resume-cursor cluster from `agent.rs`

## Status
- State: `done`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-004` (done 2026-04-21)
- Last Updated: `2026-04-21`

---

## Goal

GFR-005 的第一刀。把 `commands/agent.rs` 中纯粹的 resume-cursor 编解码集群（1 个 struct + 5 个 file-private fn，~80 LOC）以**纯位移**方式搬到 `runtime/resume_cursor.rs`。零业务变更。后续 005b/c/d/e 再搬 `start_agent_stream` 主体。

---

## Source

`src-tauri/src/commands/agent.rs`：

| 段 | 行号 | 内容 |
|---|---|---|
| `struct ResumeCursor` | 88–93 | 3 字段 (`stream_id`, `tool_loop_iter`, `token_count`) |
| `fn build_resume_cursor` | 2582–2585 | 编码 `resume_cursor:v1:...` |
| `fn parse_resume_cursor` | 2587–2603 | 反向解码 |
| `fn session_contains_resume_cursor` | 2605–2615 | 在 `&AppSession` 中查 cursor 是否已存在 |
| `fn strip_resume_cursor_marker` | 2617–2638 | 从 user message 中剥 `[resume_cursor]` 标记 |
| `fn extract_resume_cursor_marker` | 2640–2655 | 从 user message 中取 cursor 字符串 |

调用点（agent.rs 内多处，搬完后改为 `use crate::modules::runtime::resume_cursor::{...};` 即可）：
- `agent.rs:900–911`：`extract_resume_cursor_marker` / `parse_resume_cursor` / `session_contains_resume_cursor` / `strip_resume_cursor_marker`
- `agent.rs:1366 / 1691 / 2123`：`build_resume_cursor`

---

## Destination

- **新建** `src-tauri/src/modules/runtime/resume_cursor.rs`，承载上面 6 个符号
- `src-tauri/src/modules/runtime/mod.rs` 加一行 `pub mod resume_cursor;`
- `agent.rs` 顶部 `use` 列表加：
  ```rust
  use crate::modules::runtime::resume_cursor::{
      build_resume_cursor, extract_resume_cursor_marker, parse_resume_cursor,
      session_contains_resume_cursor, strip_resume_cursor_marker, ResumeCursor,
  };
  ```

> 选择 `runtime/` 而非 `application/turn_service/`：
> - 与 GFR-001 的 `runtime/block_conversion.rs` 对称（同样是从 agent.rs 抽出的纯编码 helper）
> - 不需要把 `turn_service.rs` 转目录（无 `git mv`）
> - `session_contains_resume_cursor` 需要 `&AppSession`，引入 `runtime → session` 这一新依赖；application 层 mod.rs §2 已显式禁止 import `session`，runtime 层无该限制

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/runtime/mod.rs
- src-tauri/src/modules/runtime/resume_cursor.rs

---

## Contract（CHARTER 不变量映射）

- **I1 pub 符号集合**：见下方 Verify Whitelist（白名单内 added 项允许）
- **I2 测试**：不增不减；现有 `commands::agent::tests` 不动
- **I3 事件名**：本集群无 emit/listen/invoke 调用 — 0 影响
- **I4 持久化 key**：cursor 字符串格式（`resume_cursor:v1:...`）byte-identical
- **I5 shim**：6 个符号全是 file-private，无外部调用方；不需 re-export shim
- **I6 函数体一行不改**：byte-identical move
- **I7 lint**：cargo fmt + clippy `--workspace --all-targets -D warnings` 必须 GREEN

---

## Out of Scope

- ❌ 不动 `start_agent_stream` 主体 → 留 GFR-005b
- ❌ 不动 `run_agent_turn` 主体 → 留 GFR-005c
- ❌ 不改 cursor 编码格式 / 字段命名 / 类型 / 顺序
- ❌ 不改 marker 字符串 `[resume_cursor]`
- ❌ 不改 fallback 中文文本 `请从上一次中断处继续...`
- ❌ 不顺手抽 `PersistedTurnOutcome`（仍留 agent.rs，与 turn flush 紧耦合）

---

## Execute Plan

1. 创建 `src-tauri/src/modules/runtime/resume_cursor.rs`，文件头：
   ```rust
   //! Resume-cursor encoding/decoding cluster.
   //!
   //! Extracted from `commands/agent.rs` in GFR-005a (pure structural
   //! move; function bodies byte-identical).
   ```
   依赖 `use`：
   ```rust
   use crate::modules::session::Session as AppSession;
   ```
2. 把 6 段（struct + 5 fns）原样粘入。所有符号可见性由 file-private 提升为 `pub(crate)`（白名单见下）。
3. 在 `runtime/mod.rs` 的 module 列表中插入 `pub mod resume_cursor;`（按字母序紧跟 `prompt_tools_guide` 之后、`remote` 之前）。
4. 在 `agent.rs` 中：
   - 删除 line 88–93 `struct ResumeCursor` 定义
   - 删除 line 2582–2655 的 5 个 fn 定义
   - 在删除位置各留一行注释：`// resume-cursor cluster moved to runtime::resume_cursor (GFR-005a).`（合并相邻删除区段时只留 1 行）
   - 顶部 `use` 列表加入新 import（紧跟现有 `runtime::stream_emitter::{...}` 之后、`session::Session as AppSession` 之前）
5. `cargo build --manifest-path src-tauri/Cargo.toml` PASS。
6. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 后再 build。
7. `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings` GREEN。
8. `cargo test --manifest-path src-tauri/Cargo.toml --no-run` PASS。

---

## Verify Whitelist

`./scripts/pack run GFR-005a` 中允许出现的 added pub_symbols（共 7 条）：

- `mod resume_cursor`                    （`runtime/mod.rs` 新增 `pub mod resume_cursor;`）
- `struct ResumeCursor`                  （`resume_cursor.rs`，file-private → `pub(crate)`）
- `fn build_resume_cursor`               （同上）
- `fn parse_resume_cursor`               （同上）
- `fn session_contains_resume_cursor`    （同上）
- `fn strip_resume_cursor_marker`        （同上）
- `fn extract_resume_cursor_marker`      （同上）

`pub_symbols` removed: 必须为 0。
`event_strings` added/removed: 必须为 0。
`test_names` added/removed: 必须为 0。

---

## Done Criteria

- `cargo build` / `cargo test --no-run` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` GREEN
- `./scripts/pack run GFR-005a` added pub_symbols 全在白名单内
- `agent.rs` LOC 下降 ≈ 80（2931 → ~2855）
- `runtime/resume_cursor.rs` ≈ 80 LOC
- 1 PR、1 commit，message 以 `refactor(GFR-005a):` 起头
- `docs/packs/REGISTRY.md` 中 `GFR-005a` 状态为 `done`
