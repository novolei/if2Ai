# A.1 PR-2 + PR-6a — Memory Schema Foundation Design Spec

> **Date**: 2026-05-08
> **Wave**: A.1 of memory + evolution roadmap (see [`2026-05-05-memory-evolution-product-replan.md`](../plans/2026-05-05-memory-evolution-product-replan.md))
> **Status**: design approved; ready for `writing-plans`
> **Source**: cherry-picked from `wt-mem-A0-compile-gate` worktree branch (off `origin/feat/in-flight-A-memory @ 8674725`).
> **Unblocks**: PR-3 + PR-4 plan in [`2026-05-08-a1-pr3-pr4-forgetting-conflict.md`](../plans/2026-05-08-a1-pr3-pr4-forgetting-conflict.md) (Task 0 preflight currently halts on these prerequisites).

---

## 1. Purpose

Bring the memory-entry schema foundation into vnext so the rest of Wave A.1 (PR-3 forgetting, PR-4 conflict, PR-5 procedural memory) can land. Two ship units:

1. **PR-6a** — schema + struct + trait extensions: migrations v7 (`quality_scoring_columns`) + v8 (`cognitive_layer_columns`) + `MemoryEntry` 6 new fields + `MemoryProvider::update_decay_scores` default-noop trait method + provider impl updates.
2. **PR-2** — `quality.rs` (212 LOC verbatim): stateless `QualityScorer` consuming the new fields PR-6a adds.

This spec freezes scope decisions made during the 2026-05-08 brainstorm.

## 2. Decisions

### 2.1 PR-6 split

PR-6 splits along a semantic boundary:

| Split | Migrations | Status |
|-------|-----------|--------|
| **PR-6a** (this spec) | v7 quality cols + v8 cognitive layer | SHIP |
| **PR-6b** (deferred) | v5 conversation_recall_fts + v6 conversation_recall_embeddings | DEFER |

Rationale: v5/v6 are conv-recall infrastructure with no consumer in PR-3/PR-4/PR-5. Bundling them inflates review scope. PR-6b ships when its actual consumer (likely compaction or summary work) lands.

### 2.2 Schema extensions (additive only)

Migration v7 (`quality_scoring_columns`) — `ALTER TABLE memory_entries ADD COLUMN`:
- `quality_score REAL DEFAULT 0.5`
- `source_reliability REAL DEFAULT 0.5`
- `last_validated_at TEXT` (nullable; ISO-8601)
- `contradiction_count INTEGER DEFAULT 0`

Migration v8 (`cognitive_layer_columns`) — `ALTER TABLE memory_entries ADD COLUMN`:
- `cognitive_layer INTEGER DEFAULT 2` (CoALA Deliberative tier; default chosen per replan §3.3)
- `context_tags TEXT DEFAULT '[]'` (JSON-encoded array)

All `ADD COLUMN` operations are wrapped in `add_column_if_not_exists` helper that swallows `duplicate column name` errors → migrations are rerun-safe.

### 2.3 `MemoryEntry` struct — 6 new fields

Add to `pub struct MemoryEntry` in `memory/mod.rs`:

```rust
pub quality_score: f64,             // default 0.5
pub source_reliability: f64,        // default 0.5
pub last_validated_at: Option<DateTime<Utc>>,
pub contradiction_count: u32,       // default 0
pub cognitive_layer: u8,            // default 2
pub context_tags: Vec<String>,      // default vec![]
```

All fields use `#[serde(default)]` so older JSON payloads (Trajectory imports, audit replays, etc.) remain decodable.

### 2.4 `MemoryProvider` trait extensions

Add **two** methods (per in-flight code; one new for PR-6a, one already present and re-confirmed):

