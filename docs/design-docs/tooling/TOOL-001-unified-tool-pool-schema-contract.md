# TOOL-001 Unified Tool Pool & Schema Contract

## Problem
Tool visibility must be auditable per provider call. Without a canonical pool,
it is hard to explain whether the provider failed to call tools because tools
were hidden, filtered, or schema-incompatible.

## Design
Before spawning the streaming task, TurnService builds `CanonicalToolPool`:

- start from the registered `ToolRegistry` definitions,
- apply active skill attenuation,
- apply WorkLoop policy,
- sort by tool name,
- compute a stable schema hash from name, description, and JSON schema.

The streaming task logs the pool metadata in
`provider_tool_call_diagnostics`.

## Boundary
TOOL-001 does not create another tool registry. MCP tools must be registered
through the existing registry path so they automatically appear in this pool.
