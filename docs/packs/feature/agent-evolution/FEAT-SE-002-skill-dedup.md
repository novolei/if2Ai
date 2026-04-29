# FEAT-SE-002: Skill 去重与合并

## Status
- State: active

## Goal
在 `skills/sedimentation/dedup.rs` 实现 draft 去重：对 `Vec<SkillDraft>` 用 `Embedder` trait
做余弦相似度计算（阈值 ≥ 0.85 折叠为同一组），组内保留 `description` 最长者为代表，
其余 `name` 合入 `aliases`，输出 `Vec<DedupedSkill>`。同步把 `sedimentation.rs` 重构为
`sedimentation/` 目录（将 SE-001 内容搬入 `sedimentation/mod.rs`）。

## Spec
- 两个语义相同的 draft（cosine ≥ 0.85）折叠为 1 个 DedupedSkill → `tests::skill_evolution::identical_drafts_dedup_to_one`
- 两个正交 draft（cosine < 0.85）保持独立 → `tests::skill_evolution::orthogonal_drafts_not_merged`
- 被折叠的副本 name 出现在代表的 `aliases` 列表中 → `tests::skill_evolution::aliases_preserved`
- 空输入 → 空输出，不 panic → `tests::skill_evolution::dedup_empty_input`

## Files (scope — write list)
- `src-tauri/src/modules/skills/sedimentation/mod.rs`      (new — 接管 SE-001 全部内容)
- `src-tauri/src/modules/skills/sedimentation/dedup.rs`    (new — DedupedSkill + Embedder trait + dedup_drafts)
- `src-tauri/src/modules/skills/sedimentation.rs`          (modify → `pub mod sedimentation { pub use sedimentation::*; }` shim，或直接替换为 mod 重定向)
- `src-tauri/src/modules/skills/mod.rs`                    (modify — 将 `pub mod sedimentation;` 路径更新至目录)
- `src-tauri/tests/skill_evolution.rs`                     (modify — 新增 4 条去重测试)

## Reads
- `src-tauri/src/modules/skills/sedimentation.rs`           (SE-001 完成后的真实内容)
- `src-tauri/src/modules/memory/embedding/mod.rs`           (了解现有 embedding provider 接口)

## Contract
- 不改 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- `Embedder` trait 必须 sync（避免 async 传染测试）：`fn embed(&self, text: &str) -> Vec<f32>`

## Out of Scope
- ❌ 不集成宪法层 (FEAT-SE-003)
- ❌ 不接入向量检索索引 (FEAT-SE-004)
- ❌ 不改 sedimentation pipeline 主流程 (`extract_skill_drafts`)
- ❌ 不把去重结果写入磁盘
- ❌ 不改 `skills/manager/mod.rs`

## Depends On
- FEAT-SE-001

## Verify
- `./scripts/pack run FEAT-SE-002`
- `cargo test -p if2ai-tauri --test skill_evolution 2>&1 | grep -E "PASS|FAIL"`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
