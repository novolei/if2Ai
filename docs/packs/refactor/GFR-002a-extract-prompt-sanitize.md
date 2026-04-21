# GFR-002a: Extract sanitize cluster from `agent.rs`

## Status
- State: `active`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-001` (done 2026-04-21)
- Last Updated: `2026-04-21`

---

## Goal

把 `commands/agent.rs` 的 sanitize 集群（约 162 LOC、自包含、纯函数）整体迁到 `application/prompt_planner/sanitize.rs`。第一刀切 GFR-002 的 3 个子集群中最自包含的一个。

---

## Source

`src-tauri/src/commands/agent.rs` 的连续段（line 3510–3672, 共 ~162 LOC）：

| 段 | 行号 | 内容 |
|---|---|---|
| `struct SanitizationStats` | 3510–3520 | 7 字段 |
| `impl SanitizationStats` | 3522–3548 | 4 helper methods (file-private) |
| `fn sanitize_messages_for_provider` | 3550–3649 | 主函数；调用 `remove_tool_use_blocks` |
| `fn remove_tool_use_blocks` | 3651–3661 | 移除 ToolUse blocks |
| `fn extend_sample_ids` | 3663–3672 | 追加样本 ID（去重 + 上限） |

调用点（搬完后由新模块提供）：
- `agent.rs:1610` `sanitize_messages_for_provider(...)`
- `agent.rs:1618 / 1623 / 1628` `extend_sample_ids(...)`

---

## Destination

- 把现有 `application/prompt_planner.rs` (358 LOC) 转为 `application/prompt_planner/mod.rs`（同语义，仅文件路径变化）
- 新建 `application/prompt_planner/sanitize.rs`，承载上面 5 段
- `application/prompt_planner/mod.rs` 加：
  ```rust
  pub mod sanitize;
  pub(crate) use sanitize::{sanitize_messages_for_provider, extend_sample_ids, SanitizationStats};
  ```
- `agent.rs` 加 `use crate::modules::application::prompt_planner::{sanitize_messages_for_provider, extend_sample_ids};`

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/application/prompt_planner/mod.rs
- src-tauri/src/modules/application/prompt_planner/sanitize.rs

> 注：`src-tauri/src/modules/application/prompt_planner.rs` 已 `git mv` 为
> 上面的 `mod.rs`；git 视为 rename，无独立 deletion 条目。

---

## Contract（CHARTER 不变量映射）

- **I1 pub 符号集合**：见下方 Verify Whitelist（白名单内 added 项允许）
- **I2 测试**：不增不减
- **I3 事件名**：sanitize 集群无 emit/listen/invoke 调用 — 0 影响
- **I5 shim**：原 5 个 fn/struct 都是 file-private；本 pack 不要求 shim re-export，调用方用 `application::prompt_planner::*` 直接 import
- **I6 函数体一行不改**：byte-identical move
- **I7 lint**：cargo fmt + clippy（仅本 pack scope 文件；workspace clippy 已知 baseline debt punt 到 PERF-001）

---

## Out of Scope

- ❌ 不抽 `RequestPreflightStats` / `ContextGovernor` / `apply_request_preflight_limits`（GFR-002b）
- ❌ 不抽 token/char count estimators（GFR-002c）
- ❌ 不动 `MAX_REQUEST_*` 常量值
- ❌ 不合并 sanitize 与 preflight 的逻辑边界
- ❌ 不顺手改 `prompt_planner.rs` 现有内容（仅 mv → mod.rs + 加 mod 声明）
- ❌ 不改 `SanitizationStats` 任何字段命名 / 类型 / 顺序

---

## Execute Plan

1. `git mv src-tauri/src/modules/application/prompt_planner.rs src-tauri/src/modules/application/prompt_planner/mod.rs`（先建目录然后 mv，或直接 mkdir+mv）。
2. 在 `prompt_planner/mod.rs` 顶部加 `pub mod sanitize;`，底部加 `pub(crate) use sanitize::{sanitize_messages_for_provider, extend_sample_ids, SanitizationStats};`
3. 创建 `prompt_planner/sanitize.rs`，文件头：
   ```rust
   //! Sanitize input messages before sending to the provider.
   //!
   //! Extracted from `commands/agent.rs` in GFR-002a (pure structural move,
   //! function bodies byte-identical).
   ```
4. 把 5 段（struct + impl + 3 fns）原样粘入。`use` 顶部按需补：
   ```rust
   use std::collections::HashSet;
   use crate::modules::api::{InputContentBlock, InputMessage};
   ```
5. 把 `struct SanitizationStats` / `fn sanitize_messages_for_provider` / `fn remove_tool_use_blocks` / `fn extend_sample_ids` 全部改为 `pub(crate)`（可见性提升 — 见 Verify Whitelist）。`impl` 块的 4 个 file-private helper methods 保持 file-private。
6. 在 `agent.rs` 中：
   - 删除 line 3510–3672 这 162 行的 5 段定义
   - 在删除位置留一行注释：`// sanitize cluster moved to application::prompt_planner::sanitize (GFR-002a).`
   - 顶部 `use` 列表加 `use crate::modules::application::prompt_planner::{sanitize_messages_for_provider, extend_sample_ids};`
7. `cargo build --manifest-path src-tauri/Cargo.toml` 必须通过。
8. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 后再次 build。

---

## Verify Whitelist

`./scripts/pack run GFR-002a` 中允许出现的 added pub_symbols（共 6 条）：

- `mod sanitize`                          （`prompt_planner/mod.rs` 新增 `pub mod sanitize;`）
- `struct SanitizationStats`              （`sanitize.rs`，file-private → `pub(crate)`）
- `fn sanitize_messages_for_provider`     （同上）
- `fn remove_tool_use_blocks`             （同上）
- `fn extend_sample_ids`                  （同上）
- `reexport sanitize_messages_for_provider` / `reexport extend_sample_ids` / `reexport SanitizationStats`（mod.rs 的 `pub(crate) use` — regex 不会捕获 `pub(crate) use`，预期为 0）

`pub_symbols` removed: 必须为 0。
`event_strings` added/removed: 必须为 0。
`test_names` removed: 必须为 0。

---

## Done Criteria

- `cargo build` PASS
- `cargo test` 编译通过
- `./scripts/pack run GFR-002a` 报告的 added pub_symbols 全在白名单内
- `agent.rs` LOC 下降 ≈ 162（3850 → ~3690）
- `prompt_planner/sanitize.rs` ≈ 162 LOC
- 1 PR、1 commit、信息以 `refactor(GFR-002a):` 起头
- `docs/packs/REGISTRY.md` 中 `GFR-002a` 状态由 `active` 改为 `done`
