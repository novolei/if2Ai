# DW-004: Work Loop DK Lookup（resolve_skill_plan → lookup_for_skill_resolution）

## Status
- State: active

## Goal
在 `work_loop.rs::resolve_skill_plan` 内调用 `lookup_for_skill_resolution(user_query, domain_knowledge_store)`，把返回的 `Vec<PromptContribution>` 追加到 skill plan 的贡献列表中（优先级最低，不替换现有 skill blocks）。成功时广播 `DomainKnowledge` evolution event；失败时 `tracing::warn` + 空列表降级。

## Spec (verifiable)
- `resolve_skill_plan` 调用后 `PromptContribution` 列表包含 DK lookup 结果 → `tests::dk_lookup::dk_contributions_appended_to_plan`
- DK store 返回空 vec 时 plan 不受影响（无多余空 block）→ `tests::dk_lookup::empty_dk_does_not_pollute_plan`
- lookup 返回 `Err` 时 `tracing::warn`，plan 继续正常返回 → `tests::dk_lookup::dk_lookup_error_falls_back`
- `lookup_for_skill_resolution` 成功后广播 `DomainKnowledge` envelope → `tests::dk_lookup::dk_lookup_emits_event`
- `IF2AI_DISABLE_DK_LOOKUP=1` 时跳过调用 → `tests::dk_lookup::env_flag_skips_dk_lookup`

## Files (scope — write list)
- `src-tauri/src/modules/application/turn_service/work_loop.rs`    (modify — 在 resolve_skill_plan 内调用 lookup)
- `src-tauri/tests/dk_lookup.rs`                                   (modify — 补 5 个 tests)

## Reads (read-only)
- `src-tauri/src/modules/application/turn_service/dk_lookup_hook.rs`  (lookup_for_skill_resolution 签名)
- `src-tauri/src/modules/skills/domain_knowledge/mod.rs`              (DomainKnowledgeStore trait)
- `src-tauri/src/modules/runtime/evolution_emitter.rs`                (emit_evolution_event — WU-001)
- `src-tauri/src/modules/application/turn_service/work_loop.rs`       (resolve_skill_plan 函数体 + 返回类型)

## Contract (review must check)
- 不改 `resolve_skill_plan` 对外签名（I1）
- DK contributions 追加在现有 skill blocks 之后（不调整已有顺序）
- lookup 失败不阻塞 skill plan 返回（failure isolation）
- `IF2AI_DISABLE_DK_LOOKUP=1` 检查在调用前（I3 事件名不变）

## Out of Scope
- ❌ 不修改 `lookup_for_skill_resolution` 内部实现（dk_lookup_hook.rs 只读）
- ❌ 不改 skill resolution 的向量检索逻辑（FEAT-SE-004 只读）
- ❌ 不改 prompt planner 的 block 优先级排序
- ❌ 不实现 UI 展示组件

## Depends on
- WU-001（emit_evolution_event）
- WU-008（dk_lookup_hook.rs helper done）

## Verify
- `./scripts/pack run DW-004`
- `cargo test --test dk_lookup`

## Done
- Verify 全 PASS；REGISTRY 状态改为 done
