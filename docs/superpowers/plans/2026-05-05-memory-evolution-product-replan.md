# 2026-05-05 — Memory + Evolution 产品复议（Product Replan）

> **Source archive**: `origin/feat/in-flight-A-memory @ 8674725` ("wip(in-flight topic split): Memory subsystem rework + Daydream/Evolution/Cognitive new modules + Memory UI panels")
> **Diff scope vs `vnext @ b390580`**: 54 files / **+9,859 / −765 LOC**
> **Workflow**: Superpowers SKILLS — `brainstorming` → `writing-plans` → `executing-plans` → `verification-before-completion` → `requesting-code-review` → `finishing-a-development-branch`. **Pack workflow is deprecated** per `CLAUDE.md`.

> **⚠ Important correction to original brief**: Brief stated "3 todo!() match arms blocking compile". Agent grep'd `todo!()` / `unimplemented!` across `src-tauri/src/` on branch tip and found **zero** at the cited sites (`memory_injection_service.rs:353` is file's last line; `memory_recall_assembler.rs:87` is doc comment; `memory/inject.rs:446` is fully-implemented `fetch_procedural_section`). The only `unimplemented!()` are pre-existing test-only stubs. **Real compile blocker is HARD COLLISION with vnext DT-01 work** (branch deletes files vnext just rewrote). **Wave A.0 must run `cargo check` against rebased worktree before any product decisions.**

---

## 1. 总览 & TL;DR

This document re-frames the 9.8K-LOC `feat/in-flight-A-memory` archive from a single un-shippable commit into a multi-wave roadmap with explicit product propositions, KILL/SHELVE/SHIP verdicts per sub-feature, and Superpowers SKILLS-aligned brainstorming → planning → execution cycles.

**Why replan:**
1. The branch bundles 8 distinct sub-features (daydream, evolution, cognitive, graph, forgetting, quality, conflict, UI panels) into one commit with no individual product proposition — impossible to review, impossible to roll back at sub-feature granularity.
2. **Architectural collision with vnext**: the branch deletes `agent_loop_integration.rs` (−88), shrinks `runlog_projection.rs` (380→57), and removes `skills/sedimentation/mod.rs` (−127) — all three were rewritten/extended on vnext by DT-01 S1.1/S1.2/S1.3 PRs (`401828f`, `7ef0d98`, `58932ca`) and the D+E ship (`b390580`). Naive merge silently reverts weeks of harness + skills work.
3. **9,800 LOC / 1 commit** violates the 500-LOC-per-PR Superpowers convention and `requesting-code-review` workflow.
4. Several sub-features (cognitive, graph) lack a stated user problem in code or docs — need brainstorming, not implementation.
5. Compile health unverified post-rebase against current vnext; **must be the first wave step**.

**TL;DR verdict matrix** (full table in §9):

| Sub-feature                          | Verdict       | Wave |
|--------------------------------------|---------------|------|
| Quality scoring (`quality.rs`)       | **SHIP**      | A.1  |
| Forgetting curve (`forgetting.rs`)   | **SHIP**      | A.1  |
| Migration runner v5–v8               | **SHIP**      | A.1  |
| Cognitive layer manager              | **SHELVE**    | —    |
| Conflict detector/resolver           | **SHIP-LITE** | A.1  |
| Memory graph + IPC + UI panel        | **SHELVE**    | —    |
| Daydream engine + scheduler + UI     | **SHIP**      | A.2  |
| Evolution (trajectory + reflector)   | **SHIP**      | A.3  |
| Procedural memory manager            | **SHIP**      | A.1  |
| 4 React UI panels                    | **SPLIT**     | A.2 / A.3 / A.4 |
| Candidate-extractor LLM auto-extract | **DEFER**     | C-branch / A.5 |
| Harness/skills file deletions        | **REJECT**    | —    |

---

## 2. 当前 in-flight 资产清单

Numbers from `git diff vnext..origin/feat/in-flight-A-memory --stat`. Completeness = compiles in branch / has tests / has IPC / has UI.

### 2.1 Foundation modules (no LLM dependency)

