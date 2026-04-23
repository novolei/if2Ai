# TEAM-005: Team Workspace UI

## Status
- State: draft

## Goal
把 Teams 作为一级 workspace 接入 AppShell，展示 roster、timeline、delegation graph、review inbox 与 artifacts。

## Spec
- `AppSection` 支持 `teams`，`ContentRouter` 可进入 `TeamWorkspace` → `teams_section_routes_to_workspace`
- TeamWorkspace 只读 team projection/selectors → `team_workspace_uses_projection_only`
- roster/timeline/delegation/review/artifact 面板可显示空态与 fixture 数据 → `team_workspace_renders_fixture_projection`

## Files
- `src/modules/app-shell/**`
- `src/modules/teams/**`
- `src/runtime-projection/team-*`
- `src/stores/team-store.ts`
- `src/**/*.test.*`

## Reads
- `ARCHITECTURE.md` §9.4
- `src/modules/app-shell/types.ts`
- `src/modules/app-shell/ContentRouter.tsx`
- `docs/packs/feature/agents-teams/TEAM-002-team-api-and-projection-skeleton.md`

## Contract
- UI 不直接订阅 raw Tauri events。
- 不把 Teams 塞进 ChatWorkspace 内部。
- 保持现有 chat/memory/settings routes 不变。

## Verify
- `./scripts/pack run TEAM-005`
- `npm test -- teams`
- `npm run build`

