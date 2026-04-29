# FEAT-SE-003: 宪法层 (Constitution Layer)

## Status
- State: active

## Goal
在 `skills/guard/constitution.rs` 实现 8 条硬规则检查，每条规则为
`ConstitutionRule { id, severity, predicate: fn(&SkillDraft) -> Option<Violation> }`，
暴露 `pub const RULES: &[ConstitutionRule]` 与 `pub fn evaluate_constitution(draft: &SkillDraft) -> Vec<Violation>`。
8 条规则：禁止删除根目录、禁止 sudo/管理员命令、禁止覆盖 `~/.if2ai/` 配置、
禁止 fork bomb 模式、禁止网络扫描、禁止泄露 secrets、禁止 eval/exec untrusted、禁止键盘记录。

## Spec
- 8 条规则每条至少 1 个命中反例 → `tests::skill_evolution::constitution_each_rule_has_positive`
- clean draft body → `evaluate_constitution` 返回空 violations → `tests::skill_evolution::clean_draft_passes_constitution`
- 含 `rm -rf /` 的 draft 命中 ROOT_DELETE 规则 → `tests::skill_evolution::malicious_rm_rf_flagged`
- `violations` 按 `Severity` 降序排列（Critical > High > Medium）→ `tests::skill_evolution::violations_sorted_by_severity`
- 空 body draft → 不 panic，返回空 violations → `tests::skill_evolution::empty_body_draft_safe`

## Files (scope — write list)
- `src-tauri/src/modules/skills/guard/constitution.rs`   (new — ConstitutionRule + Violation + RULES + evaluate_constitution)
- `src-tauri/src/modules/skills/guard/mod.rs`            (modify — `pub mod constitution;` + re-export Violation / evaluate_constitution)
- `src-tauri/tests/skill_evolution.rs`                   (modify — 新增 5 条宪法测试)

## Reads
- `src-tauri/src/modules/skills/guard/threat_patterns.rs`  (复用模式 regex + ThreatPattern struct)
- `src-tauri/src/modules/skills/guard/mod.rs`              (了解现有 SkillsGuard 结构，避免命名冲突)
- `src-tauri/src/modules/skills/sedimentation.rs`          (SkillDraft struct 定义，SE-001 完成后)

## Contract
- 不改 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- 不改 `SkillsGuard::scan_command` 接口（constitution 是独立检查路径）

## Out of Scope
- ❌ 不接入 sedimentation 主流程（constitution 调用留给后续 wire-up Pack）
- ❌ 不改 `SkillsGuard` 现有接口或 threat_patterns 已有规则
- ❌ 不实施 LLM-as-judge 宪法评估（规则为静态正则，不调 LLM）
- ❌ 不改 `prompt_planner/` 或前端 contracts.ts

## Depends On
- FEAT-SE-001

## Verify
- `./scripts/pack run FEAT-SE-003`
- `cargo test -p if2ai-tauri --test skill_evolution 2>&1 | grep -E "PASS|FAIL"`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
