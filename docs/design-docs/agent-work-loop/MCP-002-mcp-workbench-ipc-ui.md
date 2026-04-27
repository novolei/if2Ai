# MCP-002 MCP Workbench IPC + UI Design

Status: draft
Owner: Staff Systems Architecture + Frontend UI/UX Design
Pack: `MCP-002`

## Problem

The stdio MCP manager can discover and call tools, and now has resource/prompt
capabilities. The product surface is still configuration-first: users can configure
servers, but cannot reliably inspect capabilities, test calls, read resources, view
prompts, or audit activity from one workbench.

This slice turns MCP into a local, Codex-like workbench for stdio servers only.

## Scope

Phase 1 supports:

- stdio server discovery,
- tools list and test call,
- resources list and read,
- prompts list and get,
- roots display when available,
- activity log,
- approval visibility.

Phase 1 does not implement:

- HTTP/SSE/WebSocket transports,
- OAuth,
- sampling,
- elicitation,
- remote marketplace installation.

Unsupported transports may be displayed as configured but inactive.

## Backend IPC

Add typed commands around `McpServerManager`:

- `mcp_workbench_list_servers`
- `mcp_workbench_discover`
- `mcp_workbench_call_tool`
- `mcp_workbench_list_resources`
- `mcp_workbench_read_resource`
- `mcp_workbench_list_prompts`
- `mcp_workbench_get_prompt`
- `mcp_workbench_activity`

Commands should return sanitized DTOs, not raw internal structs. Errors should carry
server id, operation, and a user-facing message.

## Activity Ledger

Each workbench operation appends an activity entry:

- timestamp,
- server id,
- operation kind,
- target name or URI,
- status,
- duration,
- sanitized params/result summary,
- error when present.

The ledger can be in-memory for this slice, but it must be exposed through a stable
DTO so persistence can be added later without UI rewrites.

## UI

Add `MCP Workbench` tabs:

- `Servers`: configured servers, transport, status, last error.
- `Tools`: tool schemas and a small JSON test runner.
- `Resources`: resource list and read result viewer.
- `Prompts`: prompt arguments and rendered prompt preview.
- `Approvals`: approval-sensitive operations and policy notes.
- `Activity`: chronological operation log.

The workbench belongs in Settings or Developer/Runtime tools, not the main chat first
screen. It should be dense, utilitarian, and optimized for repeated inspection.

## Safety

- Tool calls from the workbench still go through the same safety/approval pipeline as
  agent tool calls when the operation is mutating or risky.
- Resource and prompt reads should sanitize secrets before display.
- Failed server startup must not crash the settings page.
- Unsupported remote transports must be visibly disabled.

## Tests

- Mock MCP server covers list/call tool.
- Mock MCP server covers list/read resource.
- Mock MCP server covers list/get prompt.
- UI reducer renders success and failure activity entries.
- Settings build succeeds without a configured MCP server.
