# Chat Prompt Dispatch Gap — 实现清单

## UClaw 对标

- Chat 主 happy path 被明确写成稳定主链：
  - [CURRENT_WORKFLOWS.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/CURRENT_WORKFLOWS.md:27)

## If2Ai 当前证据

- `chat_prompt_dispatch` 仍为 `partial`：
  - [if2ai-workflow-truth.md](../../if2ai-workflow-truth.md:72)
- 主执行文件仍然集中在：
  - [agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
- `TurnService` 明确不负责完整 turn：
  - [turn_service.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/turn_service.rs:11)

## 具体 Gap

1. provider / prompt / memory preparation 只迁出了一部分
2. tool execution / stream emission / permission wait / after_turn 仍集中在 command 层
3. 没有完整的 `prepare -> execute -> finalize -> after_turn` 统一 spine

## 为什么用户能感知到

- 发消息后的核心行为没有被“新架构”真正接管
- 新能力都只能在外围叠加，而不是成为主执行路径的一部分

## 建议整改

1. 把 `run_agent_turn` 和 `start_agent_stream` 的共用主线抽成真实 execution kernel
2. 让 `TurnService` 覆盖完整 turn 生命周期，而不是只覆盖 preflight
3. 缩小 `agent.rs` 到 adapter + wiring 级别

