# GAP-004: Command Boundary Thinning

## Status
- State: draft

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

