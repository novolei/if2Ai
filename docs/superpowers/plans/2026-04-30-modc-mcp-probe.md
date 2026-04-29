# Plan: Module C — MCP daemon probe (truth-loop iter-8)

**Date**: 2026-04-30
**Driver**: continues `2026-04-30-dw002-dw004-modc.md` §1.2 deferred item.
**Hard rule**: **不创建第三个 `McpServerManager` 单例**——任何方案必须复用 `BROWSER_USE_MCP_MANAGER` 或纯读 `runtime::config::current()`，避免同名 server 双进程。

---

## 0. 探索结论复述（来自 explorer 报告）

| 事实 | 含义 |
|------|------|
| `HealthCheck::check(&self)` **同步** | 不能 `await`、不能 `tokio::Mutex::blocking_lock` |
| `is_server_process_alive(&mut self, ..)` 需 `&mut` | 不能直接持 `&Arc<Mutex<...>>` 在 sync probe 内调用 |
| 现存 stdio MCP 长生命周期 = `BROWSER_USE_MCP_MANAGER`（仅 `"browser-use"` 一个 server） | "per-server probe" 此时只需 1 个 |
| `mcp_workbench_manager()` per-call 构建 + shutdown | 用户配置的其他 stdio MCP **没有持久 child**——daemon 看不到 |
| `runtime::config::current().mcp().servers()` 是配置态 source-of-truth | 用于 config-presence 信号 + 列名 |

→ 唯一安全做法：**snapshot-cache + 后台异步刷新 + 同步 probe 读快照**。

---

## 1. 范围 / Non-goals

### 1.1 In-scope

| 步骤 | 文件 | 内容 |
|------|------|------|
| **S1** 新建 snapshot 模块 | `src-tauri/src/modules/runtime/mcp_health.rs` | `McpHealthSnapshot`（`OnceLock<Arc<Inner>>`），`Inner = { last_alive: parking_lot::RwLock<HashMap<String, LivenessSample>> }`，`LivenessSample = { alive: AtomicBool, last_checked: AtomicI64 (unix ms), stale_after_ms: u64 }` |
| **S2** 后台刷新任务 | `desktop_host/setup.rs` 新 `spawn_mcp_health_refresher()` | 每 30s `tokio::spawn` 一次：`browser_use_mcp_manager().lock().await` → 对所有已知 server name 调 `is_server_process_alive` → 写 snapshot |
| **S3** Config-presence probe | `src-tauri/src/modules/runtime/daemon/mod.rs` 新 `make_mcp_config_presence_probe()` | sync 读 `runtime::config::current().mcp().servers()`；`Healthy` if loaded（无论 0 还是多个 server）；`Degraded` if `current()` 返默认值（未初始化） |
| **S4** Per-server liveness probe（含 stale 守卫） | `mcp_health.rs` 暴露 `make_browser_use_liveness_probe()` | 复用现有 `McpServerLivenessCheck` 模式：closure 读 snapshot；样本 stale（> 90s 未刷新）→ 视为 Healthy（避免 daemon 还没 warm 起来就误报 Failed） |
| **S5** setup.rs extras 注册 | `desktop_host/setup.rs::spawn_self_healing_daemon_with_browser_probe` | 在 extras 加 `mcp_config_presence_probe` + `browser_use_liveness_probe` |

### 1.2 Non-goals

- 用户配置的非-browser-use MCP server 的 child 监控（需要先稳定一个全局 stdio manager；下一 PR）。
- 替换 `mcp_workbench_manager` 的 per-call 构建模型（churn 6 个 commands；本次不动）。
- 真正的 `RestartMcpServer` 实现（现有 `McpServerLivenessCheck` 的 recovery 仍是 marker `NoOpSkipped`，按 explorer 引用）。
- 把 snapshot-cache 暴露到前端/UI。

---

## 2. 设计

### 2.1 数据结构

