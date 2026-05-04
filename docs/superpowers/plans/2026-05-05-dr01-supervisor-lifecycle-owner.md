# DR-01 — Supervisor as the Sole Lifecycle Owner

> **来源**：[`docs/IMPROVEMENTS-2026-05-05.md`](../../IMPROVEMENTS-2026-05-05.md) §5 DR-01  
> **总览**：[`2026-05-05-improvements-wave1-overview.md`](2026-05-05-improvements-wave1-overview.md)  
> **预计**：1 周，2-3 PR  
> **依赖**：无；**阻塞** GF-02（work_loop 拆分会涉及 supervisor 调用点，必须先收口）

---

## 1. 问题

[`runtime/supervisor.rs`](../../../src-tauri/src/modules/runtime/supervisor.rs) (449 LOC, MIG-020 / T-006) 已经定义了完整的 `SupervisorSnapshot` + `SessionSupervisor` 状态机（Idle → Running → Blocked → RecoverableFailed/Completed/Closed/Idle），并提供 7 个 lifecycle hooks。

但**当前调用面严重不全**，supervisor 仅是 **snapshot 存储者**而非真正的 lifecycle owner：

| Hook | 应在何时调用 | 实际是否调用 | 证据 |
|------|--------------|--------------|------|
| `start_run` | turn 启动 | ✅ 流式 | [`stream.rs:269`](../../../src-tauri/src/modules/application/turn_service/stream.rs) |
| `start_run` | turn 启动（非流式） | ❌ **缺失** | [`run.rs`](../../../src-tauri/src/modules/application/turn_service/run.rs) 仅 emit `run_completed` event 字符串 |
| `block_permission` | permission 提示弹起 | ❌ **从未调用** | grep 全仓库无外部调用 |
| `unblock_permission` | permission 解决 | ❌ **从未调用** | grep 全仓库无外部调用 |
| `run_streaming` | 流式 ack | ❌ **从未调用** | 同上 |
| `run_completed` | turn 成功收尾 | ✅ 流式 | [`stream_finalize.rs:1237`](../../../src-tauri/src/modules/application/turn_service/stream_finalize.rs) |
| `run_completed` | turn 成功收尾（非流式） | ❌ **缺失** | run.rs 走自己的逻辑 |
| `run_failed_*` | turn 失败 | ✅ 流式 | [`stream_finalize.rs:1232-1234`](../../../src-tauri/src/modules/application/turn_service/stream_finalize.rs) |
| `run_failed_*` | turn 失败（非流式） | ❌ **缺失** | run.rs 走自己的逻辑 |
| `run_cancelled` | 用户停止 | ❌ **从未调用** | grep 全仓库无外部调用 |
| `close_session` | session 关闭 | ✅ | [`commands/session.rs:243`](../../../src-tauri/src/commands/session.rs) |

**直接后果**：
- `pending_permission_count` 字段永远为 0（即使有持久化的 pending）。
- `active_run_status` 在非流式 turn 中永不更新。
- 用户取消 turn 时 supervisor 显示仍 Running，前端 reload 后看到 stale 状态。
- 重试预算 `retry_budget_remaining` 字段始终是 hook 内部默认值，外部状态机不感知。
- `run_streaming` 永不被调用，`Idle` 状态下若 snapshot 文件丢失则永远停留在 Idle。

**散落的并行真相**（snapshot 应该统一映射的）：
- `permission_senders: Mutex<HashMap<String, Sender<PermissionPromptDecision>>>` — `permission_service` / `stream_iteration` / `stream_task` / `stream_tool_execution` / `stream.rs` 五处散布的 channel 表（permission 阻塞的 transient 真相）
- `pending_permission` 文件 — durable 真相
- `stream_cancel_senders` — 取消信号
- 前端 `runtimeProjectionStore.runs[].status` — UI 推断

## 2. 目标

