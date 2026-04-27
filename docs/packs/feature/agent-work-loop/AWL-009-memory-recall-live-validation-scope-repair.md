# AWL-009: Memory Recall Live Validation & Scope Repair

## Status
- State: active

## Goal
Close the live gap where Memory Browser contains enough evidence for a personal
memory answer, but the agent still answers as if it does not know.

## Spec (verifiable)
- Family/food questions still use scoped recall plus bounded same-project
  fallback from AWL-007.
- Same-project older-session facts can jointly answer "what do we both like to
  eat?".
- `memory_recall` output includes deterministic common-food evidence when the
  recalled facts support it.
- Unprojected older-session facts remain blocked from fallback.
- No memory schema, IPC command name, or storage truth source changes.

## Files (scope)
- `docs/design-docs/agent-work-loop/AWL-009-memory-recall-live-validation-scope-repair.md`
- `src-tauri/src/modules/tools/builtin/memory_recall.rs`

## Reads
- `docs/packs/feature/agent-work-loop/AWL-007-memory-recall-result-dedupe-grounding.md`
- `docs/packs/feature/agent-work-loop/AWL-008-memory-scope-visibility-recall-diagnostics.md`
- `src-tauri/src/modules/tools/builtin/memory_recall.rs`
- `src-tauri/src/modules/memory/mod.rs`

## Contract
- Deterministic extraction only; no LLM reranking.
- Do not broaden recall across projects.
- Do not include old unprojected session entries in fallback.
- Keep TurnService and memory provider as the existing truth sources.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml memory_recall -- --nocapture`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- The live family/food pattern has test coverage and Pack verification remains clean.
