# FEAT-DK-001: 域知识仓库

## Status
- State: active

## Goal
在 `skills/domain_knowledge.rs` 实现纯 in-memory 域知识 store。
支持三类知识（WebsiteDomain / InteractionPrimitive / TaskSOP）+ `lookup` 检索 + `KnowledgeStore` trait 抽象层。
Draft-only，不落盘，为后续 sqlite/lancedb 接入预留 trait 替换接口。

## Spec
- upsert + lookup roundtrip 命中，结果非空 → `tests::domain_knowledge::upsert_then_lookup_returns_entry`
- kind_filter 生效：filter=WebsiteDomain 不返回 TaskSOP 条目 → `tests::domain_knowledge::kind_filter_excludes_other_kinds`
- 空 store lookup → 返回空 vec → `tests::domain_knowledge::empty_store_returns_empty`
- lookup 命中后 `access_count` 递增（MockKnowledgeStore 行为）→ `tests::domain_knowledge::lookup_increments_access_count`

## Files (scope — write list)
- `src-tauri/src/modules/skills/domain_knowledge.rs`  (new)
- `src-tauri/src/modules/skills/mod.rs`               (modify — `pub mod domain_knowledge;`)
- `src-tauri/tests/domain_knowledge.rs`               (new — 集成测试，含上述 4 个测试)

## Reads
- `src-tauri/src/modules/skills/vector_index.rs`      (MockVectorStore 模式参考)
- `src-tauri/src/modules/skills/sedimentation/mod.rs` (SkillDraft struct 参考)

## Contract
- 不改 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency（仅用 std + serde + uuid + chrono，均已在 Cargo.toml）
- `cargo clippy -D warnings` 通过 (I7)
- 全部为 in-memory，不落盘到 ~/.if2ai/

## Out of Scope
- ❌ 不接入 work_loop / prompt_planner / stream_preflight
- ❌ 不实现 sqlite / lancedb 持久化 store
- ❌ 不改前端 contracts.ts 或 IPC 命令
- ❌ 不实现向量检索（相似度 lookup 留给后续 wiring Pack）
- ❌ 不实现 DK-003 contributor 逻辑

## Verify
- `./scripts/pack run FEAT-DK-001`
- `cargo test -p if2ai-tauri --test domain_knowledge 2>&1 | grep -E "PASS|FAIL"`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
