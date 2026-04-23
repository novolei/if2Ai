# GAP-008: Contract Drift Guardrails

## Status
- State: draft

## Goal
建立前后端 contract drift guardrails，让 Rust runtime contract、TS DTO、projection event、run log schema 不再靠人工同步。

## Spec
- 新增 contract drift test，比较 Rust event kind 与 TS translator 支持集 → `runtime_event_kind_coverage_matches`
- 新增 schema snapshot 或 generated fixture，覆盖 run log entry/envelope payload → `runtime_contract_schema_snapshot_is_stable`
- CI/pack verify 能在新增未映射 event 时失败 → `unmapped_runtime_event_fails_fast`

## Files
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/tests/**`
- `src/runtime-projection/**`
- `src/transport/**`
- `src/**/*.test.*`
- `scripts/pack`

## Reads
- `ARCHITECTURE.md` §7
- `docs/packs/CHARTER.md` §4
- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`

## Contract
- 不引入新 dependency，除非另开 DEP Pack。
- drift guardrail 只检查 contract，不改变业务行为。
- 新增 event kind 必须同步 Rust tests 与 TS projection tests。

## Verify
- `./scripts/pack run GAP-008`
- `cargo test --manifest-path src-tauri/Cargo.toml runtime_contract`
- `npm test -- runtime-projection`

