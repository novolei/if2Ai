# A.1 PR-2 + PR-6a — Memory Schema Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the in-flight memory schema foundation (migrations v7+v8, MemoryEntry struct extensions, MemoryProvider::update_decay_scores trait method, all provider impl updates, plus quality.rs scorer) into vnext as 2 atomic commits so the PR-3 + PR-4 plan (commit `6b2bba6`) can execute its Task 0 preflight without halting.

**Architecture:** Cherry-pick file-at-a-time from `wt-mem-A0-compile-gate` worktree branch (off `origin/feat/in-flight-A-memory @ 8674725`). PR-6a lands first (migrations v7+v8 + struct + trait + provider impls + 1 new round-trip test). PR-2 lands second (quality.rs verbatim + module registration). Each commit must compile + pass full memory test suite before the next starts.

**Tech Stack:** Rust + tokio + rusqlite + lance/lancedb. Existing memory subsystem patterns. No new crate dependencies.

**Source files (cherry-pick base):** branch `wt-mem-A0-compile-gate` (worktree at `.claude/worktrees/wt-mem-A0`). To read a file from this branch: `git show wt-mem-A0-compile-gate:<path>`. To copy: `git show wt-mem-A0-compile-gate:<path> > <path>`.

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/src/modules/memory/migrations.rs` | **Modify** (+91 LOC) | Add `memory_v7_quality_scoring` + `memory_v8_cognitive_layer` functions; register them in the `MIGRATIONS` static array |
| `src-tauri/src/modules/memory/mod.rs` | **Modify** (+225 LOC) | Add 6 new fields to `MemoryEntry` struct; add `MemoryProvider::update_decay_scores` default-noop trait method; declare `pub mod quality;` (Task 9 only); register `pub use quality::{...}` re-exports |
| `src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs` | **Modify** (+29 LOC) | Row-mapping reads all 6 new columns from query results |
| `src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs` | **Modify** (+387 LOC) | INSERT/UPDATE statements include the 6 new fields; override `update_decay_scores` with real SQL UPDATE |
| `src-tauri/src/modules/memory/providers/vector_provider.rs` | **Modify** (+20 LOC) | Populate fixture defaults for the 6 new struct fields where this provider constructs `MemoryEntry` instances |
| `src-tauri/src/modules/memory/providers/lancedb.rs` | **Modify** (+6 LOC) | Same — populate fixture defaults |
| `src-tauri/src/modules/memory/compat.rs` | **Modify** (+7 LOC) | Migration helper for ingesting old serialized payloads |
| `src-tauri/src/modules/memory/quality.rs` | **Create** (+212 LOC) | Verbatim copy from worktree — `QualityScorer` + `QualityWeights` |

---

## Task 0: Preflight verification

**Files:** none modified — verification only.

- [ ] **Step 0.1: Confirm vnext branch + clean tree**

Run:
```bash
cd /Users/ryanliu/Documents/IfAI/if2Ai
git branch --show-current
git status --short | wc -l
```

Expected: `vnext` and `0`. If either differs, HALT.

- [ ] **Step 0.2: Confirm worktree branch is reachable**

Run:
```bash
git rev-parse wt-mem-A0-compile-gate >/dev/null 2>&1 && echo OK || echo MISSING
```

Expected: `OK`. If `MISSING`, HALT — the source files come from this branch.

- [ ] **Step 0.3: Confirm vnext does NOT yet have v7/v8 migrations or new struct fields**

Run:
```bash
grep -cE 'fn memory_v7_quality_scoring|fn memory_v8_cognitive_layer|pub quality_score:|pub contradiction_count:|pub cognitive_layer:' \
  src-tauri/src/modules/memory/migrations.rs \
  src-tauri/src/modules/memory/mod.rs
```

Expected: `0`. If any of these exist already, the prerequisites are already partially landed — STOP and ask the parent agent how to proceed.

- [ ] **Step 0.4: Baseline test count**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib memory 2>&1 | grep "test result" | tail -3
```

Expected: 3 test-result lines summing to `~25-40 passed; 0 failed`. Save the count for the final regression check.

