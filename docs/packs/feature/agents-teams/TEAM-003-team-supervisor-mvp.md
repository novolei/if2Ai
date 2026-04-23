# TEAM-003: Team Supervisor MVP

## Status
- State: draft

## Goal
实现 TeamSupervisor MVP，支持 planner -> executor -> reviewer 的串行 delegation graph。

## Spec
- team run 创建 parent TeamRun 与 member Delegation → `team_supervisor_creates_delegation_graph`
- planner/executor/reviewer 状态按 queued/running/blocked/completed/failed 迁移 → `team_member_run_status_transitions`
- stop/cancel 能取消未完成 delegation → `team_supervisor_cancel_cascades`

## Files
- `src-tauri/src/modules/team/**`
- `src-tauri/src/modules/application/**`
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §9.2-9.3
- `docs/packs/feature/agents-teams/TEAM-001-team-domain-contracts.md`
- `docs/packs/feature/migration-core/MIG-020-session-supervisor-foundation.md`

## Contract
- TeamSupervisor 不绕过 SessionSupervisor；member run 必须有关联 run_id。
- 不并行实现复杂 quorum，只做串行 MVP。
- 所有 delegation 事件必须可进入 runtime event log。

## Verify
- `./scripts/pack run TEAM-003`
- `cargo test --manifest-path src-tauri/Cargo.toml team_supervisor`