| File | LOC delta | Description | Tests | IPC | UI | Status |
|---|---|---|---|---|---|---|
| `memory/quality.rs` | **+212 NEW** | Stateless `QualityScorer` — composite score (source_reliability / freshness / usage / consistency). | 5 unit | none | indirect | **complete** |
| `memory/forgetting.rs` | **+380 NEW** | Ebbinghaus retention sweep + budget-select. Wired into `MemoryTicker::with_forgetting_runtime`. | inline | none | none | **complete, ticker-wired** |
| `memory/conflict.rs` | **+497 NEW** | Conflict detector + resolver (KeepNewer / KeepNew / KeepExisting / Merge / AskUser / KeepBothWithFlag). | yes | none | none | **complete except `KeepBothWithFlag` only logs** (provider trait lacks contradiction-count method) |
| `memory/migrations.rs` | +91 (extend) | Adds v5–v8 (FTS, conv embeddings, quality cols, cognitive layer cols). Generic runner already in vnext. | 5 tests | n/a | n/a | **complete** |
| `memory/cognitive.rs` | **+618 NEW** | CoALA 4-tier capacity manager: Reactive→Deliberative→Reflective→Meta. | partial | none | none | **complete code, no consumer in bootstrap** |
| `memory/graph.rs` | **+427 NEW** | BFS over `memory_links`, link discovery, neighborhood. | partial | yes | yes (`MemoryGraphPanel`) | **complete vertical** |
| `memory/decision_tree.rs` | +17 | Add `MemoryCategory::Procedural` arm. | n/a | n/a | n/a | **complete** |
| `memory/inject.rs` | +422 | Adds procedural memory injection block + cache hint plumbing. | unclear | n/a | n/a | **needs B-branch `CacheHint` type** |
| `memory/mod.rs` | +225 | Module registration, `Procedural` enum arm, `CognitiveLayer` enum, link/graph types. | n/a | n/a | n/a | **complete** |

### 2.2 Daydream pillar

| File | LOC | Description | Status |
|---|---|---|---|
| `daydream/mod.rs` | 77 | `DayDreamEngine` aggregate (scheduler + consolidator). | complete |
| `daydream/scheduler.rs` | 210 | Idle-detection 60-s monitor + manual trigger + history (≤50). | complete |
| `daydream/consolidator.rs` | 798 | Prune / Merge / Refresh / Reflect / cognitive-enforce / graph-discover 6-step pipeline. | complete (skips reflection/cognitive/graph unless wired) |
| `daydream/reflector.rs` | 289 | Bridges `evolution::SelfReflector` + `ProceduralMemoryManager`. | depends on evolution |
| `daydream/report.rs` | 134 | DTOs: state, config, prune/merge/refresh reports, full cycle report. | complete |
| `commands/memory/daydream.rs` | 58 | 5 IPC: status / trigger / config get / config set / history. | complete |
| `bootstrap/memory.rs` | +28 | `DayDreamEngine` constructed but **not started** (no `engine.start()` call). | **wiring gap** |
| `src/components/memory/DayDreamPanel.tsx` | 460 | Status + manual trigger + config form + history list. | complete React |

### 2.3 Evolution pillar

| File | LOC | Description | Status |
|---|---|---|---|
| `evolution/mod.rs` | 23 | Module registration. | complete |
| `evolution/trajectory.rs` | 414 | `TrajectoryCollector` + Trajectory/TurnRecord/ToolCallRecord/AgentAction/TaskOutcome. | complete data model, **no producer** wires turns into the collector — depends on C-branch `after_turn` |
| `evolution/reflector.rs` | 870 | `SelfReflector::run_reflection_cycle` extracts `Insight`s, finds `Pattern`s. Rule-based + optional LLM. | complete |
| `evolution/procedural.rs` | 427 | `ProceduralMemoryManager`: insight → procedural memory → trust score → prompt-injection format. | complete |
| `commands/memory/graph.rs` | 223 | (also covers evolution insights via shared graph IPC) | complete |
| `src/components/memory/EvolutionTimeline.tsx` | 374 | Trajectory list + per-trajectory turn drill-down. | complete |
| `src/components/memory/InsightPanel.tsx` | 430 | Insights + procedural-memory trust scores. | complete |

### 2.4 UI panels & API surface

| File | LOC | Status |
|---|---|---|
| `src/api/memory.ts` | +402 | Adds 5 daydream hooks + 4 graph hooks + types. **Imports from `@/lib/tauri` collide with RD-01** |
| 4 panels (DayDream / EvolutionTimeline / Insight / MemoryGraph) | 1,790 total | complete |
| `LearnedTraits` / `MemoryChip` tweaks | +159 | small |
| `MemorySettingsPage.tsx` | +49 | Tab integration |

### 2.5 Hot-file deletions (collision risk)

| File | Δ | Why deleted/shrunk on branch | Conflict |
|---|---|---|---|
| `harness/agent_loop_integration.rs` | −88 (deleted) | Pre-DT-01 cleanup | **HARD COLLISION** with vnext DT-01 |
| `harness/runlog_projection.rs` | 380→57 (−323) | Re-scoped before DT-01 S1.1 | **HARD COLLISION** — vnext just expanded this |
| `skills/sedimentation/mod.rs` | −127 (deleted) | Stale Phase-1 prototype | conflicts with `b390580` ship |
| `turn_service/{run,stream_finalize,stream_tool_execution}.rs` | +51 / −18 / +78 | Threads trajectory + procedural injection | overlaps GF-02 |
| `memory_candidate_extractor.rs` | +349 | LLM auto-extract + assistant-output extract | overlaps C-branch |
| `tests/{agentic_loop_unit, force_text_after_truncations, prompt_cache_hit}.rs` | each −2 | Likely import cleanup; superseded by `b390580` E ship | small |

