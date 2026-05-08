# 2026-05-05 — Wave A.0 compile health gate

> Source: `origin/feat/in-flight-A-memory @ 8674725` rebased onto `vnext @ a8e270b`
> Worktree: `/Users/ryanliu/Documents/IfAI/if2Ai/.claude/worktrees/wt-mem-A0`
> Branch (worktree-only, do not push): `wt-mem-A0-compile-gate`

## Setup

```
git worktree add .claude/worktrees/wt-mem-A0 -b wt-mem-A0-compile-gate a8e270b
cd .claude/worktrees/wt-mem-A0
git cherry-pick origin/feat/in-flight-A-memory      # commit 8674725
```

vnext base: `a8e270b` (docs(plan): memory + evolution product replan).
Cherry-pick result: `c705f16` — single commit replaying the 9.8K-LOC delta.

## Cherry-pick / merge resolution

Cherry-pick succeeded **with zero textual conflicts** — no `--ours` / `--theirs` resolution needed.

Why no conflicts: the in-flight branch's diff against its merge-base did not actually re-touch the same lines vnext now owns; the deletion-shaped Δ rows in the replan §2.5 were measured against an older vnext snapshot. With current vnext (post DT-01 + b390580 + a8e270b), the 3 hot files are simply not part of the cherry-picked patch hunk set, so they remain as on vnext.

Verified post-cherry-pick:

| Hot file | LoC | Expected (vnext) | Result |
|---|---|---|---|
| `src-tauri/src/modules/harness/agent_loop_integration.rs` | 334 | preserved | OK |
| `src-tauri/src/modules/harness/runlog_projection.rs` | 1220 | preserved/expanded | OK |
| `src-tauri/src/modules/skills/sedimentation/mod.rs` | 401 | preserved | OK |

The replan §2.5 "HARD COLLISION" assessment is **superseded** for the current rebase target; deletions never landed in the cherry-picked diff.