让 `SessionSupervisor` 成为**唯一**写 supervisor snapshot 的入口；外部模块只能通过显式 hook 调用，且每次 hook 调用都：
1. 同步读 + mutate + 持久化（best-effort，失败仅 warn）
2. 自动 emit `runtime_event(Supervisor, …)` envelope（让前端 projection 感知）
3. 有 invariant 测试覆盖（snapshot ↔ pending_permission 文件 ↔ run-log 一致性）

## 3. 非目标

- 不重构 `permission_senders` 这套 channel 协议（那是另一个 P1 议题）。
- 不引入 actor / lock-free 数据结构（保持当前 file-based snapshot 模型）。
- 不删除 `run.rs`（非流式）— 仅补足其 supervisor 调用。
- 不动前端 projection（DR-05 处理 supervisor envelope 落地 reducer）。

## 4. 设计

### 4.1 收敛 API：`SupervisorOps` facade

新增 `runtime/supervisor.rs::SupervisorOps`（`SessionSupervisor` 之上的 thin facade），把「load → mutate → write → emit」封装为一行：

```rust
pub struct SupervisorOps<'a> {
    pub app_data_dir: &'a Path,
    pub session_id: &'a str,
    pub app_handle: Option<&'a AppHandle>,        // for runtime_event emit
    pub run_event_logger: Option<&'a RunEventLogger>,
}

impl<'a> SupervisorOps<'a> {
    pub fn start_run(&self, run_id: &str) {
        self.with_snapshot(|snap| {
            SessionSupervisor::start_run(snap, run_id.to_string());
        }, "start_run");
    }
    pub fn block_permission(&self, request_id: &str) { … }
    pub fn unblock_permission(&self, request_id: &str) { … }
    pub fn run_completed(&self) { … }
    pub fn run_failed_recoverable(&self, reason: &str) { … }
    pub fn run_failed_final(&self, reason: &str) { … }
    pub fn run_cancelled(&self) { … }
    pub fn close_session(&self) { … }

    fn with_snapshot<F: FnOnce(&mut SupervisorSnapshot)>(&self, f: F, family: &str) {
        let Ok(mut snap) = SessionSupervisor::load_or_create(self.app_data_dir, self.session_id) else { return; };
        f(&mut snap);
        if let Err(e) = write_supervisor_snapshot(self.app_data_dir, &snap) {
            tracing::warn!(error=%e, "[supervisor] persist failed");
        }
        if let Some(handle) = self.app_handle {
            // emit Supervisor envelope (family = start_run / blocked / unblocked / completed / failed / cancelled / closed)
            let _ = runtime_event::dispatch(
                handle,
                RuntimeEventType::Supervisor,
                family,
                self.correlation(snap.active_run_id.as_deref()),
                &snap,
                self.run_event_logger,
            );
        }
    }
}
```

新增 `RuntimeEventType::Supervisor` family（在 `contracts/common.rs` + `src/transport/contracts.ts`）。

### 4.2 收口调用点（PR 拆分）

#### PR-DR01-A — Facade + emit + invariant test

- 新增 `SupervisorOps` + 8 个公共方法。
- 新增 `RuntimeEventType::Supervisor` 与对应 family 字符串常量。
- 把 `stream.rs:268-279` / `stream_finalize.rs:1224-1247` / `commands/session.rs:237-251` 三处现有调用 **改写为 SupervisorOps 调用**（行为零变化，只是收敛入口）。
- 新增 invariant 测试：
  - `start_run` 后必须发 envelope；snapshot 与 envelope payload 一致。
  - `pending_permission_count` 与 `pending_permission` 文件中的记录数同步（构造 1/2/0 三种情况）。
  - 跨 reload：write → read → write 后 `last_updated_at` 单调递增。

#### PR-DR01-B — 补全缺失调用

- `run.rs`（非流式）start_run / run_completed / run_failed_*。
- `permission_service::request_permission`（弹起）→ `block_permission(request_id)`。
- `permission_service::resolve_permission`（解决）→ `unblock_permission(request_id)`。
- `stream_iteration` / `stream_task` 收到 cancel 信号 → `run_cancelled`。
- `stream_event_loop` 收到第一个 token → `run_streaming`（验证 active_run_status）。

