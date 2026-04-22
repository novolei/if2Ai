# MIG-005 Real Memory Lifecycle

## Status

- State: `done`
- Owner: `@executor`
- Gap Module: [memory-write-recall-lifecycle](../../staff-remediation/gap-modules/memory-write-recall-lifecycle/01-usage-guide.md)
- Completed: `2026-04-22`
- Last Updated: `2026-04-22`

---

## Goal

把 if2Ai 的 memory 从“有 recall/write 结构但不闭环”升级成真实可用的 `prepare_context -> after_turn -> persist -> recall feedback` 生命周期。

## Why Now

1. 当前 `MemoryCoordinator.after_turn` 明确不做 persistence，这说明 memory product loop 还没成立。
2. 用户看起来像有很多 memory governance，但 agent 实际上没有拿到稳定收益。
3. UClaw 的 memory manager 已经证明 unified lifecycle 比分散 gate 更有效。

## Allowed Files

- `src-tauri/src/modules/application/memory_coordinator.rs`
- `src-tauri/src/modules/application/memory_recall_assembler.rs`
- `src-tauri/src/modules/application/memory_write_policy.rs`
- `src-tauri/src/modules/memory/**`
- `src-tauri/tests/**`

## Forbidden Files

- `src/**`
- `src-tauri/src/modules/learning/**`
- `docs/exec-plans/**`

## Source Of Truth

- [UClaw Gap Migration Audit](../../staff-remediation/uclaw-gap-migration-audit.md)
- [Memory Write Recall Lifecycle Usage](../../staff-remediation/gap-modules/memory-write-recall-lifecycle/01-usage-guide.md)
- [Memory Write Recall Lifecycle Implementation](../../staff-remediation/gap-modules/memory-write-recall-lifecycle/02-implementation.md)
- [If2Ai Workflow Truth](../../staff-remediation/if2ai-workflow-truth.md)

## UClaw References

- [memory_manager.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/memory/memory_manager.rs)

## Required Changes

1. 让 `after_turn` 进入真实 persistence 路径。
2. 让 recall assembler 不再长期保留空 placeholder 槽位。
3. 打通 write policy、quality gate、conflict resolution、persist 的单一流水线。
4. 让下一轮 recall 真正受上一轮 memory write 影响。

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- 至少一条 memory lifecycle 测试通过
- `memory_write` 与 `memory_recall` workflow 不再只是 partial skeleton

## Out Of Scope

- 不做 strategy promotion
- 不做 activation flow
- 不做前端 memory browser 美化

## Execution Notes

- 优先补闭环，不优先补治理指标。
