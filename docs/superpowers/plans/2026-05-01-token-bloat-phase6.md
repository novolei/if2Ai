# Token Bloat Phase 6 — Tool Stability + Skill Excerpting + History Compression

> Compact plan; bypasses full superpowers ceremony since each task is small and well-scoped.

**Goal:** Drop typical input tokens from ~9,248 → ~4,500–5,500 by attacking the largest non-cached components: tool definition serialization stability, full SKILL.md auto-injection, and verbose tool transcripts in old history messages.

**Worktree:** `.worktrees/token-bloat-phase6` (branch `feature/token-bloat-phase6` from `vnext` `2d59bf6`)

**Predecessor work:**
- Phase 4: tier-compression budget 30K→8K + cache observability
- Phase 5: WorkingMemory recency window (8 turns)
- Net so far: ~10,961 → ~9,248 (~16% reduction). Cache hit rate ~48%.

**Inspiration:** GenericAgent's three patterns — tool dedup (`llmcore.py:791`), history `<thinking>/<tool_use>` compression (`llmcore.py:33-44`), skill summaries instead of full bodies.

---

## Conventions
- Commit prefix: `feat(token-bloat-p6): T<N>-<tag> — <one-line>`
- Cargo gate: `cargo check --tests` + targeted tests + clippy

---

## T1 — Tool Definition Serialization Stability

**Why:** DeepSeek prompt cache is prefix-based. If `tool_defs` JSON byte-changes between turns (registration order drift, schema field reordering), cache hit rate stays partial. Goal: ensure `tool_defs` JSON is byte-stable across turns to maximize cache hits without changing what tools are available.

**Files:**
- Modify: `src-tauri/src/modules/tools/registry.rs` — `get_definitions()` should sort tools by name before serializing
- Modify: `src-tauri/src/modules/application/turn_service/work_loop.rs` — `build_canonical_tool_pool` should preserve sort
- Add: helper to canonicalize JSON object key order (alphabetical) inside each tool's `parameters` schema before sending
- Add: `[tool_pool_diag]` debug log emitting tool name list + serialized byte length

**Steps:**
- [ ] **T1.1: Audit current ordering**
  ```bash
  cd /Users/ryanliu/Documents/IfAI/if2Ai/.worktrees/token-bloat-phase6
  rg -n "fn get_definitions|fn build_canonical_tool_pool|tool_defs.*sort" src-tauri/src/modules/tools/ src-tauri/src/modules/application/turn_service/
  ```
  Confirm whether tools are registered in stable order (constant init) or DashMap iteration (non-deterministic).

- [ ] **T1.2: Sort tool list deterministically by name in `get_definitions`**
  Already collects via `.iter()` on `DashMap`; add `.sorted_by(|a, b| a.name.cmp(&b.name))` after the map step.

- [ ] **T1.3: Canonicalize parameters JSON schema key order**
  When converting `entry.input_schema` to JSON, walk the object and emit keys in sorted order (use a `BTreeMap`-backed helper). This ensures cache-stable bytes even if schema source uses a `serde_json::Value` with insertion-ordered map.

- [ ] **T1.4: Add a test asserting two consecutive calls yield byte-identical JSON**
  ```rust
  #[test]
  fn get_definitions_byte_stable_across_calls() {
      let registry = ToolRegistry::default();
      // ... register a few tools (or use a real builtin set) ...
      let a = serde_json::to_string(&registry.get_definitions(None)).unwrap();
      let b = serde_json::to_string(&registry.get_definitions(None)).unwrap();
      assert_eq!(a, b);
      // Stronger: compute SHA256 to make the failure message readable.
  }
  ```

- [ ] **T1.5: Verify GREEN + commit**
  ```bash
  cd src-tauri
  cargo check --tests
  cargo test --lib -p if2ai-backend tools::registry
  cargo test --test turn_service_stream_turn_e2e
  cargo fmt --all && cargo clippy -p if2ai-backend --tests -- -D warnings
  git add -A && git commit -m "feat(token-bloat-p6): T1 — stable tool_defs JSON serialization (boost cache hit rate)"
  ```

**Estimated impact:** May raise cache_r from 4480/9248 (~48%) to 5500-6500/9248 (~60-70%). Direct token cost unchanged.

---

## T2 — Skill Body Excerpting (Auto-Loaded SKILL.md)

**Why:** `auto_load_trusted_skill_context` injects entire SKILL.md per active skill (~500-3K tokens each). Replace with compressed signature; trust model to call `skill_view` when full body needed.

