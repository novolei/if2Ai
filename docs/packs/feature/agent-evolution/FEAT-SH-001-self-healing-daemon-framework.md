# FEAT-SH-001: Self-Healing Daemon Framework

## Status
- State: active
- Depends on: FEAT-EVO-000 (done)

## Goal
把 135 LOC 的 `runtime/self_repair.rs` 重构为通用 daemon 框架：健康检查注册表（HealthCheck registry）+ 四态状态机（DaemonState）+ 恢复编排器（RecoveryAction）。
迁移现有两项修复逻辑（memory ticker stuck-flag + broken-tool streak）成为首批 HealthCheck 注册项；保留 `spawn_self_repair_watchdog` + `record_tool_outcome` 公开 API 作 shim。

## Spec (verifiable)
- DaemonState 四种变体（Healthy/Degraded/Recovering/Failed）可按预期转换 → `tests::daemon::daemon_state_transitions`
- HealthCheckRegistry 支持 `register_check` 动态注入；轮询时调用 check_fn → `tests::daemon::health_check_registry`
- memory ticker stuck-flag 检查迁移为 HealthCheck，失败时执行 ClearStuckState → `tests::daemon::memory_ticker_health_check`
- broken-tool streak 检查迁移为 HealthCheck，失败时执行 ClearBrokenStreak → `tests::daemon::broken_tool_streak_health_check`
- RecoveryAction::ClearStuckState 幂等（重复调用结果相同）→ `tests::daemon::recovery_action_idempotent`
- `spawn_self_healing_daemon` 遵循 `IF2AI_SELF_REPAIR_WATCHDOG=0` 禁用开关 → `tests::daemon::daemon_spawn_respects_disabled_flag`

## Files (scope — write list)
- src-tauri/src/modules/runtime/daemon/mod.rs          (new — DaemonState, HealthCheck, RecoveryAction, spawn_self_healing_daemon)
- src-tauri/src/modules/runtime/daemon/health_check.rs (new — HealthCheckRegistry, HealthStatus)
- src-tauri/src/modules/runtime/daemon/recovery.rs     (new — RecoveryAction impl, attempt_recovery)
- src-tauri/src/modules/runtime/self_repair.rs         (modify → shim: pub use daemon::*; 保留旧 API re-export)
- src-tauri/src/modules/runtime/mod.rs                 (modify — pub mod daemon;)
- src-tauri/tests/evolution_phase0.rs                  (modify — 新增 daemon 模块测试)

## Reads (read-only)
- src-tauri/src/modules/runtime/self_repair.rs         (现有实现全文)
- src-tauri/src/modules/memory/mod.rs                  (MemoryTicker 接口)
- .qoder/specs/if2ai-agent-evolution-report.md §Module C (数据结构 + 关键接口设计)

## Contract (review must check)
- `record_tool_outcome` + `spawn_self_repair_watchdog` 签名不变（I1 / I5 shim）
- 不新增或修改任何 IPC 命令名 / 事件字面量（I3）
- 不修改任何持久化 key（I4）
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo fmt --check` + `cargo clippy -D warnings` 全通过（I7）

## Out of Scope
- ❌ 不实现 provider 心跳检查（→ FEAT-SH-002）
- ❌ 不实现 MCP 进程活性检查（→ FEAT-SH-002）
- ❌ 不实现浏览器会话健康检查（→ FEAT-SH-003）
- ❌ 不改 recoverability.rs / stream_outcome.rs / attempt_ledger.rs
- ❌ 不改任何前端文件

## Verify
- ./scripts/pack run FEAT-SH-001

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
