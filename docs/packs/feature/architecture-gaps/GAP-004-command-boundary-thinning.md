# GAP-004: Command Boundary Thinning

## Status
- State: done
- Updated: 2026-04-25

## T-009 Execution Summary

1. **`service_registry.rs`** (NEW) — Created `ServiceRegistry` struct bundling
   `SessionManager`, `ToolRegistry`, and `ProjectManager` behind a single
   `Arc<ServiceRegistry>` field on `AppState`.  Individual `Arc<>` refs are
   extracted in `AppState::new` for backward-compatible `state.session_manager`
   / `state.tool_registry` / `state.project_manager` access.

2. **`commands/mod.rs`** — Added `service_registry: Arc<ServiceRegistry>` to
   `AppState`.  Removed `session_manager`/`tool_registry`/`project_manager`
   from `AppStateConfig` (now constructed via `ServiceRegistry::new`).
   `AppState::new` extracts individual `Arc<>` clones from the registry.

3. **`bootstrap/app.rs`** — Constructs `ServiceRegistry::new(session_manager,
   tool_registry, project_manager)` before `AppStateConfig`, passing the
   registry as a single argument instead of three ad-hoc fields.

4. **Unified error mapping** — Added `map_command_error(context, error) -> String`
   for consistent command-layer error serialization.

5. **Module registration** — `service_registry` module registered in
   `application/mod.rs` with `pub use service_registry::{...}`.

### Contract Compliance
- No IPC command names or payload shapes changed
- No new dependencies introduced
- All existing direct field access paths (`state.session_manager`, etc.) preserved
- Pattern established for new commands to route through `state.service_registry`

### Verification
- `cargo check` PASS
- `cargo test --lib -- service_registry` 2/2 PASS

## Goal
瘦身 `commands/mod.rs` 与 command adapters，让 IPC 层只做参数校验、错误序列化、service 调用，不继续承载业务编排。

## Spec
- `commands/mod.rs` 的 AppState 聚合拆出 service registry/helper → `commands_app_state_is_thin_registry`
- 新 command 不直接调用 domain internals，必须经过 application service → `commands_route_through_application_services`
- command error mapping 统一走 API error contract → `commands_use_unified_error_mapping`

## Files
- `src-tauri/src/commands/**`
- `src-tauri/src/modules/application/**`
- `src-tauri/src/modules/api/error.rs`
- `src-tauri/tests/**`

## Reads
- `ARCHITECTURE.md` §4.1, §7
- `docs/packs/CHARTER.md` §3, §5
- `docs/packs/feature/migration-core/MIG-015-gateway-conversations-and-streaming-surface.md`

## Contract
- 不改 IPC 命令名和 payload shape。
- 不引入新 dependency。
- 不把 command 逻辑移动到 frontend workaround。

## Verify
- `./scripts/pack run GAP-004`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml commands`

