# FEAT-BR-003: 工具原子性整合（alias 表）

## Status
- State: active

## Goal
在 `tools/builtin/atomic_consolidation.rs` 实现静态 alias 解析表，将 18 个旧工具名映射到 6 个原子工具名：
memory × 6 → 3 atomic；cron × 5 → 1 atomic；skill × 7 → 2 atomic。
仅提供 alias 解析 + 单元测试，不动任何现有工具实现或注册路径，保留向下兼容。

## Spec (verifiable)
- `resolve_alias("memory_recall")` → `Some("memory_read")` → `tests::browser_refinement::test_alias_memory_recall`
- `resolve_alias("cron_create")` → `Some("schedule_manage")` → `tests::browser_refinement::test_alias_cron_create`
- `resolve_alias("skill_list")` → `Some("skill_find")` → `tests::browser_refinement::test_alias_skill_list`
- `resolve_alias("unknown_tool_xyz")` → `None` → `tests::browser_refinement::test_alias_unknown_returns_none`
- `list_atomic_tools()` 返回恰好 6 个不重复名且包含全部 atomic 工具 → `tests::browser_refinement::test_list_atomic_tools_count_and_uniqueness`

## Files (scope — write list)
- src-tauri/src/modules/tools/builtin/atomic_consolidation.rs (new)
- src-tauri/src/modules/tools/builtin/mod.rs                  (modify — add `pub mod atomic_consolidation;`)
- src-tauri/tests/browser_refinement.rs                       (modify — add 5 BR-003 tests)

## Reads (read-only)
- src-tauri/src/modules/tools/builtin/mod.rs                  (现有工具注册路径 + pub mod 列表)

## Alias Table (normative — 18 entries)
- memory: recall/export → memory_read; pin/compile → memory_write; query/memory_search → memory_search
- cron: create/list/pause/resume/delete → schedule_manage
- skill: list/search/export → skill_find; load/run/install/remove → skill_use

## Contract (review must check)
- 不新增 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- TOOL_ALIASES 为 `pub const &[ToolAlias]`（静态，零分配）
- 不删除或修改任何现有工具文件

## Out of Scope
- ❌ 不改任何现有工具的实现（memory_store、skill.rs 等）
- ❌ 不删除任何现有工具注册
- ❌ 不接入 ToolDefinition runtime（wire-up 留后续 Pack）
- ❌ 不改前端 contracts.ts
- ❌ 不改 tool_execution_broker.rs

## Depends on
- 无（独立 Pack）

## Verify
- cargo build --package if2ai-tauri
- cargo test --test browser_refinement -- test_alias_memory_recall test_alias_cron_create test_alias_skill_list test_alias_unknown_returns_none test_list_atomic_tools_count_and_uniqueness
- cargo clippy --package if2ai-tauri -D warnings
- ./scripts/pack run FEAT-BR-003

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
