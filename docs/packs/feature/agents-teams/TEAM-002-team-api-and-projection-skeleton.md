# TEAM-002: Team API And Projection Skeleton

## Status
- State: draft

## Goal
建立 Teams 的 frontend action facade、transport contract、projection skeleton 与 active team pointer。

## Spec
- `src/api/teams.ts` 暴露 create/list/get/update/member CRUD facade → `teams_api_uses_client_facade`
- `teamStore` 只保存 activeTeamId/selection，不保存 runtime truth → `team_store_is_pointer_only`
- team projection reducer 能 ingest team_created/member_added/team_updated fixtures → `team_projection_reducer_handles_basic_events`

## Files
- `src/api/teams.ts`
- `src/transport/team-contracts.ts`
- `src/stores/team-store.ts`
- `src/runtime-projection/team-*`
- `src/modules/teams/**`
- `src/**/*.test.*`

## Reads
- `ARCHITECTURE.md` §9.4
- `src/api/conversations.ts`
- `src/stores/session-store.ts`
- `src/runtime-projection/runtime-projection-store.ts`

## Contract
- Teams UI 不直接 import `@/lib/tauri`。
- 不启动真实 team run。
- projection skeleton 不复用 conversation-slice 保存 runtime truth。

## Verify
- `./scripts/pack run TEAM-002`
- `npm test -- teams`
- `npm run build`

