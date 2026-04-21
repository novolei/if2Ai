# GFR-T1-B-3: Extract MCP cluster from `runtime/config/mod.rs`

## Status
- State: `done`
- Charter: [../CHARTER.md](../CHARTER.md)
- Predecessor: `GFR-T1-B-1` (done 2026-04-21)
- Last Updated: `2026-04-21`

## Goal

T1-B 第 2 刀（B-2 schema/permission 留后处理，先做最自包含的 MCP 集群）。把 `runtime/config/mod.rs` 中所有 MCP 相关类型 + impls + 4 个 file-private parsers 搬到新建 `runtime/config/mcp.rs`。`mod.rs` 保留 `pub use mcp::{...};` 全套 shim，外部 4 个 importer (`mcp_stdio.rs` / `mcp.rs` / `mcp_client.rs`) 零修改。

## Source

`src-tauri/src/modules/runtime/config/mod.rs`：

| 段 | 行号 | 内容 |
|---|---|---|
| 9 MCP 类型 | 440–509 | `McpConfigCollection` / `ScopedMcpServerConfig` / `McpTransport` / `McpServerConfig` / `McpStdioServerConfig` / `McpRemoteServerConfig` / `McpWebSocketServerConfig` / `McpSdkServerConfig` / `McpManagedProxyServerConfig` / `McpOAuthConfig` |
| 3 impl | 1033–1067 | `impl McpConfigCollection` / `impl ScopedMcpServerConfig` / `impl McpServerConfig` |
| `fn merge_mcp_servers` | 1101–1126 | 把 source 文件里的 mcpServers 合进 target map |
| `fn parse_mcp_server_config` | 1584–1619 | 6 transport 类型分发 |
| `fn parse_mcp_remote_server_config` | 1621–1631 | sse/http 公共字段 |
| `fn parse_optional_mcp_oauth_config` | 1633–1648 | oauth 子对象 |

## Destination

- 新建 `src-tauri/src/modules/runtime/config/mcp.rs`
- mod.rs 加 `mod mcp;` + `pub use mcp::{...全部 9 个 pub 类型}` 全套 shim（`pub use` 不被 RUST_PUB_RE 捕获）
- 4 个 parser (`merge_mcp_servers` + 3 个 `parse_mcp_*`) 升级为 `pub(super)` 以便 mod.rs 内调用方继续访问；这 4 项进 Verify Whitelist

## Files (scope)

- src-tauri/src/modules/runtime/config/mod.rs
- src-tauri/src/modules/runtime/config/mcp.rs

## Contract

- I1: 9 pub 类型在 scope 内 set-equal（通过 `pub use` shim）；4 file-private fn → pub(super) → 进 whitelist
- I3: `mcpServers` / 6 transport 字符串 (`stdio`/`sse`/`http`/`ws`/`sdk`/`claudeai-proxy`) byte-identical
- I6: function bodies byte-identical

## Out of Scope

- ❌ 不抽 schema enums（B-2）
- ❌ 不抽 memory cluster（B-4）
- ❌ 不抽 ConfigLoader / RuntimeConfig（B-5）
- ❌ 不动 `read_optional_json_object`（generic IO，留 mod.rs）
- ❌ 不改 6 transport 字符串
- ❌ 不动 25+ 个 `crate::modules::runtime::config::McpXxx` 调用方

## Execute Plan

1. 创建 `runtime/config/mcp.rs`，imports：
   ```rust
   use std::collections::BTreeMap;
   use std::path::Path;

   use super::super::json::JsonValue;
   use super::json_helpers::{
       expect_object, expect_string, optional_bool, optional_string, optional_string_array,
       optional_string_map, optional_u16,
   };
   use super::{ConfigError, ConfigSource};
   ```
   把 9 类型 + 3 impl + 4 fn 原样粘入。fn 加 `pub(super)`（升级 file-private → 跨文件可见）。
   如果 `pub(super) fn` 跨 sibling 不可见，将 json_helpers 项升级为 `pub(crate)`（regex 不区分两者，snapshot 不变）。
2. 在 mod.rs 顶部加 `mod mcp;`。
3. 在 mod.rs 删除 line 440–509 / 1033–1067 / 1101–1126 / 1584–1648（4 段），各留一行 `// X moved to mcp (GFR-T1-B-3).`
4. 在 mod.rs 加 `pub use mcp::{McpConfigCollection, McpManagedProxyServerConfig, McpOAuthConfig, McpRemoteServerConfig, McpSdkServerConfig, McpServerConfig, McpStdioServerConfig, McpTransport, McpWebSocketServerConfig, ScopedMcpServerConfig};`
5. mod.rs 内 `merge_mcp_servers(...)` / `parse_mcp_server_config(...)` 调用点改为 `mcp::merge_mcp_servers(...)` 等？— 不改：因为 mod.rs 顶部已 `mod mcp; use mcp::*;`（私有 glob）。但 4 fn 是 pub(super)，不会被 glob 自动带入。所以需 `use mcp::{merge_mcp_servers, parse_mcp_server_config};`（普通 use，无 pub）。
6. cargo build / fmt / clippy / test --no-run 全 PASS。

## Verify Whitelist

- pub_symbols added: `[fn merge_mcp_servers, fn parse_mcp_remote_server_config, fn parse_mcp_server_config, fn parse_optional_mcp_oauth_config]` (4 项 pub(super) parsers)
- pub_symbols removed: `[]` (9 pub 类型走 shim 保留)
- event_strings / test_names: unchanged

## Done Criteria

- pack run added pub_symbols ⊆ whitelist
- mod.rs ≈ 2160 LOC（2409 − 250）
- mcp.rs ≈ 260 LOC
- 1 commit `refactor(GFR-T1-B-3):`