### 2.6 The "3 todo!()" claim — disposition

Searched and **not found** on branch tip. Probable history: branch already replaced placeholder `todo!()` arms with real `MemoryCategory::Procedural` handling (visible in `decision_tree.rs:332/346` and `mod.rs:148/156`). **Wave A.0 must verify by `cargo check` against worktree of branch rebased on vnext.**

---

## 3. 产品命题 (the most important section)

For each sub-feature: **User problem → Behavior → Success metric → Risk if NOT shipped → Verdict**.

### 3.1 Daydream

- **User problem**: After many turns, agent's memory store fills with low-value, redundant, or obsolete entries that crowd prompt budget and dilute recall quality. User does not want manual curation.
- **Behavior**: While user is idle (>15 min), or at session end: Prune (drop low-retention/quality) → Merge (collapse near-duplicates) → Refresh (touch stale-but-valuable) → Reflect (extract procedural rules from trajectories) → enforce CoALA capacity → discover graph links. Settings page surfaces "Daydream" tab with state, manual trigger, history, aggressiveness slider.
- **Success metric**: 7-day average memory store size stable or shrinking despite continued turns; injected memory section quality_score average ≥ 0.5; user-reported "agent remembers right things" (qualitative A/B with daydream off).
- **Risk if NOT shipped**: Memory store grows unbounded; recall precision decays; prompt budget wasted on stale entries; 8B's quality scoring + 8C learned-traits do not have a janitor to compact what they produce.
- **Verdict**: **SHIP** (Wave A.2). Strongest product proposition. Idle-trigger threshold and LLM token budget must be user-visible.

### 3.2 Evolution (trajectory + reflector + procedural memory)

- **User problem**: Agent makes the same mistake twice. No mechanism to learn "when X happens, do Y" from successful or failed turn sequences.
- **Behavior**: Each turn appends a `TurnRecord` to a `Trajectory`. On task completion or daydream cycle, `SelfReflector` extracts `Insight`s (HeuristicRule / AntiPattern / BestPractice / UserPreference / ToolUsagePattern). `ProceduralMemoryManager` internalizes each as a `Procedural`-category memory with trust_score 0.3 → 1.0 / −1.0 via subsequent task validation. Injected system prompt gains `## Procedural Rules (learned from experience)` section.
- **Success metric**: After 50 trajectories, ≥10 procedural rules with trust_score >0.5; user "agent figured out my workflow". Negative metric: prompt token cost increase < 200 tokens p95.
- **Risk if NOT shipped**: Manual procedural memory works but requires explicit teaching. Agent never auto-learns.
- **Verdict**: **SHIP** (Wave A.3). Dependency: working `after_turn` pipeline that calls `TrajectoryCollector::record_turn` and (on task end) `SelfReflector::run_reflection_cycle` — this is C-branch's extension (A.5). Procedural memory itself can land in A.1 (stateless transform).

### 3.3 Cognitive layer manager (CoALA 4-tier)

- **User problem**: Stated as "manage capacity, eviction, inter-layer promotion, procedural expiry across four cognitive tiers". User-facing problem: **not stated**.
- **Behavior**: Reactive (≤20) → Deliberative (≤100) → Reflective (≤200) → Meta (∞). Capacity overflow evicts lowest-scoring or promotes high-quality up. No UI; consulted only by `daydream/consolidator.rs` step 5.
- **Success metric**: **None defined.**
- **Risk if NOT shipped**: Same outcome as forgetting curve + capacity policy on provider — already partially covered.
- **Verdict**: **SHELVE**. 4-tier taxonomy is research-level abstraction (CoALA paper). Overlaps existing `MemoryCategory` enum + `CognitiveLayer` v8 column. Without user-visible scenario, 618 LOC has no value test. **Recommendation**: keep schema column (v8 ships in A.1) and `CognitiveLayer` enum, but **do not wire `CognitiveLayerManager` until brainstorming produces a user story**. Run capacity enforcement on `MemoryCategory` directly in A.1 if needed.

### 3.4 Memory knowledge graph