**Files:**
- Modify: `src-tauri/src/modules/application/turn_service/work_loop.rs` ~line 528-535 (the `auto_load_trusted_skill_context` injection site)
- Add env: `IF2AI_SKILL_AUTOLOAD_MODE=full|excerpt|disabled` (default `excerpt`, fall back to `full` if user prefers old behavior)
- Add unit test for the excerpting helper

**Steps:**
- [ ] **T2.1: Locate the auto-load site**
  ```bash
  rg -n "auto_load_trusted_skill_context|## Skill:|escape_markdown_skill_section" src-tauri/src/modules/application/turn_service/work_loop.rs
  ```

- [ ] **T2.2: Add `excerpt_skill_body` helper**
  Extract YAML frontmatter (between `---` markers), then take first ~200 chars of body. Append `\n\n[truncated — call \`skill_view name="<skill_name>"\` for full body]`.
  ```rust
  fn excerpt_skill_body(skill_name: &str, full: &str) -> String {
      let trimmed = full.trim();
      let mut out = String::new();
      // Preserve frontmatter as-is.
      if trimmed.starts_with("---") {
          if let Some(end) = trimmed[3..].find("\n---\n").map(|i| i + 3 + 5) {
              out.push_str(&trimmed[..end]);
              out.push('\n');
              let body = trimmed[end..].trim_start();
              let snippet: String = body.chars().take(200).collect();
              out.push_str(&snippet);
              out.push_str(&format!(
                  "\n\n[truncated — call `skill_view name=\"{}\"` for full body]",
                  skill_name,
              ));
              return out;
          }
      }
      // No frontmatter: just truncate.
      let snippet: String = trimmed.chars().take(200).collect();
      format!("{}\n\n[truncated — call `skill_view name=\"{}\"` for full body]",
          snippet, skill_name)
  }
  ```

- [ ] **T2.3: Add `skill_autoload_mode()` env helper in `budget.rs`**
  ```rust
  pub enum SkillAutoloadMode { Full, Excerpt, Disabled }
  pub fn skill_autoload_mode() -> SkillAutoloadMode {
      match std::env::var("IF2AI_SKILL_AUTOLOAD_MODE").ok().as_deref() {
          Some("full") => SkillAutoloadMode::Full,
          Some("disabled") => SkillAutoloadMode::Disabled,
          _ => SkillAutoloadMode::Excerpt,  // default
      }
  }
  ```

- [ ] **T2.4: Wire into the auto-load site**
  ```rust
  // Before: section already contains escape_markdown_skill_section(&content)
  let body = match skill_autoload_mode() {
      SkillAutoloadMode::Full => escape_markdown_skill_section(&content),
      SkillAutoloadMode::Excerpt => excerpt_skill_body(&candidate.name, &content),
      SkillAutoloadMode::Disabled => continue,  // skip injection entirely
  };
  let mut section = format!(
      "## Skill: {} [{}]\n...\n{}",
      candidate.name, candidate.source, /* metadata */, body,
  );
  ```

- [ ] **T2.5: Add tests**
  ```rust
  #[test]
  fn excerpt_skill_body_preserves_frontmatter() {
      let raw = "---\nname: foo\nversion: 1\n---\n# Foo skill\n\nThis is the body, very long...".repeat(10);
      let out = excerpt_skill_body("foo", &raw);
      assert!(out.contains("---\nname: foo"));
      assert!(out.contains("[truncated"));
      assert!(out.contains("call `skill_view name=\"foo\"`"));
  }
  
  #[test]
  fn excerpt_skill_body_handles_no_frontmatter() {
      let raw = "Plain content with no yaml header. ".repeat(20);
      let out = excerpt_skill_body("bar", &raw);
      assert!(out.contains("[truncated"));
      assert!(out.len() < raw.len());
  }
  ```

- [ ] **T2.6: Verify GREEN + commit**
  ```bash
  cargo check --tests
  cargo test --test turn_service_stream_turn_e2e --test turn_service_run_turn_e2e
  cargo fmt --all && cargo clippy -p if2ai-backend --tests -- -D warnings
  git add -A && git commit -m "feat(token-bloat-p6): T2 — skill body excerpting with skill_autoload_mode env"
  ```

**Estimated impact:** −500 to −3K tokens per turn (depends on # active skills + body sizes).

---

## T3 — Old-Message Tool Transcript Compression

**Why:** Within the WorkingMemory window (8 turns), older assistant `tool_use` blocks + the corresponding `tool_result` user blocks are full-fidelity. For messages older than the last 2-3 turns, compress to `[tool_call: <name>(<args summary>) → <result summary>]`.

**Files:**
- Add: `src-tauri/src/modules/memory/tool_transcript_compression.rs` (new module)
- Modify: `src-tauri/src/modules/application/turn_service/stream.rs` — apply compression after window but before `InputMessage` mapping
- Add env: `IF2AI_STREAM_TOOL_KEEP_RECENT` (default 3 — keep last 3 turns full)
- Tests for compression + e2e regression

**Steps:**
- [ ] **T3.1: Audit message structure**
  ```bash
  rg -n "struct ConversationMessage|tool_use|tool_result|content_blocks" src-tauri/src/modules/runtime/session.rs | head -20
  ```
  Confirm how `tool_use`/`tool_result` are represented in `ConversationMessage.content` (likely `Vec<ContentBlock>` with variants).

- [ ] **T3.2: Implement `compress_old_tool_transcripts`**
  ```rust
  /// Replace tool_use args + tool_result content with compact summaries for
  /// messages older than the last `keep_recent` turns. Recent turns stay
  /// full-fidelity for active context; old turns become reference-only.
  pub fn compress_old_tool_transcripts(
      messages: Vec<ConversationMessage>,
      keep_recent_turns: usize,
  ) -> Vec<ConversationMessage> {
      // Find the cut: index of the (keep_recent_turns)th-last user message.
      // Everything BEFORE that is old → compress.
      // Everything FROM that index onwards stays as-is.
      // For old assistant tool_use blocks: keep id+name, replace input_json with truncated/summary.
      // For old user tool_result blocks: replace content with first 100 chars + "[...]".
      // ...
  }
  ```

- [ ] **T3.3: Wire after WorkingMemory windowing in `stream.rs`**
  ```rust
  let windowed = window_recent_turns(&runtime_session.messages, streaming_window_turns());
  let compressed = compress_old_tool_transcripts(windowed, stream_tool_keep_recent());
  let messages: Vec<InputMessage> = compressed.iter().map(|msg| { ... }).collect();
  ```

- [ ] **T3.4: Add `stream_tool_keep_recent()` env helper**
  ```rust
  pub const DEFAULT_STREAM_TOOL_KEEP_RECENT: usize = 3;
  pub fn stream_tool_keep_recent() -> usize { /* parse env IF2AI_STREAM_TOOL_KEEP_RECENT */ }
  ```

- [ ] **T3.5: Tests**
  - Old assistant message with tool_use blocks → blocks have shorter input_json
  - Old user message with tool_result → content compacted
  - Recent assistant/user messages unchanged
  - Compression preserves tool_use_id ↔ tool_result.tool_use_id pairing (CRITICAL — sanitize relies on it)
  - Empty input passthrough

- [ ] **T3.6: Verify GREEN + commit**
  ```bash
  cargo check --tests
  cargo test --test turn_service_stream_turn_e2e --test turn_service_run_turn_e2e
  cargo fmt --all && cargo clippy -p if2ai-backend --tests -- -D warnings
  git add -A && git commit -m "feat(token-bloat-p6): T3 — compress tool transcripts in old history messages"
  ```

**Estimated impact:** −500 to −2K tokens per turn for tool-heavy sessions. Risk: model may need to re-discover info that's now compressed (mitigation: keep_recent=3 default preserves recent context).

---

## Exit Gate
1. `cargo fmt --all` clean
2. `cargo clippy -p if2ai-backend --tests -- -D warnings` clean
3. `cargo test --test turn_service_stream_turn_e2e --test turn_service_run_turn_e2e` PASS
4. `cargo test --lib -p if2ai-backend tools::registry budget working_memory tool_transcript_compression` PASS
5. Manual verification on tauri dev: input tokens for typical session should drop from ~9,248 → ~5,500 with cache_r ratio improving toward 60-70%.

Then merge to vnext, push, observe.

---

## Tunable env summary (all phases)

| Var | Default | Purpose |
|---|---|---|
| `IF2AI_STREAM_TIER_BUDGET` | 8000 | Phase 4 — tier-compression budget |
| `IF2AI_STREAM_WINDOW_TURNS` | 8 | Phase 5 — WorkingMemory recency window (0=disable) |
| `IF2AI_SKILL_AUTOLOAD_MODE` | excerpt | Phase 6 T2 — skill body strategy (full/excerpt/disabled) |
| `IF2AI_STREAM_TOOL_KEEP_RECENT` | 3 | Phase 6 T3 — turns to keep tool transcripts full |
