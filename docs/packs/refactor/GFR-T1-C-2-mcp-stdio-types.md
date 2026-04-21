# GFR-T1-C-2: 抽 mcp_stdio MCP 协议 DTOs

## Status
- State: `done`

## Goal

T1-C 第二刀。把 16 个 MCP 协议 DTOs（Initialize / ListTools / ToolCall / ListResources / ReadResource 系列 + ManagedMcpTool + UnsupportedMcpServer）从 `runtime/mcp_stdio/mod.rs` 抽到新建 `runtime/mcp_stdio/types.rs`。

## Source / Destination
- mod.rs lines 24-180 (16 pub struct + 4 inline impls — 都是 serde DTOs)
- → `runtime/mcp_stdio/types.rs`

## Files (scope)
- src-tauri/src/modules/runtime/mcp_stdio/mod.rs
- src-tauri/src/modules/runtime/mcp_stdio/types.rs

## Contract
- I1: 16 pub types via pub use shim → set-equal
- I3: serde rename = camelCase + 多个 `_meta` / `inputSchema` / `mimeType` field rename byte-identical
- I6: 字段定义一字不改

## Verify Whitelist
- pub_added: `[]` (16 types via pub use shim)
- pub_removed: `[]`

## Done
- mcp_stdio/mod.rs ~610 LOC (763 - 156)
