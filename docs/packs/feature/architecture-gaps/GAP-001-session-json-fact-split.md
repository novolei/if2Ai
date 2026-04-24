# GAP-001: Session JSON Fact Split

## Status
- State: active

## Task Ref
- `docs/vnext_new/task.md` T-004

## Spec Ref
- `docs/vnext_new/spec.md` §3.1, §8.1

## Design Ref
- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md` §3.1, §8.1

## Depends On
- T-001 (GAP-002 contract unified) ✓

## Goal
把 `session.json` 从混合事实源降级为 metadata/compat 存储，让 transcript/runtime facts 改由 run event log + replay projection 承担。

## Spec
- session metadata 与 transcript/runtime payload 分离 → `session_manager_metadata_without_runtime_transcript`
- history 读取优先 event log，legacy `session.json` 只作为 empty-log fallback → `history_prefers_event_log_over_session_json`
- 写入路径不得新增新的 transcript 字段到 session metadata → `session_save_rejects_runtime_fact_regression`

## Files
- `src-tauri/src/modules/session/manager.rs` — SessionMeta 增加 `project_id`
- `src-tauri/src/modules/runtime/history.rs` — 测试验证 event log 优先
- `src-tauri/src/commands/session.rs` — fallback 事件记录

## vNext 通用约束
- 无 `unwrap()` / `expect()` / `todo!()`（测试除外）
- 跨模块用 `crate::modules::*`
- B1: `cargo fmt --all` + `cargo clippy --all-targets -- -D warnings` + `cargo test`
- V1 (Contract Drift): SessionMeta 不得新增 transcript 字段

## Pack 专属约束
- V2 (Single Truth): history 读取路径：event log → session.json (fallback only)
- V3 (Event Log Integrity): fallback 事件需记录 trace
- 不删除 legacy `Session.messages`，只标记为 compatibility path
- 不改变现有 session IPC 命令名

## Contract
- 不删除 legacy fallback，只标记为 compatibility path。
- 不改变现有 session IPC 命令名。
- 所有 runtime transcript 写事实必须指向 run event log。

## Verify
- `./scripts/pack run GAP-001`
- `cargo test --manifest-path src-tauri/Cargo.toml session`
- `cargo test --manifest-path src-tauri/Cargo.toml history`