- [ ] **Step 0.5: Baseline cargo check passes**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | tail -3
```

Expected: `Finished `dev` profile ...`. No errors.

No commit — preflight is verification only. Proceed to Task 1.

---

## Task 1: PR-6a — Add `MemoryEntry` struct fields

**Files:**
- Modify: `src-tauri/src/modules/memory/mod.rs` (struct definition around line 114)

- [ ] **Step 1.1: Inspect current `MemoryEntry` struct on vnext**

Run:
```bash
sed -n '/^pub struct MemoryEntry {/,/^}$/p' src-tauri/src/modules/memory/mod.rs | head -40
```

Note: capture the field order so the next step inserts new fields in the same order as the in-flight branch (preserves serde field order).

- [ ] **Step 1.2: Inspect target `MemoryEntry` struct from worktree branch**

Run:
```bash
git show wt-mem-A0-compile-gate:src-tauri/src/modules/memory/mod.rs | \
  sed -n '/^pub struct MemoryEntry {/,/^}$/p' | head -60
```

Identify the 6 new fields and their `#[serde(default)]` attributes. They are inserted in this order, immediately after the existing fields:
- `pub quality_score: f64`
- `pub source_reliability: f64`
- `pub last_validated_at: Option<chrono::DateTime<chrono::Utc>>`
- `pub contradiction_count: u32`
- `pub cognitive_layer: u8`
- `pub context_tags: Vec<String>`

- [ ] **Step 1.3: Patch `MemoryEntry` struct in vnext**

Open `src-tauri/src/modules/memory/mod.rs`. Find the `pub struct MemoryEntry { ... }` block. Inside the struct, after the existing last field (typically `session_id` / `project_id` / similar), add (verbatim from worktree):

```rust
    /// Composite quality score in `[0, 1]` derived from source / freshness / usage / consistency.
    /// PR-2 QualityScorer writes this; PR-3 forgetting sweep + PR-4 conflict resolver consume it.
    /// Default 0.5 for entries pre-dating PR-6a (v7 column DEFAULT 0.5 in SQLite).
    #[serde(default = "default_quality_score")]
    pub quality_score: f64,

    /// Source reliability factor in `[0, 1]`. Tags origin (manual user / agent_extracted / etc.).
    /// Default 0.5 for unspecified.
    #[serde(default = "default_quality_score")]
    pub source_reliability: f64,

    /// RFC-3339 timestamp of the last quality re-validation; None if never validated.
    #[serde(default)]
    pub last_validated_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Count of times this entry was contradicted by a competing memory write.
    /// Drives QualityScorer's consistency factor.
    #[serde(default)]
    pub contradiction_count: u32,

    /// CoALA cognitive tier: 1=Reactive, 2=Deliberative (default), 3=Reflective, 4=Meta.
    /// Schema column ships in v8 even though CognitiveLayerManager is shelved (replan §3.3).
    #[serde(default = "default_cognitive_layer")]
    pub cognitive_layer: u8,

    /// JSON-encoded array of contextual tags (project, scope, source_kind, etc.).
    #[serde(default)]
    pub context_tags: Vec<String>,
```

After the struct definition, add the helper functions referenced by `#[serde(default = "...")]`:

```rust
fn default_quality_score() -> f64 {
    0.5
}

fn default_cognitive_layer() -> u8 {
    2
}
```

If these helpers already exist on the worktree branch, copy them verbatim; otherwise use the bodies above.

- [ ] **Step 1.4: Run cargo check — fail because constructors don't include new fields**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | grep -cE "missing fields"
```

Expected: a non-zero count. The struct now requires all fields at construction sites.

If the count is 0, the struct fields might already have `Default` derive that auto-fills them — that's fine, proceed. Otherwise, this confirms we need Tasks 2-7 to update each construction site.

- [ ] **Step 1.5: Commit (struct fields only — does NOT compile yet)**

DO NOT commit yet. The plan keeps the struct change uncommitted until the providers are updated to fill the new fields. The single PR-6a commit (Task 8) bundles all changes atomically.

Proceed to Task 2.

---

## Task 2: PR-6a — Add migrations v7 + v8

**Files:**
- Modify: `src-tauri/src/modules/memory/migrations.rs`

- [ ] **Step 2.1: Inspect the worktree's v7 + v8 migration bodies**

Run:
```bash
git show wt-mem-A0-compile-gate:src-tauri/src/modules/memory/migrations.rs | \
  sed -n '/fn memory_v7_quality_scoring/,/^}/p'
git show wt-mem-A0-compile-gate:src-tauri/src/modules/memory/migrations.rs | \
  sed -n '/fn memory_v8_cognitive_layer/,/^}/p'
