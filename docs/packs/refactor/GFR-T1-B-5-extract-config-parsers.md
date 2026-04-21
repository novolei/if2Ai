# GFR-T1-B-5: Extract remaining parsers from `runtime/config/mod.rs`

## Status
- State: `done`

## Goal

T1-B 收尾刀。把 mod.rs 仅剩的 7 个 file-private parser fn + 1 个 `read_optional_json_object` IO helper 搬到新建 `runtime/config/parsers.rs`。预期 mod.rs 跌至 ~680 LOC（< 800 hard limit ✓，可摘掉 lint exempt）。

ConfigLoader struct + impl 留在 mod.rs，因为它和 RuntimeConfig 强耦合，分离收益有限。

## Source

- `read_optional_json_object` (687-715) — generic JSON object IO
- `parse_optional_model` (720-725)
- `parse_optional_hooks_config` (727-741)
- `parse_optional_plugin_config` (743-771)
- `parse_optional_sandbox_config` (775-797)
- `parse_optional_control_plane_config` (800-877)
- `validate_provider_transport_config` (880-946)

## Destination

- 新建 `runtime/config/parsers.rs`
- mod.rs `mod parsers;` + 7 个 `use parsers::{...}` (private re-exports for ConfigLoader)
- 受影响 4 个 struct 字段升级为 `pub(super)` (regex 不捕获字段，snapshot 不变):
  - RuntimeHookConfig (2 fields)
  - RuntimePluginConfig (5 fields)
  - ControlPlaneGovernanceConfig (4 fields)
  - ProviderTransportConfig (6 fields)

## Files (scope)
- src-tauri/src/modules/runtime/config/mod.rs
- src-tauri/src/modules/runtime/config/parsers.rs

## Contract
- I1: 0 pub_symbols change for types (字段可见性升级不被捕获); 7 file-private fn → pub(super) → whitelist 7
- I3: settings.json 字段名 + ConfigError 错误模板 byte-identical
- I3: validate 上下限常量 (MAX_PROVIDER_TIMEOUT_MS=600_000 等) byte-identical
- I6: function bodies byte-identical

## Verify Whitelist
- pub_added: `[fn parse_optional_control_plane_config, fn parse_optional_hooks_config, fn parse_optional_model, fn parse_optional_plugin_config, fn parse_optional_sandbox_config, fn read_optional_json_object, fn validate_provider_transport_config]` (7 items)
- pub_removed: `[]`
- events / tests: `[]`

## Done
- mod.rs ~680 LOC，从 SIZE_EXEMPT_PATHS 移除（lint 自然 PASS）
- T1-B 收尾，mod.rs 累计 2640 → ~680 (-74%)
