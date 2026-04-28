# SKILL-004: Skill Frontmatter / WhenToUse / AllowedTools

## Status
- State: done

## Goal
Promote skill frontmatter from passive documentation into runtime-visible skill
metadata for provider context and Run Inspector snapshots.

## Spec (verifiable)
- `whenToUse` / `when_to_use`, `allowedTools` / `allowed_tools`, and
  `modelHint` / `model_hint` are parsed -> test
  `skill_004_parses_frontmatter_runtime_metadata`.
- Parsed metadata is attached to `SkillResolutionCandidate`.
- Loaded skill prompt blocks include runtime metadata and activation evidence.
- Remote/quarantine/untrusted skills remain blocked by AWL-003 gates.

## Files (scope)
- `src-tauri/src/modules/runtime/contracts/agent_loop.rs`
- `src-tauri/src/modules/application/turn_service/work_loop.rs`
- `src/transport/contracts.ts`

## Done
- Trusted loaded skills carry when-to-use, allowed-tools, model-hint, and
  activation evidence metadata into provider context.
