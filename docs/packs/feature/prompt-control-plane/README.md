# Prompt Control Plane Packs

## Scope

本目录承载 If2Ai `prompt control plane` 与 `prompt coordinator` 路线图。

## Why

目标不是继续堆一个更长的 system prompt，而是把 prompt 变成平台级控制面，统一管理：

1. base system
2. identity (`Soul / Persona`)
3. scenario profile
4. tool prompts
5. memory prompts
6. utility prompts
7. coordinator overlays

## Pack Order

1. `FEAT-PCP-001` Prompt coordinator + assembly decision foundation
2. `FEAT-PCP-002` Scenario / task-focus prompt catalog
3. `FEAT-PCP-003` Tool prompt catalog + conditional injection
4. `FEAT-PCP-004` Utility / coordinator prompt lanes
5. `FEAT-PCP-005` Prompt control panel settings + diagnostics UX

## Reads

- `docs/design-docs/prompt-control-plane-foundation.md`
- `docs/design-docs/identity-soul-persona-memory-foundation.md`
- `src-tauri/src/modules/application/prompt_planner/*`
- `src-tauri/src/modules/application/turn_service/*`
