# ADR-004: Token Budget Allocation

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: P0

---

## Context

LLM context windows have fixed token limits. if2Ai must allocate this budget intelligently across different memory types and system components.

The question: **How should 4000 tokens be distributed between System, Episodic, Semantic, and Working memory?**

UClaw's three-layer architecture uses:
- System: 10%
- Episodic: 20%
- Semantic: 30%
- Working: 40%

But if2Ai is a desktop Tauri app with different constraints than a CLI agent.

---

## Decision

Adopt UClaw's token budget allocation with configurable overrides:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Context Budget Slots (4000 tokens)           │
├─────────────┬─────────────┬─────────────┬─────────────────────┤
│   System    │  Episodic   │  Semantic   │      Working        │
│   (10%)     │   (20%)     │   (30%)     │       (40%)         │
│   400 tok   │   800 tok   │   1200 tok  │      1600 tok       │
├─────────────┼─────────────┼─────────────┼─────────────────────┤
│ Frozen      │ Rolling     │ LanceDB     │ Sliding Window      │
│ Snapshot    │ LLM Summary │ Vector + FTS│ (8 turns)          │
├─────────────┼─────────────┼─────────────┼─────────────────────┤
│ Compact     │ Importance  │ Weibull     │ Priority Queue      │
│ on load     │ decay       │ Decay       │ + token budget      │
└─────────────┴─────────────┴─────────────┴─────────────────────┘
```

### Rust Implementation

```rust
/// Context budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Total budget in tokens (default: 4000)
    pub total: usize,
    /// System slot percentage (default: 10%)
    pub system_pct: f32,
    /// Episodic slot percentage (default: 20%)
    pub episodic_pct: f32,
    /// Semantic slot percentage (default: 30%)
    pub semantic_pct: f32,
    /// Working slot percentage (default: 40%)
    pub working_pct: f32,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            total: 4000,
            system_pct: 0.10,
            episodic_pct: 0.20,
            semantic_pct: 0.30,
            working_pct: 0.40,
        }
    }
}

impl ContextBudget {
    pub fn system_tokens(&self) -> usize {
        (self.total as f32 * self.system_pct) as usize
    }

    pub fn episodic_tokens(&self) -> usize {
        (self.total as f32 * self.episodic_pct) as usize
    }

    pub fn semantic_tokens(&self) -> usize {
        (self.total as f32 * self.semantic_pct) as usize
    }

    pub fn working_tokens(&self) -> usize {
        (self.total as f32 * self.working_pct) as usize
    }

    /// Validate that percentages sum to 1.0
    pub fn validate(&self) -> Result<(), BudgetError> {
        let sum = self.system_pct + self.episodic_pct + self.semantic_pct + self.working_pct;
        if (sum - 1.0).abs() > 0.001 {
            return Err(BudgetError::PercentagesMustSumToOne(sum));
        }
        Ok(())
    }
}

/// Budget slots with actual token usage tracking
#[derive(Debug, Clone)]
pub struct ContextSlots {
    pub system: Slot,
    pub episodic: Slot,
    pub semantic: Slot,
    pub working: Slot,
}

#[derive(Debug, Clone)]
pub struct Slot {
    pub budget: usize,
    pub used: usize,
    pub entries: Vec<MemoryEntry>,
}

impl ContextSlots {
    pub fn new(budget: ContextBudget) -> Self {
        Self {
            system: Slot { budget: budget.system_tokens(), used: 0, entries: Vec::new() },
            episodic: Slot { budget: budget.episodic_tokens(), used: 0, entries: Vec::new() },
            semantic: Slot { budget: budget.semantic_tokens(), used: 0, entries: Vec::new() },
            working: Slot { budget: budget.working_tokens(), used: 0, entries: Vec::new() },
        }
    }

    pub fn available(&self, slot: &str) -> usize {
        match slot {
            "system" => self.system.budget.saturating_sub(self.system.used),
            "episodic" => self.episodic.budget.saturating_sub(self.episodic.used),
            "semantic" => self.semantic.budget.saturating_sub(self.semantic.used),
            "working" => self.working.budget.saturating_sub(self.working.used),
            _ => 0,
        }
    }

    pub fn total_available(&self) -> usize {
        self.system.budget.saturating_sub(self.system.used)
            + self.episodic.budget.saturating_sub(self.episodic.used)
            + self.semantic.budget.saturating_sub(self.semantic.used)
            + self.working.budget.saturating_sub(self.working.used)
    }
}
```

### Working Memory: Sliding Window

```rust
/// Working memory uses a sliding window of recent turns
pub struct WorkingMemory {
    pub turns: Vec<ConversationTurn>,
    pub max_turns: usize,  // Default: 8
    pub max_tokens: usize,
}

impl WorkingMemory {
    pub fn push(&mut self, turn: ConversationTurn) {
        self.turns.push(turn);
        while self.turns.len() > self.max_turns || self.token_count() > self.max_tokens {
            self.turns.remove(0);
        }
    }

