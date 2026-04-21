# Chat Prompt Dispatch Gap — 使用指南

> 解释为什么“发消息给 Agent”这条最核心主链在 If2Ai 里仍未 canonical。

## 1. 这个 Gap 是什么

这个模块只讨论一件事：

> 用户发出一条 prompt 之后，If2Ai 是否已经形成像 UClaw 那样稳定、单一、可解释的主执行链。

当前答案是否定的。

## 2. 为什么它优先级最高

因为只要这条链还没收口：

- 后续的 execution mode、memory、harness、self-evolution 都更像外挂层
- 用户会持续觉得“系统看起来很复杂，但回答方式没真正升级”

## 3. 你应该看什么

- UClaw 主链事实：
  - [CURRENT_WORKFLOWS.md](/Users/ryanliu/Documents/iClaw/UClaw/UClawApp/UClaw/CURRENT_WORKFLOWS.md:27)
- If2Ai 当前真相：
  - [if2ai-workflow-truth.md](../../if2ai-workflow-truth.md:72)
- 当前主脑代码：
  - [agent.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/commands/agent.rs:1)
  - [turn_service.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/turn_service.rs:11)

## 4. 使用方式

当你要回答下面这些问题时，先读本模块：

- 为什么 `TurnService` 出现了，体感却没变化
- 为什么 `agent.rs` 还这么大
- 为什么 phase 文档说完成了，但 chat 主链还是 `partial`

## 5. 结论口径

在本模块里，只有当下面三件事同时成立，才算 gap 被关闭：

1. 发消息后的 backend orchestration 有单一 spine
2. 主路径不再主要依赖 `commands/agent.rs` 大内联编排
3. workflow truth 中 `chat_prompt_dispatch` 可以从 `partial` 升级

