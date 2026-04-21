# Execution Mode Policy Routing Gap — 使用指南

> 解释为什么 execution mode、request intelligence、policy seam 目前还没有真正改变执行行为。

## 1. 这个 Gap 是什么

它讨论的是：

- classifier 已经有了，为什么还没有真正“分流”
- `prepare_step_execution` 已经有了，为什么还没有真正“管住执行”

## 2. 必看证据

- If2Ai truth：
  - [if2ai-workflow-truth.md](../../if2ai-workflow-truth.md:192)
- backend 代码：
  - [request_intelligence.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/request_intelligence.rs:1)
  - [prepare_step_execution.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/prepare_step_execution.rs:143)
- UClaw 参考：
  - [coordinator.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/policy/coordinator.rs:45)

## 3. 什么时候该看这个模块

- 讨论 execution mode 是不是“只是一个 pill”
- 讨论 policy 是不是“只是治理 trace”
- 重排 M1/M4 后续优先级

