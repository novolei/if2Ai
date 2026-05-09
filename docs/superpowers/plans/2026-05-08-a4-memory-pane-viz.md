# A.4 Memory Pane Visualization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface the daydream → procedural-memory pipeline in the Memory Settings page. Today daydream cycles produce procedural entries opaquely; users can't see what got learned or whether the reflect step is doing useful work. A.4 adds two visualizations: (1) a "Learned procedures" sub-panel inside `DaydreamSettingsSection` listing procedural entries with category + trust + content; (2) extends the daydream history modal so each cycle shows a reflect-step breakdown of extracted vs. promoted vs. rejected insights.

**Architecture:** No new module — backend gets one new Tauri command (`procedural_memory_list`) reading from the existing memory provider, plus two new optional fields on `StepOutcome` (extracted_count, rejected_count) populated in the reflect step. Frontend adds one new component (`LearnedProceduresPanel`) wired into the existing `DaydreamSettingsSection`, plus a small render extension in the history modal.

**Tech Stack:** Same as A.2 / A.3.

---

## Brainstorm decisions (2026-05-08)

- **Q1 = B (display in DaydreamSettingsSection)** — keeps the "you enabled daydream → here's what it learned" causal narrative together. Avoids adding a 9th tab to the already-busy settings page.
- **Q2 = B (category label + trust score + content; no full provenance tree)** — `category` is derivable from the key (`proc:{cat}:{hash}` via existing `extract_category_label`). `trust_score` is bumped on every re-encounter (`TRUST_INCREMENT: 0.1`), so it doubles as a "confidence" proxy without schema changes. Full insight→cycle→trajectory provenance tree is shelved (would need new join column).
- **Q3 = C (extend daydream history modal to show reflect breakdown)** — drops the trajectory-list viewer (would be ephemeral until JSONL persistence). Instead, every cycle's reflect step gains `extracted` / `promoted` / `rejected` counts so users can see "the reflect step ran on N trajectories, found M insights, promoted K to procedural memory". Promoted = `mutated`. Extracted - Rejected = M.

---

## File map

**Backend new**
| Path | Responsibility |
|------|----------------|
| `src-tauri/src/commands/memory/procedural.rs` | `procedural_memory_list` Tauri command + DTO |

**Backend modified**
| Path | Change |
|------|--------|
| `src-tauri/src/modules/memory/daydream/report.rs` | Add `extracted_count: Option<usize>` + `rejected_count: Option<usize>` to `StepOutcome` |
| `src-tauri/src/modules/memory/daydream/reflect.rs` | Populate `extracted_count` (pre-filter) + `rejected_count` (dropped by 0.7 floor); rename `mutated` semantics implicit (already = promoted = internalized) |
| `src-tauri/src/modules/memory/evolution/procedural.rs` | Make `extract_category_label` `pub(crate)` (re-use from command DTO mapper) |
| `src-tauri/src/commands/memory/mod.rs` | `pub mod procedural;` re-export |
| `src-tauri/src/commands/command_surface.rs` | Register new command |

**Frontend new**
| Path | Responsibility |
|------|----------------|
| `src/modules/settings/components/daydream/LearnedProceduresPanel.tsx` | List + render procedural entries |
| `src/modules/settings/components/daydream/use-procedural-entries.ts` | Hook + cache for the entries list |

**Frontend modified**
| Path | Change |
|------|--------|
| `src/transport/contracts.ts` | Add `ProceduralEntryDto`; extend `DaydreamStepOutcome` with `extractedCount?` + `rejectedCount?` |
| `src/api/memory.ts` | `listProceduralEntries(): Promise<ProceduralEntryDto[]>` facade |
| `src/modules/settings/components/daydream/DaydreamSettingsSection.tsx` | Mount `<LearnedProceduresPanel />` |
| `src/modules/settings/components/daydream/DaydreamHistoryModal.tsx` | When `step === 'reflect'` and `extractedCount` is present, render the breakdown |

---

## Task 1: `StepOutcome` extension + reflect step populates

**Files:**
- Modify: `src-tauri/src/modules/memory/daydream/report.rs`
- Modify: `src-tauri/src/modules/memory/daydream/reflect.rs`

- [ ] **Step 1.1: Extend `StepOutcome`**

In `report.rs`, add to `StepOutcome`:

