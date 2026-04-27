# MCP-002: MCP Workbench IPC + UI

## Status
- State: active

## Goal
Expose stdio MCP servers as a testable, auditable workbench. Users should discover
tools/resources/prompts, test safe calls, inspect activity, and see unsupported
transports without turning them into active runtimes.

## Spec (verifiable)
- IPC lists servers and discovers tools/resources/prompts from a mock stdio server -> test `tests::mcp_workbench_discovers_capabilities`.
- IPC can call a tool, read a resource, and get a prompt with sanitized activity entries -> test `tests::mcp_workbench_records_activity`.
- Settings UI renders Servers, Tools, Resources, Prompts, Approvals, and Activity tabs -> test `src/modules/settings/pages/McpWorkbench.test.tsx`.
- Unsupported HTTP/SSE/WS transports display as inactive -> test `src/modules/settings/pages/McpWorkbench.test.tsx`.

## Files (scope)
- `src-tauri/src/modules/runtime/mcp_stdio/manager.rs`
- `src-tauri/src/modules/runtime/mcp_stdio/types.rs`
- `src-tauri/src/modules/runtime/mcp_stdio/tests.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/command_surface.rs`
- `src-tauri/src/commands/settings.rs`
- `src/transport/contracts.ts`
- `src/lib/tauri.ts`
- `src/modules/settings/pages/McpServicesSettingsPage.tsx`
- `src/modules/settings/pages/McpWorkbenchPage.tsx` (new)

## Reads
- `docs/design-docs/agent-work-loop/MCP-002-mcp-workbench-ipc-ui.md`
- `src-tauri/src/modules/runtime/config/mcp.rs`
- `src/modules/settings/*`

## Contract
- Phase 1 is stdio-only.
- Workbench tool calls still use the same safety/approval path when risky.
- Sanitize params/results before UI display.
- Do not persist activity beyond the agreed DTO in this slice.

## Out of Scope
- HTTP/SSE/WS runtime transports.
- OAuth, sampling, elicitation.
- Remote MCP marketplace install.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml runtime::mcp_stdio -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- Verify commands pass.
- `REGISTRY.md` moves this pack to done with date.
