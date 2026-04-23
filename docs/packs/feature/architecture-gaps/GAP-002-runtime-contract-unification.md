# GAP-002: Runtime Contract Unification

## Status
- State: draft

## Goal
统一 `RuntimeEventEnvelope`、`StreamTokenPayload`、`RunLogEntry` 与 `run_id/stream_id` correlation 语义，消除前后端事件 contract 二次真相。

## Spec
- 所有 runtime events 都携带统一 correlation 字段 → `runtime_event_correlation_is_complete`
- stream payload 可无损转换为 canonical runtime envelope → `stream_payload_maps_to_runtime_envelope`
- run log entry 从 canonical envelope 构造，seq 单调且 run_id 稳定 → `run_log_entry_uses_canonical_envelope`

## Files
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/src/modules/runtime/event_log.rs`
- `src-tauri/src/modules/application/turn_service/**`
- `src/lib/tauri.ts`
- `src/runtime-projection/**`
- `src/**/*.test.*`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §7-8
- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`
- `docs/packs/feature/migration-core/MIG-016-canonical-run-event-log-foundation.md`
- `docs/packs/feature/migration-core/MIG-017-runtime-projection-chat-truth-cutover.md`

## Contract
- `run_id` 是 runtime 主 correlation；`stream_id` 只能作为 transport compatibility id。
- 不新增未进入 canonical envelope 的 runtime event kind。
- TS/Rust 字段命名必须保持一一对应。

## Verify
- `./scripts/pack run GAP-002`
- `cargo test --manifest-path src-tauri/Cargo.toml runtime_contract`
- `npm test -- runtime-projection`

