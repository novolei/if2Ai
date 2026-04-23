# TEAM-007: Team Tool Ledger And Review Gates

## Status
- State: draft

## Goal
把 tool attempt ledger 接入 team run，让 executor 工具结果可被 reviewer gate 阻断、批准或要求重试。

## Spec
- member tool attempts 关联 team_id/member_id/delegation_id → `team_tool_attempts_are_correlated`
- reviewer gate 可产生 approved/rejected/retry_requested event → `review_gate_emits_decision_events`
- retry_requested 不生成新的孤立 run，必须挂在同一 delegation timeline → `team_retry_stays_in_delegation`

## Files
- `src-tauri/src/modules/team/**`
- `src-tauri/src/modules/application/tool_executor.rs`
- `src-tauri/src/modules/control_plane/**`
- `src-tauri/src/modules/runtime/**`
- `src/runtime-projection/team-*`
- `src/**/*.test.*`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §9, §10
- `docs/packs/feature/migration-core/MIG-022-tool-attempt-ledger-and-timeline-contract.md`
- `docs/packs/feature/agents-teams/TEAM-004-team-aware-runtime-correlation.md`

## Contract
- reviewer decision 是 runtime event，不是 UI-only state。
- 不改变普通 single-agent tool attempt ledger。
- retry/failure 必须可由 event log replay。

## Verify
- `./scripts/pack run TEAM-007`
- `cargo test --manifest-path src-tauri/Cargo.toml team_review_gate`
- `npm test -- team-projection`

