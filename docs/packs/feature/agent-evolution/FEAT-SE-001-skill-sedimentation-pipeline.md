# FEAT-SE-001: Skill 沉淀引擎

## Status
- State: active

## Goal
替换 `skills/sedimentation.rs` 的 EVO-000 stub，实现真实的 `SedimentationPipeline`：
扫描 N 轮 `InputMessage` 历史，识别重复 ≥ 3 次的工具调用序列，调用 `UtilityLlm`
生成 skill 草稿（YAML frontmatter + markdown body），返回 `Vec<SkillDraft>`（draft-only，不落盘）。

## Spec
- 工具调用序列重复次数 < 3 → 返回空 vec → `tests::skill_evolution::below_threshold_yields_empty`
- 序列重复 ≥ 3 次 → 返回 ≥ 1 个 draft → `tests::skill_evolution::repeated_sequence_yields_draft`
- `UtilityLlm::complete` 返回 Err → 优雅降级返回空 vec，不 panic → `tests::skill_evolution::llm_error_degrades_gracefully`
- draft 的 `name` / `description` 非空，`body` 含 YAML frontmatter 起始 `---` → `tests::skill_evolution::draft_fields_well_formed`

## Files (scope — write list)
- `src-tauri/src/modules/skills/sedimentation.rs`  (modify — 替换 stub 为真实实现)
- `src-tauri/tests/skill_evolution.rs`              (new — 集成测试)

## Reads
- `src-tauri/src/modules/memory/llm.rs`             (UtilityLlm trait + MockUtilityLlm)
- `src-tauri/src/modules/api/mod.rs`                (InputMessage / InputContentBlock)
- `src-tauri/src/modules/skills/sedimentation.rs`   (当前 stub，了解占位常量)

## Contract
- 不改 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- `sedimentation.rs` 不落盘、不调 `skills/manager`（draft-only 语义）

## Out of Scope
- ❌ 不实施 skill 去重（留给 FEAT-SE-002）
- ❌ 不把 draft 写入磁盘或 skills/manager（draft-only）
- ❌ 不修改 `skills/manager/mod.rs`
- ❌ 不集成宪法层 (FEAT-SE-003)
- ❌ 不改 `prompt_planner/` 注入逻辑

## Verify
- `./scripts/pack run FEAT-SE-001`
- `cargo test -p if2ai-tauri --test skill_evolution 2>&1 | grep -E "PASS|FAIL"`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
