# TEAM-001: Team Domain Contracts

## Status
- State: draft

## Goal
建立 Agents Teams 的后端 bounded context 与核心数据模型，但不接执行主链。

## Spec
- 新增 TeamMeta/TeamMember/AgentRole/TeamPolicy/TeamRun/Delegation/TeamProjection 类型 → `team_domain_types_roundtrip`
- team 不嵌入 session/project/identity 内部结构 → `team_is_independent_bounded_context`
- team policy 支持 budget/tool approval/provider preference 基础字段 → `team_policy_defaults_are_stable`

## Files
- `src-tauri/src/modules/team/**`
- `src-tauri/src/modules/mod.rs`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §9
- `docs/design-docs/if2ai-vnext-session-runtime-blueprint.md`

## Contract
- 不接 UI，不启动 team run。
- 不把 team 字段塞入 session/project/identity schema。
- 所有 public structs 需要 `///` doc。

## Verify
- `./scripts/pack run TEAM-001`
- `cargo test --manifest-path src-tauri/Cargo.toml team_domain`

