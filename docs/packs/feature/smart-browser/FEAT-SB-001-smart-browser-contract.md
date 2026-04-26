# FEAT-SB-001: Smart Browser Contract

## Status

- State: `draft`
- Owner: `@executor`
- Depends On: none
- Last Updated: `2026-04-25`

## Goal
Create the Smart Browser contract layer so If2Ai can route browser work through multiple backends without creating multiple UI/runtime truth sources.

## Spec
- Smart Browser DTOs model backend, command, observation, risk, and lifecycle events → 测试 `modules::smart_browser::contract::tests::serializes_core_events`
- Local Rust browser can be wrapped as a backend capability without changing existing `browser` tool semantics → 测试 `modules::smart_browser::local_adapter::tests::maps_browser_actions_without_renaming`
- Backend policy defaults to `local_rust_cdp` and rejects unknown backend labels → 测试 `modules::smart_browser::policy::tests::defaults_and_rejects_unknown_backend`

## Files (scope)
- `src-tauri/src/modules/smart_browser/mod.rs` (new)
- `src-tauri/src/modules/smart_browser/contract.rs` (new)
- `src-tauri/src/modules/smart_browser/policy.rs` (new)
- `src-tauri/src/modules/smart_browser/local_adapter.rs` (new)
- `src-tauri/src/modules/mod.rs`
- `src-tauri/src/modules/tools/builtin/browser_tool.rs`

## Reads
- `docs/design-docs/smart-browser-architecture.md`
- `src-tauri/src/modules/tools/builtin/browser_tool.rs`
- `src-tauri/src/modules/browser/registry.rs`
- `src-tauri/src/modules/browser/session.rs`

## Contract
- 不删除或重命名现有 `browser` tool
- 不改现有 Tauri browser IPC 命令名或 `"browser-status"` 事件名
- 不引入新 dependency
- 不接入 browser-use MCP，本 pack 只建 contract

## Out Of Scope
- ❌ 不改 BrowserCard UI
- ❌ 不改 runtime projection
- ❌ 不新增 cloud/browser-use 配置

## Verify
- ./scripts/pack run FEAT-SB-001
- cargo test --manifest-path src-tauri/Cargo.toml smart_browser

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
