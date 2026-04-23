# GAP-001: Session JSON Fact Split

## Status
- State: draft

## Goal
把 `session.json` 从混合事实源降级为 metadata/compat 存储，让 transcript/runtime facts 改由 run event log + replay projection 承担。

## Spec
- session metadata 与 transcript/runtime payload 分离 → `session_manager_metadata_without_runtime_transcript`
- history 读取优先 event log，legacy `session.json` 只作为 empty-log fallback → `history_prefers_event_log_over_session_json`
- 写入路径不得新增新的 transcript 字段到 session metadata → `session_save_rejects_runtime_fact_regression`

## Files
- `src-tauri/src/modules/session/**`
- `src-tauri/src/modules/runtime/history.rs`
- `src-tauri/src/commands/session.rs`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §6-8
- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`
- `docs/packs/feature/migration-core/MIG-016-canonical-run-event-log-foundation.md`
- `docs/packs/feature/migration-core/MIG-018-session-history-replay-and-paging.md`

## Contract
- 不删除 legacy fallback，只标记为 compatibility path。
- 不改变现有 session IPC 命令名。
- 所有 runtime transcript 写事实必须指向 run event log。

## Verify
- `./scripts/pack run GAP-001`
- `cargo test --manifest-path src-tauri/Cargo.toml session`
- `cargo test --manifest-path src-tauri/Cargo.toml history`