```

Capture both function bodies. They are reproduced below for convenience:

```rust
fn memory_v7_quality_scoring(conn: &Connection) -> rusqlite::Result<()> {
    fn add_column_if_not_exists(conn: &Connection, sql: &str) -> rusqlite::Result<()> {
        match conn.execute(sql, []) {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate column name") => Ok(()),
            Err(e) => Err(e),
        }
    }

    add_column_if_not_exists(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN quality_score REAL DEFAULT 0.5",
    )?;
    add_column_if_not_exists(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN source_reliability REAL DEFAULT 0.5",
    )?;
    add_column_if_not_exists(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN last_validated_at TEXT",
    )?;
    add_column_if_not_exists(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN contradiction_count INTEGER DEFAULT 0",
    )?;
    Ok(())
}

fn memory_v8_cognitive_layer(conn: &Connection) -> rusqlite::Result<()> {
    fn add_col(conn: &Connection, sql: &str) -> rusqlite::Result<()> {
        match conn.execute(sql, []) {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("duplicate column name") => Ok(()),
            Err(e) => Err(e),
        }
    }

    add_col(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN cognitive_layer INTEGER DEFAULT 2",
    )?;
    add_col(
        conn,
        "ALTER TABLE memory_entries ADD COLUMN context_tags TEXT DEFAULT '[]'",
    )?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_memory_cognitive_layer ON memory_entries(cognitive_layer);",
    )?;

    // Back-fill cognitive_layer from category for existing rows.
    conn.execute_batch(
        "UPDATE memory_entries SET cognitive_layer = 1 WHERE category = 'conversation';
         UPDATE memory_entries SET cognitive_layer = 2 WHERE category IN ('working', 'daily');
         UPDATE memory_entries SET cognitive_layer = 3 WHERE category IN ('reflection', 'procedural');
         UPDATE memory_entries SET cognitive_layer = 4 WHERE category = 'core';",
    )?;
    Ok(())
}
```

- [ ] **Step 2.2: Append the two functions to vnext's `migrations.rs`**

Open `src-tauri/src/modules/memory/migrations.rs`. Find the last existing migration function (`memory_v6_*` or whatever vnext's highest version is — likely v6). After the closing `}` of the last function, paste both function bodies from Step 2.1.

- [ ] **Step 2.3: Register both migrations in the `MIGRATIONS` array**

Find the `MIGRATIONS` static array (or whatever it's called in vnext — likely `static MIGRATIONS: &[Migration] = &[...]`). At the end of the array, before the closing `]`, add:

```rust
    Migration {
        version: 7,
        name: "quality_scoring_columns",
        up: memory_v7_quality_scoring,
    },
    Migration {
        version: 8,
        name: "cognitive_layer_columns",
        up: memory_v8_cognitive_layer,
    },
```

- [ ] **Step 2.4: Run cargo check — should pass for migrations.rs alone**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | grep -E "^error" | head -10
```

Expected: errors are limited to MemoryEntry construction sites (from Task 1) and possibly from `mod.rs` if it references new fields elsewhere. The migrations file itself should not have errors.

If migrations.rs has its own errors (e.g. `Migration` struct field name mismatch), inspect the in-flight branch's exact `Migration` definition with:
```bash
git show wt-mem-A0-compile-gate:src-tauri/src/modules/memory/migrations.rs | grep -A3 "pub struct Migration"
```
And adjust to match.

Proceed to Task 3.

---

## Task 3: PR-6a — Add `MemoryProvider::update_decay_scores` trait method

**Files:**
- Modify: `src-tauri/src/modules/memory/mod.rs` (trait definition around line 434)

- [ ] **Step 3.1: Inspect the in-flight trait method definition**

Run:
```bash
git show wt-mem-A0-compile-gate:src-tauri/src/modules/memory/mod.rs | \
  grep -B5 -A12 "fn update_decay_scores"
```

- [ ] **Step 3.2: Add the method to the trait body in vnext**

Open `src-tauri/src/modules/memory/mod.rs`. Find `pub trait MemoryProvider` (around line 434). Find a sensible location near the other update-style methods (e.g. after `update_content`). Add:

```rust
    /// Persist updated `importance` and `quality_score` for a single entry.
    /// Used by the forgetting-curve sweep after computing decay (PR-3).
    ///
    /// The default implementation is a no-op so providers that don't yet
    /// support partial field updates remain functional. Production providers
    /// (SQLite) override this to issue an UPDATE; missing key returns
    /// `MemoryError::KeyNotFound`. Does NOT bump `updated_at` — forgetting
    /// modifies decay scores, not user-visible content.
    ///
    /// Spec: docs/superpowers/specs/2026-05-08-a1-pr2-pr6a-memory-foundation-design.md §2.4
    async fn update_decay_scores(
        &self,
        key: &str,
        importance: f64,
        quality_score: f64,
    ) -> Result<(), MemoryError> {
        let _ = (key, importance, quality_score);
        Ok(())
    }
```

