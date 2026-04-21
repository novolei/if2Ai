# GFR-T1-B-1: Extract JSON parse helpers from `runtime/config.rs`

## Status
- State: `done`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: T1-A cancelled (orphan dead code) → T1-B activated 2026-04-21
- Last Updated: `2026-04-21`

---

## Goal

GFR-T1-B 的第一刀（5 个 sub-pack 之一）。把 `runtime/config.rs` 末尾的 12 个**纯 JSON parsing primitive** helper（all file-private, ~210 LOC, 无 domain 知识）以**纯位移**搬到新建 `runtime/config/json_helpers.rs`。同步 `git mv runtime/config.rs runtime/config/mod.rs` 完成目录化（同 GFR-002a 的 prompt_planner.rs → prompt_planner/mod.rs 模式）。

预期 `pack run` clean GREEN——12 个 helpers 都是 file-private（regex 不捕获），目录化只是 path 变化不影响 pub_symbols 集合。剩 4 个 sub-pack（schema / permission / sandbox+boundary / merge+mcp+memory）后续推。

---

## Source

`src-tauri/src/modules/runtime/config.rs`：

| 段 | 行号 | 内容 |
|---|---|---|
| `fn expect_object` | 1647–1654 | `&JsonValue → &BTreeMap`，类型断言 |
| `fn expect_string` | 1656–1665 | `(map, key) → &str`（必填） |
| `fn optional_string` | 1667–1679 | `(map, key) → Option<&str>` |
| `fn optional_bool` | 1681–1693 | `(map, key) → Option<bool>` |
| `fn optional_u8` | 1695–1714 | `(map, key) → Option<u8>` 含范围检查 |
| `fn optional_u16` | 1716–1735 | 同上 u16 |
| `fn optional_u32` | 1737–1756 | 同上 u32 |
| `fn optional_u64` | 1758–1777 | 同上 u64 |
| `fn parse_bool_map` | 1779–1795 | `JsonValue → BTreeMap<String, bool>` |
| `fn optional_string_array` | 1797–1823 | `(map, key) → Option<Vec<String>>` |
| `fn optional_string_map` | 1825–1853 | `(map, key) → Option<BTreeMap<String, String>>` |
| `fn deep_merge_objects` | 1855–1869 | 递归合并 BTreeMap (3-source merge 用) |
| `fn extend_unique` + `fn push_unique` | 1871–1881 | 去重追加 Vec<String> |

> 上面 14 段（实际 12 functions + 2 small utilities）共 ~235 LOC，全部 file-private，仅在 config.rs 内被 ~30 处 parse_* 调用。

---

## Destination

- **`git mv src-tauri/src/modules/runtime/config.rs src-tauri/src/modules/runtime/config/mod.rs`**（沿用 GFR-002a 模式；外部 import 路径 `crate::modules::runtime::config::*` 完全不变 — 25+ 个 importer 零修改）
- **新建** `src-tauri/src/modules/runtime/config/json_helpers.rs`，承载上面 14 段
- `runtime/config/mod.rs` 顶部加：
  ```rust
  mod json_helpers;
  use json_helpers::*;
  ```
  （**私有 `mod`** + 私有 `use *;` — 不污染 mod.rs 的 pub 表面，原所有调用点因 glob `use` 直接见到符号）

---

## Files (scope)

- src-tauri/src/modules/runtime/config.rs
- src-tauri/src/modules/runtime/config/mod.rs
- src-tauri/src/modules/runtime/config/json_helpers.rs

---

## Contract（CHARTER 不变量映射）

- **I1 pub 符号集合**：set-equal — 14 段全是 file-private，RUST_PUB_RE 不捕获；`mod json_helpers` 是私有 mod (no `pub`)，也不捕获；mod.rs 继承 config.rs 的 32 个 pub_symbols 一字不少
- **I2 测试**：不增不减；config.rs 的 `#[cfg(test)] mod tests` 跟随 git mv 进入 mod.rs，测试 `use crate::modules::runtime::config::{...}` 路径不变
- **I3 事件名 / 字符串字面量**：所有 `ConfigError::Parse(format!("{context}: ..."))` 错误信息 byte-identical
- **I4 持久化 key**：本 cluster 不解析具体字段名，仅做类型断言；不影响 settings.json schema
- **I5 shim**：14 段无外部调用方（file-private）；不需 `pub use` shim
- **I6 函数体一行不改**：byte-identical move（含 `Result<...>` 闭包链 / `match` 分支 / `unwrap_or_else` 错误模板）
- **I7 lint**：cargo fmt + clippy `--workspace --all-targets -D warnings` 必须 GREEN

