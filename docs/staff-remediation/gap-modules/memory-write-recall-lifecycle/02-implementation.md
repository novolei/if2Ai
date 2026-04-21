# Memory Write Recall Lifecycle Gap — 实现清单

## UClaw 对标

- `prepare_context` 和 `after_turn` 都是 MemoryManager 的真实生命周期一部分：
  - [memory_manager.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/memory/memory_manager.rs:172)
  - [memory_manager.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/memory/memory_manager.rs:304)

## If2Ai 当前证据

- `memory_capture / recall / write` 都还是 `partial`：
  - [if2ai-workflow-truth.md](../../if2ai-workflow-truth.md:120)
- `after_turn` 明确不持久化：
  - [memory_coordinator.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/memory_coordinator.rs:36)
- write policy 仍是 skeleton allow：
  - [memory_write_policy.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/memory_write_policy.rs:1)
- recall 仍有 placeholder slots：
  - [memory_recall_assembler.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/memory_recall_assembler.rs:1)

## 具体 Gap

1. typed memory governance 已出现
2. 真实 memory usefulness 还没 fully 形成
3. 写入、冲突、召回顺序、长期收益还没有稳定闭环