- [ ] **Step 3.3: Run cargo check**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | grep -cE "error\["
```

Expected: error count from MemoryEntry construction sites only (from Task 1). The new trait method has a default impl, so providers don't need to override it to compile.

Proceed to Task 4.

---

## Task 4: PR-6a — Update `SqliteMemoryProvider`

**Files:**
- Modify: `src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs` (row-mapping)
- Modify: `src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs` (INSERT/UPDATE + override of `update_decay_scores`)

- [ ] **Step 4.1: Inspect the in-flight sqlite_provider/mod.rs row-mapping**

Run:
```bash
git diff vnext..wt-mem-A0-compile-gate -- \
  src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs
```

Pay attention to the field-index comments (e.g. `12=last_validated_at, 13=contradiction_count`) and the row.get(...) lines. The diff shows additions for indices 9–14 mapping to `quality_score / source_reliability / last_validated_at / contradiction_count / cognitive_layer / context_tags`.

- [ ] **Step 4.2: Apply the row-mapping diff to vnext**

Run:
```bash
git checkout wt-mem-A0-compile-gate -- \
  src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs
```

This brings the worktree branch's row-mapping verbatim onto vnext. Verify with:
```bash
git diff --cached src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs | head -60
```

- [ ] **Step 4.3: Apply the in-flight provider_impl.rs in two passes**

First pull the worktree's version onto vnext:
```bash
git checkout wt-mem-A0-compile-gate -- \
  src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs
```

Then verify the file contains both:
- `update_decay_scores` override
- `increment_contradiction_count` override (already covered in PR-4 plan — but if present in the worktree, that's fine; the override is harmless until the trait method is added in PR-4)

Run:
```bash
grep -n "fn update_decay_scores\|fn increment_contradiction_count" \
  src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs
```

If `increment_contradiction_count` is present, this is fine — it'll match the trait extension when PR-4 adds it. The default-impl-noop pattern means partial states still compile.

If `increment_contradiction_count` references a trait method that doesn't exist on vnext, cargo will fail with `not a member of trait`. In that case, **edit out the override** (delete its function body) and let PR-4 add it back. Specifically:
```bash
# Find the function body
grep -n "async fn increment_contradiction_count" \
  src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs
# Edit the file to delete that fn body if it exists and trait method doesn't yet exist on vnext
```

- [ ] **Step 4.4: Run cargo check**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | grep -E "^error" | head -10
```

Expected: errors limited to other providers (vector_provider, lancedb) constructing `MemoryEntry` without the new fields. SqliteMemoryProvider should now compile cleanly.

If `update_decay_scores` is rejected with "not a member of trait" — verify Task 3 actually added the trait method.

Proceed to Task 5.

---

## Task 5: PR-6a — Update `VectorMemoryProvider`

**Files:**
- Modify: `src-tauri/src/modules/memory/providers/vector_provider.rs`

- [ ] **Step 5.1: Apply the in-flight vector_provider.rs**

Run:
```bash
git checkout wt-mem-A0-compile-gate -- \
  src-tauri/src/modules/memory/providers/vector_provider.rs
```

This brings any fixture-default initializations for the 6 new struct fields. Vector provider uses default trait noop for `update_decay_scores`, so no override needed.

- [ ] **Step 5.2: Run cargo check**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | grep -E "^error" | head -10
```

Expected: remaining errors limited to lancedb / compat / in-memory provider sites. Vector provider should compile cleanly.

Proceed to Task 6.

---

## Task 6: PR-6a — Update `LanceDBMemoryProvider`

**Files:**
- Modify: `src-tauri/src/modules/memory/providers/lancedb.rs`

- [ ] **Step 6.1: Apply the in-flight lancedb.rs**

Run:
```bash
git checkout wt-mem-A0-compile-gate -- \
  src-tauri/src/modules/memory/providers/lancedb.rs
```

The worktree's diff is small (~6 LOC) — just fixture defaults for new struct fields.

- [ ] **Step 6.2: Run cargo check**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | grep -E "^error" | head -10
```

Expected: remaining errors limited to compat.rs / in-memory provider / migration tests.

Proceed to Task 7.

---

## Task 7: PR-6a — Update remaining `MemoryEntry` construction sites

**Files:**
- Modify: `src-tauri/src/modules/memory/compat.rs`
- Modify: `src-tauri/src/modules/memory/mod.rs` (any test fixtures + InMemoryMemoryProvider that constructs MemoryEntry)

- [ ] **Step 7.1: Apply the in-flight compat.rs**

