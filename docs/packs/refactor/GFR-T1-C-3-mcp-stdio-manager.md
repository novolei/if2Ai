# GFR-T1-C-3: 抽 mcp_stdio Server Manager

## Status
- State: `done`

## Goal

T1-C 收尾刀。把 McpServerManager 集群（McpServerManagerError + ToolRoute + ManagedMcpServer + McpServerManager + 4 impl）从 mcp_stdio/mod.rs 抽到 `mcp_stdio/manager.rs`。mod.rs 跌至 270 LOC（< 500 target ✓，可摘除 SIZE_EXEMPT）。

## Source / Destination
- mod.rs lines 35-389 (McpServerManagerError enum + 3 impl + ToolRoute + ManagedMcpServer + McpServerManager + impl)
- → `runtime/mcp_stdio/manager.rs`

## Files (scope)
- src-tauri/src/modules/runtime/mcp_stdio/mod.rs
- src-tauri/src/modules/runtime/mcp_stdio/manager.rs

## Contract
- I1: 2 pub types (McpServerManager + McpServerManagerError) via pub use shim → set-equal
- I3: error message templates byte-identical (`"unknown MCP tool"`, `"MCP server returned JSON-RPC error for X"` etc)
- I6: function bodies byte-identical
- 跨文件依赖: manager.rs 通过 `super::{spawn_mcp_stdio_process, default_initialize_params, McpStdioProcess}` 访问 mod.rs 中的 stdio transport；后者 default_initialize_params 提升为 `pub(super)`

## Verify Whitelist
- pub_added: `[fn default_initialize_params]` (升 pub(super))
- pub_removed: `[]` (2 pub types via shim)

## Done
- mcp_stdio/mod.rs 270 LOC (< 500 target ✓)
- T1-C arc 收尾：mcp_stdio.rs 1725 LOC -> 5-file dir (mod 270 + manager 373 + tests 914 + types 171 + rpc 55)
