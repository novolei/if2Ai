# ADR-016: HRR (Holographic Reduced Representations) — Keep, Wire, or Remove

- Status: **Open** (decision pending)
- Date proposed: 2026-04-22
- Owner: TBD
- Supersedes parts of: [ADR-007 HRR Introduction Timing P2a](./ADR-007-HRR-Introduction-Timing-P2a.md)
- Related audit: `.cursor/plans/memory_系统审计_*.plan.md` (Memory System Audit, P1 #7)

---

## Context

The `src-tauri/src/modules/memory/hrr/` module exists in three files:

- [`hrr/operations.rs`](../../../../src-tauri/src/modules/memory/hrr/operations.rs)
  — pure HRR algebra: `bind` / `unbind` / `bundle` / `similarity` over
  `HRRVector`. Standalone, no I/O.
- [`hrr/store.rs`](../../../../src-tauri/src/modules/memory/hrr/store.rs)
  — in-memory `HolographicStore` with LRU eviction and capacity
  management (`O(√dim)` ≈ 700 entries at 384d).
- [`hrr/integration.rs`](../../../../src-tauri/src/modules/memory/hrr/integration.rs)
  — a `HybridMemoryProvider` shim intended to glue HRR onto the
  existing memory pipeline.

Total: ~600 LOC, all gated behind module-level `#![allow(dead_code)]`.

### What works

- The math (operations.rs) is correct and unit-tested in isolation.
- The store can hold and probe vectors with sensible eviction.
- The module compiles cleanly and adds no runtime cost when unused.

### What does not work / is unverified

- **No production caller**: `grep -r "hrr::"` outside the module
  itself returns only `bootstrap/memory.rs` (which references the
  fastembed provider, not HRR specifically) and module-internal
  imports. No `application/*` or `commands/*` code path consults HRR.
- **No empirical evaluation**: there is no benchmark showing HRR
  improves recall quality, latency, or token budget vs the current
  `ActiveRetrievalManager` (3-fold `recall` + RRF fusion).
- **No design doc on user-visible behavior**: ADR-007 lays out the
  *technical* rationale (algebraic reasoning, role-filler binding)
  but does not specify which user query or agent intent HRR would
  uniquely answer better than the existing pipeline.
- **Capacity ceiling is low**: `O(√384) ≈ 19` items per fully crisp
  bundle, ~700 items total store capacity. The current SQLite library
  routinely holds 10k+ entries; HRR cannot serve as a primary recall
  path, only as a complementary algebraic layer for a small hot set.

### The cost of "parking" indefinitely

Keeping the code dead-on-arrival has accumulating costs:

1. **Cognitive load** — every newcomer who reads `mod.rs` asks "is HRR
   used?", finds it unwired, then has to reason about whether new
   memory work should target it.
2. **API drift risk** — when `MemoryProvider`, `MemoryEntry`, or
   `embedding::*` evolve, the HRR integration layer may quietly fall
   out of sync, blocking any future revival without rework.
3. **Test debt** — the operations tests pass but the store / integration
   never get exercised end-to-end, so a regression there is invisible.
4. **Build time** — modest but non-zero (~600 LOC of generic numeric
   code with `f32` SIMD-adjacent math).

---

## Decision (to be made)

Pick **exactly one** of the three options below.

### Option A — KEEP UNWIRED (status quo, document)

Leave the code in place but make the parking deliberate:

- Update `hrr/mod.rs` doc comment to say *unwired by design, see
  ADR-016*. (Already done as part of P0 cleanup.)
- Add a tracking issue with a 6-month sunset clock: if no one starts
  the integration PoC by then, this ADR auto-promotes to Option C.
- Cost: 0 lines of code, 0 risk, 0 user value.

### Option B — WIRE (PoC then default-on)

Commit to integration. Concrete next steps:

1. Define the *one* recall scenario where HRR demonstrably wins
   (e.g. "given a user query 'what did I say about X with respect to
   Y?', HRR's `unbind` recovers the role-filler structure faster than
   fanning out through 3-fold recall + RRF").
2. Build `HybridMemoryProvider::recall_with_hrr` that runs the existing
   pipeline + an HRR pass + an empirically-tuned fusion. Land behind a
   `MEMORY_HRR_ENABLED` config flag (defaults off).
3. Write a benchmark suite using `harness/eval/` that measures recall
   quality (gold-set NDCG), latency, and token budget impact across
   100+ representative turns. Bake the numbers into this ADR before
   defaulting on.
4. Move HRR from `#[allow(dead_code)]` to first-class, with full test
   coverage on the integration path.

Estimated cost: 5-8 person-days plus eval gold-set construction.
Risk: medium — if the benchmark fails to show an edge, we will have
sunk effort to reach Option C.

### Option C — REMOVE

Delete the entire `hrr/` directory.

- ~600 LOC removed
- Update `mod.rs` to drop `pub mod hrr;`
- Update `embedding/mod.rs` to drop the `MockEmbedder` re-export note
  (HRR was its only declared consumer)
- Document the removal in this ADR + a changelog entry

If a future use case for VSA (Vector Symbolic Architectures) emerges,
git history preserves the implementation and we can resurrect it on
solid product grounds.

Estimated cost: 1 person-day. Risk: low — current pipeline works
without HRR, no consumer breaks.

---

## Recommendation (author's view)

**Lean Option C** unless someone in the next sprint commits to
Option B with a concrete benchmark plan. Reasoning:

- The current `ActiveRetrievalManager` (3-fold recall + RRF) already
  delivers good recall in practice; we have no failing user complaint
  attributable to a "missing algebraic reasoning layer".
- HRR's capacity ceiling (~700 items) makes it a niche augmentation
  at best, never a primary index. Niche augmentations should be added
  *after* a measured gap, not before.
- "Park indefinitely" (Option A) is the worst outcome: pays the
  cognitive cost without ever extracting the value.

But this is genuinely a product call — if the team has a design vision
for symbolic memory composition that HRR enables, Option B is right
and the eval cost is well-spent.

---

## Decision deadline

**Resolve by 2026-05-31** (≈ 6 weeks from today). Default if no one
takes ownership: Option C (remove), executed by whoever is on memory
maintenance that sprint.

## Sign-off

| Reviewer | Vote (A / B / C) | Note |
|----------|------------------|------|
| TBD      | TBD              |      |
| TBD      | TBD              |      |
