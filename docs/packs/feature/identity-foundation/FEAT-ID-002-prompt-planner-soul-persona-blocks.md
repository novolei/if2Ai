# FEAT-ID-002: Prompt Planner Soul / Persona Blocks

## Status

- State: `active`
- Owner: `@executor`
- Depends On: `FEAT-ID-001`
- Last Updated: `2026-04-22`

---

## Goal

将 `ResolvedIdentity` 结构化接入 prompt planner，使 Soul 与 Persona 以独立 block 的形式进入 `PromptPlan`，并进一步把 `scenario profile` 也纳入统一的 prompt control plane 语义。

---

## Why Now

1. Identity 的产品价值最终必须体现在 prompt 行为上。
2. If2Ai 当前已经有 `PromptPlan` / `PromptBlock` / diagnostics 基础，这是最稳的接入层。
3. 当前 `PromptBlockKind::Persona` 已预留但未真正打通，是天然扩展点。

---

## Spec

1. `PromptBlockKind` 新增 `Soul`，并正式启用 `Persona` block 生成  
   → 测试 `prompt_planner::tests::identity_block_kinds_are_emitted`

2. `BuildPromptPlanRequest` 新增 identity 输入字段，至少包含 `resolved_identity` 或等价结构  
   → 测试 `prompt_planner::tests::build_request_accepts_identity_input`

3. `build_prompt_plan` 会在 `System` 后插入 `Soul` block  
   → 测试 `prompt_planner::tests::soul_block_appears_after_system`

4. 当存在 Persona 时，在 `Soul` 后插入 `Persona` block  
   → 测试 `prompt_planner::tests::persona_block_appears_after_soul`

5. diagnostics 必须记录 identity block kinds，且 redacted preview 不泄露敏感实现细节  
   → 测试 `prompt_planner::tests::diagnostics_include_identity_blocks`

6. 当 Persona 与 Soul 冲突时，不得让 Persona 覆盖 Soul；必须以 resolver 结果为准  
   → 测试 `prompt_planner::tests::persona_cannot_override_soul_contract`

7. `scenario profile` 必须以独立 block 进入 `PromptPlan`，而不是被揉进 system prompt  
   → 测试 `prompt_planner::tests::scenario_block_is_emitted_for_non_default_profile`

8. `TurnService::prepare_chat_inputs` 或等价路径能够将 resolved identity 与 scenario profile 一并传入 prompt planner  
   → 测试 `application::tests::prepare_chat_inputs_passes_identity_to_prompt_planner`

---

## Files (scope)

- `src-tauri/src/modules/application/prompt_planner/mod.rs`
- `src-tauri/src/modules/application/prompt_planner/scenario.rs` (new if needed)
- `src-tauri/src/modules/application/turn_service/mod.rs`
- `src-tauri/src/modules/application/turn_service/run.rs`
- `src-tauri/src/modules/application/turn_service/stream.rs` or equivalent call site
- `src-tauri/src/modules/identity/prompt.rs` (new if needed)

---

## Reads

- `src-tauri/src/modules/identity/**`
- `src-tauri/src/modules/runtime/prompt/mod.rs`
- `docs/design-docs/identity-soul-persona-memory-foundation.md`
- `docs/design-docs/prompt-control-plane-foundation.md`
- `docs/packs/feature/prompt-planner-alignment/MIG-006-prompt-contribution-mechanism.md`

---

## Contract

- Identity block 必须进入 `PromptPlan.blocks`
- 不允许把 Soul / Persona 直接拼进 `SystemPromptBuilder` 的 static section
- 不允许把 scenario profile 直接拼进 base system prompt
- `PromptPlan.join_into_text()` 的 block 顺序必须稳定
- diagnostics 必须能体现 identity block 存在
- 若没有 Persona，PromptPlan 仍合法

---

## Implementation Notes

1. 推荐新增 `PromptBlockKind::Soul`，不要把 Soul 借道 `ActiveStrategyOverlay` 或 append section 混入。
2. 推荐新增 identity block renderer：
   - `render_soul_block(&SoulDefinition) -> String`
   - `render_persona_block(&PersonaDefinition) -> String`
3. 推荐同步引入 `scenario profile` block renderer，覆盖 `chat / coding / research / planning / review`。
4. 如果 `ResolvedIdentity` 里只保存 id/version，则 planner 可通过 registry 再拿 definition；但长期仍应优先保持调用层显式传递，避免 planner 深耦合 registry。
5. 本 Pack 是 prompt control plane 的第一步，不负责 memory/tool/utility/coordinator prompt catalog 的完整实现。

---

## Verify

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- `cargo test --manifest-path src-tauri/Cargo.toml prompt_planner`

---

## Acceptance

- PromptPlan 中可见 Soul / Persona block
- PromptPlan 中可见非默认 scenario profile block
- diagnostics 中可见 identity block kinds
- identity block 顺序稳定且可测试
- call site 已将 identity 显式接入 planner

---

## Out Of Scope

- 不接 Settings UI
- 不接 session persistence
- 不接 memory tagging
- 不实现 project/request identity override
- 不实现完整 prompt control panel UI
- 不实现 utility prompt catalog / tool prompt catalog
