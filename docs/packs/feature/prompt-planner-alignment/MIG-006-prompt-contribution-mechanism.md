# MIG-006 Prompt Contribution Mechanism (Phase 2)

## Status

- State: `done`
- Owner: `@executor`
- Gap Analysis: [prompt-planner-gap-analysis.md](../../../staff-remediation/prompt-planner-gap-analysis.md) §7.2
- Depends On: `MIG-005`
- Last Updated: `2026-04-22`
- Completed: `2026-04-22`

---

## Goal

引入 PromptContribution 机制和 PromptBuildMode，让子系统（Memory、MCP、Skills、Learning）可以独立贡献 prompt blocks，实现 planner 与子系统解耦。

## Why Now

1. MIG-005 已完成数据结构对齐，具备 source/priority 基础。
2. 当前 memory_injection 通过 `Option<MemoryInjectionArtifacts>` 传入是特例，不是通用机制。
3. 未来 MCP、Skills、Learning 都需要类似能力，不应每次修改 planner 核心。
4. UClaw 的 external contributions 机制已证明有效。

## Spec

1. 创建 PromptContribution 结构体（kind + title + body + source）。
2. 创建 PromptBuildMode 枚举（Chat / Coding）。
3. 创建 PromptBuildOptions 结构体（include_diagnostics + strict_block_validation）。
4. 更新 BuildPromptPlanRequest：
   - 添加 `mode: PromptBuildMode`
   - 添加 `persona_id: Option<String>`
   - 添加 `active_skill_ids: Vec<String>`
   - 添加 `options: PromptBuildOptions`
5. 更新 build_prompt_plan 签名：添加 `external_contributions: Vec<PromptContribution>` 参数。
6. 实现 merge_external_contributions 逻辑：
   - 禁止外部覆盖 System/UserIntent blocks（strict mode 下直接失败）
   - 按 kind 分组，插入到对应 core block 之后
   - UserIntent 始终保持最后
   - 收集 validation issues
7. 生成 Persona block（如果 persona_id 存在）。
8. 生成 Skill block（如果 active_skill_ids 非空）。
9. 更新 TurnService::prepare_chat_inputs 调用点。

## Contract

**输入变更**：
- BuildPromptPlanRequest 新增 4 个字段
- build_prompt_plan 新增 external_contributions 参数

**输出扩展**：
- PromptPlan.blocks 可能包含 external contributions
- PromptPlan.diagnostics.validation_issues 可能包含验证警告

**行为变更**：
- strict_block_validation=true 时，外部尝试覆盖敏感 block 会导致构建失败
- strict_block_validation=false 时，记录警告但继续

## Files (scope)

- `src-tauri/src/modules/application/prompt_planner/mod.rs`
- `src-tauri/src/modules/application/turn_service/mod.rs`

## Forbidden Files

- `src/**`
- `src-tauri/src/modules/memory/**` (不修改 memory 子系统)
- `src-tauri/src/modules/learning/**` (不修改 learning 子系统)

## Source Of Truth

- [prompt-planner-gap-analysis.md](../../../staff-remediation/prompt-planner-gap-analysis.md) §4.1, §4.2, §7.2

## UClaw References

- [planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs:14-41)
- [planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs:234-311)
- [build_request.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/build_request.rs:5-37)

## Execute Plan

1. 定义 PromptContribution、PromptBuildMode、PromptBuildOptions。
2. 更新 BuildPromptPlanRequest 结构体。
3. 更新 build_prompt_plan 签名和实现：
   - 构建 core blocks（包含 Persona、Skill）
   - 调用 merge_external_contributions
   - 验证 + 排序
   - 生成 diagnostics（包含 validation_issues）
4. 实现 merge_external_contributions 函数。
5. 更新 TurnService::prepare_chat_inputs 调用点。
6. 添加测试覆盖 external contributions 场景。

## Verify Whitelist

- PromptContribution 结构体新增
- PromptBuildMode 枚举新增
- PromptBuildOptions 结构体新增
- BuildPromptPlanRequest 字段新增
- build_prompt_plan 签名变更
- merge_external_contributions 函数新增

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml prompt_planner`
- 新增至少 4 个测试覆盖 external contributions 场景

## Out Of Scope

- 不实现 Coding mode 专用逻辑（Phase 4）
- 不让 Memory/MCP/Skills 实际使用 contributions（后续 Pack）
- 不拆分 mod.rs 为多个子模块（Phase 3）