Run:
```bash
git checkout wt-mem-A0-compile-gate -- \
  src-tauri/src/modules/memory/compat.rs
```

- [ ] **Step 7.2: Find remaining construction sites**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | \
  grep -B2 "missing fields" | head -30
```

This identifies any remaining `MemoryEntry { ... }` construction sites that don't fill the new fields. Common locations:
- `mod.rs` — InMemoryMemoryProvider's `store` impl
- Test fixtures inside `#[cfg(test)] mod tests` blocks
- Any decision_tree / retrieval / quality test fixture

For each site, add the 6 fields with their defaults. Pattern:

```rust
MemoryEntry {
    // ... existing fields ...
    quality_score: 0.5,
    source_reliability: 0.5,
    last_validated_at: None,
    contradiction_count: 0,
    cognitive_layer: 2,
    context_tags: Vec::new(),
}
```

OR — if the file exists in the worktree branch with the fixture changes already applied, use:
```bash
git checkout wt-mem-A0-compile-gate -- <path-to-file>
```

When in doubt, prefer the latter (verbatim copy from worktree).

- [ ] **Step 7.3: Repeat cargo check until clean**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | tail -5
```

Repeat Step 7.2 + 7.3 until output ends with `Finished `dev` profile ...` (no errors).

Common remaining sites identified by repeated `cargo check` output:
- `src-tauri/src/modules/memory/decision_tree.rs` — test fixture
- `src-tauri/src/modules/memory/retrieval.rs` — test fixture
- `src-tauri/src/modules/memory/conflict.rs` — exists only on worktree, not on vnext yet (PR-4 will bring it)
- `src-tauri/src/modules/memory/forgetting.rs` — same as above (PR-3)

For files that don't exist on vnext yet (conflict.rs, forgetting.rs, daydream/, evolution/, etc. — see worktree diff), do NOT bring them in this task. They land in subsequent PRs (PR-3, PR-4, PR-5).

The strategy for any remaining error:
1. If error site is in a file that EXISTS on vnext — fix the construction site by adding the 6 default fields.
2. If error site is in a file that DOES NOT exist on vnext — that file shouldn't be referenced; investigate why mod.rs declares it.

- [ ] **Step 7.4: Run cargo check workspace-wide**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --workspace 2>&1 | tail -3
```

Expected: `Finished` line. No errors workspace-wide.

Proceed to Task 8.

---

## Task 8: PR-6a — Add the v7 round-trip integration test

**Files:**
- Modify: `src-tauri/src/modules/memory/migrations.rs` (append to its `#[cfg(test)] mod tests` block)

- [ ] **Step 8.1: Add the test**

Open `src-tauri/src/modules/memory/migrations.rs`. Find the existing `#[cfg(test)] mod tests` block. Append:

```rust
#[tokio::test]
async fn v7_columns_round_trip_via_sqlite_provider() {
    // Spec 2026-05-08-a1-pr2-pr6a-memory-foundation-design.md §5.1:
    // open fresh SQLite, apply all migrations, store entry, verify all
    // 6 new columns have default values, call update_decay_scores,
    // re-read, assert values land. Confirm KeyNotFound on missing key.
    use crate::modules::memory::providers::sqlite_provider::SqliteMemoryProvider;
    use crate::modules::memory::{MemoryCategory, MemoryError, MemoryProvider};
    use tempfile::tempdir;

    let dir = tempdir().expect("tempdir");
    let provider = SqliteMemoryProvider::new(dir.path())
        .await
        .expect("provider init");

    // Store an entry — exercises INSERT path that includes new fields.
    let key = "v7-rt-key";
    provider
        .store(key, "round-trip content", MemoryCategory::Semantic)
        .await
        .expect("store ok");

    // Read back — verify defaults.
    let entries = provider.export(None).await.expect("export ok");
    let entry = entries
        .iter()
        .find(|e| e.key == key)
        .expect("entry present");
    assert!(
        (entry.quality_score - 0.5).abs() < f64::EPSILON,
        "default quality_score should be 0.5, got {}",
        entry.quality_score
    );
    assert!(
        (entry.source_reliability - 0.5).abs() < f64::EPSILON,
        "default source_reliability should be 0.5, got {}",
        entry.source_reliability
    );
    assert!(
        entry.last_validated_at.is_none(),
        "default last_validated_at should be None"
    );
    assert_eq!(entry.contradiction_count, 0);
    // Default cognitive_layer for category=Semantic — backfilled by v8
    // UPDATE: Semantic isn't in the back-fill SQL (only conversation,
    // working/daily, reflection/procedural, core), so the column
    // default of 2 (Deliberative) applies.
    assert_eq!(entry.cognitive_layer, 2);
    assert_eq!(entry.context_tags.len(), 0);

    // Update decay scores — exercises the new trait method.
    provider
        .update_decay_scores(key, 0.7, 0.85)
        .await
        .expect("update_decay_scores ok");

    // Re-read — verify values landed.
    let entries = provider.export(None).await.expect("export ok 2");
    let entry = entries
        .iter()
        .find(|e| e.key == key)
        .expect("entry present 2");
    assert!(
        (entry.importance - 0.7).abs() < f64::EPSILON,
        "importance should be 0.7 after update, got {}",
        entry.importance
    );
    assert!(
        (entry.quality_score - 0.85).abs() < f64::EPSILON,
        "quality_score should be 0.85 after update, got {}",
        entry.quality_score
    );

    // Missing key — should error with KeyNotFound.
    let result = provider
        .update_decay_scores("does-not-exist", 0.5, 0.5)
        .await;
    assert!(
        matches!(result, Err(MemoryError::KeyNotFound(_))),
        "expected KeyNotFound for missing key, got {result:?}"
    );
}
```

