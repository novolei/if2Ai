# Phase M4 Execution And Report Contracts File-Level Plan

> 将 `M4` 的 `m4.1 + m4.2` 细化为文件级实施方案。
>
> 最后更新: 2026-04-20

## 1. 适用范围

本计划只覆盖：

1. `m4.1` Implement `prepare_step_execution` on real execution path
2. `m4.2` Introduce Harness V2 run report contracts

目标是先让 If2Ai 的执行链真正出现统一 preflight 决策入口，并且让 harness 从“只录事件”升级到“有正式结果契约”。

## 2. 当前事实基线

### 2.1 tool execution 已有 broker，但还没有正式 preflight contract

当前 [src-tauri/src/modules/control_plane/tool_execution_broker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs:1) 已经承担统一工具调度，但它当前更像：

1. audit / deny helper 的承载点
2. request guard 的承载点
3. dispatch entry

还不是 blueprint 中的：

`prepare_step_execution -> boundary -> permission -> sandbox -> execution`

### 2.2 harness 当前已有 observability 基础，但没有 run report 契约

当前 harness 已有：

1. [src-tauri/src/modules/harness/event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs:1)
2. [src-tauri/src/modules/harness/session_recorder.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/session_recorder.rs:1)
3. [src-tauri/src/modules/harness/telemetry.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/telemetry.rs:1)
4. [harness/runner.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/runner.py:1)
5. [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

但还缺：

1. `HarnessRunReport`
2. `TaskRunResult`
3. `AggregateMetrics`
4. `BlockingFailures`
5. `Recommendation`

### 2.3 当前治理证据仍不够结构化

现有 `AgentEvent` 已能记录 turn/tool/permission/reflection 等事实，但还没有正式承载：

1. execution mode evidence
2. memory write / recall decision evidence
3. policy version / candidate version
4. baseline/candidate compare identity

## 3. 实施原则

1. 先打通真实执行链，再谈 grader。
2. 先定义 report contract，再扩 compare 和 blocker。
3. `prepare_step_execution` 必须进入真实调用点，不能只做 demo helper。
4. 这一轮允许保留旧 recorder/telemetry 路径，但新的治理口径必须走正式 report contract。

## 4. 严格执行顺序

1. `E0` preflight governance inventory
2. `E1` 建 `prepare_step_execution` contract
3. `E2` 接入 `ToolExecutionBroker`
4. `E3` 回写 control-plane 导出
5. `E4` 建 run report contracts
6. `E5` 建 compare result contract
7. `E6` 回写 harness runner / Rust harness seam
8. `E7` compile + manual verification

禁止并行：

1. broker 深改
2. harness report contract
3. grader/blocker 规则

因为这三项一起动时，最容易让执行链和治理链同时没有稳定接口。

## 5. 文件级实施方案

## 5.1 `E0` Preflight Governance Inventory

### 必查文件

- [src-tauri/src/modules/control_plane/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/mod.rs:1)
- [src-tauri/src/modules/control_plane/tool_execution_broker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs:1)
- [src-tauri/src/modules/control_plane/boundary_resolver.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/boundary_resolver.rs:1)
- [src-tauri/src/modules/runtime/permissions.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/runtime/permissions.rs:1)
- [src-tauri/src/modules/harness/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/mod.rs:1)
- [src-tauri/src/modules/harness/event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs:1)
- [src-tauri/src/modules/harness/session_recorder.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/session_recorder.rs:1)
- [harness/runner.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/runner.py:1)
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

### 必做动作

1. 盘出真实工具执行前目前有哪些 boundary / permission / sandbox 决策点。
2. 盘出 Rust harness 与 Python harness 当前的职责分界。
3. 标出哪些结果字段当前只能从日志推断，尚未结构化。

## 5.2 `E1` 建 `prepare_step_execution` contract

### 新增文件

- `src-tauri/src/modules/control_plane/prepare_step_execution.rs`

### 修改文件

- [src-tauri/src/modules/control_plane/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/mod.rs:1)

### 最低结构建议

```rust
pub struct PrepareStepExecutionInput<'a> { ... }
pub struct PrepareStepExecutionOutput { ... }
pub enum StepExecutionDisposition { Allow, RequiresApproval, Deny, Escalate }
```

### 第一版必须承载

1. boundary decision
2. permission decision
3. sandbox policy summary
4. machine-readable reason codes

### 这一步不要做的事

1. 不要把真实工具 dispatch 写进 `prepare_step_execution.rs`
2. 不要把 harness report 逻辑塞进 control plane

## 5.3 `E2` 接入 `ToolExecutionBroker`

### 修改文件

- [src-tauri/src/modules/control_plane/tool_execution_broker.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/tool_execution_broker.rs:1)

### 必须落实

1. broker 在真正 dispatch 前调用 `prepare_step_execution(...)`
2. allow / requires_approval / deny / escalate 都有明确分支
3. audit 中能拿到 preflight artifacts

### 第一阶段允许保留

1. 现有 skill guard / strict mode deny helper
2. 现有 audit emitter 调用

前提是这些逻辑开始围绕 preflight 输出统一收口。

## 5.4 `E3` 回写 control-plane 导出

### 修改文件

- [src-tauri/src/modules/control_plane/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/mod.rs:1)

### 必须落实

1. `prepare_step_execution` 成为 control-plane 正式导出入口
2. 后续 worker skeleton / broker / policy path 统一引用它

## 5.5 `E4` 建 run report contracts

### 新增文件

- `src-tauri/src/modules/harness/run_report.rs`
- `src-tauri/src/modules/harness/report_contracts.rs`

### 修改文件

- [src-tauri/src/modules/harness/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/mod.rs:1)

### 第一版建议结构

1. `HarnessRunReport`
2. `TaskRunResult`
3. `AggregateMetrics`
4. `BlockingFailure`
5. `Recommendation`
6. `RunIdentity`

### 最低要求

report 至少要包含：

1. `run_id / task_id / suite_id / trace_id / session_id`
2. policy version / candidate version
3. success / blocking failures
4. token / latency / tool / retry / resume metrics
5. memory / policy / execution-mode evidence摘要

## 5.6 `E5` 建 compare result contract

### 新增文件

- `src-tauri/src/modules/harness/compare_report.rs`

### 修改文件

- `src-tauri/src/modules/harness/run_report.rs`

### 第一版必须承载

1. baseline identity
2. candidate identity
3. diff summary
4. blocker summary
5. recommendation

### 约束

compare contract 先于 compare engine 落地，避免后续实现时边写边改 shape。

## 5.7 `E6` 回写 harness runner / Rust harness seam

### 修改文件

- [src-tauri/src/modules/harness/event_bus.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/event_bus.rs:1)
- [src-tauri/src/modules/harness/session_recorder.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/session_recorder.rs:1)
- [src-tauri/src/modules/harness/agent_loop_integration.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/harness/agent_loop_integration.rs:1)
- [harness/runner.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/runner.py:1)
- [harness/gate.py](/Users/ryanliu/Documents/IfAI/if2Ai/harness/gate.py:1)

### 必须落实

1. Rust harness 事件和 recorder 至少能为 run report 提供结构化输入。
2. Python harness runner 能识别新的 report/compare shape。
3. 旧 telemetry/JSONL 流仍可作为兼容层存在，但不再是唯一产物。

## 5.8 `E7` Compile + Manual Verification

### 必跑

- `cargo check --manifest-path src-tauri/Cargo.toml`

### 建议补跑

- `python3 -m harness.runner --help`

### 必做人工检查

1. preflight 已经进入真实工具执行链，而不是死代码。
2. harness run report contract 已可被人和脚本同时理解。
3. compare contract 已有明确 baseline/candidate shape。

## 6. 完成定义

只有当 reviewer 可以明确指出“真实执行链已经有正式 preflight，且 harness 已有稳定 run report 契约”时，这一段才算完成。
