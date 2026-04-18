---
name: rust-async-patterns
description: Tokio/async 要点与 If2Ai（Tauri 2）相关模式速查。完整 JoinSet、通道、流、shutdown 示例见 references/patterns-full.md。在实现或调试 async Rust、并发、取消与背压时使用；简单同步逻辑不必读本 skill。
---

# Rust Async Patterns（精简）

> **完整代码示例**（Pattern 1–7、调试片段）→ [references/patterns-full.md](references/patterns-full.md)。仅在需要抄/改复杂并发时再打开，避免默认加载 ~400 行示例。

## 何时读完整 reference

- `JoinSet` / `buffer_unordered` 并发上限、`select!` 竞态
- `mpsc` / `broadcast` / `oneshot` / `watch` 选型
- `CancellationToken` + graceful shutdown
- `async_trait`、Stream、`Semaphore` 池化
- tokio-console / `#[instrument]` 调试

## If2Ai 速查（与 `rust.mdc` 一致）

| 场景 | 做法 |
|------|------|
| Tauri 命令内阻塞 IO | `spawn_blocking`；禁止 `block_on` |
| 跨 await 持锁 | 用 `tokio::sync::Mutex` / `RwLock`，禁止 `std::sync::Mutex` 跨 await |
| 取消长任务 | `tokio_util::sync::CancellationToken` + `select!` |
| 并发上限 | `Semaphore` 或 `futures::stream::buffer_unordered(n)` |
| 错误 | 边界 `thiserror`；`tracing` 记录，禁止裸 `println!` |

## 心智模型（保留）

```
Future (lazy) → poll() → Ready(value) | Pending
                ↑           ↓
              Waker ← Runtime schedules
```

| 概念 | 作用 |
|------|------|
| `Future` | 惰性、可能稍后完成 |
| `async fn` | 返回 `impl Future` |
| `await` | 挂起直到完成 |
| `Task` | `spawn` 出的并发单元 |
| `Runtime` | 轮询 executor |

## Do / Don't（无长代码）

**Do：** `tokio::select!` 竞态；优先 channel；`JoinSet` 管多任务；`tracing` 打点；取消用 `CancellationToken`。

**Don't：** `std::thread::sleep`；锁跨 await；无界 `spawn`；吞错误；`spawn` 的 future 缺 `Send`。

## Quick deps（备忘）

```toml
tokio = { version = "1", features = ["full"] }
futures = "0.3"
async-trait = "0.1"
tracing = "0.1"
tracing-subscriber = "0.3"
tokio-util = { version = "0.7", features = ["rt"] }
```

更多可复制粘贴的代码块见 [references/patterns-full.md](references/patterns-full.md)。
