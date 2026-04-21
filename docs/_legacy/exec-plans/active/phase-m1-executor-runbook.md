# Phase M1 Executor Runbook

> 将 `phase-m1-backend-service-extraction.yaml` 细化为 executor 可直接执行的后端拆分手册。
>
> 最后更新: 2026-04-20

## 1. 目的

`M1` 的任务不是“把大文件挪小一点”，而是把 If2Ai 后端从 command-first 结构拉回到：

`command adapter -> application service -> control plane / runtime / memory`

只要 `commands/agent.rs` 还是主脑，后续 `M2/M3/M4/M5` 都会继续围着错误边界打补丁。

## 2. 本阶段必须产出的结果

1. turn / provider / prompt / memory injection / stream / request intelligence / activation 都出现正式 service 边界。
2. `commands/agent.rs` 明显降责，不再负责主业务编排。
3. `prepare_step_execution` 的 control-plane 接缝出现。
4. 新 service 边界能被 `M2` 前端投影层直接消费，而不是继续绑定旧 command 细节。

## 3. 读取顺序

### 3.1 执行者最小必读

执行者开工 `M1` 时，只要求先读下面 5 个入口：

1. [phase-m-remediation-execution-index.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m-remediation-execution-index.md:1)
2. [phase-m1-backend-service-extraction.yaml](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-backend-service-extraction.yaml:1)
3. 本 runbook
4. 当前 slice 对应的 file-level plan
5. 当前 slice 明确引用的真实代码入口文件

### 3.2 `M1` file-level plan 入口

1. [phase-m1-initial-slices-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-initial-slices-file-level-plan.md:1)
2. [phase-m1-memory-and-stream-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-memory-and-stream-file-level-plan.md:1)
3. [phase-m1-routing-activation-control-plane-file-level-plan.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/exec-plans/active/phase-m1-routing-activation-control-plane-file-level-plan.md:1)

### 3.3 设计依据按需查阅

只有当当前 slice 需要核对边界或命名口径时，再回查以下设计文档：

1. [backend-application-control-plane-refactor-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/backend-application-control-plane-refactor-design.md:1)
2. [if2ai-worker-adoption-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/if2ai-worker-adoption-design.md:1)
3. [runtime-contracts-and-event-projection-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/runtime-contracts-and-event-projection-design.md:1)
4. [activation-gate-and-license-lifecycle-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/activation-gate-and-license-lifecycle-design.md:1)
5. [request-intelligence-and-execution-mode-routing-design.md](/Users/ryanliu/Documents/IfAI/if2Ai/docs/staff-remediation/request-intelligence-and-execution-mode-routing-design.md:1)

## 4. 落地决策

### 4.1 application 层以 `modules/application/` 落地

蓝图里使用了 `src-tauri/src/application/` 作为目标表达，但当前 crate 真实模块树是 [modules/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/mod.rs:1)。

因此 `M1` 必须在下面路径落地：

- `src-tauri/src/modules/application/mod.rs`
- `src-tauri/src/modules/application/turn_service.rs`
- `src-tauri/src/modules/application/provider_service.rs`
- `src-tauri/src/modules/application/prompt_planner.rs`
- `src-tauri/src/modules/application/memory_injection_service.rs`
- `src-tauri/src/modules/application/request_intelligence_service.rs`
- `src-tauri/src/modules/application/activation_service.rs`
- `src-tauri/src/modules/application/license_lifecycle_service.rs`

同时要在 [modules/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/mod.rs:1) 导出 `pub mod application;`。

### 4.2 control plane 继续使用现有 `modules/control_plane/`

不要再平行创建 `src-tauri/src/control_plane/` 新树。  
真实落点应该是：

- `src-tauri/src/modules/control_plane/prepare_step_execution.rs`
- 或等价拆分文件，但必须在现有 `modules/control_plane/` 下。

### 4.3 `commands/agent.rs` 只保留三类职责

`M1` 完成后 [commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1) 只应保留：

1. IPC 参数接收与校验
2. AppState/service resolve
3. service 调用与结果桥接

不允许继续保留 prompt assembling、provider resolving、memory injection、stream emitting 的主逻辑。

## 5. 不允许做的事

1. 不要在 `M1` 同时开启前端大改。
2. 不要把所有逻辑原封不动搬进“service”文件后就算完成。
3. 不要让 service 反向依赖具体 command。
4. 不要在没有 contract 的情况下私自发明新的 stream payload。
5. 不要让 activation 和 request intelligence 再次退回前端 heuristics。

## 6. 严格执行顺序

1. `m1.0` preflight backend inventory
2. `m1.1` turn service
3. `m1.2` provider service
4. `m1.3` prompt planning skeleton
5. `m1.4` memory injection service
6. `m1.5` stream emitter boundary
7. `m1.6` request intelligence service
8. `m1.7` activation + license lifecycle services
9. `m1.8` prepare_step_execution seam

## 7. Slice 详细执行说明

## 7.1 `m1.0` Preflight Backend Inventory

### 必查文件

