# GFR-002c: Extract preflight estimators from `agent.rs`

## Status
- State: `active`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-002a` (done 2026-04-21)
- Last Updated: `2026-04-21`

---

## Goal

把 `commands/agent.rs` 的 preflight estimators 集群（约 130 LOC、4 个纯函数）整体迁到 `application/prompt_planner/preflight.rs`。这是 GFR-002 的第二刀；先抽 estimators 让后续 GFR-002b（governor 集群）边界更清晰。

---

## Source

`src-tauri/src/commands/agent.rs` 行号区间（post-GFR-002a，agent.rs 当前 3694 LOC）：

| 段 | 行号 | 内容 |
|---|---|---|
| `fn estimate_messages_char_count` | 3380–3425 | 纯函数；无外部 fn 依赖（除 api types） |
| `fn estimate_messages_token_count` | 3427–3429 | 调用 `estimate_token_count_from_chars`（已从 runtime/compact import）+ self |
| `fn summarize_message_for_budget` | 3431–3489 | 调用 `truncate_middle_chars`，使用 `TOOL_RESULT_PREVIEW_CHARS`（已从 runtime/block_conversion import） |
| `fn truncate_middle_chars` | 3491–3508 | 纯函数；无外部 fn 依赖 |

调用点（搬完后由新模块提供）：
- `agent.rs:3083 / 3085 / 3088 / 3091`（ContextGovernor::token_budget_gate）
- `agent.rs:3089`（summarize_message_for_budget）
- `agent.rs:3119 / 3186`（ContextGovernor::admit / artifact_gate）
- `agent.rs:3354 / 3361 / 3364 / 3365 / 3368`（apply_request_preflight_limits）

> `truncate_middle_chars` 只被本集群内部调用（agent.rs 其他地方零引用）。

---

## Destination

- 新建 `application/prompt_planner/preflight.rs` 承载 4 个 fn
- `prompt_planner/mod.rs` 加：
  ```rust
  pub mod preflight;
  pub(crate) use preflight::{
      estimate_messages_char_count, estimate_messages_token_count,
      summarize_message_for_budget,
  };
  ```
  （`truncate_middle_chars` 内部调用，不 re-export）
- `agent.rs` 加 `use crate::modules::application::prompt_planner::{estimate_messages_char_count, estimate_messages_token_count, summarize_message_for_budget};`

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/application/prompt_planner/mod.rs
- src-tauri/src/modules/application/prompt_planner/preflight.rs

---

## Contract（CHARTER 不变量映射）

- **I1 pub 符号**：见 Verify Whitelist
- **I2 测试**：不增不减
- **I3 事件名**：preflight 集群无 emit/listen/invoke — 0 影响
- **I5 shim**：原 4 个 fn 都是 file-private；本 pack 不要求 shim re-export
- **I6 函数体一行不改**：byte-identical move
- **I7 lint**：cargo build PASS（workspace clippy 已知 baseline debt punt 到 PERF-001）

---

## Out of Scope

- ❌ 不抽 `RequestPreflightStats` / `ContextGovernor` / `apply_request_preflight_limits`（GFR-002b）
- ❌ 不动任何 estimator 算法
- ❌ 不顺手优化 truncate_middle_chars

---

## Execute Plan

1. 创建 `prompt_planner/preflight.rs`，文件头：
   ```rust
   //! Per-request preflight estimators (char/token budgeting helpers).
   //!
   //! Extracted from `commands/agent.rs` in GFR-002c (pure structural move,
   //! function bodies byte-identical).
   ```
2. 把 4 个 fn 原样粘入。`use` 顶部按需补：
   ```rust
   use crate::modules::api::InputMessage;
   use crate::modules::api::InputContentBlock;
   use crate::modules::runtime::block_conversion::TOOL_RESULT_PREVIEW_CHARS;
   use crate::modules::runtime::compact::estimate_token_count_from_chars;
   ```
3. 把 3 个外部需要的 fn 改为 `pub(crate)`，`truncate_middle_chars` 保持 file-private（仅内部使用）。
4. `prompt_planner/mod.rs` 加 mod 声明 + re-export（同 GFR-002a 风格）。
5. `agent.rs`：
   - 删除 line 3380–3508（4 个 fn 定义）
   - 留注释：`// preflight estimators moved to application::prompt_planner::preflight (GFR-002c).`
   - 顶部 `use` 列表加上 3 个 fn 的导入
6. `cargo build` 必须通过。

---

## Verify Whitelist

允许出现的 added pub_symbols（共 4 条）：

- `mod preflight`                          （`prompt_planner/mod.rs` 新增 `pub mod preflight;`）
- `fn estimate_messages_char_count`        （`preflight.rs`，file-private → `pub(crate)`）
- `fn estimate_messages_token_count`       （同上）
- `fn summarize_message_for_budget`        （同上）

`pub_symbols` removed: 必须为 0。
`event_strings` added/removed: 必须为 0。
`test_names` removed: 必须为 0。

---

## Done Criteria

- `cargo build` PASS
- `cargo test --no-run` PASS
- `agent.rs` LOC 下降 ≈ 129
- `prompt_planner/preflight.rs` ≈ 130 LOC
- 1 commit `refactor(GFR-002c): ...`
- REGISTRY GFR-002c → done
