# GFR-T1-D-1: sqlite_provider 目录化 + 4 路抽出

## Status
- State: `done`

## Goal

T1-D 单刀完成。`memory/providers/sqlite_provider.rs` (1579 LOC) 一次性切成 4 文件目录：

1. `git mv sqlite_provider.rs → sqlite_provider/mod.rs`
2. tests block (~643 LOC) → `sqlite_provider/tests.rs`
3. impl MemoryProvider trait impl (~575 LOC) → `sqlite_provider/provider_impl.rs`
4. 4 个 scope helpers + parse_category (~88 LOC) → `sqlite_provider/scope.rs`

mod.rs 留 SqliteMemoryProvider struct + impl SqliteMemoryProvider 内部 helpers (constructors / setup / connection helpers, 268 LOC, < 500 target ✓)。

## Source / Destination

- mod.rs lines 268-354 (4 fns + parse_category) → `scope.rs`
- mod.rs lines 356-932 (`#[async_trait] impl MemoryProvider`) → `provider_impl.rs`
- mod.rs lines 934-1579 (`#[cfg(test)] mod tests { ... }`) → `tests.rs`

## Files (scope)
- src-tauri/src/modules/memory/providers/sqlite_provider.rs (deleted via mv)
- src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs (rename target + reduced)
- src-tauri/src/modules/memory/providers/sqlite_provider/scope.rs (new)
- src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs (new)
- src-tauri/src/modules/memory/providers/sqlite_provider/tests.rs (new)

## Contract
- I1: 1 pub struct (SqliteMemoryProvider) 仍在 mod.rs，外部 importer 路径不变
- I2: 测试名集合 byte-identical (sed dedent only)
- I6: 函数体一字不改（包括所有 SQL 字符串模板 + audit emission paths）
- I3: SQL CASE/WHERE/ORDER BY 模板 byte-identical (3 个 scope helper 的 schema-bound 字符串)

## Verify Whitelist
- pub_added: `[]` (1 pub type via mod.rs，3 sub-files private with pub(super) helpers — 不被外部捕获)
- pub_removed: `[]`
- 实际验证: cargo test --no-run PASS (所有原 tests 编译通过)

## Done
- mod.rs 1579 → 268 LOC (-83%)
- 从 SIZE_EXEMPT 摘除（< 500 target）
- T1-D arc 单刀完成
