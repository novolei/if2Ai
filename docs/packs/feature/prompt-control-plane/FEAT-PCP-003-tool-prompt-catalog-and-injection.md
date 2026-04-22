# FEAT-PCP-003: Tool Prompt Catalog And Injection

## Status
- State: active

## Goal
建立 `ToolPromptCatalog` 与按条件注入机制，优先覆盖 web / file / memory / ask-user 四类 tool family。

## Spec
- tool prompt catalog 可按 family 返回 entries → 测试 `modules::application::prompt_catalog::tests::tool_catalog_groups_by_family`
- 协调器只在匹配场景下注入 tool policy，不做无脑全注入 → 测试 `modules::application::prompt_coordinator::tests::tool_entries_are_conditionally_activated`
- web family 继续兼容现有 routing block → 测试 `modules::application::prompt_coordinator::tests::web_tool_family_preserves_existing_routing_contract`

## Files (scope)
- `src-tauri/src/modules/application/prompt_catalog/tool.rs` (new)
- `src-tauri/src/modules/application/prompt_coordinator.rs`
- `src-tauri/src/modules/runtime/prompt_tools_guide.rs`

## Reads
- `docs/design-docs/prompt-control-plane-foundation.md`
- `src-tauri/src/modules/runtime/prompt_tools_guide.rs`

## Contract
- 不移除现有 web routing block
- 不把每个 tool prompt 常驻注入
- 不引入新 dependency

## Out Of Scope
- ❌ 不实现 UI 设置项
- ❌ 不实现 utility/coordinator prompts

## Verify
- ./scripts/pack run FEAT-PCP-003

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
