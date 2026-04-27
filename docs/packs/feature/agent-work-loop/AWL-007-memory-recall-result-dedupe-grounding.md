# AWL-007: Memory Recall Result Dedupe & Answer Grounding

## Status
- State: active

## Goal
Memory recall can return the same fact through both canonical records and
`Episode:*` records, causing repeated tool result rows and repeated final-answer
bullets.

Return deduplicated recall evidence and tell the model to answer from that
deduplicated evidence only.

This slice also closes the adjacent live gap found during validation: personal
memory questions such as "what do my son and I both like to eat?" must enter the
memory recall path, and `memory_recall` may use same-project fallback queries so
facts visible in the Memory Browser are not missed solely because they were
written by an older session.

## Spec (verifiable)
- `memory_recall` hides duplicate Episode/canonical facts -> test `memory_recall_dedupes_episode_and_canonical_fact`.
- Recall output includes evidence counts and grounding guidance -> test `memory_recall_output_includes_grounding_header`.
- Empty recall still returns a clear grounded no-results message -> test `memory_recall_empty_output_is_grounded`.
- Memory-introspection prompt tells the model to avoid repeating duplicate recall facts -> test `memory_recall_prompt_asks_for_deduped_answer`.
- Family/food memory questions route to memory recall instead of DirectAnswer -> test `memory_recall_intent_routes_family_food_question_to_direct_execute`.
- Family/food recall adds bounded fallback search terms and keeps only same-project cross-session entries -> tests `memory_recall_family_food_query_adds_fallback_terms` and `memory_recall_project_fallback_keeps_same_project_cross_session_entry`.

## Files (scope)
- `docs/design-docs/agent-work-loop/AWL-007-memory-recall-result-dedupe-grounding.md` (new)
- `src-tauri/src/modules/tools/builtin/memory_recall.rs`
- `src-tauri/src/modules/application/turn_service/work_loop.rs`

## Reads
- `docs/packs/feature/agent-work-loop/AWL-006-memory-recall-intent-enforcement.md`
- `docs/design-docs/agent-work-loop/AWL-006-memory-recall-intent-enforcement.md`

## Contract
- Keep memory storage schema unchanged.
- Keep IPC command names and runtime event names unchanged.
- Do not change memory provider ranking/query semantics; fallback happens in the
  tool adapter after the canonical provider call.
- Do not introduce a second memory truth source.

## Out of Scope
- Memory database migrations.
- Frontend memory inspector UI.
- Semantic embedding reranking.
- Memory write/merge policy changes.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml memory_recall -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml application::turn_service -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- Verify commands pass.
- Code reviewer confirms repeated recall facts are deduped and grounded.
