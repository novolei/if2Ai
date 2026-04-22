# MIG-008 Coding Mode Prompt Enhancement (Phase 4)

## Status

- State: `done`
- Owner: `@executor`
- Gap Analysis: [prompt-planner-gap-analysis.md](../../../staff-remediation/prompt-planner-gap-analysis.md) §7.4
- Depends On: `MIG-007`
- Completed: `2026-04-22`
- Last Updated: `2026-04-22`

---

## Goal

为 Coding mode 实现专用 prompt 增强，包括 continuation block 生成、compaction policy、workspace context augmentation，支持长会话场景。

## Why Now

1. MIG-005/006/007 已完成基础设施对齐，具备 mode 区分能力。
2. 当前所有请求走相同 prompt 路径，无法针对 Coding 场景优化。
3. 长 coding 会话会超出 token budget，需要 compaction + continuation。
4. UClaw 的 Coding mode 增强已证明有效。

## Spec

1. 当 BuildPromptPlanRequest.mode == Coding 时：
   - 生成 CodingContext blocks（workspace + tool_surface）
   - 根据 options.coding_compaction_* 参数决定是否生成 Continuation block
2. 创建 CodingContext block kind（或复用现有 kind）。
3. 创建 Continuation block kind。
4. 实现 build_coding_augment_blocks 函数：
   - workspace context（cwd、git status、instruction files）
   - tool surface（可用工具列表）
5. 实现 build_coding_continuation_block 函数：
   - 检查 estimated_tokens 和 message_count
   - 如果超过阈值，生成 continuation block
   - 包含 current_work、pending_work、key_files
6. 实现 CompactionPolicy trait（或简化版）：
   - should_compact(estimated_tokens, message_count) -> bool
   - build_continuation(session_snapshot) -> ContinuationSummary
7. 更新 PromptBuildOptions：
   - coding_compaction_estimated_tokens: Option<usize>
   - coding_compaction_message_count: Option<usize>
   - coding_session_snapshot: Option<String>

## Contract

**输入扩展**：PromptBuildOptions 新增 3 个 coding 相关字段。
**输出扩展**：Coding mode 下 PromptPlan.blocks 包含 CodingContext 和可选 Continuation blocks。
**行为变更**：mode=Coding 时 prompt 结构与 Chat 模式明显不同。

## Files (scope)

- `src-tauri/src/modules/application/prompt_planner/planner.rs`
- `src-tauri/src/modules/application/prompt_planner/build_request.rs`
- `src-tauri/src/modules/application/prompt_planner/coding_augment.rs` (新建)
- `src-tauri/src/modules/application/prompt_planner/compaction.rs` (新建)

## Forbidden Files

- `src/**`
- `src-tauri/src/modules/application/turn_service/mod.rs` (暂不修改调用方)

## Source Of Truth

- [prompt-planner-gap-analysis.md](../../../staff-remediation/prompt-planner-gap-analysis.md) §5.1, §7.4

## UClaw References

- [planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs:156-191) (build_coding_augment_blocks)
- [planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs:193-232) (build_coding_continuation_block)
- [coding_prompt_augment.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/coding_prompt_augment.rs)

## Execute Plan

1. 更新 PromptBlockKind：添加 CodingContext、Continuation（如果不存在）。
2. 更新 PromptBuildOptions：添加 3 个 coding 字段。
3. 创建 coding_augment.rs：
   - build_coding_augment_blocks 函数
   - 生成 workspace context block
   - 生成 tool surface block
4. 创建 compaction.rs：
   - CompactionPolicy trait（或简化版）
   - DefaultCompactionPolicy 实现
   - should_compact 逻辑（阈值：4000 tokens 或 16 messages）
   - build_continuation 逻辑（解析 session_snapshot）
5. 更新 planner.rs 的 build_prompt_plan：
   - 检查 request.mode
   - 如果 mode == Coding，调用 build_coding_augment_blocks
   - 如果 mode == Coding 且满足 compaction 条件，调用 build_coding_continuation_block
   - 插入到合适位置（priority 60 for CodingContext, 50 for Continuation）
6. 添加测试：
   - Coding mode 包含 CodingContext blocks
   - Continuation block 在超过阈值时生成
   - Continuation block 在未超过阈值时不生成
   - Chat mode 不包含 Coding 专用 blocks

## Verify Whitelist

- PromptBlockKind 新增 CodingContext、Continuation
- PromptBuildOptions 字段新增
- coding_augment.rs 文件新增
- compaction.rs 文件新增
- build_prompt_plan 函数体修改（新增 Coding mode 分支）

## Acceptance

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml prompt_planner`
- 新增至少 4 个测试覆盖 Coding mode 场景
- Chat mode 行为不变

## Out Of Scope

- 不实现 Automation/Compose mode（长期）
- 不让 TurnService 实际传入 Coding mode（后续 Pack）
- 不实现完整的 session snapshot 解析（简化版即可）
- 不实现 key_files 自动提取（使用 session_snapshot 中的信息）
