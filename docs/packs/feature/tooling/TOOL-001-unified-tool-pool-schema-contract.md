# TOOL-001: Unified Tool Pool & Schema Contract

## Status
- State: done

## Goal
Create a canonical provider-visible tool pool for each turn and log its stable
tool names, policy, and schema hash before provider request execution.

## Spec (verifiable)
- Tool definitions are sorted deterministically and hashed
  -> test `tool_001_canonical_tool_pool_sorts_and_hashes_schema`.
- Skill attenuation and work-loop policy are applied in one helper before
  provider request assembly.
- `provider_tool_call_diagnostics` records visible tool names, canonical tool
  names, schema hash, and policy.

## Files (scope)
- `src-tauri/src/modules/application/turn_service/stream.rs`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/application/turn_service/work_loop.rs`

## Done
- Existing built-in/registered tools now flow through one canonical pool seam.
- MCP-origin tools will join this pool when MCP-002 registers them through the
  same tool registry surface.