- **User problem**: Stated as "BFS traversal, relationship discovery, graph-enhanced retrieval". User-facing: **not stated** beyond "show a graph".
- **Behavior**: 4 IPC commands, neighborhood lookup, link discovery, 526-LOC React panel.
- **Success metric**: **None defined.**
- **Risk if NOT shipped**: `memory_links` table (v2) and `MemoryLink` already exist on vnext for `memory_link` / `memory_consolidate` tools. No regression.
- **Verdict**: **SHELVE**. 1,176 LOC for "look at a pretty graph". `find_related` (semantic recall + link hydration) has the strongest case but is a 30-LOC method that can be re-extracted. **Brainstorming**: would users prefer "find related memories to this one" surfaced as a chip on a memory card rather than a separate tab?

### 3.5 Forgetting curve

- **User problem**: Old memories should fade unless reinforced; otherwise a 6-month-old fact dominates yesterday's fact.
- **Behavior**: Every 12h, `MemoryTicker` runs `ForgettingCurveEngine::sweep` — recompute retention = `importance × e^(−λ·t_hours) × (1+α·ln(access+1))`, archive entries below threshold by halving quality_score.
- **Success metric**: Old (>30-day) untouched memories' average quality_score < 0.3; recall surfaces yesterday's facts before week-old facts.
- **Risk if NOT shipped**: Recall ranking biased toward stale memories; `quality_score` field exists but never decays.
- **Verdict**: **SHIP** (Wave A.1). Self-contained, testable, ticker-wired, no UI required.

### 3.6 Quality scoring

- **User problem**: Provider has no notion of "this memory is more reliable than that one". Conflict resolution and recall ranking need a comparable score.
- **Behavior**: `QualityScorer::score` = weighted sum of `source_reliability + freshness + usage_frequency + consistency`. Computed lazily by ticker sweep + by `after_turn` writes.
- **Success metric**: Conflict resolver picks higher-quality entry on auto-resolve >80% of cases (A/B vs naive newest-wins).
- **Risk if NOT shipped**: Conflict resolution falls back to "keep newer", losing high-quality older memories; daydream prune step has no signal beyond access_count.
- **Verdict**: **SHIP** (Wave A.1). 212 LOC, 5 unit tests, foundational to conflict + forgetting + daydream.

### 3.7 Conflict detection & resolution

- **User problem**: Two writes about the same topic create silent contradictions ("user is in Shanghai" + "user is in Tokyo" — both stored, both recalled).
- **Behavior**: Before persisting, detect candidates ≥0.75 bigram similarity; classify Duplicate / PartialOverlap / Contradiction; resolve via `KeepNew/Existing/Merge/AskUser/KeepBothWithFlag`.
- **Success metric**: Duplicate-detection rate (`memory_export | grep duplicate` count drops 50%+).
- **Risk if NOT shipped**: Memory contradictions survive forever; daydream merge step does the same job lazily but synchronous write path stays sloppy.
- **Verdict**: **SHIP-LITE** (Wave A.1). Ship without `KeepBothWithFlag` (currently only logs because `MemoryProvider` trait lacks "increment contradiction_count" method — needs one provider-trait extension first). Defer `Merge` action (LLM-bounded merged_content generation is its own scope).

### 3.8 Procedural memory manager

- **User problem**: How do `Insight`s become persisted prompt-injectable rules?
- **Behavior**: `internalize_insight(insight)` → upsert `Procedural`-category memory with key `proc:{category}:{hash}`, initial trust 0.3, dedup via 0.7-overlap. `format_for_prompt_injection(scope, max=10)` → markdown block.
- **Success metric**: Procedural rules with trust >0.5 reach steady-state count 5–20 per project.
- **Risk if NOT shipped**: Evolution insights have nowhere to go.
- **Verdict**: **SHIP** (Wave A.1, even before evolution producer). Pure transform — once shipped, evolution producers in A.3 plug in cleanly.

### 3.9 4 React UI panels

- **DayDreamPanel** (460 LOC): SHIP with A.2.
- **EvolutionTimeline** (374 LOC): SHIP with A.3.
- **InsightPanel** (430 LOC): SHIP with A.3.
- **MemoryGraphPanel** (526 LOC): SHELVE with §3.4.

### 3.10 LLM auto-extract candidate path (`memory_candidate_extractor.rs +349`)

- **User problem**: Memory entries currently come almost exclusively from explicit `memory_store` tool calls. Real memory extraction should be implicit ("user mentioned they're moving to Tokyo next month — capture that").
- **Behavior**: After-turn pipeline scans assistant text >20 chars, prompts utility LLM to extract candidates with `object_kind` inference.
- **Success metric**: Auto-extracted candidates per turn ≤ 2 (precision over recall); user "I don't agree" rate < 10%.
- **Verdict**: **DEFER** to C-branch (Wave A.5). High collision with current C-branch work; cost analysis (extra utility LLM call per turn) needs explicit brainstorming.

---

