# Plan: iter-9 — Global user-MCP manager + workbench redirect

**Date**: 2026-04-30
**Driver**: extends iter-8 (`2026-04-30-modc-mcp-probe.md`) §1.2 deferred item.
**Hard rules**:
- **必须**在新 singleton 初始化 / reload 时排除 `BROWSER_USE_MCP_SERVER_NAME = "browser-use"`，避免与 `BROWSER_USE_MCP_MANAGER` 同名双 spawn。
- **必须**在 `set_mcp_service_config` 写盘后触发 `reload()`，保留「编辑 settings.json 立即生效」的现有行为契约。
- **必须**保留 `mcp_workbench_list_servers` 既有「读盘 → DTO」语义不变；只切 6 个真实使用 manager 的命令。
- **不允许**在本 PR 重构 `McpServerManager` 内部 trait / 引入新依赖（parking_lot 等）。

---

## 0. 探索结论复述

| 事实 | 影响 |
|------|------|
| 6 个 `mcp_workbench_*` 命令均 per-call `shutdown()` | 改 singleton 后必须**全部移除** `shutdown()`，否则 break 持久化目的 |
| `discover_tools` re-handshake but **进程不重生** if `process.is_some()` | singleton 大幅省 spawn 成本 |
| `RuntimeConfig::current()` 是 boot 一次性 OnceLock | 不能用作 reload 数据源；改用 `ConfigLoader::default_for(cwd).load()`（与今天 workbench 一致） |
| `BROWSER_USE_MCP_SERVER_NAME = "browser-use"` 仅 hard-coded，**默认 config 不含它**，但**用户可手动加**（无 schema 阻拦） | dedup filter 是防御性必需 |
| 无 `Drop` 自动 shutdown stdio child | App exit 必须显式 shutdown 一次，否则孤儿进程 |
| `cleanup_processes()` 仅 `pkill if2ai-backend`，不杀 MCP 子进程 | 必须新增 graceful shutdown hook（或接受退出泄露） |
| `set_mcp_service_config` 同步 `pub fn` | reload 必须 dispatch 到 tokio runtime |

---

## 1. 范围 / Non-goals

### 1.1 In-scope

| Step | 文件 | 内容 |
|------|------|------|
| **S1** 新模块 | `src-tauri/src/modules/runtime/mcp_workbench.rs` | `OnceLock<Arc<tokio::sync::Mutex<McpServerManager>>>` + `global_user_mcp_manager()` + `reload_user_mcp_manager_from_disk(cwd)` + `shutdown_user_mcp_manager()`，含 `build_user_servers(config)` 过滤 helper |
| **S2** 模块注册 | `runtime/mod.rs` | `pub mod mcp_workbench;` |
| **S3** Workbench 切换 | `commands/settings.rs` | 6 个 `mcp_workbench_*` 命令改用 `global_user_mcp_manager().lock().await`；移除 6 处 `let _ = manager.shutdown().await` |
| **S4** Reload trigger | `commands/settings.rs::set_mcp_service_config` | 写盘后 `tauri::async_runtime::spawn` 调 `reload_user_mcp_manager_from_disk` |
| **S5** Aggregate probe | `runtime/mcp_health.rs` | 新增 `make_user_mcp_aggregate_liveness_probe()` —— 读 snapshot 中所有非 `browser-use` 项，worst-wins，最差 Failed 项触发 `RestartMcpServer{name}` |
| **S6** Refresher 扩展 | `desktop_host/setup.rs` | 在现有 `spawn_mcp_health_refresher`（browser-use）之外，新增 `spawn_user_mcp_health_refresher`：每 30s 锁 user manager，对每个 server 调 `is_server_process_alive` 写快照 |
| **S7** Daemon extras | `desktop_host/setup.rs` | extras 加 aggregate probe（不为每 server 注册——动态名集合 + 静态注册不匹配，aggregate 模式更鲁棒） |
| **S8** App-exit shutdown | `desktop_host/setup.rs` tray Quit + `cleanup_processes` | tray Quit 前 `tauri::async_runtime::block_on(shutdown_user_mcp_manager())`（best-effort，超时 2s） |

### 1.2 Non-goals

