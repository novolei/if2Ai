# MIG-005 Prompt Block Structure Alignment (Phase 1)

## Status

- State: `done`
- Owner: `@executor`
- Gap Analysis: [prompt-planner-gap-analysis.md](../../../staff-remediation/prompt-planner-gap-analysis.md) §7.1
- Last Updated: `2026-04-22`
- Completed: `2026-04-22`

---

## Goal

对齐 If2Ai 的 PromptBlock 和 PromptPlanDiagnostics 数据结构与 UClaw，添加 source、priority、is_sensitive、validation_issues 字段，为 harness 归因和未来 token budget 优化做准备。

## Why Now

1. MIG-004 已完成基础 traceability（trace_id、block_hash），但缺少 block 来源追踪。
2. Harness 无法归因 prompt block 来自哪个子系统。
3. 未来 token budget 优化需要基于 priority 裁剪 blocks。
4. Diagnostics 只是快照，不是诊断报告（缺 validation_issues）。

## Spec

1. PromptBlock 新增三个字段：
   - `source: PromptBlockSource` — 追踪 block 来源（subsystem + optional reference）
   - `priority: i32` — 显式优先级（100=最高，0=最低）
   - `is_sensitive: bool` — 敏感内容标记
2. PromptBlock.title 从 `&'static str` 改为 `String`。
3. 创建 PromptBlockSource 结构体（subsystem + reference）。
4. PromptPlanDiagnostics 新增 `validation_issues: Vec<PromptValidationIssue>`。
5. 创建 PromptValidationIssue 结构体（code + message）。
6. 所有现有 block 构建代码填充新字段（保持当前顺序不变）。
7. 更新 build_diagnostics 使用 block.is_sensitive 而非 kind 判断。

## Contract

**输入不变**：BuildPromptPlanRequest 签名不变。
**输出扩展**：PromptPlan.blocks 每个元素多 3 个字段，diagnostics 多 1 个字段。
**行为不变**：PromptPlan.join_into_text() 输出完全一致。

## Files (scope)

- `src-tauri/src/modules/application/prompt_planner/mod.rs`
- `src-tauri/src/modules/application/turn_service/mod.rs` (仅读，验证调用点无需改动)

## Forbidden Files

- `src/**`
- `docs/packs/**` (除本文件)

## Source Of Truth

- [prompt-planner-gap-analysis.md](../../../staff-remediation/prompt-planner-gap-analysis.md) §3.1, §3.2, §7.1

## UClaw References

- [block.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/block.rs:18-33)
- [diagnostics.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/diagnostics.rs:5-18)

## Execute Plan

1. 在 prompt_planner/mod.rs 定义 PromptBlockSource 和 PromptValidationIssue。
2. 修改 PromptBlock：添加 source、priority、is_sensitive；title 改 String。
3. 修改 PromptPlanDiagnostics：添加 validation_issues 字段。
4. 更新 build_prompt_plan 中所有 PromptBlock 构建：
   - System: priority=100, is_sensitive=true, source={subsystem:"system_prompt"}
   - WebToolsRoutingGuide: priority=90, is_sensitive=false, source={subsystem:"tool_routing"}
   - ActiveStrategyOverlay: priority=85, is_sensitive=false, source={subsystem:"learning"}
   - MemoryInjectionPinned: priority=80, is_sensitive=false, source={subsystem:"memory"}
   - MemoryInjectionCompiled: priority=70, is_sensitive=false, source={subsystem:"memory"}
   - MemoryInjectionRules: priority=60, is_sensitive=false, source={subsystem:"memory"}
   - RetrievedMemory: priority=50, is_sensitive=true, source={subsystem:"memory"}
5. 更新 build_diagnostics：使用 block.is_sensitive 判断是否 redact。
6. 更新 compute_block_hash：包含 source.subsystem（保持 hash 稳定性）。
7. 更新所有测试：填充新字段。

## Verify Whitelist

- PromptBlock 字段新增（source、priority、is_sensitive）
- PromptBlock.title 类型变更（&'static str → String）
- PromptBlockSource 结构体新增
- PromptValidationIssue 结构体新增
- PromptPlanDiagnostics.validation_issues 字段新增

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml prompt_planner`
- 所有现有测试通过（3 个单元测试）
- PromptPlan.join_into_text() 输出与 MIG-004 完全一致

## Out Of Scope

- 不实现 external contributions 机制（Phase 2）
- 不实现 PromptBuildMode（Phase 2）
- 不实现 strict validation（Phase 2）
- 不拆分 mod.rs 为多个子模块（Phase 3）
