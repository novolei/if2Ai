# GFR-005b: Extract stream-error-reason classification from `agent.rs`

## Status
- State: `done`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-005a` (done 2026-04-21)
- Last Updated: `2026-04-21`

---

## Goal

GFR-005 的第二刀。把 `commands/agent.rs` 中两个纯字符串分类 helper（`format_stream_error_reason` 把任意 `Display` error 归类为 `network_timeout / request_validation_error / permission_error / network_transport_error / model_stream_error` 五类前缀；`is_network_timeout_reason` 检测前缀）以**纯位移**搬到 `runtime/stream_error_reason.rs`。零业务变更。

下游 `harness/event_bus.rs` 已在 doc 注释里点名"already produced by `format_stream_error_reason`"，新模块即此 reason-string 协议的 single source of truth。

---

## Source

`src-tauri/src/commands/agent.rs`：

| 段 | 行号 | 内容 |
|---|---|---|
| `fn format_stream_error_reason` | 2552–2574 | 错误 → reason 前缀（5 个 kind） |
| `fn is_network_timeout_reason` | 2744–2746 | reason → 是否网络超时（用于 start-stream retry） |

调用点（agent.rs 内 2 对）：
- `agent.rs:1332–1333`：start-stream first-shot 失败时分类 + retry 判定
- `agent.rs:1651–1652`：tool-loop iteration 内 stream 失败时同样分类 + retry 判定

---

## Destination

- **新建** `src-tauri/src/modules/runtime/stream_error_reason.rs`，承载这 2 个 fn
- `src-tauri/src/modules/runtime/mod.rs` 加一行 `pub mod stream_error_reason;`
- `agent.rs` 顶部 `use` 列表加：
  ```rust
  use crate::modules::runtime::stream_error_reason::{
      format_stream_error_reason, is_network_timeout_reason,
  };
  ```

> 选择 `runtime/` 而非 `application/`：和 GFR-001 (`block_conversion`) / GFR-005a (`resume_cursor`) 对称——这是与 stream 协议绑定的纯字符串编解码，无业务编排，runtime 是它最自然的家。

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/runtime/mod.rs
- src-tauri/src/modules/runtime/stream_error_reason.rs

---

## Contract（CHARTER 不变量映射）

- **I1 pub 符号集合**：见下方 Verify Whitelist（白名单内 added 项允许）
- **I2 测试**：不增不减
- **I3 事件名 / 字符串字面量**：5 个 reason kind (`network_timeout` / `request_validation_error` / `permission_error` / `network_transport_error` / `model_stream_error`) + 检测前缀 `network_timeout:` byte-identical
- **I4 持久化 key**：无影响
- **I5 shim**：2 个 fn 都是 file-private，无外部调用方；不需 re-export shim
- **I6 函数体一行不改**：byte-identical move
- **I7 lint**：cargo fmt + clippy `--workspace --all-targets -D warnings` 必须 GREEN

---

## Out of Scope

- ❌ 不改 5 个 reason kind 字符串字面量（I3）
- ❌ 不改 `network_timeout:` 前缀检测语义（I6）
- ❌ 不改 `MAX_STREAM_RETRY_ON_TIMEOUT` 常量值或位置（仍留 agent.rs）
- ❌ 不动 `harness/event_bus.rs` 的 doc 注释（已经引用本函数名，迁移后名称不变）
- ❌ 不顺手抽 retry 决策逻辑（属于 turn 主体，留 GFR-005c+）

---

## Execute Plan

1. 创建 `src-tauri/src/modules/runtime/stream_error_reason.rs`，文件头：
   ```rust
   //! Classify stream errors into a stable `kind: raw` reason string.
   //!
   //! Extracted from `commands/agent.rs` in GFR-005b (pure structural
   //! move; function bodies byte-identical). The 5 reason-kind prefixes
   //! produced here form the agreed protocol consumed by
   //! `harness/event_bus.rs` / `harness/run_report.rs` / `harness/trace_aggregator.rs`.
   ```
2. 把 2 段 fn 原样粘入。可见性由 file-private 提升为 `pub(crate)`（白名单见下）。
3. 在 `runtime/mod.rs` 中按字母序插入 `pub mod stream_error_reason;`（在 `stream_emitter` 之后、`usage` 之前）。
4. 在 `agent.rs` 中：
   - 删除 line 2552–2574 `format_stream_error_reason` 定义
   - 删除 line 2744–2746 `is_network_timeout_reason` 定义
   - 在删除位置各留一行注释：`// stream-error-reason cluster moved to runtime::stream_error_reason (GFR-005b).`
   - 顶部 `use` 列表加新 import（紧跟 `runtime::resume_cursor::{...}` 之后）
5. `cargo build --manifest-path src-tauri/Cargo.toml` PASS。
6. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 后再 build。
7. `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings` GREEN。
8. `cargo test --manifest-path src-tauri/Cargo.toml --no-run` PASS。

---

## Verify Whitelist

`./scripts/pack run GFR-005b` 中允许出现的 added pub_symbols（共 3 条）：

- `mod stream_error_reason`               （`runtime/mod.rs` 新增 `pub mod ...`）
- `fn format_stream_error_reason`         （`stream_error_reason.rs`，file-private → `pub(crate)`）
- `fn is_network_timeout_reason`          （同上）

`pub_symbols` removed: 必须为 0。
`event_strings` added/removed: 必须为 0。
`test_names` added/removed: 必须为 0。

---

## Done Criteria

- `cargo build` / `cargo test --no-run` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` GREEN
- `./scripts/pack run GFR-005b` added pub_symbols 全在白名单内
- `agent.rs` LOC 下降 ≈ 30（2858 → ~2828）
- `runtime/stream_error_reason.rs` ≈ 35 LOC
- 1 PR、1 commit，message 以 `refactor(GFR-005b):` 起头
- `docs/packs/REGISTRY.md` 中 `GFR-005b` 状态为 `done`
