# GAP-005: Stream Task Decomposition

## Status
- State: draft

## Goal
拆分 `turn_service/stream_task.rs` 的 model loop、tool loop、event emission、permission wait、finalization 职责，降低执行主链 god-file 风险。

## Spec
- stream task orchestration 文件低于 800 LOC hard limit → `stream_task_under_god_file_limit`
- model loop/tool loop/event emission 分离为可测模块 → `stream_task_components_are_separately_testable`
- 行为不变：run start/complete/error/tool events 顺序保持稳定 → `stream_event_order_is_preserved`

## Files
- `src-tauri/src/modules/application/turn_service/**`
- `src-tauri/src/modules/application/tool_executor.rs`
- `src-tauri/src/modules/runtime/**`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §4.2, §7, §11
- `docs/packs/CHARTER.md` §5
- `docs/packs/feature/migration-core/MIG-016-canonical-run-event-log-foundation.md`
- `docs/packs/feature/migration-core/MIG-022-tool-attempt-ledger-and-timeline-contract.md`

## Contract
- 不改变 public IPC、event names、run log event names。
- 不重写 provider/tool behavior。
- 拆分必须保留 reviewer 可对照的 behavior tests。

## Verify
- `./scripts/pack run GAP-005`
- `cargo test --manifest-path src-tauri/Cargo.toml turn_service`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

