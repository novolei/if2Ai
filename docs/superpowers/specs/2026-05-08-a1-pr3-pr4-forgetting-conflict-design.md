# A.1 PR-3 / PR-4 — Forgetting Curve + Conflict Resolver Design Spec

> **Date**: 2026-05-08
> **Wave**: A.1 of memory + evolution roadmap (see [`2026-05-05-memory-evolution-product-replan.md`](../plans/2026-05-05-memory-evolution-product-replan.md))
> **Status**: design approved; ready for `writing-plans`
> **Source code**: cherry-picked into `wt-mem-A0-compile-gate` worktree from `origin/feat/in-flight-A-memory @ 8674725`. Production code lives at:
> - `src-tauri/src/modules/memory/forgetting.rs` (380 LOC)
> - `src-tauri/src/modules/memory/conflict.rs` (497 LOC)
> - `src-tauri/src/modules/memory/quality.rs` (212 LOC, dependency from PR-2)

---

## 1. Purpose

Define the configurable parameters and behavioral semantics for two foundation modules in Wave A.1 of the memory roadmap:

1. **PR-3 — Forgetting curve engine** (`forgetting.rs`): time-based memory aging that demotes (not deletes) low-retention entries via the Ebbinghaus formula `retention = importance × e^(−λ·t_hours) × (1 + α·ln(access+1))`.
2. **PR-4 — Conflict resolver (LITE)** (`conflict.rs`): pre-write detection + auto-resolution of near-duplicate, contradictory, or partially-overlapping memories, with forward-compat tracking for memories we know are conflicting but can't auto-resolve.

This spec freezes the four parameter decisions made during the 2026-05-08 brainstorm and the trait-extension design for `KeepBothWithFlag`.

## 2. Decisions

### 2.1 PR-3 forgetting parameters

| Field | Value | Rationale |
|-------|-------|-----------|
| `lambda` | **0.02 / hour** | Half-life ~35h; "forgotten" (retention < 0.1) at ~5 days for unaccessed importance=1.0 entry. Matches Ebbinghaus research scaled to per-hour units. The in-flight default of 0.1 was unintentionally aggressive (~7h half-life). |
| `archive_threshold` | **0.1** | Aligns demotion timeline with the lambda choice; importance=1 entry demoted at ~115h ≈ 4.8 days. Conservative enough that important-but-unused memories survive ~1 week. |
| `sweep_interval_hours` | **12** | Unchanged from in-flight default. Low overhead at 10K-entry scale (~3ms/sweep observed in tests). |
| `alpha` (access reinforcement) | **unchanged from in-flight** | Out of scope for this brainstorm; revisit if telemetry shows reinforcement imbalance. |

### 2.2 PR-4 conflict parameters

| Field | Value | Rationale |
|-------|-------|-----------|
| `similarity_threshold` | **0.75** | Unchanged. Bigram similarity gate for entering the conflict path. |
| `auto_resolve_threshold` | **0.1** | Quality gap ≥ 0.1 → auto-pick higher-quality side. Aggressive setting compensates for absent AskUser UI: silent "keep both" of low-gap conflicts is worse than occasional wrong auto-pick (the loser was already similar; access reinforcement re-promotes valid entries). Revisit when AskUser UI ships in a later PR. |
| Internal classify thresholds | **unchanged**: 0.95→Duplicate / 0.85→Contradiction / 0.75→PartialOverlap | Already documented in in-flight `classify_conflict`. |

### 2.3 PR-4 KeepBothWithFlag — Hybrid C trait method

Add to `MemoryProvider` trait:

```rust
pub trait MemoryProvider {
    // ... existing methods ...

    /// Atomically bump the `contradiction_count` column on the entry
    /// identified by `key`. Default impl is a noop so providers that
    /// don't track conflicts (e.g. vector-only) continue to compile.
    /// Used by the conflict resolver when `KeepBothWithFlag` fires.
    ///
    /// Returns `Ok(())` even if the key is unknown — this is best-effort
    /// telemetry, not a load-bearing operation.
    async fn increment_contradiction_count(&self, _key: &str) -> Result<()> {
        Ok(())
    }
}
```