---

## Out of Scope

- ❌ 不改 `expect_*` / `optional_*` / `parse_bool_map` 等 fn 签名（参数顺序、`&str` vs `&String`、生命周期）
- ❌ 不改 `ConfigError::Parse` 错误信息模板
- ❌ 不抽其他 cluster（schema / permission / sandbox / boundary / merge / mcp / memory）→ 留 T1-B-2..5
- ❌ 不动 25+ 个 `use crate::modules::runtime::config::*` 调用方（git mv 后 path 不变）
- ❌ 不改 `pub` 项的可见性 / 字段顺序 / derive
- ❌ 不顺手把 `extend_unique` / `push_unique` 改成 `Vec::dedup` 或 itertools 调用

---

## Execute Plan

1. `git mv src-tauri/src/modules/runtime/config.rs src-tauri/src/modules/runtime/config/mod.rs`
   - 创建目录后 mv（git 自动识别 rename）
2. 创建 `src-tauri/src/modules/runtime/config/json_helpers.rs`，文件头：
   ```rust
   //! JSON parsing primitives for runtime config (settings.json / project.json).
   //!
   //! Extracted from `runtime/config.rs` in GFR-T1-B-1 (pure structural
   //! move; function bodies byte-identical). All helpers are
   //! file-private and consumed via a glob `use` in `super`.
   ```
   依赖 `use`：
   ```rust
   use std::collections::BTreeMap;

   use serde_json::Value as JsonValue;

   use super::ConfigError;
   ```
3. 把 14 段（12 fn + 2 helper fn）原样粘入。可见性保持 file-private（no `pub`）。
4. 在 `runtime/config/mod.rs` 顶部（紧跟 module-level doc 之后、第一个 `use` 之前）插入：
   ```rust
   mod json_helpers;
   use json_helpers::*;
   ```
5. 在 `mod.rs` 中删除 line 1647–1881 的 14 段定义（完整连续区段；删除位置留 1 行注释 `// JSON parse primitives moved to json_helpers (GFR-T1-B-1).`）。
6. `cargo build --manifest-path src-tauri/Cargo.toml` PASS。
7. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 后再 build。
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings` GREEN。
9. `cargo test --manifest-path src-tauri/Cargo.toml --no-run` PASS。

---

## Verify Whitelist

`./scripts/pack run GFR-T1-B-1` 预期允许 14 条 added pub_symbols（pack 设计修正：`pub(super) fn` 仍被 RUST_PUB_RE 捕获，与 `pub use` 不同——只有 `pub use` / `pub(crate) use` 被排除）：

- `fn expect_object`, `fn expect_string`
- `fn optional_string`, `fn optional_bool`
- `fn optional_u8`, `fn optional_u16`, `fn optional_u32`, `fn optional_u64`
- `fn parse_bool_map`
- `fn optional_string_array`, `fn optional_string_map`
- `fn deep_merge_objects`, `fn extend_unique`, `fn push_unique`

`pub_symbols` removed: 必须为 0（I1 不允许丢符号；上述 14 个原本就是 file-private，所以 removed 仅会出现 = 0）
`event_strings` added/removed: 必须为 0
`test_names` added/removed: 必须为 0
LOC drift 容忍 ≤ 5%

---

## Done Criteria

- `cargo build` / `cargo test --no-run` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` GREEN
- `./scripts/pack run GFR-T1-B-1` 报告 `pub_symbols: <N> (unchanged)` 且 exit 0
- `runtime/config/mod.rs` ≈ 2410 LOC（2640 − 230）
- `runtime/config/json_helpers.rs` ≈ 240 LOC
- 1 PR、1 commit，message 以 `refactor(GFR-T1-B-1):` 起头
- `docs/packs/REGISTRY.md` 中 `GFR-T1-B-1` 状态为 `done`