## 4. 拆分提议 (Wave structure)

Each wave = `brainstorming` → `writing-plans` → `executing-plans` (with `subagent-driven-development` for parallel slices) → `verification-before-completion` → `requesting-code-review` → `finishing-a-development-branch`. PRs ≤ 500 LOC each. Each wave's first PR opens with `using-git-worktrees` invocation against fresh branch off vnext.

### Wave A.0 — Compile health gate (DO FIRST)

- **Goal**: Verify branch compile state against current vnext, document actual diff vs original "3 todo!()" claim.
- **Steps** (1 PR, 0–50 LOC):
  1. Create worktree `wt-mem-A0-compile-gate` off vnext.
  2. Cherry-pick or rebase `8674725` onto vnext-tip; resolve harness/skills collisions by **rejecting in-flight deletions** (keep vnext's DT-01 expansions + b390580 D+E ship).
  3. Run `cargo check --workspace` and document every error.
  4. Save compile-error log as `docs/superpowers/notes/2026-05-05-A0-mem-compile-health.md`.
- **PR count**: 1 (note-only).
- **LOC**: ~50.
- **Brainstorming questions**: none — pure mechanical gate.
- **Risk-mitigation tests**: `cargo check`, `cargo test --workspace --no-run`.
- **Success criteria**: Clear public list of compile errors + ordered fix backlog. If zero errors, declare A.0 done.

### Wave A.1 — Memory subsystem foundation

- **Goal**: Ship `quality.rs`, `forgetting.rs`, `conflict.rs` (lite), `migrations.rs` v5–v8, `evolution/procedural.rs`, plus `MemoryCategory::Procedural` arm fixes. **No daydream, no graph, no cognitive manager, no UI.**
- **Scope**:
  - 6 files cherry-picked from in-flight (~1,800 LOC ship-ready).
  - 1 trait extension: `MemoryProvider::increment_contradiction_count(key) -> Result<()>` so `KeepBothWithFlag` becomes executable.
  - Bootstrap wiring of `MemoryTicker::with_forgetting_runtime` (already on branch).
- **Dependencies**: A.0 only.
- **Brainstorming questions** (resolve before `writing-plans`):
  1. Forgetting `lambda` and `archive_threshold` defaults — what's user-visible behaviour at 0.1 / 0.05 / 0.2? Pick one + document.
  2. Conflict `auto_resolve_threshold` default 0.3 — does test corpus show this is right gap?
  3. Should v8 cognitive_layer column ship even though manager shelved? **Yes** (cheap, forward-compatible, used as tag).
  4. Should `KeepBothWithFlag` ship now (with new trait method) or defer? Prefer **ship now**, simple ALTER + UPDATE.
- **Risk-mitigation tests**:
  - Migration round-trip: open in-memory DB, apply v1–v8, open v0 DB simulating pre-P0 install, verify backfill, verify rerun is no-op (existing).
  - **New**: SQLite WAL durability — kill mid-migration, reopen, verify partial rollback.
  - Quality scorer property test: changing access_count / contradiction_count / freshness moves score expected direction.
  - Forgetting sweep on 10K-entry fixture: completes <1s, archived count == expected.
- **Success criteria**: `cargo test --lib` green; manual "store 100 memories → wait 12h → verify forgetting sweep ran via audit log".
- **Estimated PR count**: 4 (migration v5-v8 / quality+forgetting / conflict-lite / procedural-mgr).
- **Estimated LOC** (after trim): ~2,000.

### Wave A.2 — Daydream feature

- **Goal**: Ship daydream pipeline (sans reflection step, depends on A.3) + scheduler + 5 IPCs + `DayDreamPanel`.
- **Scope**: 1,768 LOC backend + 460 LOC panel + 58 LOC IPC + ~30 LOC bootstrap wiring (`engine.start()` on app boot).
- **Dependencies**: A.1.
- **Brainstorming questions**:
  1. **When does daydream trigger?** Default 15 min idle borrowed from human sleep cycles — fits typical session pattern? Should there be a "kick" hotkey?
  2. **Cost cap.** Each cycle calls utility LLM 0–N times. What's worst-case token cost per cycle and how do we expose to user?
  3. **Failure visibility.** If a daydream cycle errors, what does user see? Toast? Settings-tab badge?
  4. **Reflection step disabled in A.2** — clean degradation? (Existing code path already gates on `with_reflector` being attached.)
- **Risk-mitigation tests**:
  - Scheduler concurrency: `trigger_manual` while cycle mid-flight returns "already running" error (existing CAS guard).
  - Long-cycle abort: 5-min consolidation cycle still releases running flag on panic.
  - LLM failure: `with_llm` returns garbage → consolidator falls back to rule-based merge.
- **Success criteria**: 24-hour soak: app left idle, cycle fires once, prune/merge counts logged, store size shrinks, no panics.
- **Estimated PR count**: 4 (scheduler+report / consolidator core / IPC+bootstrap+UI / soak-test fixtures).
- **Estimated LOC**: ~2,400.

### Wave A.3 — Evolution feature (trajectory + reflector + insight UI)

- **Goal**: Ship `evolution/{trajectory,reflector}.rs` + `commands/memory/graph.rs` insight subsection + `EvolutionTimeline` + `InsightPanel`. **Procedural memory manager already in A.1**, so insights have somewhere to land.
- **Scope**: 1,284 LOC backend + 804 LOC UI + ~150 LOC IPC.
- **Dependencies**: A.1, **B-branch's `CacheHint`**, and turn-level producer that records `TurnRecord`s into `TrajectoryCollector` — **C-branch dependency is critical**.
- **Brainstorming questions**:
  1. **Trajectory accumulation cost.** `Arc<RwLock<HashMap>>` with one record/turn × N sessions × M turns becomes memory hog. Eviction policy? (Branch has none.)
  2. **Trust score curve.** +0.1 on success, −0.15 on fail — asymmetric demotion right? Brainstorm degenerate case.
  3. **Insight category boundaries.** HeuristicRule vs BestPractice — useful to user, or over-classifying?
  4. **Privacy.** Trajectories include user input summaries. Confirm threat-scanner/PII-redaction covers them.
- **Risk-mitigation tests**:
  - Producer integration: simulated 10-turn trajectory → reflection → verify insights match expected categories.
  - Procedural promotion: 5 successful invocations → trust 0.3 → 0.8; 3 failed → demotion symmetric.
  - Memory pressure: 1,000 trajectories → collector size capped, oldest evicted (must add cap).
- **Success criteria**: Manual "make agent fail at task X 3 times, then succeed once, observe HeuristicRule insight in `InsightPanel` with trust_score promoted on next success".
- **Estimated PR count**: 4 (trajectory + collector cap / reflector / insight IPC + panel / timeline panel).
- **Estimated LOC**: ~2,200.

### Wave A.4 — Memory UI consolidation (RD-01 alignment)

- **Goal**: Take 4 panels delivered in A.2/A.3, consolidate import surface to comply with **RD-01** (shrink `lib/tauri.ts`). Move daydream/graph/insight DTOs out of `lib/tauri.ts` into `api/memory.ts` (or new `api/memory.daydream.ts`).
- **Scope**: pure FE refactor; ~400 LOC moved, no new logic. `MemoryGraphPanel` excluded (shelved).
- **Dependencies**: A.2, A.3.
- **Brainstorming questions**: How does this interact with parallel RD-01 lib/tauri shrinkage? Confirm whether RD-01 owner has migrated `memory*` IPCs or this is first pass.
- **Success criteria**: `lib/tauri.ts` LOC strictly less; no React component imports memory IPC types from `@/lib/tauri`.

### Wave A.5 — C-branch follow-on (after-turn extract pipeline)

- **Goal**: Land in-flight branch's `memory_candidate_extractor.rs +349` extensions on top of C-branch, not as part of A.
- **Scope**: 349 LOC, only relevant once C-branch's `after_turn` skeleton merges.
- **Dependencies**: A.1, C-branch tip.
- **Brainstorming questions**: §3.10 listed.
- **Risk**: collision with C-branch authoring; coordinate with C-branch owner before forking.
- **Verdict**: parallel-eligible after A.1.

### Wave A.shelf — shelved sub-features

- **Cognitive manager + tier UI**: archive in `docs/superpowers/shelved/2026-05-05-cognitive-tier-manager.md` with reasoning + original branch SHA.
- **Memory graph engine + IPC + panel**: same — `2026-05-05-memory-graph-shelf.md`.

---

## 5. 架构风险清单

### 5.1 SQLite migration safety (v5–v8)

- **Forward**: Migration runner + per-migration tx wraps each `up`. Tests exist for fresh install, upgrade, no-op rerun, rollback on failure.
- **Backward**: **No down-migrations.** User who installs v8 build then downgrades to v7 will see "unknown column cognitive_layer" warnings. Document as known limitation; v7 still functions because doesn't SELECT the v8 column.
- **Downgrade test plan**: Currently absent. **Add a test** that opens v8 DB with v7-only migrations slice — verify v7 binary still reads `memory_entries` rows successfully.
- **WAL durability**: Add kill-mid-migration test (panic in up function) and reopen — verify transactional rollback held.

### 5.2 IPC compatibility & schema versioning

- New IPCs: 5 daydream + 4 graph = 9 commands.
- No `schema_version` field on wire; payload shapes mirror `derive(Serialize, Deserialize)`. Adding fields forward-compatible; **renaming fields silently breaks FE**. Document: any field rename is breaking change requiring major IPC version bump.
- 4 React panels currently import via barrel re-export from `@/lib/tauri` — collides with **RD-01**. Wave A.4 fixes.

### 5.3 `runtime_event` envelope additions

- Branch does **not** add new `event_type` family for daydream/evolution/cognitive — emits only via `tracing::*` macros + IPC return values, not canonical run-log.
- **Risk**: violates **DT-01**'s "single source of truth = run-log" invariant. Future audit (DT-01 §2.2 already lists "memory:after_turn" as candidate gap) may require these to mirror.
- **Recommendation**: Decide in A.2 brainstorming whether daydream cycles should emit `runtime_event` family `memory:daydream_cycle_completed`. Cost: one envelope per cycle (cheap). Benefit: DR-05 telemetry closure.

### 5.4 Performance

- **Daydream scheduling cost**: 60s monitor loop is cheap. Cycle itself runs O(N) over `export_scoped` then O(N²) bigram pairs in merge step (capped at 50 entries on graph; uncapped in consolidator merge — **flag for review**).
- **Trajectory accumulation**: `Arc<RwLock<HashMap>>` uncapped. With ~50 turns/session × ~10 sessions/day = 500 records/day. Each ~1 KB → 500 KB/day RAM. Acceptable for 90-day window but **must add LRU eviction** before A.3 ships.
- **Forgetting sweep**: Iterates every entry every 12h. With 10K entries, ~3 ms. Scales linearly; document 100K cap.

### 5.5 Storage

- v6 `conversation_recall_embeddings` BLOB column: each f32 × 768 dims = 3 KB. With 10K turns → 30 MB. Acceptable; document.
- Trajectory data stored only in RAM (`TrajectoryCollector`). For cross-restart durability, add v9 migration. **Recommendation**: defer — durability not stated user goal.

### 5.6 Conflict with current vnext

| Branch file Δ | vnext owner | Conflict severity |
|---|---|---|
| `harness/agent_loop_integration.rs` deleted | DT-01 S1.x recently expanded | **HARD** |
| `harness/runlog_projection.rs` 380→57 | DT-01 S1.1 created skeleton, S1.2/S1.3 expanded fold | **HARD** |
| `skills/sedimentation/mod.rs` deleted | b390580 ship + future skills work | **HARD** |
| `application/turn_service/{run,stream_finalize,stream_tool_execution}.rs` | GF-02 work_loop split + DT-01 S1.3 emits | **MEDIUM** — semantic merge |
| `application/memory_candidate_extractor.rs +349` | C-branch (parallel) | **MEDIUM** — split via A.5 |
| `application/memory_injection_service.rs +23` | DT-04 memory UI cleanup | **LOW** — additive |
| `application/memory_recall_assembler.rs +6` | DT-04 | **LOW** |
| `bootstrap/memory.rs +28` | F-branch | **LOW** — additive |
| `lib/tauri.ts` (via FE imports) | RD-01 | **MEDIUM** — refactor collision |

**Mitigation**: Wave A.0 explicitly **rejects** the harness/skills/integration deletions when rebasing.

---

## 6. 立即下一步建议 (immediate next step)

**Recommendation**: start **Wave A.0** (compile health gate) immediately. 1-PR, ≤50-LOC investigation that unblocks every subsequent decision. If result is "branch already compiles cleanly post-rebase against vnext", we have green-field for A.1. If errors exist, the audit log becomes A.1 backlog.

**Brainstorming questions to resolve for A.0** (use `superpowers:brainstorming`):
1. **Scenario**: "user opens worktree, rebases in-flight branch onto vnext, runs `cargo check`. What's the verifiable output?"
2. **Scope cuts**: Do we run `cargo test` in A.0, or strictly `check`? Recommend `check` only — tests come in A.1.
3. **Verifiable invariants**: (a) zero `unimplemented!()` outside test code; (b) every match arm exhaustive; (c) no `cargo check` warnings on module surfaces planned for A.1.

**Effort**: **small** (1 dev-day for A.0; 1 dev-week for A.1).

**Calendar**: A.0 this week (W19); A.1 next week (W20); A.2 W21–W22 after brainstorming + writing-plans cycle; A.3 W24+ (gated by C-branch); A.4 W25; A.5 parallel with A.2/A.3 once C-branch lands.

---

## 7. SHELVE / KILL 候选 (within in-flight A scope)

| Sub-feature | Verdict | Reasoning |
|---|---|---|
| **Cognitive layer manager** (`cognitive.rs`, 618 LOC) | **SHELVE** (don't kill) | Code is well-structured; `cognitive_layer` column ships in v8. Manager itself is 4-tier abstraction without stated user scenario. Archive in `docs/superpowers/shelved/` with brainstorming-needed flag. Resurrect when (a) UX evidence users want tier-aware recall or (b) capacity policy on `MemoryCategory` proves insufficient. |
| **Memory graph engine + IPC + panel** (`graph.rs` 427 + `commands/memory/graph.rs` 223 + `MemoryGraphPanel.tsx` 526 = **1,176 LOC**) | **SHELVE** | No stated user problem beyond visualization. `find_related` (graph-enhanced semantic recall) is genuinely useful and 30 LOC — re-extract that one method later. The full graph panel is a research spike. |
| **Branch's deletions of harness files** | **KILL** (reject during rebase) | Hard collision with DT-01. Deletions were premature cleanup before MIG-023 stabilized. |
| **`memory_candidate_extractor +349` LLM auto-extract** | **DEFER to C-branch** | Belongs in C, not A. |
| **`MemoryGraphPanel` link discovery via LLM** | **KILL** | Uses utility LLM to judge every memory-pair relationship. Quadratic cost without proven win. |

Nothing else within A's scope is recommended for KILL.

---

## 8. 时间盒 (timebox)

| Wave | Target window | Calendar | Gate to next wave |
|---|---|---|---|
| A.0 compile gate | 2026-W19 (this week) | 1 dev-day | Compile-error log committed |
| A.1 foundation | 2026-W19–W20 | 1 week | All A.1 tests green; ticker forgetting sweep observed in 24h soak |
| A.2 daydream | 2026-W21–W22 | 2 weeks (incl. brainstorming) | 24-h soak with cycle fired ≥1× |
| A.3 evolution | 2026-W24+ | gated by C-branch availability | 50-trajectory test corpus produces ≥10 procedural rules |
| A.4 UI consolidation | 2026-W25 | 3 days | `lib/tauri.ts` LOC measurably down |
| A.5 C-branch follow-on | parallel after A.1 + C-branch land | depends | A.1 + C-branch tip both green |

---

## 9. 决策矩阵 (final summary)

| Sub-feature | Verdict | Wave | Brainstorm-needed? | Risk | Notes |
|---|---|---|---|---|---|
| Migrations v5–v8 | SHIP | A.1 | No | Low (tested) | downgrade test missing |
| `quality.rs` | SHIP | A.1 | No | Low | foundational |
| `forgetting.rs` | SHIP | A.1 | Yes (lambda tuning) | Low | ticker-wired |
| `conflict.rs` (lite) | SHIP-LITE | A.1 | Yes (auto-resolve threshold) | Medium | needs new provider trait method for `KeepBothWithFlag` |
| `evolution/procedural.rs` | SHIP | A.1 | No | Low | pure transform |
| `cognitive.rs` manager | SHELVE | — | **Yes (no user story)** | n/a | keep schema column only |
| `daydream/*` engine | SHIP | A.2 | **Yes** (idle-trigger, cost cap, failure visibility) | Medium | 1,768 LOC backend |
| Daydream IPC + panel | SHIP | A.2 | No | Low | 5 commands + 460 LOC React |
| `evolution/trajectory.rs` | SHIP | A.3 | **Yes** (eviction policy) | Medium | needs LRU cap |
| `evolution/reflector.rs` | SHIP | A.3 | Yes (insight categories) | Medium | depends on trajectory producer |
| Insight + Timeline panels | SHIP | A.3 | No | Low | 804 LOC React |
| Memory graph engine | SHELVE | — | **Yes (no user story)** | n/a | 1,176 LOC archived |
| `memory_candidate_extractor +349` | DEFER | A.5 | **Yes** (cost analysis) | High | belongs in C-branch |
| Harness/skills deletions | KILL | — | No | n/a | reject during rebase |
| UI consolidation per RD-01 | SHIP | A.4 | No | Low | coordinate with RD-01 owner |

---

## 10. Critical files to reference during execution

- `src-tauri/src/modules/memory/migrations.rs` — v5–v8 migrations land here
- `src-tauri/src/bootstrap/memory.rs` — wiring for ticker + daydream engine
- `src-tauri/src/modules/memory/mod.rs` — `Procedural` enum arm + `CognitiveLayer` enum
- `src-tauri/src/modules/application/memory_candidate_extractor.rs` — A.5 / C-branch overlap
- `docs/IMPROVEMENTS-2026-05-05.md` — cross-reference RD-01 / DR-05 / DT-01 alignment
- `docs/superpowers/plans/2026-04-30-agent-evolution-truth-loop.md` — original product intent
- `origin/feat/in-flight-A-memory @ 8674725` — source archive for cherry-picks
