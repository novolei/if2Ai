# AWL-008: Memory Scope Visibility & Recall Diagnostics

## Status
- State: active

## Goal
Make the difference between "visible in Memory Browser" and "available to the
agent's current scoped recall" obvious. Users should understand why an entry can
appear in the browser's `all` view but not answer a chat question.

## Spec (verifiable)
- Memory cards show an agent recall visibility diagnostic.
- Global entries are marked as agent-visible.
- Current-session entries are marked as agent-visible.
- Current-project entries are marked as agent-visible.
- Same-project older-session entries are marked as bounded fallback candidates,
  not normal scoped recall.
- Unprojected older-session and other-project entries are marked browser-only.
- The browser explains that `all` is a management view, not the agent's scoped
  recall truth.

## Files (scope)
- `docs/design-docs/agent-work-loop/AWL-008-memory-scope-visibility-recall-diagnostics.md`
- `src/components/memory/MemoryBrowser.tsx`
- `src/components/memory/MemoryCard.tsx`
- `src/components/memory/memoryScopeDiagnostics.ts` (new)
- `src/components/memory/memoryScopeDiagnostics.test.ts` (new)

## Reads
- `docs/packs/feature/agent-work-loop/AWL-007-memory-recall-result-dedupe-grounding.md`
- `src-tauri/src/modules/memory/mod.rs`
- `src-tauri/src/modules/tools/builtin/memory_recall.rs`
- `src/components/memory/MemoryBrowser.tsx`

## Contract
- Do not change memory storage schema.
- Do not change memory IPC command names.
- Do not introduce a second memory truth source.
- Frontend diagnostics must mirror backend scoped recall rules.

## Out of Scope
- Memory migration or promotion policy changes.
- Semantic reranking.
- New backend commands.

## Verify
- `npm run test -- memoryScopeDiagnostics`
- `npm run build:web`
- `cargo test --manifest-path src-tauri/Cargo.toml memory_recall -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`

## Done
- Verify commands pass or unrelated existing blockers are documented.
