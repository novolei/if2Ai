# FEAT-ID-001: Identity Domain And Resolution

## Status

- State: `done`
- Owner: `@executor`
- Depends On: `none`
- Last Updated: `2026-04-22`
- Completed Commit: `d902a06`

---

## Goal

建立 If2Ai 的 identity 基础领域模型，包括：

- `SoulDefinition`
- `PersonaDefinition`
- `ResolvedIdentity`
- identity registry
- global default settings 读取
- session/global/fallback 的 resolution 规则

本 Pack 的目标是让后端第一次能够回答：  
“当前这一个 turn，到底应该使用哪个 Soul 和 Persona？”

---

## Why Now

1. 后续 prompt planner、session persistence、memory tagging 都依赖统一 identity 解析结果。
2. 如果不先定义 resolution rules，后续写入的 `soul_id/persona_id` 都没有稳定语义。
3. Settings UI 需要有可靠的后端列表与默认值来源。

---

## Spec

1. 新增 identity 领域模块与核心数据结构  
   → 测试 `identity::tests::definitions_round_trip`

2. 新增内建 Soul / Persona registry，至少包含 1 个 Soul 与 2 个 Persona  
   → 测试 `identity::tests::builtin_registry_contains_default_identity`

3. 新增 `IdentitySettings` 配置读取，支持 `identity.defaultSoulId` 与 `identity.defaultPersonaId`  
   → 测试 `identity::tests::config_reads_identity_defaults`

4. 新增 `ResolvedIdentity` 解析函数，支持 resolution order：`session override > global default > built-in fallback`  
   → 测试 `identity::tests::session_override_wins_over_global_default`

5. Persona 必须校验 `persona.soul_id == resolved_soul_id`，不匹配时丢弃 Persona 并记录 warning  
   → 测试 `identity::tests::mismatched_persona_is_dropped`

6. 无效 Soul / Persona id 不得导致 fatal failure，必须 fallback  
   → 测试 `identity::tests::invalid_identity_falls_back_safely`

---

## Files (scope)

- `src-tauri/src/modules/identity/mod.rs` (new)
- `src-tauri/src/modules/identity/definition.rs` (new)
- `src-tauri/src/modules/identity/registry.rs` (new)
- `src-tauri/src/modules/identity/resolver.rs` (new)
- `src-tauri/src/modules/identity/settings.rs` (new)
- `src-tauri/src/modules/runtime/config/mod.rs` (modify)
- `src-tauri/src/modules/runtime/config/parsers.rs` (modify)
- `src-tauri/src/modules/runtime/config/schema.rs` (modify if needed)
- `src-tauri/src/modules/runtime/mod.rs` (modify)

---

## Reads

- `src-tauri/src/modules/application/prompt_planner/mod.rs`
- `src-tauri/src/modules/session/manager.rs`
- `src-tauri/src/modules/runtime/config/**`
- `docs/design-docs/identity-soul-persona-memory-foundation.md`

---

## Contract

- Identity registry 首版只支持 built-in definitions
- 默认值读取必须来自 runtime settings 命名空间 `identity.*`
- Resolver 不得直接依赖 UI / Tauri command 层
- Resolver 输出必须是稳定的纯数据对象，不得执行 I/O 副作用
- 无效 identity 必须降级 fallback，不得阻断 agent turn

---

## Implementation Notes

1. 模块命名优先使用 `modules/identity/**`，不要塞进 prompt / session / memory 子模块中。
2. Resolver 建议暴露一个主入口：
   - `resolve_identity(global_defaults, session_override, registry) -> ResolvedIdentity`
3. Warning 收集可先用简单 `Vec<String>`，后续再并入 diagnostics。
4. 首版 registry 可内建在 Rust 常量或 small static list 中，不需要文件系统扫描。

---

## Verify

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- `cargo test --manifest-path src-tauri/Cargo.toml identity`

---

## Acceptance

- 后端存在稳定 identity 领域模型
- 可解析默认 Soul / Persona
- 可在不依赖 session/prompt/memory 的前提下单测 resolution rules
- 所有 Spec 对应测试通过

---

## Out Of Scope

- 不接 prompt planner
- 不接 session persistence
- 不接 Tauri commands
- 不接前端 UI
- 不接 memory tagging
