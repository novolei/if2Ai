# WU-002: Daemon 探针注册（SH-002/003 Wire-up）

## Status
- State: active

## Goal
在 `desktop_host/setup.rs` 的 `spawn_self_repair_watchdog` 调用处，把 SH-002 的 `provider_heartbeat_check_fn` + `mcp_liveness_check_fn` 和 SH-003 的 `BrowserSessionLivenessCheck` 注册到 `default_registry`，使三个探针真正参与 daemon 健康轮询。任一探针错误时降级为 `Healthy`（不中断 daemon）并通过 `emit_evolution_event(DaemonHealth, …)` 向前端广播。

## Spec (verifiable)
- `default_registry` 含 provider + mcp + browser 共 3 个探针 → `tests::daemon_probe::default_registry_has_three_probes`
- 任意探针 `Err` 时 daemon 仍运行（降级为 Healthy）→ `tests::daemon_probe::probe_error_degrades_gracefully`
- 探针 Unhealthy 结果触发 `DaemonHealth` envelope 广播 → `tests::daemon_probe::unhealthy_probe_emits_daemon_health_event`
- `IF2AI_DISABLE_DAEMON_PROBES=1` 时三个探针全不注册 → `tests::daemon_probe::env_flag_disables_probe_registration`

## Files (scope — write list)
- `src-tauri/src/modules/desktop_host/setup.rs`           (modify — 注册 3 探针 + emit)
- `src-tauri/src/modules/runtime/daemon/mod.rs`           (modify — default_registry 接受 probe list)
- `src-tauri/tests/daemon_probe.rs`                       (new — 4 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/runtime/daemon/health_check.rs`  (HealthCheck trait)
- `src-tauri/src/modules/runtime/daemon/recovery.rs`      (DaemonState)
- `src-tauri/src/modules/smart_browser/session_health.rs` (BrowserSessionLivenessCheck)
- `src-tauri/src/modules/api/resilience.rs`               (provider_heartbeat_check_fn)
- `src-tauri/src/modules/runtime/mcp_stdio/mod.rs`        (mcp_liveness_check_fn)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`    (emit_evolution_event — WU-001)

## Contract (review must check)
- 不改 `spawn_self_repair_watchdog` 函数签名（I1）
- 不新增 IPC 命令名（I3）
- 探针错误只影响事件广播，不影响 daemon 主循环（failure isolation）
- `IF2AI_DISABLE_DAEMON_PROBES=1` 环境变量可完全跳过探针注册

## Out of Scope
- ❌ 不修改 SH-001/002/003 probe 内部逻辑
- ❌ 不改 daemon 轮询间隔
- ❌ 不实现新的 recovery action
- ❌ 不接入 AE / SE / TE 类模块

## Depends on
- WU-001（emit_evolution_event 辅助函数）
- FEAT-SH-001/002/003（probe 实现 done）

## Verify
- `./scripts/pack run WU-002`
- `cargo test --test daemon_probe`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