```rust
    /// A.4 — number of items the step considered before any internal
    /// filter. For `reflect`: insights produced by `SelfReflector` before
    /// the confidence floor. `None` for steps where the concept doesn't
    /// apply (prune / merge / refresh).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_count: Option<usize>,
    /// A.4 — number of items the step dropped by an internal filter.
    /// For `reflect`: insights below `MIN_INSIGHT_CONFIDENCE` (0.7).
    /// `None` for steps where the concept doesn't apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_count: Option<usize>,
```

Also: every existing construction of `StepOutcome` in the daydream/* modules (prune.rs, merge.rs, refresh.rs, reflect.rs, engine.rs) sets these fields. The simplest approach: leverage `Default` via field-init shorthand:

```rust
impl Default for StepOutcome {
    fn default() -> Self {
        Self {
            step: String::new(),
            examined: 0,
            mutated: 0,
            duration_ms: 0,
            error: None,
            extracted_count: None,
            rejected_count: None,
        }
    }
}
```

Then existing call sites can use `..Default::default()` to fill the two new fields. Or — cleaner — just append `extracted_count: None, rejected_count: None,` to each existing construction (5-7 sites).

Pick the cleaner one: append explicit `None` to the prune/merge/refresh/error-path constructions. Reflect step gets real values.

- [ ] **Step 1.2: Populate in reflect step**

In `reflect.rs`, find the `run` function. Currently:

```rust
let mut insights: Vec<Insight> = Vec::new();
for t in &trajectories { ... }
insights.retain(|i| i.confidence >= MIN_INSIGHT_CONFIDENCE);
```

Refactor to capture pre/post-filter counts:

```rust
let mut all_insights: Vec<Insight> = Vec::new();
for t in &trajectories {
    if was_successful(t) {
        all_insights.extend(reflector.reflect_on_success(t));
    } else {
        all_insights.extend(reflector.reflect_on_failure(t));
    }
}
let extracted_count = all_insights.len();
let promoted: Vec<Insight> = all_insights
    .into_iter()
    .filter(|i| i.confidence >= MIN_INSIGHT_CONFIDENCE)
    .collect();
let rejected_count = extracted_count - promoted.len();
```

In the success-path `StepOutcome`:
```rust
StepOutcome {
    step: "reflect".into(),
    examined,
    mutated,
    duration_ms: ...,
    error: None,
    extracted_count: Some(extracted_count),
    rejected_count: Some(rejected_count),
}
```

In the error-path `StepOutcome` (when `internalize_insight` fails partway), still populate:
```rust
extracted_count: Some(extracted_count),
rejected_count: Some(rejected_count),
```

- [ ] **Step 1.3: Update other steps**

In `prune.rs`, `merge.rs`, `refresh.rs`, `engine.rs` (the trajectory-source-failure stub), add `extracted_count: None, rejected_count: None,` to every `StepOutcome { ... }` literal.

- [ ] **Step 1.4: Verify + commit**

```bash
cd /Users/ryanliu/Documents/IfAI/if2Ai
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib memory::daydream
```

Expected: cargo check clean, all daydream tests pass.

```bash
git add src-tauri/src/modules/memory/daydream/
git commit -m "feat(daydream): add extracted_count + rejected_count to StepOutcome (A.4 Task 1)"
```

---

## Task 2: `procedural_memory_list` command

**Files:**
- Create: `src-tauri/src/commands/memory/procedural.rs`
- Modify: `src-tauri/src/modules/memory/evolution/procedural.rs` (visibility change only)
- Modify: `src-tauri/src/commands/memory/mod.rs`
- Modify: `src-tauri/src/commands/command_surface.rs`

- [ ] **Step 2.1: Make `extract_category_label` reachable from commands**

In `src-tauri/src/modules/memory/evolution/procedural.rs`, find the `fn extract_category_label`. Change to `pub(crate)`:

```rust
pub(crate) fn extract_category_label(key: &str) -> &'static str {
```

(Note: change return type from `&str` to `&'static str` since all match arms return string literals — verify the function body. If any arm returns the variable `other`, keep `&str` and return `&'static str` only when possible.)

Inspecting the current source: the function has `other => other` which is `&str` lifetime-tied to input. Keep `pub(crate) fn extract_category_label(key: &str) -> &str` — don't change the signature, only visibility.

- [ ] **Step 2.2: Create command**

```rust
//! A.4 — list procedural memory entries for the Memory Settings UI.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::evolution::procedural::extract_category_label;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProceduralEntryDto {
    /// The full storage key (e.g. `proc:heuristic:abc12345`).
    pub key: String,
    /// Human-readable category label (`HeuristicRule`, `AntiPattern`, ...).
    /// Derived from the key.
    pub category: String,
    /// Rule text body.
    pub content: String,
    /// Trust score in `[0, 1]`. Bumped each time the same procedure is
    /// re-encountered; doubles as a confidence proxy in the UI.
    pub trust_score: f64,
    /// Number of times the agent has acted on this procedure (or had it
    /// recalled in a turn). 0 for entries that have never been hit since
    /// internalization.
    pub access_count: u64,
    /// RFC-3339 string. Latest internalization or re-encounter.
    pub updated_at: String,
}

/// List all procedural memory entries (category = `Procedural`).
/// Newest-first.
#[tauri::command]
pub async fn procedural_memory_list(
    state: State<'_, AppState>,
) -> Result<Vec<ProceduralEntryDto>, String> {
    let entries = state
        .memory_provider
        .export(Some("procedural"))
        .await
        .map_err(|e| format!("export failed: {e}"))?;

    let mut dtos: Vec<ProceduralEntryDto> = entries
        .into_iter()
        .map(|e| ProceduralEntryDto {
            category: extract_category_label(&e.key).to_string(),
            key: e.key,
            content: e.content,
            trust_score: e.trust_score,
            access_count: e.access_count,
            updated_at: e.updated_at.to_rfc3339(),
        })
        .collect();
    dtos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(dtos)
}
```

- [ ] **Step 2.3: Re-export + register**

In `src-tauri/src/commands/memory/mod.rs`, after `pub mod daydream;`:

```rust
pub mod procedural;
```

And below the existing `pub use daydream::{...};`:

```rust
pub use procedural::{procedural_memory_list, ProceduralEntryDto};
```

In `src-tauri/src/commands/command_surface.rs`, in the `tauri::generate_handler!` block, add:

```rust
            crate::commands::memory::procedural_memory_list,
```

- [ ] **Step 2.4: Verify + commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/modules/memory/evolution/procedural.rs \
        src-tauri/src/commands/memory/ \
        src-tauri/src/commands/command_surface.rs
git commit -m "feat(commands): procedural_memory_list — list daydream-internalized procedures (A.4 Task 2)"
```

---

## Task 3: Frontend transport types + API facade + hook

**Files:**
- Modify: `src/transport/contracts.ts`
- Modify: `src/api/memory.ts`
- Create: `src/modules/settings/components/daydream/use-procedural-entries.ts`

- [ ] **Step 3.1: Transport types**

In `src/transport/contracts.ts`, find `DaydreamStepOutcome` and add two optional fields:

```ts
export interface DaydreamStepOutcome {
  step: 'prune' | 'merge' | 'refresh' | 'reflect'
  examined: number
  mutated: number
  durationMs: number
  error: { kind: string; message: string } | null
  /** A.4 — reflect step only: total insights produced before the
   * confidence filter. `undefined` for non-reflect steps. */
  extractedCount?: number
  /** A.4 — reflect step only: insights dropped by the confidence floor. */
  rejectedCount?: number
}
```

Append at the bottom of the file:

```ts
/** A.4 — procedural memory entry surfaced in the Memory Settings UI. */
export interface ProceduralEntryDto {
  key: string
  category: string
  content: string
  trustScore: number
  accessCount: number
  updatedAt: string
}
```

- [ ] **Step 3.2: API facade**

In `src/api/memory.ts`, append:

```ts
export async function listProceduralEntries(): Promise<ProceduralEntryDto[]> {
  return getApiClient().call<ProceduralEntryDto[]>('procedural_memory_list')
}
```

(Adapt to the existing facade pattern; if `getApiClient` already used elsewhere in the file, mirror.)

Add `ProceduralEntryDto` to the existing `@/transport/contracts` import at the top.

- [ ] **Step 3.3: Hook**

```ts
import { useEffect, useState, useCallback } from 'react'
import { toast } from 'sonner'
import { listProceduralEntries } from '@/api/memory'
import type { ProceduralEntryDto } from '@/transport/contracts'