    fn token_count(&self) -> usize {
        self.turns.iter().map(|t| t.token_count()).sum()
    }
}
```

---

## Rationale

### Why UClaw Percentages?

1. **Working memory 40%**: Most important for current context, recent turns dominate
2. **Semantic memory 30%**: Long-term facts important but not always relevant
3. **Episodic memory 20%**: Session history useful but summarizable
4. **System 10%**: Frozen snapshot fixed size

### Desktop vs CLI Differences

if2Ai is a Tauri desktop app with:
- **Longer sessions**: Users may have extended multi-hour sessions
- **File access**: More context from local files
- **Lower latency needs**: Not real-time CLI

However, the UClaw percentages are well-tested and conservative enough to work for desktop.

### Why NOT Dynamic Allocation?

Dynamic allocation (adjusting percentages based on query type) is complex:
- Requires profiling and tuning per use case
- Can cause oscillation between slots
- UClaw's fixed percentages work well in practice

Future work (P4+) may add adaptive budgeting.

---

## Consequences

### Positive
- Clear, predictable token allocation
- Working memory gets largest budget (current context priority)
- Fixed percentages simplify implementation
- Configurable for different context window sizes

### Negative
- Fixed allocation may not suit all use cases
- Some queries need more semantic, others more episodic

### Neutral
- Slots are independent (no cross-slot borrowing)

---

## Implementation Notes

### Frozen Snapshot (System Slot)

System prompt snapshot taken at session load:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrozenSnapshot {
    pub prompt: String,
    pub prompt_hash: String,
    pub created_at: DateTime<Utc>,
    pub version: String,
}

impl FrozenSnapshot {
    pub fn capture(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            prompt_hash: compute_hash(prompt),
            created_at: chrono::Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn verify(&self, current_prompt: &str) -> bool {
        self.prompt_hash == compute_hash(current_prompt)
    }
}

fn compute_hash(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
```

### Episodic Compaction

When episodic exceeds budget, trigger LLM summarization:

```rust
pub async fn compact_episodic_if_needed(
    session: &mut Session,
    budget_tokens: usize,
) -> Result<bool> {
    let current_tokens = estimate_episodic_tokens(&session.messages);

    if current_tokens <= budget_tokens {
        return Ok(false);
    }

    // Select entries to preserve (highest importance)
    let mut entries: Vec<_> = session.episodic_entries.iter().collect();
    entries.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());

    let mut preserved = Vec::new();
    let mut tokens = 0;

    for entry in entries {
        let entry_tokens = estimate_tokens(&entry.content);
        if tokens + entry_tokens > budget_tokens * 80 / 100 {
            break;
        }
        preserved.push(entry.clone());
        tokens += entry_tokens;
    }

    // Generate summary of removed entries
    let removed: Vec<_> = entries.into_iter()
        .filter(|e| !preserved.contains(e))
        .cloned()
        .collect();

    if !removed.is_empty() {
        let summary = generate_llm_summary(&removed).await?;
        preserved.push(MemoryEntry {
            key: format!("episodic_summary_{}", chrono::Utc::now().timestamp()),
            content: summary,
            category: MemoryCategory::Episodic,
            importance: 0.7,  // Summaries are important
            ..Default::default()
        });
    }

    session.episodic_entries = preserved;
    Ok(true)
}
```

### Weibull Decay (Semantic Memory)

```rust
/// Weibull decay function for semantic memory importance
pub fn weibull_decay(age_hours: f32, initial_importance: f32) -> f32 {
    let lambda = 24.0 * 7.0;  // 7-day scale
    let k = 1.2;              // shape parameter

    initial_importance
        * (k / lambda)
        * (age_hours / lambda).powf(k - 1.0)
        * (-(age_hours / lambda).powf(k)).exp()
}

pub fn compute_memory_importance(entry: &MemoryEntry) -> f32 {
    let now = chrono::Utc::now();
    let age_hours = (now - entry.created_at).num_hours() as f32;

    let base = entry.importance + (entry.access_count as f32 * 0.01);
    let trust_boost = (entry.trust_score + 1.0) * 0.05;
    let decay = weibull_decay(age_hours, 1.0);

    (base + trust_boost) * decay
}
```

---

## Review Checklist

- [ ] `ContextBudget` struct with 4 percentage fields
- [ ] `validate()` ensures percentages sum to 1.0
- [ ] `ContextSlots` tracks used tokens per slot
- [ ] `available()` returns remaining budget per slot
- [ ] Working memory uses sliding window (8 turns default)
- [ ] `FrozenSnapshot` captures and verifies system prompt
- [ ] Episodic compaction triggers at budget threshold
- [ ] Weibull decay computed correctly
- [ ] Memory importance uses decay + access + trust

---

## References

- [UClaw Context Budget Slots](https://github.com/1tius/iClaw/blob/main/agent/context_budget.py)
- [Weibull distribution](https://en.wikipedia.org/wiki/Weibull_distribution)
- [hermes-agent frozen snapshot](https://github.com/1tius/hermes-agent/blob/main/tools/memory_tool.py)