**Overrides**:
- `SqliteMemoryProvider`: real implementation — `UPDATE memory_entries SET contradiction_count = contradiction_count + 1 WHERE key = ?` (~10 LOC).
- `VectorMemoryProvider`: inherits the default noop. Can delegate to a SQLite child later if hybrid mode requires; not required for PR-4.
- `HybridMemoryProvider` (if exists): inherits noop; future extension delegates to its inner SQLite.

**Storage**: `contradiction_count INTEGER NOT NULL DEFAULT 0` column added by v8 migration in PR-6. PR-4 strictly depends on PR-6 landing first.

## 3. Behavior

### 3.1 Forgetting sweep

```text
Every 12 hours, MemoryTicker invokes ForgettingCurveEngine::sweep:

  for each entry in memory_provider.export_all():
    t_hours = (now - entry.last_access) / 3600
    decay   = exp(-0.02 * t_hours)
    bonus   = 1 + alpha * ln(access_count + 1)
    retention = entry.importance * decay * bonus

    if retention < 0.1:
      provider.update_quality_score(entry.key, entry.quality_score * 0.5)
      report.demoted_count += 1
    else:
      report.untouched_count += 1

  return SweepReport {
    swept_count, demoted_count, untouched_count, duration_ms
  }
```

**No deletion**. Demoted entries remain queryable but rank lower in recall ordering (quality_score is a multiplier).

### 3.2 Conflict resolution paths

```text
On every memory write:

  candidates = provider.find_similar(new_entry, top_k=5)

  for each candidate:
    sim = bigram_similarity(new.content, candidate.content)
    if sim < 0.75: skip

    type = classify_conflict(sim):
      sim ≥ 0.95 → Duplicate
      sim ≥ 0.85 → Contradiction
      sim ≥ 0.75 → PartialOverlap

    quality_gap = abs(new.quality - candidate.quality)

    Resolution:
      Duplicate                                    → KeepNewer (always)
      Contradiction + gap ≥ 0.1                    → auto-pick by quality
      Contradiction + gap <  0.1                   → KeepBothWithFlag
                                                       └→ provider.increment_contradiction_count(loser_key)
      PartialOverlap + gap ≥ 0.1                   → auto-pick by quality
      PartialOverlap + gap <  0.1                  → AskUser (no UI yet → silently keeps both)
```

**Note on "loser_key"**: in `KeepBothWithFlag` we keep both entries; the bumped counter tags **the existing entry** (not the new one) so a future conflict-review UI can surface "your existing memory has been challenged N times".

## 4. Components affected

| File | Δ scope | LOC est. |
|------|---------|----------|
| `src-tauri/src/modules/memory/forgetting.rs` | Defaults: `lambda=0.02`, `archive_threshold=0.1`, `sweep_interval_hours=12` (unchanged) | tweak ~3 lines from in-flight |
| `src-tauri/src/modules/memory/conflict.rs` | Defaults: `auto_resolve_threshold=0.1` (was 0.3); `KeepBothWithFlag` resolution path calls `increment_contradiction_count` | tweak ~10 lines from in-flight |
| `src-tauri/src/modules/memory/mod.rs` (or trait module) | Add `MemoryProvider::increment_contradiction_count` with default noop | ~10 LOC |
| `src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs` | Override `increment_contradiction_count` with SQL UPDATE | ~10 LOC |
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | No change (inherits noop) | 0 |
| `src-tauri/src/modules/memory/migrations.rs` (PR-6 dep) | v8 adds `contradiction_count INTEGER NOT NULL DEFAULT 0` column | already in in-flight |
| `src-tauri/src/bootstrap/memory.rs` | Wire `MemoryTicker::with_forgetting_runtime(ForgettingConfig::default())` | already in in-flight; verify default values match the new spec |

## 5. Tests

### 5.1 Forgetting (PR-3)

