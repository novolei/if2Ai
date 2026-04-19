# If2Ai Worker Adoption Design

> 参考 UClaw 的 worker 能力层设计，为 If2Ai 定义“该引入哪种 worker、何时引入、哪些工具先接入”的正式迁移方案。
>
> 最后更新: 2026-04-20

## 1. 文档目标

本设计解决三个容易混淆的问题：

1. UClaw 的 `worker` 到底是在解决什么问题。
2. If2Ai 是否应该引入 `worker`。
3. If2Ai 应该引入哪一层 `worker`，又不应该过早引入哪一层。

本设计的核心结论是：

> If2Ai 应引入的是“统一工具执行 worker contract + registry + policy-preflight”这一层，
> 而不是立即引入 first-class worker process / background worker runtime。

## 2. UClaw Worker 的两层含义

必须先区分 UClaw 中两种不同的 worker 叙事。

### 2.1 工具执行 worker 层

这是当前最成熟、最值得借鉴的一层。

核心证据：

- 统一契约：[workers/contract.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/workers/contract.rs:1)
- capability / risk / scope：[workers/capability.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/workers/capability.rs:1)
- worker registry：[workers/registry.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/workers/registry.rs:1)
- policy 前置决策：[policy/coordinator.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/policy/coordinator.rs:45)
- chat 主路径接入：[chat_ws_stream.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/api/routes/chat_ws_stream.rs:3227)

这一层的核心思想是：

1. 每个工具执行后端都实现统一 `Worker` trait。
2. 上下文通过 `WorkerExecutionInput` 注入，worker 自己不能猜环境。
3. capability、risk、permission、sandbox 在执行前统一结算。
4. registry 保证“暴露出来的工具”和“真正可执行后端”一致。
5. output / failure / event emitter 形状统一，便于审计和治理。

### 2.2 独立 worker process / task worker 层

这是更重的一层，当前不应作为 If2Ai 的首要迁移目标。

核心证据：

- [agent_runtime/worker.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/agent_runtime/worker.rs:1)
- [agent_runtime/task_worker.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/agent_runtime/task_worker.rs:1)

但这里要注意：

- `agent_runtime/worker.rs` 文件头已经写明 `STATUS: dormant / not-on-authoritative-path`

因此，这层更像是“未来的一等执行单元设计叙事”，不是当前最权威、最值得优先复制的主路径结构。

## 3. If2Ai 当前问题

If2Ai 当前已有：

- `ToolRegistry`
- builtin tools
- `ToolExecutionBroker`
- permission / boundary / context 相关能力

但还没有形成下面这套统一真相：

1. 一个稳定的 `tool execution backend contract`
2. capability / risk / scope 对齐到执行后端
3. `prepare_step_execution` 与工具执行后端的真实接线
4. tool catalog 暴露面与可执行后端的一致性审计
5. worker 级别的统一 failure / audit / harness trace 语义

## 4. Staff 级判断

### 4.1 应不应该引入 worker

应该。

但引入的不是“多智能体 worker runtime”，而是：

> 统一工具执行后端抽象

### 4.2 为什么有必要

因为 If2Ai 后续至少有四条链都需要一个统一执行对象：

1. control plane
2. harness governance
3. execution-mode / route hint 落地
4. self-evolution 对工具策略的 compare

### 4.3 为什么现在不该引入重 worker runtime

因为 If2Ai 当前更大的问题还在主骨架：

1. `commands/agent.rs` 还过胖
2. `tauri.ts` / `App.tsx` / `chat-ui.tsx` 还没收口
3. runtime projection 还没站稳
4. memory coordinator 还没统一

## 5. If2Ai 应引入哪一种 worker

建议引入：

### 5.1 Phase 1 Worker

即：

> Tool Execution Worker Layer

包含四个最小组成：

1. `Worker` trait
2. `WorkerExecutionInput / Output / Failure`
3. `WorkerRegistry`
4. `prepare_step_execution -> worker execution` 接缝

### 5.2 Phase 2 Worker

只有在 `M1 ~ M4` 稳定后，才评估是否需要扩展为：

- long-running worker
- cancellable background worker
- resumable worker task
- first-class worker UI lifecycle

## 6. 推荐的 If2Ai Worker 分层

建议 If2Ai 把 worker 放在：

- `modules/workers/`

最小文件结构建议：

- `modules/workers/mod.rs`
- `modules/workers/contract.rs`
- `modules/workers/capability.rs`
- `modules/workers/registry.rs`
- `modules/workers/emitter.rs`
- `modules/workers/adapters/`

## 7. 哪些工具先接入

### 7.1 第一批建议接入

1. `bash`
2. `file_read`
3. `file_write` / `file_edit`
4. `glob_search` / `grep_search`
5. `web_fetch` 或等价网络只读工具

### 7.2 第二批建议接入

1. browser worker
2. skills / skill search 相关 worker
3. memory tool workers

### 7.3 暂缓接入

1. activation lifecycle
2. onboarding flow
3. harness internal jobs

## 8. 何时引入

### 8.1 M1 引入 skeleton

`M1` 只做：

1. `Worker` 基础契约
2. `WorkerRegistry` skeleton
3. `prepare_step_execution` 与 worker 之间的正式接缝
4. 第一批 2~3 个 worker 的骨架接入

### 8.2 M4 接入治理链

`M4` 再做：

1. worker trace 进入 harness
2. worker failure typed data 进入 run report
3. candidate policy 通过 worker compare 评估

### 8.3 M5 之后再评估重 worker runtime

只有当 If2Ai 已明确需要长任务后台执行、跨 turn 可恢复任务或并发任务执行可视化时，再考虑更重的 worker runtime。

## 9. 与现有 If2Ai 结构的关系

Worker 不替代：

1. `ToolRegistry`
2. `application/turn_service`
3. `request_intelligence_service`
4. `memory coordinator`

它承担的是：

> tool execution backend abstraction

推荐关系如下：

`turn_service`
-> `request_intelligence_service`
-> `prepare_step_execution`
-> `worker_registry.resolve(tool_name)`
-> `worker.execute(input, emitter)`
-> `stream_emitter / harness_trace / audit`

## 10. 与 M1 / M4 的映射

### M1

新增或明确：

1. `modules/workers/contract.rs`
2. `modules/workers/capability.rs`
3. `modules/workers/registry.rs`
4. `prepare_step_execution` 的 worker-facing 接口

### M4

新增或明确：

1. worker trace fields
2. worker failure taxonomy
3. worker compare visibility
4. worker policy blocker rules

## 11. 验收口径

Worker adoption 的完成，不以“worker 文件数量”衡量，而以这 5 件事衡量：

1. 至少一批高价值工具不再走 ad-hoc backend
2. `prepare_step_execution` 真正前置到工具执行前
3. worker failure 是 typed 的
4. harness 能看见 worker 级 evidence
5. 文档里不再把 worker 混写成 agent/sub-agent/background task

## 12. 结论

1. 引入 worker，但只引入“统一工具执行后端抽象”
2. 先不要引入重型 worker runtime
3. worker adoption 以 `M1 skeleton -> M4 governance -> M5+ evaluate` 的顺序推进
