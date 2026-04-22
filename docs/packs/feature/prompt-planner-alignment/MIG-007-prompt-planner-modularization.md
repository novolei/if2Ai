# MIG-007 Prompt Planner Modularization (Phase 3)

## Status

- State: `draft`
- Owner: `@executor`
- Gap Analysis: [prompt-planner-gap-analysis.md](../../../staff-remediation/prompt-planner-gap-analysis.md) §7.3
- Depends On: `MIG-006`
- Last Updated: `2026-04-22`

---

## Goal

将 prompt_planner/mod.rs 拆分为多个子模块（block.rs、diagnostics.rs、build_request.rs、planner.rs），提升代码可维护性和扩展性，对齐 UClaw 的模块化结构。

## Why Now

1. MIG-005/006 已完成功能对齐，mod.rs 已超过 600 LOC。
2. 当前单文件承载过多职责（数据结构 + 构建逻辑 + 验证逻辑）。
3. 未来 Coding mode 增强（Phase 4）会进一步增加复杂度。
4. UClaw 的模块化结构已证明易于维护和扩展。

## Spec

1. 创建 `prompt_planner/block.rs`：
   - PromptBlockKind 枚举
   - PromptBlock 结构体
   - PromptBlockSource 结构体
   - PromptContribution 结构体
2. 创建 `prompt_planner/diagnostics.rs`：
   - PromptPlanDiagnostics 结构体
   - PromptValidationIssue 结构体
3. 创建 `prompt_planner/build_request.rs`：
   - BuildPromptPlanRequest 结构体
   - PromptBuildMode 枚举
   - PromptBuildOptions 结构体
4. 创建 `prompt_planner/planner.rs`：
   - PromptPlan 结构体
   - PromptPlanResult 结构体
   - build_prompt_plan 函数
   - merge_external_contributions 函数
   - compute_block_hash 函数
   - compute_trace_id 函数
   - build_diagnostics 函数
   - title_for_memory_section 函数
5. 更新 `prompt_planner/mod.rs`：
   - 声明子模块
   - 重新导出公共类型
   - 保持现有测试（或移到 tests.rs）

## Contract

**零行为变更**：这是纯 refactor pack。
**导入路径不变**：所有外部调用方（TurnService）的 import 路径保持不变。
**测试不变**：所有现有测试保持通过，测试名称不变。

## Files (scope)

- `src-tauri/src/modules/application/prompt_planner/mod.rs`
- `src-tauri/src/modules/application/prompt_planner/block.rs` (新建)
- `src-tauri/src/modules/application/prompt_planner/diagnostics.rs` (新建)
- `src-tauri/src/modules/application/prompt_planner/build_request.rs` (新建)
- `src-tauri/src/modules/application/prompt_planner/planner.rs` (新建)

## Forbidden Files

- `src-tauri/src/modules/application/turn_service/mod.rs` (不修改调用方)
- `src/**`

## Source Of Truth

- [prompt-planner-gap-analysis.md](../../../staff-remediation/prompt-planner-gap-analysis.md) §2.1, §7.3

## UClaw References

- [prompt/mod.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/mod.rs)
- [prompt/block.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/block.rs)
- [prompt/diagnostics.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/diagnostics.rs)
- [prompt/build_request.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/build_request.rs)
- [prompt/planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs)

## Execute Plan

1. 创建 block.rs，移动 PromptBlockKind、PromptBlock、PromptBlockSource、PromptContribution。
2. 创建 diagnostics.rs，移动 PromptPlanDiagnostics、PromptValidationIssue。
3. 创建 build_request.rs，移动 BuildPromptPlanRequest、PromptBuildMode、PromptBuildOptions。
4. 创建 planner.rs，移动 PromptPlan、PromptPlanResult、build_prompt_plan 及所有辅助函数。
5. 更新 mod.rs：
   - 声明 4 个子模块
   - `pub use block::*;`
   - `pub use diagnostics::*;`
   - `pub use build_request::*;`
   - `pub use planner::*;`
   - 保留现有 sanitize、preflight、governor 子模块声明
   - 保留或移动 tests 模块
6. 验证所有导入路径正确（内部 use 语句调整）。
7. 运行 cargo fmt + clippy + test。

## Verify Whitelist

- 新建 4 个文件（block.rs、diagnostics.rs、build_request.rs、planner.rs）
- mod.rs 大幅缩减（仅保留模块声明和重新导出）
- 所有 pub 符号集合不变
- 所有测试名称不变

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml prompt_planner`
- 所有现有测试通过（测试名称不变）
- mod.rs < 100 LOC
- 每个新文件 < 400 LOC

## Out Of Scope

- 不实现 Coding mode 专用逻辑（Phase 4）
- 不修改任何函数体（纯移动代码）
- 不修改测试逻辑
