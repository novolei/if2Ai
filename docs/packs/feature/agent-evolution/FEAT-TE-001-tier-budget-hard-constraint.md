# FEAT-TE-001: 上下文层级预算硬约束

## Status
- State: pending
- Depends on: FEAT-EVO-000

## Goal
将现有 `ContextBudget`（建议值）升级为 5 tier 硬约束，在每次 LLM 调用前由
`stream_preflight.rs` 强制执行。目标：平均请求上下文从 ~120K chars 降到 ≤30K chars，
不丢失任务连贯性；压缩失败时降级到现有字符裁剪逻辑（非致命）。

## Spec (verifiable — 每条配 1 个 test name)
- `ContextTier` 枚举与 `TierBudgetAllocation` 结构体编译通过 → `tests::context_compression::tier_budget_allocation_default_sums_to_total`
- `compress_for_request` 对超预算输入返回截断后的消息列表，总 token ≤ budget → `tests::context_compression::compress_for_request_respects_hard_limit`
- `stream_preflight` 集成后，超出 tier 总限的消息被丢弃而非透传 → `tests::context_compression::preflight_integration_hard_drops_overflow`
- 压缩失败（空消息输入）时降级返回原始消息，不 panic → `tests::context_compression::compress_graceful_fallback_on_empty`

## Files (scope — write list)
- src-tauri/src/modules/runtime/context_compression.rs                          (new)
- src-tauri/src/modules/runtime/budget.rs                                       (modify)
- src-tauri/src/modules/application/turn_service/stream_preflight.rs            (modify)
- src-tauri/tests/context_compression_tests.rs                                  (new)

## Reads (read-only inputs allowed beyond Files)
- src-tauri/src/modules/runtime/compact.rs
- src-tauri/src/modules/runtime/stream_outcome.rs
- .qoder/specs/if2ai-agent-evolution-report.md §Module A（核心数据结构定义）

## Contract (review must check)
- 不改任何 IPC 命令名或事件字面量（I3）
- 不改 sqlite schema / localStorage key（I4）
- 不引入新 Cargo.toml dependency（仅使用现有 `tiktoken-rs` 或 `estimate_tokens`）
- `ContextBudget::default()` 返回值不变（现有调用不破坏）（I4 精神）
- `cargo clippy -D warnings` 通过（I7）

## Out of Scope
- ❌ 不实现 LLM 辅助摘要（属于 FEAT-TE-002）
- ❌ 不实现迷你索引（属于 FEAT-TE-003）
- ❌ 不实现工具结果压缩（属于 FEAT-TE-004）
- ❌ 不修改前端 contracts.ts
- ❌ 不顺手重构 compact.rs

## Verify
- ./scripts/pack run FEAT-TE-001

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
