# GFR-T1-G-1: runtime/prompt 单刀目录化 + 3 路抽出

## Status
- State: `done`

## Goal

T1-G 单刀完成。`runtime/prompt.rs` (1258 LOC) 一次性切成 4 文件目录：

1. `git mv prompt.rs → prompt/mod.rs`
2. tests block (~370 LOC) → `prompt/tests.rs`
3. skills index cluster (~257 LOC) → `prompt/skills_index.rs`
4. instruction file helpers + git context (~215 LOC) → `prompt/instruction_files.rs`

mod.rs 留 PromptBuildError + ContextFile + ProjectContext + SystemPromptBuilder + load_system_prompt + simple section renderers (~428 LOC, < 500 target ✓ — no SIZE_EXEMPT 条目)。

## Source / Destination
- mod.rs lines 46-302 → `skills_index.rs` (incl const MAX_SKILLS_INDEX_COUNT + SkillIndexEntry + collect_skill_index_entries + build_skills_index)
- mod.rs lines 580-794 → `instruction_files.rs` (discover/render/dedupe/normalize/truncate/git helpers, 14 fns)
- mod.rs lines 887-1257 → `tests.rs`

## Files (scope)
- src-tauri/src/modules/runtime/prompt.rs (deleted via mv)
- src-tauri/src/modules/runtime/prompt/mod.rs
- src-tauri/src/modules/runtime/prompt/skills_index.rs
- src-tauri/src/modules/runtime/prompt/instruction_files.rs
- src-tauri/src/modules/runtime/prompt/tests.rs

## Contract
- I1: pub items via `pub use` shim:
  - `pub use skills_index::{build_skills_index, invalidate_skills_index_cache, SkillIndexEntry};`
  - `pub(crate) use skills_index::collect_skill_index_entries;` (4 external importers in skills_list / skill_search / skill_find / skills_categories — paths unchanged)
- I2: 测试名 set byte-identical
- I6: 函数体一字不改

## Verify
- cargo build PASS clean (no warnings)
- cargo fmt clean
- cargo clippy --workspace --all-targets -D warnings GREEN
- cargo test --no-run PASS

## Done
- mod.rs 1258 → 428 LOC (-66%)
- 不需 SIZE_EXEMPT (under 500 target)
- T1-G arc 单刀完成