- [src-tauri/src/commands/agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
- [src-tauri/src/commands/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/mod.rs:1)
- [src-tauri/src/modules/provider/service.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/provider/service.rs:1)
- [src-tauri/src/modules/runtime/prompt.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/prompt.rs:1)
- [src-tauri/src/modules/memory/inject.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/memory/inject.rs:1)
- [src-tauri/src/modules/control_plane/](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/mod.rs:1)

### 必做动作

1. 标出 `agent.rs` 中每段逻辑应迁往的目标模块。
2. 标出 AppState 当前承载了哪些 service-like 依赖。
3. 记录当前 stream emission 入口。

## 7.2 `m1.1` Turn Service

### 输出文件

- `src-tauri/src/modules/application/turn_service.rs`
- `src-tauri/src/modules/application/mod.rs`
- `src-tauri/src/commands/agent.rs`

### 必须落实

1. 建立 `TurnService` 或等价对象。
2. 暴露单一 `run_turn` / `execute_turn` 入口。
3. 把 command 层改成 adapter，只负责把请求交给 service。

### 完成定义

reviewer 打开 `agent.rs` 时，能一眼看出“主编排已不在这里”。

## 7.3 `m1.2` Provider Service

### 输出文件

- `src-tauri/src/modules/application/provider_service.rs`
- `src-tauri/src/commands/agent.rs`

### 必须落实

1. provider/model resolving 形成独立 service API。
2. fallback 语义保持不变。
3. `turn_service` 通过 service 调用，不再自己解析 provider。

### 验收关注点

不能破坏现有 provider transport policy。

## 7.4 `m1.3` Prompt Planning Skeleton

### 输出文件

- `src-tauri/src/modules/application/prompt_planner.rs`
- `src-tauri/src/modules/runtime/contracts/common.rs`

### 最低结构

1. `PromptPlan`
2. `PromptBlock`
3. `PromptContribution`

### 必须落实

1. 把当前 prompt assembling 从 command 层迁出。
2. 为 harness 记录 prompt composition 预留字段。

## 7.5 `m1.4` Memory Injection Service

### 输出文件

- `src-tauri/src/modules/application/memory_injection_service.rs`
- `src-tauri/src/modules/memory/inject.rs`

### 必须落实

1. pinned / compiled / rules / summary 注入不再在 command 层直接拼接。
2. 输出 typed injection result，而不是裸字符串拼接副作用。

### 风险提示

不要在 `M1` 就试图完成 `MemoryCoordinator`，那是 `M3`。

## 7.6 `m1.5` Stream Emitter Boundary

### 输出文件

- `src-tauri/src/modules/runtime/stream_emitter.rs`
- `src-tauri/src/commands/agent.rs`

### 必须落实

1. canonical envelope 与 legacy payload 的发射统一由 emitter 管理。
2. execution-mode / memory / activation 事件都能复用 emitter。

### checkpoint

`m1.5` 后必须人工确认没有破坏现有前端流式主路径。

## 7.7 `m1.6` Request Intelligence Service

### 输出文件

- `src-tauri/src/modules/application/request_intelligence_service.rs`
- `src-tauri/src/modules/control_plane/ingress_classifier.rs`
- `src-tauri/src/modules/runtime/contracts/execution_mode.rs`

### 必须落实

1. 建立 `classify_request` 正式入口。
2. 输出 `execution_mode`、`risk_level`、`complexity_level`、`reason_codes`、`route_hint`。
3. 至少有 deterministic + heuristic 骨架。

### 不可接受

把 execution mode 判定塞回前端，或把 classifier 做成临时 helper。

## 7.8 `m1.7` Activation And License Lifecycle Services

### 输出文件

- `src-tauri/src/modules/application/activation_service.rs`
- `src-tauri/src/modules/application/license_lifecycle_service.rs`
- `src-tauri/src/commands/activation.rs`

### 必须落实

1. activation request / redeem / refresh / revoke-check / deactivate 有统一后端入口。
2. command 只做 IPC 桥接。

## 7.9 `m1.8` Prepare Step Execution Seam

### 输出文件

- `src-tauri/src/modules/control_plane/prepare_step_execution.rs`
- `src-tauri/src/modules/control_plane/mod.rs`

### 必须落实

1. `prepare_step_execution` 可接入真实执行链路。
2. 输出至少包含 `resolved_boundary`、`permission_decision`、`sandbox_policy`。

## 8. 交付物检查清单

- [ ] `modules/application/` 已创建并导出
- [ ] `agent.rs` 已明显降责
- [ ] provider 解析已从 command 层迁出
- [ ] prompt assembling 已有 skeleton
- [ ] memory injection 已有 service 边界
- [ ] stream emitter 已出现
- [ ] request intelligence service 已出现
- [ ] activation / license lifecycle services 已出现
- [ ] `prepare_step_execution` 已出现
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` 已通过

## 9. 推荐提交顺序

1. `m1.1 + m1.2`
2. `m1.3 + m1.4 + m1.5`
3. `m1.6 + m1.7 + m1.8`

## 10. 完成标志

只有当 reviewer 能清楚指出“command adapter 在哪里结束、application service 从哪里开始、control plane 从哪里插入”时，`M1` 才算完成。
