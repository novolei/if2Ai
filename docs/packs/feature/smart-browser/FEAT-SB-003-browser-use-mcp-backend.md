# FEAT-SB-003: browser-use MCP Backend

## Status

- State: `draft`
- Owner: `@executor`
- Depends On: `FEAT-SB-001`, `FEAT-SB-002`
- Last Updated: `2026-04-25`

## Goal
Add browser-use as a Smart Browser backend through the existing MCP stdio runtime, while keeping If2Ai's browser UI, policy, and projection as the source of truth.

## Spec
- Runtime can register a managed `browser-use` stdio server using `uvx browser-use[cli] --mcp` → 测试 `modules::smart_browser::browser_use_mcp::tests::builds_stdio_config`
- MCP tools map to Smart Browser commands for navigate/state/screenshot/click/type → 测试 `modules::smart_browser::browser_use_mcp::tests::maps_core_tools`
- MCP results emit Smart Browser projection events and never update UI directly → 测试 `modules::smart_browser::browser_use_mcp::tests::emits_projected_observations`

## Files (scope)
- `src-tauri/src/modules/smart_browser/browser_use_mcp.rs` (new)
- `src-tauri/src/modules/smart_browser/mod.rs`
- `src-tauri/src/modules/runtime/mcp_stdio/manager.rs`
- `src-tauri/src/modules/runtime/config/mcp.rs`
- `src-tauri/src/modules/tools/builtin/browser_tool.rs`
- `src-tauri/tests/smart_browser_mcp_tests.rs` (new)

## Reads
- `docs/design-docs/smart-browser-architecture.md`
- `src-tauri/src/modules/runtime/mcp_stdio/manager.rs`
- `src-tauri/src/modules/runtime/config/mcp.rs`
- `https://github.com/browser-use/browser-use/blob/main/browser_use/mcp/server.py`
- `https://github.com/browser-use/browser-use/blob/main/browser_use/skill_cli/README.md`

## Contract
- Do not embed Python in-process; use stdio MCP only
- Do not expose raw `mcp__browser_use__*` tools directly to the main agent
- No automatic install of browser-use; surface missing dependency as a diagnostic
- No new npm dependency

## Out Of Scope
- ❌ 不接 `retry_with_browser_use_agent`
- ❌ 不接 browser-use cloud
- ❌ 不做 cookie/profile sync

## Verify
- ./scripts/pack run FEAT-SB-003
- cargo test --manifest-path src-tauri/Cargo.toml smart_browser_mcp
- manual: `uvx browser-use --mcp` diagnostic path reports available/missing cleanly

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