export function useProceduralEntries() {
  const [entries, setEntries] = useState<ProceduralEntryDto[] | null>(null)

  const refresh = useCallback(async () => {
    try {
      const list = await listProceduralEntries()
      setEntries(list)
    } catch (err) {
      toast.error(`Failed to load procedural entries: ${err}`)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return { entries, refresh }
}
```

- [ ] **Step 3.4: Build + commit**

```bash
npm run build:web
git add src/transport/contracts.ts src/api/memory.ts src/modules/settings/components/daydream/use-procedural-entries.ts
git commit -m "feat(web/daydream): transport types + API facade + hook for procedural memory (A.4 Task 3)"
```

---

## Task 4: `LearnedProceduresPanel` component + mount

**Files:**
- Create: `src/modules/settings/components/daydream/LearnedProceduresPanel.tsx`
- Modify: `src/modules/settings/components/daydream/DaydreamSettingsSection.tsx`
- Modify: `src/modules/settings/components/daydream/DaydreamHistoryModal.tsx`

- [ ] **Step 4.1: Procedures panel**

```tsx
import { useState } from 'react'
import { useProceduralEntries } from './use-procedural-entries'
import type { ProceduralEntryDto } from '@/transport/contracts'

const CATEGORY_LABEL: Record<string, string> = {
  HeuristicRule: 'Heuristic',
  AntiPattern: 'Anti-pattern',
  BestPractice: 'Best practice',
  UserPreference: 'User preference',
  ToolUsagePattern: 'Tool usage',
}

function formatTrust(t: number): string {
  if (t >= 0.85) return 'high'
  if (t >= 0.5) return 'med'
  return 'low'
}

function ProcedureRow({ entry }: { entry: ProceduralEntryDto }) {
  const [open, setOpen] = useState(false)
  const label = CATEGORY_LABEL[entry.category] ?? entry.category
  return (
    <li className="border border-border rounded-md p-2">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full text-left text-sm flex items-center gap-2"
      >
        <span className="px-1.5 py-0.5 rounded bg-muted text-xs">{label}</span>
        <span className="flex-1 truncate">{entry.content}</span>
        <span className="text-xs text-muted-foreground">trust {formatTrust(entry.trustScore)}</span>
      </button>
      {open && (
        <div className="mt-2 text-xs text-muted-foreground space-y-1">
          <div>Key: <code className="break-all">{entry.key}</code></div>
          <div>Trust score: {entry.trustScore.toFixed(2)} · Access count: {entry.accessCount}</div>
          <div>Updated: {new Date(entry.updatedAt).toLocaleString()}</div>
          <div className="pt-1">{entry.content}</div>
        </div>
      )}
    </li>
  )
}

export function LearnedProceduresPanel() {
  const { entries, refresh } = useProceduralEntries()

  if (entries === null) {
    return <p className="text-xs text-muted-foreground">Loading…</p>
  }
  if (entries.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">
        No procedures learned yet. Run a few chat turns with daydream enabled,
        then trigger a cycle. Failure runs and repeat tool patterns are most
        likely to produce insights.
      </p>
    )
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <h5 className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          Learned procedures ({entries.length})
        </h5>
        <button
          type="button"
          onClick={() => void refresh()}
          className="text-xs text-muted-foreground hover:text-foreground"
        >
          Refresh
        </button>
      </div>
      <ul className="space-y-1.5 max-h-64 overflow-auto">
        {entries.map((e) => (
          <ProcedureRow key={e.key} entry={e} />
        ))}
      </ul>
    </div>
  )
}
```

- [ ] **Step 4.2: Mount in `DaydreamSettingsSection`**

In `DaydreamSettingsSection.tsx`, add the import:
```ts
import { LearnedProceduresPanel } from './LearnedProceduresPanel'
```

Inside the rendered section, **after** `<DaydreamStatusRow />` (the status row already in place from A.2-wiring), add:

```tsx
      <div className="pt-3 border-t border-border">
        <LearnedProceduresPanel />
      </div>
```

- [ ] **Step 4.3: Extend history modal for reflect breakdown**

In `DaydreamHistoryModal.tsx`, find the `<ol>` step list inside the cycle entry. Replace the current per-step `<li>` content rendering. Look for the existing render of `step.examined / mutated`. Add a special case for `reflect` when `extractedCount` / `rejectedCount` are defined:

```tsx
{r.steps.map((s) => (
  <li key={s.step}>
    <span className="font-medium">{s.step}</span>
    {s.step === 'reflect' && s.extractedCount !== undefined ? (
      <>
        : extracted {s.extractedCount} · rejected {s.rejectedCount ?? 0} · promoted {s.mutated}
        {' '}({s.durationMs}ms)
      </>
    ) : (
      <>
        : examined {s.examined} / mutated {s.mutated} ({s.durationMs}ms)
      </>
    )}
    {s.error ? (
      <span className="text-destructive"> — {s.error.kind}: {s.error.message}</span>
    ) : null}
  </li>
))}
```

(Match the file's actual render shape; adapt CSS classes / wrapper.)

- [ ] **Step 4.4: Build + commit**

```bash
npm run build:web
git add src/modules/settings/components/daydream/
git commit -m "feat(web/daydream): LearnedProceduresPanel + reflect breakdown in history modal (A.4 Task 4)"
```

---

## Task 5: Smoke test

- [ ] **Step 5.1: Boot**

```bash
cd /Users/ryanliu/Documents/IfAI/if2Ai
npm run tauri:dev
```

Open Memory Settings → Daydream consolidation. The new "Learned procedures" sub-panel should render below the existing status row. With no procedures yet, it shows the placeholder text.

- [ ] **Step 5.2: Generate a procedure**

Drive several chat turns including at least one tool failure (e.g. ask agent to read a non-existent file). Multi-tool patterns are best.

- [ ] **Step 5.3: Run a cycle**

Settings → "Run now". Observe:
- Status row updates with the latest cycle
- History modal: reflect step now shows `extracted X · rejected Y · promoted Z` instead of the generic `examined / mutated` line
- If `promoted > 0`: refresh the Learned procedures panel — at least one entry appears

- [ ] **Step 5.4: Verify**

If `promoted > 0` and the procedures panel renders entries with category labels and trust scores, A.4 acceptance criteria are met.

If `promoted === 0` even after multiple failure turns: capture the cycle's `extracted` / `rejected` numbers. Either SelfReflector's rules genuinely don't fire (need richer trajectory data — A.3.2 fallback `args_summary` empty might be the cause) OR the confidence floor (0.7) is too strict. Document the observed numbers in the commit body for future tuning.

---

## Acceptance criteria

- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` clean
- [ ] `cargo test --lib memory::` ≥ existing baseline pass count (no regression)
- [ ] `npm run build:web` clean
- [ ] **Manual smoke 5.1**: panel renders with placeholder when no procedures exist
- [ ] **Manual smoke 5.3**: history modal shows `extracted / rejected / promoted` for reflect step
- [ ] **Manual smoke 5.3 (best-effort)**: at least one procedure appears in the panel after running ≥ 3 chat turns including failures, with `promoted > 0`. If not reached, document the cycle counts as future tuning data.

---

## Out of scope

- **Insight → cycle → trajectory provenance tree** (Q2 maximal): would need a join column or new schema field linking procedural entries to source `cycle_id`. Today the entry has no such backref.
- **Standalone Trajectory History viewer** (Q3 alternative): trajectory ring is in-memory, ephemeral. Wait for JSONL persistence (A.5+).
- **`args_summary` enrichment**: A.3.2 ships with empty `args_summary`. Could improve insight extraction quality.
- **Live UI re-render on cycle completion**: `LearnedProceduresPanel` currently relies on the user clicking "Refresh" or remounting the section. Auto-refresh on `daydream_cycle` envelopes is a Wave A.5 polish.
- **Dedicated procedural-memory tab**: rejected per Q1=B.

---

## Self-review

- **Brainstorm coverage:** Q1=B → procedures panel embedded in DaydreamSettingsSection (Task 4.2). Q2=B → category from key + trust as confidence proxy (Tasks 2.1, 2.2, 4.1). Q3=C → history modal reflect breakdown (Task 4.3) backed by `extractedCount` / `rejectedCount` on `StepOutcome` (Task 1).
- **Scope:** 4 implementation tasks + 1 smoke. ~250 LOC backend + ~200 LOC frontend. Single PR.
- **Type consistency:** `ProceduralEntryDto` defined Task 2.2 (Rust) and Task 3.1 (TS) with matching fields (camelCase serde). `StepOutcome` extension consistent across Task 1.1 (Rust) and Task 3.1 (TS). `CATEGORY_LABEL` keys in Task 4.1 match the strings produced by `extract_category_label` (HeuristicRule / AntiPattern / BestPractice / UserPreference / ToolUsagePattern).
- **Verify-before-implement notes:** Task 2.1 calls out the visibility-only change (don't accidentally tighten signature); Task 3.2 notes facade pattern adaptation; Task 4.3 notes the file's existing render shape may differ.
