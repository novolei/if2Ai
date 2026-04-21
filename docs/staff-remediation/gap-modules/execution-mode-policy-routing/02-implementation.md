# Execution Mode Policy Routing Gap — 实现清单

## UClaw 对标

- `prepare_step_execution` 是真实三层策略决策入口：
  - [coordinator.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/policy/coordinator.rs:45)

## If2Ai 当前证据

- `execution_mode_routing` 仍是 `not_established`：
  - [if2ai-workflow-truth.md](../../if2ai-workflow-truth.md:192)
- classifier 是 advisory：
  - [request_intelligence.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/request_intelligence.rs:1)
- preflight seam 仍是 shadow：
  - [prepare_step_execution.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/control_plane/prepare_step_execution.rs:143)
  - [agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:2474)

## 具体 Gap

1. execution mode 只是 judgment，不是 route
2. policy seam 只是 typed decision，不是 real enforcement
3. specialized surface / plan_then_confirm 还没有进入正式主链

## 建议整改

1. 先把一批工具切到真实 preflight enforcement
2. 让至少一种 execution mode 真正影响 route
3. 把前端 explainability 与 backend route 绑定到同一 truth