- **不删除** `BROWSER_USE_MCP_MANAGER`（其 hard-coded args + 长生命周期约束已被 smart_browser 多处依赖）。
- 不为 user MCP servers **逐个注册** 静态 `McpServerLivenessCheck`（动态 server 集合用 aggregate 模式更合适）。
- 不实现真正的 `RestartMcpServer` recovery 执行（仍 SH-002 marker）。
- 不引入文件 watcher 自动 reload；reload 只由 `set_mcp_service_config` 触发。
- 不改 `mcp_workbench_list_servers`（继续读盘）。

---

## 2. 设计

### 2.1 单例 + reload

```rust
// runtime/mcp_workbench.rs
static GLOBAL_USER_MCP_MANAGER: OnceLock<Arc<tokio::sync::Mutex<McpServerManager>>> = OnceLock::new();

fn build_user_servers(
    config: &RuntimeConfig,
) -> BTreeMap<String, ScopedMcpServerConfig> {
    config.mcp().servers().iter()
        .filter(|(name, _)| name.as_str() != BROWSER_USE_MCP_SERVER_NAME)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn global_user_mcp_manager() -> Arc<tokio::sync::Mutex<McpServerManager>> {
    Arc::clone(GLOBAL_USER_MCP_MANAGER.get_or_init(|| {
        let initial = if config::is_initialised() {
            McpServerManager::from_servers(&build_user_servers(config::current()))
        } else {
            McpServerManager::from_servers(&BTreeMap::new())
        };
        Arc::new(tokio::sync::Mutex::new(initial))
    }))
}

pub async fn reload_user_mcp_manager_from_disk(cwd: &Path) -> Result<usize, String> {
    let config = ConfigLoader::default_for(cwd).load().map_err(|e| e.to_string())?;
    let new_servers = build_user_servers(&config);
    let count = new_servers.len();
    let arc = global_user_mcp_manager();
    let mut guard = arc.lock().await;
    // graceful: tear down old children before swap so we don't leak
    let _ = guard.shutdown().await;
    *guard = McpServerManager::from_servers(&new_servers);
    Ok(count)
}

pub async fn shutdown_user_mcp_manager() {
    if let Some(arc) = GLOBAL_USER_MCP_MANAGER.get() {
        let mut guard = arc.lock().await;
        let _ = guard.shutdown().await;
    }
}
```

### 2.2 Workbench command 改造模板

before:
```rust
let mut manager = mcp_workbench_manager()?;
let result = manager.foo().await.map_err(...);
let _ = manager.shutdown().await;
```

after:
```rust
let manager_arc = crate::modules::runtime::mcp_workbench::global_user_mcp_manager();
let mut manager = manager_arc.lock().await;
let result = manager.foo().await.map_err(...);
// no shutdown — singleton owns lifecycle
```

`mcp_workbench_manager()` 旧函数留下作为 deprecated alias（标 `#[deprecated]` + `#[allow(dead_code)]`，下个 PR 再删），避免本次 diff 过大。

`unsupported_servers` 在 `discover` 命令里仍要读：从 `manager.unsupported_servers()` 读即可（`&self`，无需可变锁——但既然已经持锁，直接读也无副作用）。

### 2.3 Reload trigger

```rust
// commands/settings.rs::set_mcp_service_config
pub fn set_mcp_service_config(request: McpServiceConfigInput) -> Result<McpServiceConfig, String> {
    write_user_mcp_services_file(&request)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    tauri::async_runtime::spawn(async move {
        match crate::modules::runtime::mcp_workbench::reload_user_mcp_manager_from_disk(&cwd).await {
            Ok(n) => tracing::info!("[mcp-workbench] reloaded {n} user MCP server(s) after settings write"),
            Err(e) => tracing::warn!("[mcp-workbench] reload after settings write failed: {e}"),
        }
    });
    get_mcp_service_config()
}
```

也对其它 mutate 命令（`add_user_mcp_service` 等，若有）做同样处理——iter 起步先确认是否还有别的 mutate 入口；本计划假设只有 `set_mcp_service_config`，实施时 grep 确认。

### 2.4 Aggregate liveness probe

```rust
// runtime/mcp_health.rs
pub fn make_user_mcp_aggregate_liveness_probe(snapshot: Arc<McpHealthSnapshot>) -> Arc<dyn HealthCheck> {
    // check():
    //   for (name, sample) in snapshot if name != "browser-use":
    //     if sample never refreshed → skip (innocent)
    //     if stale → degraded (worst stale)
    //     if !alive → failed (track first dead name)
    //   pick worst across all servers
    // recovery():
    //   if worst is Failed → RestartMcpServer{name}
    //   else None
}
```

