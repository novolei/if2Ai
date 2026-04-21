# GFR-T1-C-1: mcp_stdio 目录化 + tests 抽出 + JSON-RPC 抽出

## Status
- State: `done`

## Goal

T1-C 第一刀。三件套合并：(1) `git mv runtime/mcp_stdio.rs → runtime/mcp_stdio/mod.rs` 完成目录化；(2) `#[cfg(test)] mod tests` block (~916 LOC) 抽到 sibling `tests.rs`；(3) JSON-RPC 2.0 framing 4 个类型抽到 `rpc.rs`。一刀解决最大杠杆，mod.rs 跌至 ~760 LOC（< 800 hard limit）。

## Source / Destination

- `runtime/mcp_stdio.rs` → `runtime/mcp_stdio/mod.rs` (git mv)
- mod.rs lines 17-62 (JsonRpcId / JsonRpcRequest / JsonRpcError / JsonRpcResponse) → `mcp_stdio/rpc.rs`
- mod.rs lines 807-1725 (`#[cfg(test)] mod tests { ... }`) → `mcp_stdio/tests.rs`

## Files (scope)
- src-tauri/src/modules/runtime/mcp_stdio.rs (deleted via mv)
- src-tauri/src/modules/runtime/mcp_stdio/mod.rs (new dest)
- src-tauri/src/modules/runtime/mcp_stdio/rpc.rs (new)
- src-tauri/src/modules/runtime/mcp_stdio/tests.rs (new)

## Contract
- I1: 4 RPC pub types via `pub use rpc::{...}` shim → set-equal
- I2: 23 test names byte-identical (跟随移动)
- I6: 函数体一字不改

## Verify Whitelist
- pub_added: `[mod rpc]` (mod.rs 顶部 `mod rpc;` 是私有 mod，不被 regex 捕获 → 白名单为空) — 实际预期 `pub_added=[]`
- pub_removed: `[]`
- events / tests: `[]`

## Done
- mcp_stdio/mod.rs ~760 LOC (从 SIZE_EXEMPT 改为新路径)
