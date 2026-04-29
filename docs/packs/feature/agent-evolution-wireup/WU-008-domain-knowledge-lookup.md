# WU-008: Domain Knowledge Lookup（DK-001 Wire-up）

## Status
- State: active

## Goal
在 `turn_service/work_loop.rs::resolve_skill_plan` 内，在现有 skill resolution 后调用 `KnowledgeStore::lookup_domain_knowledge(query)` 拉取相关域知识条目，并通过 `PromptContribution` 注入 prompt planner（source="domain_knowledge", priority=70），广播 `DomainKnowledge`（family="lookup"）event。lookup 失败或返回空时静默跳过，不影响 skill resolution 结果。

## Spec (verifiable)
- `lookup_domain_knowledge` 在 resolve_skill_plan 后被调用 → `tests::dk_lookup::lookup_called_after_skill_resolution`
- lookup 成功时返回 PromptContribution 注入 external_contributions → `tests::dk_lookup::knowledge_entries_produce_contribution`
- lookup 失败时 resolve_skill_plan 正常返回（不 propagate error）→ `tests::dk_lookup::lookup_failure_does_not_propagate`
- 广播 `DomainKnowledge` envelope（family="lookup"）→ `tests::dk_lookup::lookup_emits_domain_knowledge_event`
- `IF2AI_DISABLE_DK_LOOKUP=1` 时跳过 lookup → `tests::dk_lookup::env_flag_disables_lookup`

## Files (scope — write list)
- `src-tauri/src/modules/application/turn_service/work_loop.rs`   (modify — resolve_skill_plan 内调用 lookup)
- `src-tauri/tests/dk_lookup.rs`                                  (new — 5 unit tests)

## Reads (read-only)
- `src-tauri/src/modules/skills/domain_knowledge/mod.rs`          (KnowledgeStore::lookup_domain_knowledge)
- `src-tauri/src/modules/application/prompt_planner/block.rs`     (PromptContribution 结构)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`            (emit_evolution_event — WU-001)

## Contract (review must check)
- `PromptContribution` 的 source 必须为 `"domain_knowledge"`，priority=70（低于 system=100 / mini_index=95 / memory=80）
- lookup query 直接使用 user message text（不做额外 embedding，DK-001 内部处理）
- 不改 `resolve_skill_plan` 签名（I1）
- 不新增 IPC 命令（I3）

## Out of Scope
- ❌ 不修改 KnowledgeStore 内部检索逻辑
- ❌ 不向 UI 暴露 domain knowledge 查询结果
- ❌ 不接入 DK-003 自动贡献逻辑（DK-003 wire-up 在 WU-003 完成）
- ❌ 不实现知识条目 CRUD 命令

## Depends on
- WU-001（emit_evolution_event）
- FEAT-DK-001（KnowledgeStore 实现 done）

## Verify
- `./scripts/pack run WU-008`
- `cargo test --test dk_lookup`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