- [ ] **Step 8.2: Run the test**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib \
  v7_columns_round_trip_via_sqlite_provider 2>&1 | tail -10
```

Expected: `test result: ok. 1 passed; ...`.

Common failure modes:
- "default cognitive_layer should be 2" fails — the back-fill SQL in v8 sets cognitive_layer based on category. If `MemoryCategory::Semantic` actually serializes to a string matching one of the back-fill rules, the column gets a different value. Adjust the assertion to match actual behavior or use a category not in the back-fill list (e.g. `Episodic`).
- KeyNotFound test fails because the SQL `UPDATE` returned 0 changes but the impl returned `Ok` — verify Step 4.3's provider_impl.rs is the worktree version that explicitly checks `changed == 0`.

- [ ] **Step 8.3: Run all migration tests**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib migrations 2>&1 | tail -5
```

Expected: `test result: ok. ...; 0 failed; ...`.

- [ ] **Step 8.4: Run all memory tests for regression check**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib memory 2>&1 | tail -5
```

Expected: count = baseline (Task 0.4) + 1 new test. No failures.

- [ ] **Step 8.5: Format check the touched files**

Run:
```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- \
  src-tauri/src/modules/memory/migrations.rs \
  src-tauri/src/modules/memory/mod.rs \
  src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs \
  src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs \
  src-tauri/src/modules/memory/providers/vector_provider.rs \
  src-tauri/src/modules/memory/providers/lancedb.rs \
  src-tauri/src/modules/memory/compat.rs
cargo fmt --check --manifest-path src-tauri/Cargo.toml -- \
  src-tauri/src/modules/memory/migrations.rs \
  src-tauri/src/modules/memory/mod.rs \
  src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs \
  src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs \
  src-tauri/src/modules/memory/providers/vector_provider.rs \
  src-tauri/src/modules/memory/providers/lancedb.rs \
  src-tauri/src/modules/memory/compat.rs
```

Expected: fmt --check exits 0 (no diff).

- [ ] **Step 8.6: Commit PR-6a**

Run:
```bash
git add \
  src-tauri/src/modules/memory/migrations.rs \
  src-tauri/src/modules/memory/mod.rs \
  src-tauri/src/modules/memory/providers/sqlite_provider/mod.rs \
  src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs \
  src-tauri/src/modules/memory/providers/vector_provider.rs \
  src-tauri/src/modules/memory/providers/lancedb.rs \
  src-tauri/src/modules/memory/compat.rs

git commit -m "feat(memory): schema foundation v7+v8 + struct ext + trait ext (A.1 PR-6a)

Brings memory schema foundation onto vnext as one atomic commit.
Unblocks PR-3 (forgetting) + PR-4 (conflict) plans (commit 6b2bba6
Task 0 preflight halts on these prerequisites).

Migrations:
- v7 (quality_scoring_columns): adds quality_score / source_reliability
  / last_validated_at / contradiction_count to memory_entries
- v8 (cognitive_layer_columns): adds cognitive_layer (DEFAULT 2 =
  Deliberative) + context_tags + idx_memory_cognitive_layer; back-fills
  cognitive_layer from category for existing rows

MemoryEntry struct: 6 new fields with #[serde(default)] for backward
compat with old serialized payloads.

MemoryProvider trait: adds update_decay_scores(key, importance,
quality_score) with default noop. SqliteMemoryProvider override issues
SQL UPDATE; returns KeyNotFound on missing key; does NOT bump
updated_at (forgetting modifies decay scores, not user-visible content).

