# AWL-006: Memory Recall Intent Enforcement

## Status
- State: active

## Goal
Symptom: asking "你记得关于我的什么事情" can end after a filler sentence without
calling `memory_recall` or producing a useful final summary.

Ensure memory-introspection requests are routed as evidence-seeking work, not
no-tool direct answers.

## Spec (verifiable)
- Memory-introspection text does not route to `DirectAnswer` -> test `tests::memory_recall_intent_routes_to_direct_execute`.
- Memory-introspection turns expose only read-only memory tools in the direct execution path -> test `tests::memory_recall_intent_exposes_memory_read_tools`.
- Provider prompt includes an explicit instruction to call memory recall before answering -> test `tests::memory_recall_intent_prompt_requires_recall_tool`.
- After a memory recall succeeds, the next loop iteration must force a final answer instead of recalling again -> test `tests::memory_recall_success_forces_final_response`.
- A filler "checking memory" response with no tool evidence is not reported as `Completed` -> test `tests::memory_recall_no_tool_filler_report_failed`.

## Files (scope)
- `docs/design-docs/agent-work-loop/AWL-006-memory-recall-intent-enforcement.md` (new)
- `src-tauri/src/modules/application/turn_service/mod.rs`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src-tauri/src/modules/runtime/recoverability.rs`
- `src-tauri/src/modules/runtime/stream_outcome.rs`

## Reads
- `docs/design-docs/agent-work-loop/AWL-006-memory-recall-intent-enforcement.md`
- `docs/packs/feature/agent-work-loop/AWL-002-work-loop-router-enforcement.md`

## Contract
- Keep `TurnService` as the only runtime entry point.
- Do not change IPC command names, runtime event names, or sqlite schema.
- Do not introduce new dependencies.
- Preserve AWL-002 DirectAnswer no-tool behavior for ordinary direct questions.

## Out of Scope
- Memory database schema changes.
- New memory UI.
- MCP workbench or remote memory providers.
- Rewriting provider clients or the full streaming loop.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml memory_recall_intent -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml application::turn_service -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run build:web`

## Done
- Verify commands pass.
- Code reviewer confirms the symptom cannot silently complete without memory evidence.
