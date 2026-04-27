# AWL-003: Skill Auto-Load Runtime

## Status
- State: active

## Goal
Make `SkillResolutionPlan` load trusted local skills into provider context before
the loop starts. Remote/community skills may be discovered, but must stay quarantined
or proposed until explicitly approved.

## Spec (verifiable)
- Active builtin/review-passed local skills are loaded into a dedicated skill prompt block -> test `tests::skill_auto_load_trusted_local_skill`.
- Remote/community candidates are never silently loaded -> test `tests::skill_auto_load_blocks_remote_candidate`.
- Loader failure records a warning and the run still finalizes -> test `tests::skill_auto_load_failure_reports_warning`.
- `FinalRunReport` includes loaded and blocked skill names -> test `tests::final_report_lists_skill_resolution`.

## Files (scope)
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src-tauri/src/modules/application/turn_service/mod.rs`
- `src-tauri/src/modules/application/turn_service/stream_task.rs`
- `src-tauri/src/modules/application/turn_service/stream_finalize.rs`
- `src-tauri/src/modules/application/prompt_planner/*`
- `src-tauri/src/modules/runtime/contracts/agent_loop.rs`
- `src-tauri/src/modules/tools/builtin/skill_search.rs`
- `src-tauri/src/modules/tools/builtin/skill_view.rs`
- `src-tauri/src/modules/tools/builtin/skill_find.rs`
- `src/runtime-projection/types.ts`

## Reads
- `docs/design-docs/agent-work-loop/AWL-003-skill-auto-load-runtime.md`
- `docs/design-docs/postCLI/Skill-Control-Plane-v2.md`
- `src-tauri/src/modules/skills/*`

## Contract
- Auto-load only builtin, active local, or review-passed skills.
- Do not install, enable, or trust remote skills without user approval.
- Skill context must be represented as structured prompt contribution, not user text.
- Preserve current skill tool safety behavior.

## Out of Scope
- Skill marketplace install flow.
- Full skill settings redesign.
- LoopDelegate extraction.

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml skill_auto_load -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml application::turn_service -- --nocapture`
- `npm run build:web`

## Done
- Verify commands pass.
- `REGISTRY.md` moves this pack to done with date.
