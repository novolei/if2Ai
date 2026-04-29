# FEAT-DK-003: 域知识自动贡献

## Status
- State: active

## Goal
将 `skills/domain_knowledge.rs` 重构为 `skills/domain_knowledge/` 目录，
新增 `contributor.rs`：用 `UtilityLlm` 从对话历史异步抽取候选 `DomainKnowledgeEntry`，
复用 `evaluate_constitution` 过滤恶意内容（`verify_knowledge_safety`）。Draft-only，不落盘。

## Spec
- 空 history → 返回空 vec → `tests::domain_knowledge::empty_history_yields_no_candidates`
- history 含明确 selector 描述 → 返回 ≥ 1 个 WebsiteDomain 候选 → `tests::domain_knowledge::selector_history_yields_website_domain`
- `MockUtilityLlm::complete` 返回 Err → 优雅降级返回空 vec，不 panic → `tests::domain_knowledge::llm_error_degrades_gracefully`
- 含恶意 selector（含 `rm -rf /`）的 entry → `verify_knowledge_safety` 返回 `safe == false` → `tests::domain_knowledge::malicious_selector_fails_safety_check`

## Files (scope — write list)
- `src-tauri/src/modules/skills/domain_knowledge/mod.rs`         (new — DK-001 内容搬入此处)
- `src-tauri/src/modules/skills/domain_knowledge/contributor.rs` (new — extract + verify 逻辑)
- `src-tauri/src/modules/skills/domain_knowledge.rs`             (modify → `pub mod domain_knowledge;` 路由或删除，视重构方式)
- `src-tauri/src/modules/skills/mod.rs`                          (modify — 路径更新为目录)
- `src-tauri/tests/domain_knowledge.rs`                          (modify — 加上述 4 个测试)

## Reads
- `src-tauri/src/modules/skills/domain_knowledge.rs`             (DK-001 产出，DomainKnowledgeEntry 定义)
- `src-tauri/src/modules/skills/guard/mod.rs`                    (evaluate_constitution 签名)
- `src-tauri/src/modules/skills/sedimentation/mod.rs`            (SkillDraft surrogate 构造参考)
- `src-tauri/src/modules/memory/llm.rs`                          (UtilityLlm + MockUtilityLlm)
- `src-tauri/src/modules/api/types.rs`                           (InputMessage)

## Contract
- 不改 IPC 命令名 / 事件字面量 (I3)
- 不改 sqlite schema / localStorage key (I4)
- 不引入 Pack 未声明的新 Cargo dependency
- `cargo clippy -D warnings` 通过 (I7)
- 全部为 draft-only / in-memory，不落盘到 ~/.if2ai/

## Out of Scope
- ❌ 不接入 stream_finalize 钩子（wiring 留给后续 Pack）
- ❌ 不实现 verification_evidence 的执行验证（Module K 留给后续 Pack）
- ❌ 不落盘
- ❌ 不改前端 contracts.ts
- ❌ 不修改 evaluate_constitution 本身（只调用，不改签名）

## Depends on
- FEAT-DK-001 (DomainKnowledgeEntry / KnowledgeStore)
- FEAT-SE-003 (evaluate_constitution / ConstitutionRule)

## Verify
- `./scripts/pack run FEAT-DK-003`
- `cargo test -p if2ai-tauri --test domain_knowledge 2>&1 | grep -E "PASS|FAIL"`

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
