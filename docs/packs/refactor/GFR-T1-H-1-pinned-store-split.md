# GFR-T1-H-1: memory/pinned/store 单刀目录化 + 2 路抽出

## Status
- State: `done`

## Goal

T1-H 单刀完成。`memory/pinned/store.rs` (1009 LOC) → 3 文件目录：

1. `git mv pinned/store.rs → pinned/store/mod.rs`
2. tests block (~292 LOC) → `pinned/store/tests.rs`
3. `impl PinnedStore for SqlitePinnedStore` + AddOutcome enum (~340 LOC) → `pinned/store/provider_impl.rs`

mod.rs 留 PinnedStore trait + SqlitePinnedStore struct + impl SqlitePinnedStore (内部 helpers) + NullPinnedStore (~369 LOC, < 500 target ✓)。

## Files (scope)
- src-tauri/src/modules/memory/pinned/store.rs (deleted via mv)
- src-tauri/src/modules/memory/pinned/store/mod.rs
- src-tauri/src/modules/memory/pinned/store/provider_impl.rs
- src-tauri/src/modules/memory/pinned/store/tests.rs

## Contract
- I1: pub trait PinnedStore + SqlitePinnedStore + NullPinnedStore + 2 consts (MAX_PINS_PER_SCOPE / MAX_PIN_CONTENT_CHARS) 全部留 mod.rs；外部 importer 路径不变
- SELECT_COLUMNS 由 file-private const 升 pub(super) 让 provider_impl.rs 访问
- I3: 7 步 add 流水线 + 全部 SQL 字符串 byte-identical
- I6: function bodies byte-identical

## Verify
- cargo build PASS clean
- cargo fmt clean
- cargo clippy --workspace --all-targets -D warnings GREEN
- cargo test --no-run PASS

## Done
- mod.rs 1009 → 369 LOC (-63%)
- 不需 SIZE_EXEMPT (under 500 target)
- T1-H arc 单刀完成
