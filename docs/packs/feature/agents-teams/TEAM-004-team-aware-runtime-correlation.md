# TEAM-004: Team-aware Runtime Correlation

## Status
- State: draft

## Goal
扩展 runtime correlation，让 team/member/role/delegation facts 进入 canonical event log 与 projection。

## Spec
- `CorrelationIds` 支持 team_id/member_id/role_id/parent_run_id/delegation_id → `correlation_ids_include_team_fields`
- team member run event 可被 event log replay 到 team projection → `team_events_replay_from_run_log`
- 单人 session 不带 team fields 时保持兼容 → `non_team_runtime_events_remain_compatible`

## Files
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/src/modules/runtime/event_log.rs`
- `src-tauri/src/modules/team/**`
- `src/runtime-projection/team-*`
- `src/**/*.test.*`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §9.3
- `docs/packs/feature/architecture-gaps/GAP-002-runtime-contract-unification.md`
- `docs/packs/feature/agents-teams/TEAM-003-team-supervisor-mvp.md`

## Contract
- 不破坏既有 run log replay。
- team correlation 必须是 optional，不能污染普通 chat path。
- 新增 team event kind 必须有 TS translator 覆盖。

## Verify
- `./scripts/pack run TEAM-004`
- `cargo test --manifest-path src-tauri/Cargo.toml runtime_contract`
- `npm test -- team-projection`

