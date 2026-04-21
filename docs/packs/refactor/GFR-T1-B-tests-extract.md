# GFR-T1-B-tests: Extract `#[cfg(test)] mod tests` into sibling `tests.rs`

## Status
- State: `done`

## Goal

Quick win: `runtime/config/mod.rs` 末尾的 757-line inline `#[cfg(test)] mod tests { ... }` block 整体提到 sibling `tests.rs`，mod.rs 用 `#[cfg(test)] mod tests;` 一行替换。零业务变更，纯目录化。

## Source / Destination
- src-tauri/src/modules/runtime/config/mod.rs (lines 1090–1847 删除)
- src-tauri/src/modules/runtime/config/tests.rs (new, 755 LOC)

## Files (scope)
- src-tauri/src/modules/runtime/config/mod.rs
- src-tauri/src/modules/runtime/config/tests.rs

## Contract
- I1: pub_symbols set-equal (tests do not declare pub items)
- I2: `test_names` set-equal
- I6: function bodies byte-identical (sed dedent of leading 4 spaces, no semantic change)

## Verify Whitelist
- pub_added/removed: `[]`
- test_names_added/removed: `[]`
- events: `[]`