Provider impls:
- SqliteMemoryProvider: row-mapping for 6 new columns; INSERT/UPDATE
  populate defaults; update_decay_scores override
- VectorMemoryProvider / LanceDBMemoryProvider: fixture defaults
- compat.rs: ingestion helper for old payloads

NEW test v7_columns_round_trip_via_sqlite_provider exercises:
- store + read defaults (0.5 / None / 0 / 2 / [])
- update_decay_scores happy path (0.7 / 0.85 round-trip)
- update_decay_scores missing-key path (KeyNotFound error)

CognitiveLayerManager NOT shipped (per replan §3.3 — schema column
ships, manager defers). PR-6b (v5 FTS + v6 conv-recall embeddings)
also deferred (separate consumer chain).

Refs:
- Spec: docs/superpowers/specs/2026-05-08-a1-pr2-pr6a-memory-foundation-design.md
- Replan: docs/superpowers/plans/2026-05-05-memory-evolution-product-replan.md §3.3 §3.6
- Source: origin/feat/in-flight-A-memory @ 8674725 via wt-mem-A0-compile-gate"
```

Verify:
```bash
git log --oneline -1
git diff --shortstat HEAD~1..HEAD
```

Expected: 1 new commit; ~700 LOC added across 7 files.

Proceed to Task 9.

---

## Task 9: PR-2 — Bring `quality.rs` into vnext

**Files:**
- Create: `src-tauri/src/modules/memory/quality.rs`
- Modify: `src-tauri/src/modules/memory/mod.rs` (declare module + re-export)

- [ ] **Step 9.1: Copy `quality.rs` from worktree**

Run:
```bash
git show wt-mem-A0-compile-gate:src-tauri/src/modules/memory/quality.rs \
  > src-tauri/src/modules/memory/quality.rs
```

Verify:
```bash
wc -l src-tauri/src/modules/memory/quality.rs
```

Expected: 212 lines.

- [ ] **Step 9.2: Register the module in `mod.rs`**

Open `src-tauri/src/modules/memory/mod.rs`. Find the existing `pub mod` declarations. Add:

```rust
pub mod quality;
```

After the new `pub mod` line (or near the bottom of the module declarations block), add:

```rust
pub use quality::{QualityScorer, QualityWeights};
```

To find the right location:
```bash
grep -n '^pub mod \|^pub use ' src-tauri/src/modules/memory/mod.rs | head -20
```

- [ ] **Step 9.3: Run cargo check**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib 2>&1 | tail -3
```

Expected: `Finished` clean. quality.rs imports `MemoryEntry` from the parent module (already extended in Task 1) and `chrono::DateTime<Utc>` (existing in workspace).

If the check fails with import errors, the worktree's quality.rs may reference symbols not on vnext (e.g. evolution module). Inspect:
```bash
head -30 src-tauri/src/modules/memory/quality.rs
```

- [ ] **Step 9.4: Run quality tests**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib quality 2>&1 | tail -5
```

Expected: `test result: ok. 5 passed; 0 failed; ...` (the 5 in-flight unit tests in quality.rs).

- [ ] **Step 9.5: Run all memory tests for full regression**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib memory 2>&1 | tail -5
```

Expected: count = (baseline from Task 0.4) + 1 (Task 8 round-trip) + 5 (quality) = baseline + 6. All passing.

- [ ] **Step 9.6: Format check**

Run:
```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- \
  src-tauri/src/modules/memory/quality.rs \
  src-tauri/src/modules/memory/mod.rs
cargo fmt --check --manifest-path src-tauri/Cargo.toml -- \
  src-tauri/src/modules/memory/quality.rs \
  src-tauri/src/modules/memory/mod.rs
```

Expected: fmt --check exits 0.

- [ ] **Step 9.7: Commit PR-2**

