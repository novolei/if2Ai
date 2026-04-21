# GFR-T1-F-1: memory/ticker 单刀目录化 + 4 路抽出

## Status
- State: `done`

## Goal

T1-F 单刀完成。`memory/ticker.rs` (1363 LOC) 一次性切成 5 文件目录：

1. `git mv ticker.rs → ticker/mod.rs`
2. tests block (~462 LOC) → `ticker/tests.rs`
3. config + state types (~76 LOC) → `ticker/types.rs`
4. daily pipeline runner + helpers (~160 LOC) → `ticker/daily.rs`
5. `impl TurnHook` (~147 LOC) → `ticker/turn_hook.rs`

mod.rs 留 MemoryTicker struct + impl Debug + impl MemoryTicker (~526 LOC, < 800 hard limit ✓ 略超 500 target — kept on SIZE_EXEMPT)。

## Source / Destination
- mod.rs lines 57-132 (TickerConfig + Default + DailyStep + TickerState) → `types.rs`
- mod.rs lines 589-748 (daily_step_name + run_daily_inline + finish_in_progress + InProgressGuard) → `daily.rs`
- mod.rs lines 750-896 (`impl TurnHook for MemoryTicker`) → `turn_hook.rs`
- mod.rs lines 898-1363 (`#[cfg(test)] mod tests { ... }`) → `tests.rs`

## Files (scope)
- src-tauri/src/modules/memory/ticker.rs (deleted via mv)
- src-tauri/src/modules/memory/ticker/mod.rs (rename target + reduced)
- src-tauri/src/modules/memory/ticker/types.rs (new)
- src-tauri/src/modules/memory/ticker/daily.rs (new)
- src-tauri/src/modules/memory/ticker/turn_hook.rs (new)
- src-tauri/src/modules/memory/ticker/tests.rs (new)

## Contract
- I1: pub types (TickerConfig + DailyStep + TickerState) via `pub use types::{...}` shim
- I2: 测试名集合 byte-identical
- I6: 函数体一字不改
- I3: audit emission paths byte-identical

## Verify
- cargo build PASS clean
- cargo fmt clean
- cargo clippy --workspace --all-targets -D warnings GREEN
- cargo test --no-run PASS (all tests still compile)

## Done
- mod.rs 1363 → 526 LOC (-61%)
- T1-F arc 单刀完成