读取 snapshot 需要遍历 `samples.read()`，所以 `McpHealthSnapshot` 需要新增 `entries() -> Vec<(String, Arc<LivenessSample>)>`（读锁 snapshot copy）。

### 2.5 Refresher 扩展

```rust
// desktop_host/setup.rs
fn spawn_user_mcp_health_refresher() {
    if env_disabled() { return; }
    const INTERVAL: Duration = Duration::from_secs(30);
    const LOCK_TIMEOUT: Duration = Duration::from_millis(100);
    let snapshot = global_mcp_health();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let arc = global_user_mcp_manager();
            let Ok(mut guard) = tokio::time::timeout(LOCK_TIMEOUT, arc.lock()).await else { continue };
            // names known only at lock time (config-driven, dynamic)
            let names: Vec<String> = guard.server_names().to_vec();
            for name in names {
                let alive = guard.is_server_process_alive(&name);
                snapshot.record(&name, alive);
            }
        }
    });
}
```

需新增 `McpServerManager::server_names() -> Vec<String>`（仅 `&self` 借用 `self.servers.keys().cloned().collect()`），最小可能的公共 API 扩展。

### 2.6 App-exit shutdown

```rust
// desktop_host/setup.rs tray Quit branch
HostTrayAction::Quit => {
    // best-effort: 2s budget
    let _ = tauri::async_runtime::block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            crate::modules::runtime::mcp_workbench::shutdown_user_mcp_manager(),
        ).await
    });
    cleanup_processes();
    app.exit(0);
}
```

---

## 3. 测试矩阵

### 3.1 新单测（`mcp_workbench` 模块内 `#[cfg(test)]`）

- `build_user_servers_filters_browser_use`：input 含 `"browser-use"` + 其它 → output 不含 `"browser-use"`，其它保留。
- `global_user_mcp_manager_lazy_init_when_config_uninitialised`：在 `cargo test --lib` 环境下（`is_initialised()=false`）第一次调用得空 manager，且第二次调用返回相同 `Arc`（单例语义）。
- `reload_swaps_inner_manager_in_place`：用 `tempdir` + 写一个最小 settings.json → `reload_user_mcp_manager_from_disk(tempdir)` → 锁内 `server_names()` 反映新内容。

### 3.2 新单测（`mcp_health` 扩展）

- `aggregate_user_probe_skips_browser_use`：snapshot 含 `"browser-use"`(dead) + `"foo"`(alive) → aggregate 报 Healthy（browser-use 由专用 probe 负责）。
- `aggregate_user_probe_worst_wins_failed`：snapshot 含 `"foo"`(alive) + `"bar"`(dead) → Failed + recovery RestartMcpServer{"bar"}。
- `aggregate_user_probe_stale_yields_degraded`。

### 3.3 不破坏的现有测试

- `mcp_workbench_discovers_capabilities` (`mcp_stdio/tests.rs:1092+`) 直接构造 manager，**不走** singleton 路径，无影响。
- `manager_shutdown_terminates_spawned_children_and_is_idempotent` 同上。
- 任何引用 `mcp_workbench_manager()` 的测试（应为 0，已确认） — 无需改。

---

## 4. Exit Gate（plan §5 类比 iter-8）

1. `cargo fmt --all` clean
2. `cargo clippy --all-targets -- -D warnings` clean
3. `cargo test --lib` PASS
4. `cargo test --tests --test-threads=1` PASS
5. 至少 6 个新单测全 PASS
6. **诚实声明（commit message）**：
   - dedup 仅过滤 `"browser-use"`；其他配置中的同名重复仍是用户自己的责任。
   - reload 只在 `set_mcp_service_config` 触发，**不监听文件系统**——直接编辑 settings.json + 不调 set 命令仍要重启。
   - aggregate probe 报告 Failed 时只把 **第一个** dead name 放入 `RestartMcpServer`，多 dead 时其余 server 等下一 tick 升 Failed。
   - 退出 shutdown 是 best-effort + 2s 超时；超时后让 tray Quit 继续，可能留 stdio 子进程被 OS reap。

---

## 5. 提交策略

单 commit + push：`feat(mcp-daemon): truth-loop iter-9 — global user-MCP manager + workbench redirect`
