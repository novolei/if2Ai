# GFR-002b: Extract governor cluster from `agent.rs`

## Status
- State: `active`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-002a` + `GFR-002c` both done 2026-04-21
- Last Updated: `2026-04-21`

---

## Goal

把 `commands/agent.rs` 的 governor 集群（约 190 LOC）整体迁到 `application/prompt_planner/governor.rs`。GFR-002 系列的最后一刀，完成 prompt pipeline 的全部抽离。

---

## Source

`src-tauri/src/commands/agent.rs` (post-002a/002c, 现 3569 LOC)：

| 段 | 行号 | 内容 |
|---|---|---|
| `struct RequestPreflightStats` | 3041–3049 | 6 字段 |
| `impl RequestPreflightStats` | 3051–3055 | 1 helper (`has_changes`) |
| `struct ContextGovernor` (含 harness symbol marker comment) | 3057–3059 | unit struct |
| `impl ContextGovernor` | 3061–3200 | 4 methods (admit + 3 gates) |
| `fn apply_request_preflight_limits` | 3350–3379 | 由 governor::message_char_budget_gate 调用 |

调用点（必须保持外部可访问）：
- `agent.rs:1591 / 1666` — `ContextGovernor.admit(...)`
- `agent.rs:1597–1610 / 1653 / 1672–1685` — `preflight_stats.{has_changes, dropped_messages, trimmed_chars, before_messages, after_messages, before_chars, after_chars}`

---

## Destination

- 新建 `application/prompt_planner/governor.rs`
- `prompt_planner/mod.rs` 加：
  ```rust
  pub mod governor;
  pub(crate) use governor::{ContextGovernor, RequestPreflightStats};
  ```
- `agent.rs` 加 `use crate::modules::application::prompt_planner::{ContextGovernor, RequestPreflightStats};`

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/application/prompt_planner/mod.rs
- src-tauri/src/modules/application/prompt_planner/governor.rs

---

## Contract

- `RequestPreflightStats` 和它的 6 个字段：file-private → `pub(crate)`（外部访问）
- `ContextGovernor`：file-private → `pub(crate)`（外部 admit 调用）
- `RequestPreflightStats::has_changes`：`fn` → `pub(crate) fn`（外部调用）
- `ContextGovernor::admit`：`fn` → `pub(crate) fn`（外部调用）
- 其余 inherent methods（token_budget_gate / message_char_budget_gate / artifact_gate）：保持 file-private（仅 admit 内部调用）
- `apply_request_preflight_limits`：保持 file-private（仅 message_char_budget_gate 内部调用，搬到新文件后两者同模块）
- 保留 `// harness symbol marker: ContextGovernor\|token_budget_gate\|...` 注释（harness 工具靠 grep 这条匹配）

---

## Out of Scope

- ❌ 不改 governor 算法（保留 token_budget_gate / artifact_gate / message_char_budget_gate 实现一字不变）
- ❌ 不改 `MAX_REQUEST_*` 常量（在 agent.rs 顶部）
- ❌ 不改 `RequestPreflightStats` 字段顺序

---

## Execute Plan

1. 创建 `prompt_planner/governor.rs` 文件头：
   ```rust
   //! Per-request governor: artifact gate, token budget gate, char/message
   //! budget gate. Owns RequestPreflightStats.
   //!
   //! Extracted from `commands/agent.rs` in GFR-002b (pure structural move,
   //! function bodies byte-identical).
   ```
2. 粘入 5 段（struct + impl + struct + impl + fn），按上面 Contract 调整可见性。
3. `use` 顶部：
   ```rust
   use crate::modules::api::{InputContentBlock, InputMessage};
   use crate::modules::application::prompt_planner::preflight::{
       estimate_messages_char_count, estimate_messages_token_count, summarize_message_for_budget,
   };
   use crate::modules::runtime::block_conversion::summarize_tool_result_for_model;
   ```
4. mod.rs 加 `pub mod governor;` + re-export
5. agent.rs:
   - 删除 cluster (3041–3055, 3057–3200, 3350–3379)
   - 留注释
   - 顶部 use 加 `ContextGovernor, RequestPreflightStats`
6. cargo build PASS

---

## Verify Whitelist

允许 added pub_symbols（共 4 条）：
- `mod governor`
- `struct ContextGovernor`
- `struct RequestPreflightStats`
- `fn admit`（impl ContextGovernor 的 inherent pub(crate)）
- `fn has_changes`（impl RequestPreflightStats 的 pub(crate) — 但 has_changes 也存在于 SanitizationStats，set 去重后可能不显示为 added）

`pub_symbols` removed: 0 / `event_strings` change: 0 / `test_names` change: 0

---

## Done Criteria

- cargo build + cargo test --no-run PASS
- agent.rs LOC 下降 ≈ 190
- governor.rs ≈ 200 LOC
- REGISTRY GFR-002b → done
