# TEAM-006: Team Memory And Permission Policy

## Status
- State: draft

## Goal
增加 team memory scope 与 team/member 级 permission policy，避免团队协作污染 global/project memory。

## Spec
- MemoryScope 支持 `team` 且默认不提升到 global → `team_memory_scope_is_explicit`
- TeamPolicy 能定义 tool allow/deny、approval mode、member escalation → `team_permission_policy_resolves_member_request`
- reviewer notes 进入 event log，但 long-term memory promotion 需要 policy gate → `team_memory_promotion_requires_policy`

## Files
- `src-tauri/src/modules/memory/**`
- `src-tauri/src/modules/application/permission_service.rs`
- `src-tauri/src/modules/control_plane/**`
- `src-tauri/src/modules/team/**`
- `src-tauri/src/modules/runtime/contracts/**`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §9.5
- `docs/packs/feature/agents-teams/TEAM-001-team-domain-contracts.md`
- `docs/packs/feature/migration-core/MIG-019-pending-permission-recovery.md`

## Contract
- 不让 team memory 默认写入 global。
- permission decision 必须带 team/member correlation。
- 不降级现有 session/project/global memory behavior。

## Verify
- `./scripts/pack run TEAM-006`
- `cargo test --manifest-path src-tauri/Cargo.toml team_memory`
- `cargo test --manifest-path src-tauri/Cargo.toml permission`