Files staged from in-flight (`git show --stat c705f16`): 45 files changed, 9784 insertions(+), 92 deletions(-). 21 new files (cognitive, conflict, daydream/*, evolution/*, forgetting, graph, quality, 4 React panels, 2 IPC modules).

## `cargo check --workspace` raw output

Command: `cargo check --workspace --manifest-path Cargo.toml` (and equivalently `cargo check --manifest-path src-tauri/Cargo.toml --lib`).

Exit status: **non-zero** — `error: could not compile if2ai-backend (lib) due to 3 previous errors`.

Total error lines: **3** (plus 1 summary line). Total warning lines: **0** (compilation halted at hard errors before warning emission for downstream code).

Verbatim error set:

```
error[E0004]: non-exhaustive patterns: `MemoryInjectionSectionKind::Procedural` not covered
   --> src-tauri/src/modules/application/prompt_planner/planner.rs:348:50
    |
348 |             let (priority, is_sensitive) = match section.kind {
    |                                                  ^^^^^^^^^^^^ pattern `MemoryInjectionSectionKind::Procedural` not covered
note: `MemoryInjectionSectionKind` defined here
   --> src-tauri/src/modules/application/memory_injection_service.rs:78:10
    | 81 |     Procedural,
    |     ---------- not covered

error[E0004]: non-exhaustive patterns: `MemoryInjectionSectionKind::Procedural` not covered
  --> src-tauri/src/modules/application/prompt_planner/block.rs:82:15
   | 82 |         match kind {

error[E0004]: non-exhaustive patterns: `MemoryInjectionSectionKind::Procedural` not covered
   --> src-tauri/src/modules/application/prompt_planner/planner.rs:441:11
   | 441 |     match kind {
```

Site context (vnext post-cherry-pick):

- `block.rs:82` — `PromptBlockKind::from_memory_section`, 4 arms (Pinned/Compiled/Rules/Retrieved). `PromptBlockKind` itself has **no** `MemoryInjectionProcedural` variant defined; A.1 must add one.
- `planner.rs:348` — `(priority, is_sensitive)` tuple match in block-emit loop. Need a sensible priority for Procedural (proposal: 65, between Rules=60 and Compiled=70; `is_sensitive=false`).
- `planner.rs:441` — `title_for_memory_section`. Need `"memory_procedural"` (or similar) string literal.

## Errors by root cause

| Cat | Description | Count | Notes |
|---|---|---|---|
| (a) B-branch `CacheHint` missing | 0 | not encountered — `CacheHint` plumbing in `inject.rs` either guarded or unused at this rebase point |
| (b) Deleted hot file referenced | 0 | hot files preserved; replan §2.5 collision did not materialize |
| (c) Match arm not exhaustive (`Procedural` enum arm missing) | **3** | all in `prompt_planner/{block,planner}.rs`; one consumer not updated when `MemoryInjectionSectionKind::Procedural` was added in `memory_injection_service.rs:81` |
| (d) Type mismatch / provider-trait extension (`KeepBothWithFlag`) | 0 | not surfaced — provider trait gap is runtime-incomplete (logs only) but compiles |
| (e) Missing imports | 0 | none |
| (f) Other | 0 | |

## A.1 backlog (ordered fix list)

1. **Add `PromptBlockKind::MemoryInjectionProcedural` variant** in `src-tauri/src/modules/application/prompt_planner/block.rs` (and its render/dispatch consumers). **Effort: ~10 LOC, ~15 min.** Mirrors existing `MemoryInjectionPinned` / `MemoryInjectionCompiled` / `MemoryInjectionRules` / `RetrievedMemory` siblings.

2. **Extend `block.rs:82` `from_memory_section`** with `MemoryInjectionSectionKind::Procedural => Self::MemoryInjectionProcedural`. **Effort: 1 LOC, 2 min** (after step 1).

3. **Extend `planner.rs:348` priority/is_sensitive match** with `MemoryInjectionSectionKind::Procedural => (65, false)` (priority value to confirm during A.1 brainstorming — between Rules=60 and Compiled=70 makes semantic sense given §3.8 procedural-rule positioning). **Effort: 1 LOC + brainstorming on priority value, ~10 min.**

4. **Extend `planner.rs:441` title map** with `MemoryInjectionSectionKind::Procedural => "memory_procedural"`. **Effort: 1 LOC, 2 min.**

5. **Verify `KeepBothWithFlag` provider-trait gap** (`memory/conflict.rs`) — replan §3.7 flagged this as runtime-incomplete (only logs) due to missing `MemoryProvider::increment_contradiction_count`. Compiles today; A.1 SHIP-LITE plan is to either (a) add the trait method + impls or (b) ship without `KeepBothWithFlag` and document. **Effort: ~30 LOC + 2 provider impls (sqlite, lancedb), ~1–2 hr.**

Total A.1 unblock effort to clean compile: **<30 min** for items 1–4. Item 5 is product-decision-bound, not compile-blocking.

## Verdict

**Partial — near-clean.** The branch is **3 trivial match-arm fixes** away from compiling. There are no:

- structural collisions with vnext hot files (DT-01 / sedimentation work intact)
- missing B-branch types (`CacheHint` not surfaced as a blocker)
- import resolution failures
- duplicate symbol definitions

The original brief's "3 todo!() blocking compile" claim has been **operationally confirmed** in shape (3 missing match-arm cases) but the location was misidentified — the gaps are in `prompt_planner`, not `memory_injection_service` / `memory_recall_assembler` / `memory/inject.rs`.

## Recommended next action

1. Proceed to **Wave A.1 brainstorming** (`superpowers:brainstorming`) per replan §4 / §6 — focus on:
   - Priority value for Procedural section (65 vs 70 vs 75)
   - `KeepBothWithFlag` SHIP vs DEFER decision
   - Forgetting `lambda` / `archive_threshold` defaults
2. First A.1 PR can incorporate items 1–4 above as the **compile-fix slice** (≤ 20 LOC), unblocking subsequent A.1 PRs that need `cargo test`.
3. Worktree at `.claude/worktrees/wt-mem-A0` left intact for inspection. No commits beyond the cherry-pick; not pushed. Safe to delete via `git worktree remove` when no longer needed.

## Artifacts

- Worktree: `/Users/ryanliu/Documents/IfAI/if2Ai/.claude/worktrees/wt-mem-A0`
- Cherry-pick commit (worktree-local): `c705f16`
- Raw cargo logs: `/tmp/wt-mem-A0-cargo-check.log`, `/tmp/wt-mem-A0-workspace.log`
