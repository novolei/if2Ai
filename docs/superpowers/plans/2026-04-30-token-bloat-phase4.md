# Token Bloat Phase 4 — Quick Wins (F2 + F3)

> Compact plan; bypass the full superpowers ceremony since the spec is small.

**Goal:** Reduce input token count on streaming agent turns + improve cache observability. Two tactical commits, low risk.

**Worktree:** `.worktrees/token-bloat-phase4` (branch `feature/token-bloat-phase4` from `vnext` `0a37349`)

**Investigation source:** Earlier subagent investigation (Apr 30) found that streaming path bypasses `WorkingMemory` and tier-compression default budget (30K) is too loose for typical sessions. Cache stats are also unobservable due to hardcoded zeros.

**Deferred:** F1 (full `WorkingMemory` streaming parity) — separate future program with its own design pass.

---

## Conventions
- Commit prefix: `feat(token-bloat): T<N>-<tag> — <one-line>`
- Cargo gate: `cargo check --tests` + targeted e2e + clippy

---

## T1 — F2: Lower streaming tier-compression budget + env-var override

**Files:**
- Modify: `src-tauri/src/modules/runtime/budget.rs` (or wherever `TierBudgetAllocation::default_const` lives)
- Modify: `src-tauri/src/modules/runtime/context_compression/mod.rs` (or where `compress_for_request` reads the budget) — add env-var read with fallback
- Add test: a unit test asserting the env var is honored

**Steps:**
- [ ] Audit current `TierBudgetAllocation::default_const` value (currently 30K total)
- [ ] Reduce default for streaming path to **8000** tokens (typical session header + last 4-6 turns)
- [ ] Add `IF2AI_STREAM_TIER_BUDGET` env var with `parse()` fallback to default
- [ ] Add doc-comment noting how to tune
- [ ] Verify e2e tests still pass (sanitize/governor still handles edge cases)
- [ ] Commit `feat(token-bloat): T1 — F2 lower streaming tier-compression budget to 8K with env override`

---

## T2 — F3: Parse cache fields from openai_compat usage

**Files:**
- Modify: `src-tauri/src/modules/api/providers/openai_compat.rs` (around lines 455-461 where `Usage` is constructed from chunk)

**Steps:**
- [ ] Audit current `chunk.usage` struct shape — does it carry `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens` / similar?
- [ ] If yes: parse them into `Usage::cache_creation_input_tokens` / `Usage::cache_read_input_tokens` instead of hardcoded 0
- [ ] If no: investigate DeepSeek API docs for what field names they emit; add minimal struct deser
- [ ] Add 1 unit test confirming round-trip
- [ ] Commit `feat(token-bloat): T2 — F3 parse provider cache fields into Usage`

---

## Exit Gate
1. `cargo fmt --all` clean
2. `cargo clippy -p if2ai-backend --tests -- -D warnings` clean
3. `cargo test --test turn_service_stream_turn_e2e --test turn_service_run_turn_e2e` PASS
4. `cargo test --test agentic_loop_unit --test loop_config_force_text` PASS

Then merge to vnext, push, observe in dev that input token count drops.

---

## F1 Future Program Note
Full `WorkingMemory` streaming-parity adoption deserves its own design pass:
- How does `WorkingMemory::evict_if_needed` interact with tool_use→tool_result pairing?
- Does it need pairing-aware slicing?
- 8-turn default vs token-budget eviction priority?
- Should it run BEFORE tier-compression (cheap window) or AFTER (preserve recent tool chains)?

Track as `feature/working-memory-streaming-parity` when ready.
