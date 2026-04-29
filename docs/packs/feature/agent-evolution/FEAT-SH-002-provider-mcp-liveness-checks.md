# FEAT-SH-002: Provider + MCP Liveness Checks

## Status
- State: pending
- Depends on: FEAT-SH-001

## Goal
在 FEAT-SH-001 daemon 框架上注册两个新 HealthCheck：(1) provider 心跳——每 60s 向当前 LLM provider 发送轻量 ping，3 次连续失败触发熔断；(2) MCP stdio 进程活性——检查所有注册 MCP server 进程存活，死进程触发 RestartMcpServer 恢复动作。

## Spec (verifiable)
- `api/resilience.rs` 暴露 `provider_heartbeat_check_fn()` → 返回 `HealthCheckFn`，成功时 Healthy，HTTP 4xx/5xx/超时时 Critical → `tests::resilience::provider_heartbeat_returns_health_status`
- provider 3 连续失败后 daemon 触发 `RecoveryAction::DegradeGracefully` → `tests::daemon::provider_liveness_three_strike`
- `mcp_stdio/manager.rs` 暴露 `is_server_process_alive(server_name: &str) -> bool` → `tests::mcp_manager::server_process_alive_query`
- MCP 进程死亡时 daemon 触发 `RecoveryAction::RestartMcpServer` → `tests::daemon::mcp_liveness_restarts_dead_server`

## Files (scope — write list)
- src-tauri/src/modules/api/resilience.rs                    (new — provider_heartbeat_check_fn + ProviderCircuitState)
- src-tauri/src/modules/api/mod.rs                           (modify — pub mod resilience;)
- src-tauri/src/modules/runtime/mcp_stdio/manager.rs         (modify — 新增 pub fn is_server_process_alive)
- src-tauri/src/modules/runtime/daemon/mod.rs                (modify — 在启动时注册 provider + MCP 两个 HealthCheck)
- src-tauri/tests/evolution_phase0.rs                        (modify — 新增 resilience + mcp liveness 测试)

## Reads (read-only)
- src-tauri/src/modules/api/client.rs                        (现有 API client 结构)
- src-tauri/src/modules/api/providers/manager.rs             (ProviderManager 接口)
- src-tauri/src/modules/runtime/mcp_stdio/manager.rs         (McpServerManager 现有实现)
- src-tauri/src/modules/runtime/daemon/mod.rs                (HealthCheck trait + register_check 接口)
- .qoder/specs/if2ai-agent-evolution-report.md §Module C §"需要注册的健康检查"

## Contract (review must check)
- `is_server_process_alive` 为只读查询，不副作用 MCP 进程状态（I6 精神）
- 不新增或修改 IPC 命令名 / 事件字面量（I3）
- 不修改现有 provider API 的调用签名（I1）
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo fmt --check` + `cargo clippy -D warnings` 全通过（I7）

## Out of Scope
- ❌ 不实现浏览器会话健康检查（→ FEAT-SH-003）
- ❌ 不实现 MCP server 的重启逻辑（RecoveryAction 已在 FEAT-SH-001 中定义；此 pack 仅触发）
- ❌ 不修改 provider 配置加载逻辑
- ❌ 不改任何前端文件

## Verify
- ./scripts/pack run FEAT-SH-002

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