```rust
/// Persist updated `importance` and `quality_score` for a single entry.
/// Used by the forgetting-curve sweep after computing decay.
///
/// Default impl is a noop so providers that don't track quality
/// (vector-only, in-memory) continue to compile. SQLite overrides.
/// Returns `KeyNotFound` on missing key; does NOT bump `updated_at`
/// (forgetting modifies decay scores, not user-visible content).
async fn update_decay_scores(
    &self,
    key: &str,
    importance: f64,
    quality_score: f64,
) -> Result<(), MemoryError> {
    let _ = (key, importance, quality_score);
    Ok(())
}

/// MEM-MOD-P1 — already on vnext per its existing definition.
/// Listed here for completeness because the in-flight branch
/// references it from learning_traits + audit emitter paths.
async fn adjust_trust_score(&self, key: &str, delta: f64) -> Result<f64, MemoryError> { ... }
```

(Note: `adjust_trust_score` is already on vnext — no schema change needed; verify present.)

### 2.5 Provider impl updates

| Provider | Change | LOC est. |
|----------|--------|----------|
| `SqliteMemoryProvider` | Row-mapping reads all 6 new columns; INSERT/UPDATE statements include new fields with safe defaults; override `update_decay_scores` with real SQL | ~387 (largest) |
| `VectorMemoryProvider` | Default-impl trait method coverage; struct field-default population in any local fixtures | ~20 |
| `LanceDBMemoryProvider` | Same — fixture default values for new struct fields | ~6 |
| `InMemoryMemoryProvider` | Same — populate new MemoryEntry fields with defaults on store/recall | covered by struct change |
| `compat.rs` | Migration helper that defaults new fields when ingesting old serialized payloads | +7 |

### 2.6 PR-2 `quality.rs` (212 LOC, verbatim)

Public surface:

```rust
pub struct QualityWeights {
    pub source_weight: f64,         // default 0.3
    pub freshness_weight: f64,      // default 0.2
    pub usage_weight: f64,          // default 0.25
    pub consistency_weight: f64,    // default 0.25
}
// Sum = 1.0

pub struct QualityScorer { weights: QualityWeights }

impl QualityScorer {
    pub fn new() -> Self;
    pub fn with_weights(weights: QualityWeights) -> Self;
    pub fn score(&self, entry: &MemoryEntry, now: DateTime<Utc>) -> f64;
    pub fn batch_rescore(&self, entries: &mut [MemoryEntry], now: DateTime<Utc>);
}
```

Formula:
```
source      = entry.source_reliability                       // already [0,1]
freshness   = 1 / (1 + hours_since_update / 168)             // 1-week half-life
usage       = min(1, access_count / 50)                      // caps at 50 accesses
consistency = 1 / (1 + contradiction_count)                  // 50%-per-contradiction drop
final       = (source*0.3 + freshness*0.2 + usage*0.25 + consistency*0.25).clamp(0,1)
```

Module registration: `pub mod quality;` + `pub use quality::{QualityScorer, QualityWeights};` in `memory/mod.rs`.

### 2.7 Tests

**Existing migration tests preserved.**

**NEW** (added in PR-6a):
- `v7_columns_round_trip_via_sqlite_provider` — open fresh SQLite, run all migrations, store entry, verify all 6 new columns have default values, call `update_decay_scores(key, 0.7, 0.85)`, re-read, assert importance=0.7 + quality_score=0.85; call `update_decay_scores("missing-key", ...)` and assert `KeyNotFound`.

**Existing quality.rs tests preserved (5 unit tests in in-flight code):**
- High-quality entry scores high
- High contradiction count lowers score
- Old freshness lowers score
- Heavy usage raises score
- Custom weights affect outcome

**No new tests required for PR-2** — in-flight test suite is sufficient.

## 3. Behavior

### 3.1 Migration runner sequence

```
on app start (memory subsystem init):
  for each migration in [v1..v8]:
    if version > current_db_version:
      execute up()
      record version → migration_history table

  resulting state: all 6 new columns present with default values
  for any pre-existing rows.
```

Backfill: `add_column_if_not_exists` populates defaults via SQLite's `DEFAULT` clause; no explicit UPDATE pass needed.

### 3.2 `update_decay_scores` semantics

```
provider.update_decay_scores(key, importance, quality_score):
  - SQLite: UPDATE memory_entries SET importance=?, quality_score=? WHERE key=?
    - changed=0 → return MemoryError::KeyNotFound
    - changed=1 → return Ok
  - Vector / Hybrid / InMemory: default noop, return Ok regardless of key
```

