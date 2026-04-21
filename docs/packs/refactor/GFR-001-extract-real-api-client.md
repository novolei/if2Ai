# GFR-001: Extract `RealApiClient` from `agent.rs`

## Status
- State: `active`
- Owner: `@executor`
- Charter: [../CHARTER.md](../CHARTER.md)
- Last Updated: `2026-04-21`

---

## Goal

把 `commands/agent.rs` 中 `RealApiClient` 这一段（约 150 行、自包含、无全局状态）整体迁到 `application/real_api_client.rs`，对应 Charter 中 **provider** bounded context。

这是 RFP Loop 的**首次试运行**，目标是验证流程，不是争取最大重构。

---

## Source

- `src-tauri/src/commands/agent.rs`，**仅以下三段**：
  - `struct RealApiClient { ... }`            （约 line 329–337）
  - `impl RealApiClient { fn new(...) }`      （约 line 339–353）
  - `impl ApiClient for RealApiClient { ... }` （约 line 355–408）
  - `impl RealApiClient { async fn call_api(...) }` （约 line 410–481）

> 行号以当前 `agent.rs` (LOC 4058) 为准；执行时若 ±5 行也按原样搬走整段。

---

## Destination

- 新建 `src-tauri/src/modules/application/real_api_client.rs`（≤ 200 行）。
- 在 `src-tauri/src/modules/application/mod.rs` 中加：
  ```rust
  pub mod real_api_client;
  pub use real_api_client::RealApiClient;
  ```
- 在 `src-tauri/src/commands/agent.rs` 中加 shim 注释 + 重新导入：
  ```rust
  // RealApiClient moved to crate::modules::application::real_api_client in GFR-001.
  use crate::modules::application::RealApiClient;
  ```
- 删除 `agent.rs` 中原本的 `struct RealApiClient` / `impl RealApiClient` / `impl ApiClient for RealApiClient` 三段定义。

---

## Files (scope)

- src-tauri/src/commands/agent.rs
- src-tauri/src/modules/application/mod.rs
- src-tauri/src/modules/application/real_api_client.rs

---

## Contract（Charter 不变量映射）

- **I1 pub 符号**：`RealApiClient` 之前是 *file-private* `struct`。本 RFP 把它提升为 `pub(crate)`（最小可见性，仅供 `commands::agent` 使用）。该提升是**唯一允许的可见性变化**，并写入 verify 白名单（详见 Out of Scope）。其余 fn / impl 全部按原样搬。
- **I2 测试**：不增不减 `agent.rs` 内现有测试。
- **I3 事件名**：`RealApiClient` 不调用 `app.emit` / `listen` / `invoke`，本 RFP 应当 0 影响。
- **I5 shim**：保留 `use crate::modules::application::RealApiClient;` 的 thin re-import，不修改 `start_agent_stream` (line 914) 的调用代码。
- **I6 函数体一行不改**：`call_api` / `stream` / `new` 的实现必须 byte-identical（除 `crate::modules::api::*` 等 use 路径需补齐）。

---

## Out of Scope

- ❌ 不抽 `ToolRegistryExecutor`（留给 RFP-002）。
- ❌ 不抽 `ControlPlaneRuntimeSwitches` / `load_control_plane_switches`（留给 RFP-002，与 ToolRegistryExecutor 共生）。
- ❌ 不调整 `RealApiClient::new` 签名（即使看起来可以加 builder）。
- ❌ 不改 `ApiClient` trait 本身。
- ❌ 不更新 M0.6 inventory 文档（RFP 流程不再依赖该文档）。
- ✅ 唯一允许的"非纯位移"变化：`struct RealApiClient` 由 file-private 提升为 `pub(crate)`。verify 时该符号会出现在 after 的 pub_symbols 集合而 before 没有；**修复方式**：执行前手工把 before snapshot 中的对应行加进去（见下面 Verify 节"已知白名单"）。

---

## Execute Plan（agent 必读）

1. 读 `agent.rs` 的 line 329–481，确认要搬的代码块边界（以 `// Bridge from async ToolRegistry` 之前结束为准）。
2. `mkdir` 不需要——`application/` 已存在。
3. 创建 `application/real_api_client.rs`，文件头加：
   ```rust
   //! RealApiClient — adapter that implements the runtime `ApiClient` trait
   //! over the outbound `ProviderClient`.
   //!
   //! Extracted from `commands/agent.rs` in GFR-001 (pure structural move).
   ```
4. 把 4 段 `struct` / `impl` 原样粘入。`use` 顶部按需补齐：
   ```rust
   use std::sync::Arc;
   use std::time::Duration;
   use tokio::time::timeout;

   use crate::modules::api::{
       InputContentBlock, InputMessage, MessageRequest, ProviderClient, ToolDefinition,
   };
   use crate::modules::runtime::conversation::{
       ApiClient, ApiRequest, AssistantEvent, RuntimeError,
   };
   ```
5. 把 `struct RealApiClient` 改为 `pub(crate) struct RealApiClient`，把 `fn new` 改为 `pub(crate) fn new`（其余字段 / 方法可见性保持）。
6. 在 `application/mod.rs` 加 `pub mod real_api_client;` + `pub use real_api_client::RealApiClient;`。
7. 在 `agent.rs` 中：
   - 删除原 4 段 `struct` / `impl`。
   - 在原位置留一行注释：`// RealApiClient moved to application::real_api_client (GFR-001).`
   - 顶部 `use` 列表添加 `use crate::modules::application::RealApiClient;` 即可（行 24-28 已有 `crate::modules::application::*` 多导入）。
8. `cargo build --manifest-path src-tauri/Cargo.toml` 必须通过。
9. `cargo fmt --manifest-path src-tauri/Cargo.toml --all` 后再次 build。

---

## Verify

```bash
# Step ① snapshot before（已由人在 RFP 启动前跑过；agent 不再跑）
# Step ② agent 执行 Execute Plan
# Step ③
./scripts/rfp verify GFR-001
```

**已知白名单**（verify FAIL 时人工核对，确实是允许的可见性变化才放行）：

- `pub_symbols` added（共 3 条，全部来自新建 destination 文件 / module 注册）：
  - `mod real_api_client`     （来自 `application/mod.rs` 新增的 `pub mod real_api_client;`）
  - `struct RealApiClient`    （来自 `real_api_client.rs`，对应 file-private → `pub(crate)`）
  - `fn new`                  （来自 `real_api_client.rs`，对应 inherent `pub(crate) fn new`）
- `pub_symbols` removed: 必须为 0。
- `event_strings` added/removed: 必须为 0。
- `test_names` removed: 必须为 0。

任何超出上面 3 条 added 的 pub 符号变化、任何 removed、任何 event_strings 变化 → **回滚**而不是"修一下"。

如果 verify 报告除上述白名单外的差异，**回滚**而不是"修一下"。

---

## Done Criteria

- `verify` 仅剩白名单允许的差异
- `cargo build` + `cargo fmt --check` + `cargo clippy -D warnings` 通过
- `agent.rs` LOC 下降 ≈ 150；`real_api_client.rs` LOC ≈ 150–170
- `start_agent_stream` 在 line ~914 处调用 `RealApiClient::new(...)` 仍编译通过
- 1 PR、1 commit、信息：
  ```
  refactor(GFR-001): extract RealApiClient to application::real_api_client

  Pure structural move per docs/refactor/CHARTER.md.
  Verified by ./scripts/rfp verify GFR-001.
  ```
- `docs/refactor/REGISTRY.md` 中 `GFR-001` 状态由 `active` 改为 `done`