每个改动配对一个最小测试（mock app_data_dir + 触发路径）。

#### PR-DR01-C（可选）— 移除散落 read

- 把前端 `useRuntimeProjectionSelector` 中所有「从 conversation-slice 推断 active run」的旁路读改为 `subscribeSupervisor`（待 DR-05 设计 reducer 切面时一并完成；本 PR 只标 TODO）。

### 4.3 不变量 / Invariant

任何时刻：

```
∀ session s:
  let snap = read_supervisor_snapshot(s);
  let pending_count = pending_permission_files(s).len();
  assert snap.pending_permission_count == pending_count

  if snap.active_run_id.is_some():
    assert snap.status ∈ { Running, Blocked }
  else:
    assert snap.status ∈ { Idle, Completed, RecoverableFailed, Closed }

  for each runtime_event(Supervisor, family, _, payload):
    assert payload.session_id == snap.session_id
    assert payload.last_updated_at >= prior.last_updated_at  // 单调
```

测试用例：
- `pending_permission_count_invariant_holds_after_block_unblock`
- `active_run_id_present_iff_status_is_running_or_blocked`
- `supervisor_envelope_monotonic_last_updated_at`

### 4.4 RuntimeEventType::Supervisor envelope schema

```rust
// contracts/common.rs
pub enum RuntimeEventType {
    Conversation,
    Tool,
    Permission,
    Memory,
    Activation,
    ExecutionMode,
    Supervisor,   // 新增
}

pub mod supervisor_family {
    pub const START_RUN: &str = "start_run";
    pub const BLOCKED: &str = "blocked";
    pub const UNBLOCKED: &str = "unblocked";
    pub const STREAMING: &str = "streaming";
    pub const COMPLETED: &str = "completed";
    pub const FAILED: &str = "failed";
    pub const CANCELLED: &str = "cancelled";
    pub const CLOSED: &str = "closed";
}
```

Payload = `SupervisorSnapshot` 本身（已 `Serialize` + `camelCase`）。

前端 `src/transport/contracts.ts` 镜像 `RuntimeEventType.Supervisor` 与 family 常量；本 PR 暂不接 reducer（DR-05 / 后续 PR 处理 UI 消费）。

## 5. TDD 策略

### 5.1 Pure-function 测试（PR-A）

`SupervisorOps::with_snapshot` 内部逻辑可以在不挂 Tauri AppHandle 的前提下测试：
- 用 `tempfile::tempdir()` 做 `app_data_dir`。
- `app_handle = None` 跳过 emit；只验证 snapshot 持久化。
- 单独测试 emit 路径用 mock `AppHandle`（已有 `runtime_event` 测试基础设施可参考）。

### 5.2 Invariant 测试（PR-A）

```rust
#[test]
fn pending_count_matches_pending_permission_files() {
    let dir = tempdir().unwrap();
    let ops = SupervisorOps { app_data_dir: dir.path(), session_id: "s1", app_handle: None, run_event_logger: None };

    ops.start_run("r1");
    assert_eq!(load(&dir, "s1").pending_permission_count, 0);

    write_pending_permission(dir.path(), &mock_record("req-1")).unwrap();
    ops.block_permission("req-1");
    assert_eq!(load(&dir, "s1").pending_permission_count, 1);

    clear_pending_permission(dir.path(), "s1", "req-1").unwrap();
    ops.unblock_permission("req-1");
    assert_eq!(load(&dir, "s1").pending_permission_count, 0);
}
```

### 5.3 集成验证（PR-B）

每个新接入点写一个 `#[tokio::test]`：
- 触发 hook 路径
- 读 snapshot
- 断言 status 与 `last_updated_at` 变化

## 6. 文件清单

### PR-DR01-A