Run:
```bash
git add src-tauri/src/modules/memory/quality.rs src-tauri/src/modules/memory/mod.rs

git commit -m "feat(memory): QualityScorer + QualityWeights (A.1 PR-2)

Brings src/modules/memory/quality.rs (212 LOC verbatim from in-flight
branch) into vnext. Stateless scorer over MemoryEntry consuming the
6 new fields PR-6a added.

Public surface:
- QualityWeights { source / freshness / usage / consistency } with
  Default summing to 1.0 (0.3 / 0.2 / 0.25 / 0.25)
- QualityScorer { weights }
  · new() / with_weights(...)
  · score(&entry, now) -> f64 in [0, 1]
  · batch_rescore(&mut entries, now)

Formula:
  source      = entry.source_reliability
  freshness   = 1 / (1 + hours_since_update / 168)   // 1-week half-life
  usage       = min(1, access_count / 50)            // caps at 50
  consistency = 1 / (1 + contradiction_count)        // 50%-per-bump
  final       = (weighted sum).clamp(0, 1)

5 in-flight unit tests preserved: high-quality scores high, high
contradiction count lowers score, old freshness lowers score, heavy
usage raises score, custom weights affect outcome.

Module registered via 'pub mod quality;' + 'pub use
quality::{QualityScorer, QualityWeights};' re-export.

This is the second of two atomic commits unblocking PR-3 + PR-4
(commit 6b2bba6 Task 0 preflight). PR-3 forgetting sweep is the
first real consumer of QualityScorer::score.

Refs:
- Spec: docs/superpowers/specs/2026-05-08-a1-pr2-pr6a-memory-foundation-design.md §2.6
- Replan: docs/superpowers/plans/2026-05-05-memory-evolution-product-replan.md §3.6
- Source: origin/feat/in-flight-A-memory @ 8674725 via wt-mem-A0-compile-gate"
```

Verify:
```bash
git log --oneline -2
git diff --shortstat HEAD~1..HEAD
```

Expected: 2 new commits total (PR-6a + PR-2); PR-2 commit shows ~225 LOC added (212 quality.rs + ~13 mod.rs).

---

## Task 10: Final verification

After Tasks 8 + 9 land, run a full regression sweep:

- [ ] **Step 10.1: Full backend test suite**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib 2>&1 | tail -5
```

Expected: all tests pass; no regressions outside the memory module.

- [ ] **Step 10.2: Workspace check + clippy**

Run:
```bash
cargo check --manifest-path src-tauri/Cargo.toml --workspace 2>&1 | tail -3
cargo clippy --manifest-path src-tauri/Cargo.toml --lib 2>&1 | \
  grep -E "^error|^warning" | grep -E "memory/migrations|memory/mod\.rs|memory/quality|memory/compat|memory/providers" || echo "clean"
```

Expected: `Finished` line + `clean` for the touched files (pre-existing baseline clippy debt in unrelated files is acceptable).

- [ ] **Step 10.3: Frontend untouched check**

Run:
```bash
npm test 2>&1 | tail -5
npm run build:web 2>&1 | tail -3
```

Expected: both green. (No frontend changes in this plan.)

- [ ] **Step 10.4: Spec acceptance criteria**

Confirm against design spec §8:

PR-6a:
- ✓ `cargo check --workspace` clean
- ✓ `cargo test --lib memory::migrations` passes (new + existing)
- ✓ `cargo test --lib memory` no regression
- Manual smoke: open fresh app, observe `~/.if2ai/memory.db` schema includes all 6 new columns — record in commit body if executed; otherwise defer to QA

PR-2:
- ✓ `cargo check --workspace` clean
- ✓ `cargo test --lib memory::quality` passes (5/5)
- ✓ `pub use quality::{QualityScorer, QualityWeights}` resolvable

If all green, the PR-2 + PR-6a series is complete and unblocks the PR-3 + PR-4 plan (`6b2bba6` Task 0 preflight should now pass on next run).

---

## Self-review notes

- **Spec coverage**: §2.1 PR-6 split → Tasks 2/8 (only v7+v8 land); §2.2 schema → Task 2; §2.3 struct fields → Task 1; §2.4 trait method → Task 3; §2.5 provider impls → Tasks 4/5/6/7; §2.6 quality.rs → Task 9; §2.7 round-trip test → Task 8.
- **Type consistency**: `MemoryEntry` field set used uniformly across all task code blocks (`quality_score: f64`, `source_reliability: f64`, `last_validated_at: Option<DateTime<Utc>>`, `contradiction_count: u32`, `cognitive_layer: u8`, `context_tags: Vec<String>`). The `default_quality_score` / `default_cognitive_layer` helper function names match across Task 1 introduction and any subsequent reference. `update_decay_scores` signature is identical in Task 3 trait def and Task 4 SqliteMemoryProvider override.
- **Placeholder scan**: no TBD/TODO; all code blocks complete; all commands concrete. Task 7 has a "find remaining sites" search-loop pattern (acceptable because the exact list depends on what's on vnext at execution time; the loop terminates on cargo check passing).
- **Scope check**: 2 atomic commits (PR-6a then PR-2); PR-6b explicitly out of scope; CognitiveLayerManager out of scope; PR-3/PR-4/PR-5 out of scope.
