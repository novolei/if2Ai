# GFR-T1-B-4: Extract memory cluster from `runtime/config/mod.rs`

## Status
- State: `done`

## Goal

T1-B 第 3 刀。把 `runtime/config/mod.rs` 中所有 memory 相关 types + impls + parsers + If2AiMemoryOverrides + read_if2ai_memory_overrides 搬到新建 `runtime/config/memory.rs`。

## Source

- `runtime/config/mod.rs` lines 91–136 (MemoryRecallMode + MemoryPolicyEnforceMode enums + impls)
- lines 138–198 (CompilerConfig + Default)
- lines 200–333 (MemoryFeatureConfig + Default + 7-method impl)
- lines 1273–1352 (parse_optional_memory_feature_config)
- lines 1354–1393 (If2AiMemoryOverrides struct + read_if2ai_memory_overrides)
- lines 1395–1415 (parse_memory_recall_mode_label + parse_memory_policy_enforce_mode_label)

## Destination

- 新建 `runtime/config/memory.rs`
- mod.rs `mod memory;` + `pub use memory::{MemoryRecallMode, MemoryPolicyEnforceMode, CompilerConfig, MemoryFeatureConfig};`
- mod.rs `use memory::{If2AiMemoryOverrides, parse_optional_memory_feature_config};` (private re-exports — let tests still use `super::If2AiMemoryOverrides` from inside #[cfg(test)] mod tests)

## Files (scope)
- src-tauri/src/modules/runtime/config/mod.rs
- src-tauri/src/modules/runtime/config/memory.rs

## Contract
- I1: 4 pub types via `pub use` shim (set-equal); 1 pub(super) parser + 0 file-private fns made pub(super) for cross-file → whitelist
- I3: settings.json keys (`memory.controlPlaneV1Enabled`/`recallMode`/`policyEnforceMode`/`timezone`/`logicalDayCutoffHour`/`injectToPrompt`/`maxInjectTokens`) byte-identical
- I3: `~/.if2ai/memory_config.json` snake_case fields byte-identical
- I3: 4 mode strings (lexical/hybrid/shadow/enforce) byte-identical
- I6: function bodies byte-identical

## Out of Scope
- ❌ 不动 ConfigLoader / RuntimeConfig (B-5/6)
- ❌ 不动 schema enums (B-2)
- ❌ 不改 7 字段 keys / 4 mode strings
- ❌ 不动外部 6 个调用方 (memory/* + commands/* + session/manager.rs)

## Verify Whitelist

- pub_added: `[fn parse_optional_memory_feature_config, fn read_if2ai_memory_overrides, fn parse_memory_recall_mode_label, fn parse_memory_policy_enforce_mode_label, struct If2AiMemoryOverrides]` (5 items: 4 fn + 1 struct, all pub(super))
- pub_removed: `[]` (4 pub types preserved via shim)
- events / tests: `[]`

## Verify
```
./scripts/pack snapshot GFR-T1-B-4 --phase before
./scripts/pack verify GFR-T1-B-4
```
