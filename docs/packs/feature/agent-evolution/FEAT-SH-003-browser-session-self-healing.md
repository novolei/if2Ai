# FEAT-SH-003: Browser Session Self-Healing

## Status
- State: pending
- Depends on: FEAT-SH-001

## Goal
新建 `smart_browser/session_health.rs`，实现 BrowserSessionHealth 状态机（5 种状态）+ BrowserRecoveryPlan（4 种动作）。接入 `smart_browser/runtime.rs` 的活跃会话监控，以及 `browser/session.rs` 的心跳+崩溃检测。注册为 FEAT-SH-001 daemon 的 HealthCheck。

## Spec (verifiable)
- BrowserHealthStatus 五种变体（Connected/Stale/Disconnected/Recovering/Crashed）状态转换正确 → `tests::browser_session_health::health_status_transitions`
- `check_session_health()` stale（心跳超时 > 30s）时返回 `HealthStatus::Critical` → `tests::browser_session_health::stale_session_detected`
- `build_recovery_plan()` stale → Reconnect，crashed → RestartProcess，连续 3 次 Reconnect 失败 → EscalateToCloud → `tests::browser_session_health::recovery_plan_escalation`
- `browser/session.rs` 新增 `last_heartbeat_at() -> Option<Instant>` 查询接口 → `tests::browser_session::heartbeat_query`
- 注册为 daemon HealthCheck 后，daemon 检测到 stale 并执行 ResetBrowserSession → `tests::daemon::browser_session_health_check_registered`

## Files (scope — write list)
- src-tauri/src/modules/smart_browser/session_health.rs (new — BrowserSessionHealth, BrowserHealthStatus, BrowserRecoveryPlan, BrowserRecoveryAction, check_session_health, build_recovery_plan)
- src-tauri/src/modules/smart_browser/mod.rs            (modify — pub mod session_health;)
- src-tauri/src/modules/smart_browser/runtime.rs        (modify — 接入 session_health 监控钩子)
- src-tauri/src/modules/browser/session.rs              (modify — 新增 last_heartbeat_at 查询 + crash flag)
- src-tauri/tests/evolution_phase0.rs                   (modify — 新增 browser session health 测试)

## Reads (read-only)
- src-tauri/src/modules/smart_browser/contract.rs       (SmartBrowserSessionId + trait 定义)
- src-tauri/src/modules/smart_browser/runtime.rs        (现有运行时管理逻辑)
- src-tauri/src/modules/smart_browser/policy.rs         (evaluate_cloud_escalation 策略接口)
- src-tauri/src/modules/browser/session.rs              (现有 session 结构)
- src-tauri/src/modules/runtime/daemon/mod.rs           (HealthCheck + RecoveryAction 接口)
- .qoder/specs/if2ai-agent-evolution-report.md §Module F (数据结构 + 集成点)

## Contract (review must check)
- `last_heartbeat_at` 为只读查询，不改变 session 状态（I6 精神）
- 不新增或修改 IPC 命令名 / 事件字面量（I3）
- 不修改 browser/session.rs 中已有 pub 函数的签名（I1）
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo fmt --check` + `cargo clippy -D warnings` 全通过（I7）

## Out of Scope
- ❌ 不实现坐标优先浏览器策略（→ FEAT-BR-002）
- ❌ 不改 browser 的 CDP 底层实现
- ❌ 不改任何前端文件
- ❌ 不改 smart_browser/cloud.rs / policy.rs 的云升级判断逻辑

## Verify
- ./scripts/pack run FEAT-SH-003

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