| 文件 | 改动 |
|------|------|
| `src-tauri/src/modules/runtime/supervisor.rs` | 新增 `SupervisorOps` + `supervisor_family` 常量 + 5 个 invariant 测试 |
| `src-tauri/src/modules/runtime/contracts/common.rs` | `RuntimeEventType::Supervisor` |
| `src/transport/contracts.ts` | 镜像 `Supervisor` event type 与 family 常量 |
| `src-tauri/src/modules/application/turn_service/stream.rs` | 替换 supervisor 调用为 `SupervisorOps::start_run` |
| `src-tauri/src/modules/application/turn_service/stream_finalize.rs` | 替换 supervisor 调用为 `SupervisorOps::run_*` |
| `src-tauri/src/commands/session.rs` | 替换 supervisor 调用为 `SupervisorOps::close_session` |

### PR-DR01-B

| 文件 | 改动 |
|------|------|
| `src-tauri/src/modules/application/turn_service/run.rs` | 加 `SupervisorOps::start_run` / `run_completed` / `run_failed_*` |
| `src-tauri/src/modules/application/permission_service.rs` | 加 `block_permission` / `unblock_permission` 调用 |
| `src-tauri/src/modules/application/turn_service/stream_iteration.rs` 或 `stream_task.rs` | 加 `run_cancelled` 调用 |
| `src-tauri/src/modules/application/turn_service/stream_event_loop.rs` | 加 `run_streaming` 调用（首次 token 后） |

### PR-DR01-C（可选）

文档 + 前端 reducer 标 TODO，不动行为。

## 7. 验证

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml supervisor --lib
cargo test --manifest-path src-tauri/Cargo.toml turn_service --lib
cargo test --manifest-path src-tauri/Cargo.toml permission_service --lib
npm test
npm run build:web
```

手工 smoke：
1. 启动 turn → 触发权限 → 同时检查 `~/.if2ai/runtime/supervisor/<sid>.json` 显示 `status=blocked`，`pending_permission_count=1`。
2. 响应权限 → snapshot `status=running`，`pending_permission_count=0`。
3. 完成 turn → snapshot `status=completed`。
4. 用户取消 turn → snapshot `status=idle`，`active_run_status=cancelled`。
5. Reload → 前端 supervisor envelope 重放（projection 暂未消费，此步仅看后端日志确认 emit）。

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 加入 hook 后回放 / e2e 行为变化 | PR-A 仅替换调用入口，invariant 测试覆盖；PR-B 每个接入点单独验证 |
| `block_permission` 计数与 channel 散落不一致 | 用 `pending_permission` 文件作为权威源；invariant 测试强制对齐 |
| `pending_permission_count` 重启后回到 0 但实际有 pending 文件 | `load_or_create` 应在加载时校正字段（PR-A 中加固） |
| 前端在 DR-05 未上线前看不到 envelope | 暂存即可；envelope 已经写入 run-log 和 channel，可观测 |
| `SupervisorOps` 借用 `AppHandle` 引入 lifetime 依赖 | 用 `Option<&AppHandle>` 让 caller 自由选择是否 emit；测试中传 None |

## 9. PR 描述模板（PR-A）

```markdown
## 改善 ID
DR-01 (PR A — facade + emit + invariant tests)

## 变更摘要
新增 SupervisorOps facade 与 RuntimeEventType::Supervisor family。
将 stream.rs / stream_finalize.rs / commands/session.rs 三处现有 supervisor
调用收敛到 facade（行为零变化）。新增 5 个 invariant 测试验证
snapshot ↔ pending_permission 文件 ↔ envelope 单调性。

## Before / After
- supervisor 调用入口：3 处散落 `SessionSupervisor::*` + 手动 write
- → 1 处 `SupervisorOps::*`（自动 write + emit envelope）

## 验证
- [ ] cargo fmt --check + clippy -D warnings
- [ ] cargo test supervisor / turn_service / commands::session
- [ ] npm test + build:web
- [ ] 手工 smoke：turn 完成后 ~/.if2ai/runtime/supervisor/<sid>.json 仍正确

## 关联
- Plan: docs/superpowers/plans/2026-05-05-dr01-supervisor-lifecycle-owner.md
- Blocks: GF-02 (work_loop 拆分前必须收口 supervisor 调用)
```