Used by the forgetting sweep (PR-3 Task 1+2) and re-confirmed in this PR by the round-trip test.

### 3.3 `QualityScorer::score` semantics

Stateless function over `&MemoryEntry`. No I/O, no time except `now: DateTime<Utc>`. Used by:
- Forgetting sweep (PR-3) — rescore + halve-on-archive
- Conflict resolver (PR-4) — quality_gap calculation
- Memory recall ranking (future) — sort by score before returning to caller

## 4. Components affected

| File | Δ scope | LOC est. |
|------|---------|----------|
| `src-tauri/src/modules/memory/migrations.rs` | Add `memory_v7_quality_scoring` + `memory_v8_cognitive_layer` + entries in `MIGRATIONS` array | +91 |
| `src-tauri/src/modules/memory/mod.rs` | Add 6 fields to `MemoryEntry`; add `update_decay_scores` trait method default-noop; `pub mod quality;` + re-export | +225 |
| `src-tauri/src/modules/memory/quality.rs` | NEW (212 LOC verbatim from worktree) | +212 |
| `src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs` | Row-mapping for 6 new columns | +29 |
| `src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs` | INSERT/UPDATE statements include new fields; `update_decay_scores` override | +387 |
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | Fixture defaults for new struct fields | +20 |
| `src-tauri/src/modules/memory/providers/lancedb.rs` | Fixture defaults | +6 |
| `src-tauri/src/modules/memory/compat.rs` | Migration helper for ingesting old serialized payloads | +7 |

**Total**: ~951 LOC across 8 files. PR-6a portion ≈ 739; PR-2 portion ≈ 212.

## 5. Tests

### 5.1 PR-6a

Existing tests preserved.

**NEW**:
- `v7_columns_round_trip_via_sqlite_provider` (≈ 50 LOC)
  - Open in-memory SQLite via tempdir
  - Run all migrations v1–v8
  - Store entry "test-key"
  - Read back; assert `quality_score == 0.5`, `source_reliability == 0.5`, `last_validated_at == None`, `contradiction_count == 0`, `cognitive_layer == 2`, `context_tags == []`
  - Call `update_decay_scores("test-key", 0.7, 0.85)`
  - Re-read; assert `importance == 0.7`, `quality_score == 0.85`
  - Call `update_decay_scores("missing-key", 0.5, 0.5)`; assert `Err(MemoryError::KeyNotFound)`

### 5.2 PR-2

Existing 5 in-flight unit tests sufficient. No new tests.

## 6. Ship order

```
PR-6a (this spec — schema + struct + trait + provider impls)
  │
  ▼
PR-2  (quality.rs)
  │
  ▼
PR-1  ✅ already on vnext (commit 6561968)
  │
  ▼
PR-3  (forgetting.rs)        ┐
  │                          │ blocked by PR-2 + PR-6a;
  ▼                          │ unblocks once they land
PR-4  (conflict.rs + Hybrid) ┘
```

Both PR-6a and PR-2 land together as 2 atomic commits before PR-3 plan execution.

## 7. Out of scope (deferred)

- **PR-6b** — v5 FTS + v6 embeddings, separate consumer chain
- **CognitiveLayerManager** — column ships, manager defers per replan §3.3
- **WAL kill-mid-migration test** — defensive but non-blocking
- **Quality weight tuning** — use in-flight defaults; revisit via telemetry
- **Down-migration support** — none today; document as known limitation

## 8. Acceptance criteria

PR-6a ships when:
- `cargo check --workspace` clean on vnext after the commit
- `cargo test --lib memory::migrations` passes (existing tests + new round-trip)
- `cargo test --lib memory` no regression
- Manual smoke: open fresh app, observe `~/.if2ai/memory.db` schema includes all 6 new columns

PR-2 ships when:
- `cargo check --workspace` clean
- `cargo test --lib memory::quality` passes (5/5 in-flight tests)
- `pub use quality::{QualityScorer, QualityWeights}` resolvable from a downstream consumer (PR-3 forgetting will be the first real consumer)
