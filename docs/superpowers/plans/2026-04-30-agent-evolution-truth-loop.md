# Agent Evolution — Self-Evolving Truth Loop

> **For agentic workers:** REQUIRED SUB-SKILL — `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Each iteration is one bite-sized loop pass.

**Goal:** Drive `.qoder/specs/if2ai-agent-evolution-report.md` to 100% truth alignment with code by running a permanent audit → fix → verify loop. No 二次真相 (secondary truth) drift, no honest stubs masquerading as done.

**Architecture:** Each iteration is a single, bounded delta: re-audit one slice, write one fact-source patch, ship one verification commit. Loop terminates only when the **Exit Gate** passes.

**Tech Stack:** Rust (`src-tauri/**`), TypeScript (`src/**`), Pack docs (`docs/packs/**`), spec (`.qoder/specs/**`).

---

## 0. Source-of-Truth Hierarchy

When the spec, REGISTRY, Pack files, and code disagree, **code wins**. All other artifacts must conform to code, in this order:

1. **Code** (Rust/TS production hot paths — not tests, not module-private helpers)
2. **REGISTRY.md** (board-level state)
3. **Pack files** (`State:` field)
4. **Spec** (`.qoder/specs/if2ai-agent-evolution-report.md`)
5. **Gap reports** (`docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-YYYY-MM-DD.md`)

If a code path is "honest stub" (real call site + placeholder dependency), it is **NOT done** for spec purposes. It is `landed-stub` — a distinct state introduced by this loop (see §3).

---

## 1. Per-Iteration Steps (rigid; do not adapt)

Each loop iteration:

1. **Read latest gap report** (the one with the most recent date in `docs/design-docs/agent-evolution/`).
2. **Re-verify ≤ 1 slice** that the prior report flagged. Use `Grep` / `Read` against current code; do not trust prior findings older than one iteration.
3. **Pick exactly ONE highest-leverage gap** for this iteration.  Apply `superpowers:systematic-debugging` Phase 1–2 to confirm root cause.
4. **Write or extend a Pack** under `docs/packs/feature/agent-evolution-wireup/` (or `agent-evolution-deeper/` for sub-step wiring). Use `superpowers:writing-plans` if the Pack is multi-step.
5. **Implement via** `superpowers:test-driven-development` — failing test first.
6. **Verify** via `superpowers:verification-before-completion` — run targeted tests, full `cargo check`, and at minimum the matching Pack's `./scripts/pack verify <PACK-ID>`.
7. **Update truth artifacts** in this exact order:
   - Pack file `State:` field
   - `docs/packs/REGISTRY.md` row
   - Append a "Iteration N (YYYY-MM-DD)" delta block to the latest gap report
   - If spec contradiction emerges, patch `.qoder/specs/if2ai-agent-evolution-report.md` Part 1.3 / Part 6
8. **Commit** (one iteration = one commit, message: `chore(evolution-truth): iter-N — <one-line>`).
9. **Stop and write iteration N+1's target** at the bottom of the gap report.

**Hard rule:** never bundle two iterations. If you find a second gap mid-iteration, log it under "Iteration N+2 candidates" and finish the current one.

---

## 2. Exit Gate (loop terminates here)

All of:

- Every Agent Evolution Pack in REGISTRY is either `done` (full production wire) or `deferred` (with explicit deferral note).
- Zero Pack remains `landed-stub`.
- `.qoder/specs/if2ai-agent-evolution-report.md` Part 1.3 + Part 6 do not contradict each other and both reference the latest gap report's date.
- The latest gap report contains a final "Closed" block with no open items.
- `./scripts/pack suite harness/suites/self-healing-e2e.yaml` passes (spec Part 5.9).

When the Exit Gate passes, write a final entry to the gap report titled "**Loop closed YYYY-MM-DD**" and stop.

---

## 3. State Vocabulary (additions to standard registry states)

Standard registry states (from `docs/packs/REGISTRY.md`): `pending` / `active` / `done` / `blocked` / `merged`.

This loop introduces one observational sub-state used **only inside gap reports** (registry stays standard):

- `landed-stub` — Pack's algorithmic + emit + call-site code is shipped, but at least one production dependency is a `MockX::empty()` / `StubX` / `ConstX` / placeholder closure. Code self-documents this with a comment of the form "until a deeper-wiring Pack plumbs …". Counted as **NOT done** for Exit Gate purposes.

`landed-stub` Packs require a follow-up `deeper-wiring` Pack that swaps the placeholder for the real handle.

---

## 4. Anti-Patterns That Trip the Loop

These thoughts mean STOP and re-read the rules above:

| Thought | Reality |
|---|---|
| "Helper exists in module → done" | Modules ≠ production. Re-grep call sites. |
| "Pack file says done, registry says done → done" | Both can be wrong. Code is truth. |
| "Test calls the helper → wired" | Tests don't ship to users. Production paths only. |
| "Comment says TODO but it works → ship" | Honest stub is a deferred bug. Mark `landed-stub`. |
| "Let me batch fixes A and B" | One iteration = one fix. Always. |
| "Spec is authoritative because user wrote it" | Spec is a *description*; code is the *thing*. Patch spec to match code. |

---

## 5. Iteration 1 — Closed (2026-04-30)

**Audit slice:** All 41-Pack rollout (`aa2deed`) — verify wire-up depth.

**Outcome:** Documented in `docs/design-docs/agent-evolution/AGENT-EVOLUTION-SPEC-GAP-REPORT-2026-04-30.md`. Four `landed-stub` Packs identified (DW-001, DW-002, DW-004, WU-002). REGISTRY status flips and spec Part 1.3 reconciliation deferred to iteration 2 (write-only, no code change required) and iteration 3 (deep-wiring Pack: provider/utility/store handle threading).

---

## 6. Iteration 2 — Target (queued)

**Slice:** Truth-artifact reconciliation only (zero code).

**Tasks (in order):**

1. Flip REGISTRY rows for WU-001..008, DW-001..005, UI-001..006 from `active` → either `done` or `landed-stub` per gap report 2026-04-30 §3.
2. Update each Pack file's `State:` field to match.
3. Patch `.qoder/specs/if2ai-agent-evolution-report.md` Part 1.3 with a "**Last code alignment: 2026-04-30**" note + link to the gap report; mark Part 6 entries that are `landed-stub` with ⚠️ instead of ✅.
4. Commit `chore(evolution-truth): iter-2 — registry/spec reconciliation`.

**Verification:** `git grep -nE 'active' docs/packs/REGISTRY.md | head` should not return any agent-evolution row that the gap report classifies as done; spec Part 1.3 ↔ Part 6 must agree on the four `landed-stub` items.

---

## 7. Iteration 3 — Target (queued)

**Slice:** One deeper-wiring Pack to retire the four `landed-stub` items as a group.

**Pack:** `WU-009-deep-utility-handle-threading` (to be authored; see iteration 1 report §6 for the three concrete handle threads).

**Stop:** Iteration 3 is the next session's job. Do not start it in iteration 2.