**Existing tests preserved** (5 unit tests in in-flight `forgetting.rs`):
- `sweep_demotes_low_retention_entries`
- `sweep_preserves_high_retention_entries`
- `retention_formula_matches_ebbinghaus_at_known_points`
- `access_reinforcement_extends_retention`
- `sweep_report_counts_match_observed_changes`

**NEW** (added in PR-3):
- `forgetting_stable_for_week_of_normal_use`: synthesize 100 entries with mixed importance ∈ {0.3, 0.5, 0.7, 1.0} and access patterns ∈ {0, 5, 20, 50}; run 14 consecutive 12h sweeps; assert ≥80% of high-importance entries (importance ≥0.7) retain quality_score ≥0.5 after week.
- `forgetting_default_config_uses_lambda_002`: literal default-value assertion to prevent silent regressions.

### 5.2 Conflict (PR-4)

**Existing tests preserved** (8+ unit tests in in-flight `conflict.rs`):
- per-variant resolution tests (Duplicate / Contradiction / PartialOverlap)
- threshold boundary tests
- text similarity calculation tests

**NEW** (added in PR-4):
- `conflict_keep_both_increments_contradiction_count`: store entry A; store conflicting B with `quality_gap < 0.1`; verify `contradiction_count` on A increments by 1 in the SQLite backend.
- `conflict_default_threshold_is_01`: literal default-value assertion.
- `provider_increment_contradiction_count_default_is_noop`: instantiate `VectorMemoryProvider` directly, call the trait method, assert `Ok(())` and no side effects.
- `auto_resolve_threshold_01_picks_higher_quality_at_small_gaps`: store A (quality 0.4) + conflicting B (quality 0.55, gap=0.15 ≥ 0.1) → assert auto-pick of B.

## 6. Wave A.1 ship order (dependency-respecting)

1. **PR-6** — migrations v5–v8 (foundation; adds `contradiction_count` column among others)
2. **PR-2** — `quality.rs` (no schema deps; pure scoring transform)
3. **PR-3** — `forgetting.rs` (depends on PR-2 quality_score field)
4. **PR-4** — `conflict.rs` LITE + `MemoryProvider` trait extension (depends on PR-2 quality + PR-6 column)
5. **PR-5** — `evolution/procedural.rs` (depends on A.1 PR-1 enum variant + producer slot)

**Risk**: shipping PR-4 before PR-6 → `UPDATE ... contradiction_count` fails at runtime because the column doesn't exist. **Mitigation**: PR ordering enforced via plan dependencies in `writing-plans`.

## 7. Out of scope (deferred)

- **AskUser UI** — fired by `PartialOverlap + small gap`; PR-4 silently keeps both. UI is a later PR.
- **KeepBothWithFlag UI surfacing** — query-by-`contradiction_count > 0`; depends on AskUser UI.
- **`forgetting.alpha`** — keep in-flight default; revisit when telemetry shows reinforcement imbalance.
- **Daydream consolidator interaction** — daydream's prune/merge step calls into forgetting + conflict; brainstormed in Wave A.2.
- **Vector / Hybrid provider increment_contradiction_count override** — defaults to noop; only SQLite tracks. Hybrid can later delegate to its SQLite child.
- **PR-2 / PR-5 / PR-6 details** — separate plans/specs; this spec covers PR-3 + PR-4 only.

## 8. Acceptance criteria

PR-3 ships when:
- `cargo test forgetting --lib` passes (existing 5 + new 2 = 7 cases)
- `cargo test memory --lib` no regression
- 24-hour soak: app left idle, ticker fires once, demoted_count > 0, store stays internally consistent

PR-4 ships when:
- `cargo test conflict --lib` passes (existing 8+ + new 4 = 12+ cases)
- `cargo test memory_provider --lib` no regression
- v8 migration test green (column present + default value 0)
- Manual smoke: store entry A, store similar B with gap < 0.1 → check `SELECT contradiction_count FROM memory_entries WHERE key = '<a>'` returns 1
