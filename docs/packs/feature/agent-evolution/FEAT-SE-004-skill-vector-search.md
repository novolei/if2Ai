# FEAT-SE-004: Skill 向量检索接入

## Status
- State: active

## Goal
在 `skills/vector_index.rs` 实现 skill 向量索引与检索：对每个保存的 skill
（description + body 前 500 字符）计算 embedding，通过 `VectorStore` trait
（抽象层，测试用 Mock，生产接 sqlite/lancedb）存取，`kind = "skill"` 字段区分
其他向量记录。暴露 `index_skill` / `search_skills` 两个 async API，
`SkillSearchHit { skill_id, score, payload }` 作为检索结果类型。

## Spec
- index 成功后 search 同主题 → score ≥ 0.3 且命中 → `tests::skill_evolution::indexed_skill_searchable`
- 查询与索引内容无关 → score < 0.3 → `tests::skill_evolution::unrelated_query_low_score`
- `search_skills(..., top_k: 2)` 返回 ≤ 2 条 → `tests::skill_evolution::top_k_limit_respected`
- 空 store → search 返回空 vec，不 panic → `tests::skill_evolution::empty_store_returns_empty`

## Files (scope — write list)
- `src-tauri/src/modules/skills/vector_index.rs`   (new — VectorStore trait + MockVectorStore + index_skill + search_skills + SkillSearchHit + IndexError)
- `src-tauri/src/modules/skills/mod.rs`            (modify — `pub mod vector_index;`)
- `src-tauri/tests/skill_evolution.rs`             (modify — 新增 4 条向量检索测试)

## Reads
- `src-tauri/src/modules/memory/conversation_recall_vector.rs`  (cosine_similarity / f32_slice_to_blob 工具函数)
- `src-tauri/src/modules/memory/embedding/mod.rs`               (FastEmbedProvider 接口，用于 Embedder trait 对齐)
- `src-tauri/src/modules/skills/sedimentation.rs`               (SkillDraft struct，SE-001 完成后)

## Contract
- 不改 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)；`kind = "skill"` 通过 VectorStore trait 注入，不直接改 conversation_recall 表 schema
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- `VectorStore` trait 必须 `Send + Sync`；async fn 用 `async_trait`

## Out of Scope
- ❌ 不改 `conversation_recall_vector.rs` schema（仅借用工具函数）
- ❌ 不接入 `prompt_planner` 注入（检索结果如何使用留给后续 wire-up Pack）
- ❌ 不改前端 contracts.ts
- ❌ 不为 VectorStore 提供真实 sqlite/lancedb 实现（Mock 足够验证）
- ❌ 不修改 `runtime/daemon/`

## Depends On
- FEAT-SE-001
- FEAT-SE-002

## Verify
- `./scripts/pack run FEAT-SE-004`
- `cargo test -p if2ai-tauri --test skill_evolution 2>&1 | grep -E "PASS|FAIL"`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
