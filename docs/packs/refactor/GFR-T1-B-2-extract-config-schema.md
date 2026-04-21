# GFR-T1-B-2: Extract schema simple types from `runtime/config/mod.rs`

## Status
- State: `done`

## Goal

T1-B 第 4 刀。把 `runtime/config/mod.rs` 中 5 个 schema 简单类型 + 5 个相关 parser 搬到新建 `runtime/config/schema.rs`。

## Source / Destination

- ResolvedPermissionMode (enum + impl) lines 38-42, 109-129
- BoundaryEnforceMode (enum + impl) lines 45-60
- ConfigEntry struct lines 132-135
- OAuthConfig struct lines 213-221
- parse_optional_permission_mode (823-841)
- parse_permission_mode_label (843-861)
- parse_boundary_enforce_mode_label (1040-1048)
- parse_filesystem_mode_label (1050-1058)
- parse_optional_oauth_config (1061-1084)

→ `runtime/config/schema.rs`

## Files (scope)
- src-tauri/src/modules/runtime/config/mod.rs
- src-tauri/src/modules/runtime/config/schema.rs

## Contract
- I1: 4 pub types via pub use shim (set-equal); 4 pub(super) parsers → whitelist
- I3: 13 字符串别名（permission/boundary/filesystem mode）byte-identical
- I6: function bodies byte-identical

## Verify Whitelist
- pub_added: `[fn parse_optional_permission_mode, fn parse_boundary_enforce_mode_label, fn parse_filesystem_mode_label, fn parse_optional_oauth_config]` (4 项 pub(super))
- pub_removed: `[]` (4 pub 类型走 shim)
- events / tests: `[]`
