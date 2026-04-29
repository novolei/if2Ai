# FEAT-DK-002: 工作检查点系统

## Status
- State: active

## Goal
在 `runtime/working_checkpoint.rs` 实现两个纯函数：
`extract_checkpoint` 从 agent 输出文本解析 `<key_info>` 标签；
`inject_checkpoint` 将检查点注入 `Vec<InputMessage>`（BeforeLastUserMessage / AsSystemBlock）。
复用 `estimate_tokens` 做 200 token 硬截断，零副作用。

## Spec
- 含 `<key_info>foo</key_info>` 文本 → extraction.key_info == Some("foo") → `tests::domain_knowledge::extract_key_info_from_tagged_text`
- 不含标签的文本 → key_info == None → `tests::domain_knowledge::extract_returns_none_without_tag`
- 含 `<task_complete/>` 标签 → should_clear == true → `tests::domain_knowledge::extract_task_complete_sets_should_clear`
- inject BeforeLastUserMessage → checkpoint 插入在 messages 最后一条 User 消息之前 → `tests::domain_knowledge::inject_inserts_before_last_user_message`
- key_info 超过 200 token → inject 后注入内容 estimate_tokens ≤ 200 → `tests::domain_knowledge::inject_truncates_to_max_tokens`

## Files (scope — write list)
- `src-tauri/src/modules/runtime/working_checkpoint.rs`  (new)
- `src-tauri/src/modules/runtime/mod.rs`                 (modify — `pub mod working_checkpoint;`)
- `src-tauri/tests/domain_knowledge.rs`                  (modify — 加上述 5 个测试)

## Reads
- `src-tauri/src/modules/runtime/budget.rs`              (`estimate_tokens` fn 签名，line 371)
- `src-tauri/src/modules/api/types.rs`                   (`InputMessage` / `InputContentBlock` struct，lines 30+74)

## Contract
- 不改 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency（仅 std + regex；regex 已在 Cargo.toml）
- `cargo clippy -D warnings` 通过 (I7)
- 全部为纯函数，不落盘，不调用任何异步上下文

## Out of Scope
- ❌ 不接入 stream_preflight 或 stream_finalize（钩子留给 wiring Pack）
- ❌ 不落盘
- ❌ 不改 prompt_planner/
- ❌ 不改前端 contracts.ts
- ❌ 不实现持久化或跨 session 恢复

## Depends on
- FEAT-TE-001 (`estimate_tokens` 已在 `runtime/budget.rs`)

## Verify
- `./scripts/pack run FEAT-DK-002`
- `cargo test -p if2ai-tauri --test domain_knowledge 2>&1 | grep -E "PASS|FAIL"`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