```rust
// runtime/mcp_health.rs
pub struct LivenessSample {
    pub alive: AtomicBool,
    pub last_checked_unix_ms: AtomicI64,   // 0 = never checked
}

pub struct McpHealthSnapshot {
    samples: RwLock<HashMap<String, Arc<LivenessSample>>>,  // parking_lot::RwLock
    stale_after: Duration,                                  // default 90s
}

static GLOBAL_MCP_HEALTH: OnceLock<Arc<McpHealthSnapshot>> = OnceLock::new();
pub fn global_mcp_health() -> Arc<McpHealthSnapshot>;
```

### 2.2 三态语义（per-server probe）

| 条件 | HealthStatus |
|------|-------------|
| sample 不存在（refresher 还没跑过） | `Healthy`（"unknown == innocent until proven dead"） |
| sample 存在 + `last_checked` 在 `stale_after` 内 + alive=true | `Healthy` |
| sample 存在 + `last_checked` 在 `stale_after` 内 + alive=false | `Failed { reason }` → `RecoveryAction::RestartMcpServer { server_name }` |
| sample 存在 + 但 `last_checked` 已 stale | `Degraded { reason: "snapshot stale ..s ago" }` |

### 2.3 Config-presence probe 三态

| 条件 | HealthStatus |
|------|-------------|
| `runtime::config::CURRENT_CONFIG` 已 set | `Healthy`（带 `note: "{N} stdio MCP servers configured"`） |
| 走默认 fallback (warning case) | `Degraded { reason: "runtime config not initialised" }` |

判断「default fallback」：`runtime::config::current_is_default()` 不存在；新增一个返 `bool` 的 helper `runtime::config::is_initialised()` —— 唯一改动 `config/mod.rs`。

### 2.4 后台刷新任务

```rust
// desktop_host/setup.rs
fn spawn_mcp_health_refresher() {
    let interval = Duration::from_secs(30);
    let snapshot = global_mcp_health();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            let manager = browser_use_mcp_manager();
            // try_lock with short timeout to avoid blocking the only
            // browser-use stdio path
            let mut guard = match tokio::time::timeout(
                Duration::from_millis(100),
                manager.lock(),
            ).await {
                Ok(g) => g,
                Err(_) => { continue; }
            };
            for name in [BROWSER_USE_MCP_SERVER_NAME] {
                let alive = guard.is_server_process_alive(name);
                snapshot.record(name, alive);
            }
        }
    });
}
```

- **没有专用 runtime guard 必要**（已经在 setup 阶段，tokio runtime 必存在）。
- **kill switch**: `IF2AI_DISABLE_MCP_HEALTH=1` 跳过 spawn + register。
- **失败隔离**: panic catch、日志 warn、继续下一 tick（参考 `spawn_self_edit_scanner_interval` 模式）。

---

## 3. 测试矩阵

| 测试 | 覆盖 |
|------|------|
| `mcp_health::tests::sample_default_is_unknown_healthy` | snapshot empty → probe = Healthy |
| `mcp_health::tests::recent_alive_true_yields_healthy` | record(alive=true) → probe = Healthy |
| `mcp_health::tests::recent_alive_false_yields_failed_with_restart_recovery` | record(alive=false) → Failed + RestartMcpServer |
| `mcp_health::tests::stale_sample_yields_degraded` | 手动把 last_checked 调到 stale 之外 → Degraded |
| `daemon::tests::mcp_config_presence_probe_*` | uninitialised → Degraded; initialised → Healthy |

---

## 4. 提交策略

单 commit + push：`feat(mcp-daemon): truth-loop iter-8 — MCP snapshot-cache liveness probe + config presence`

---

## 5. Exit Gate

1. `cargo fmt --all` clean
2. `cargo clippy --all-targets -- -D warnings` clean
3. `cargo test --lib`、`cargo test --tests --test-threads=1` 全 PASS（容许已知 dk_lookup 并行 race，仍跑 serial）
4. 新增 5 个单测全部 PASS
5. **诚实声明**：probe 只覆盖 browser-use 子集，**不覆盖**用户在 `runtime config` 里配置的其他 stdio MCP——下一 iteration 才能解决。
