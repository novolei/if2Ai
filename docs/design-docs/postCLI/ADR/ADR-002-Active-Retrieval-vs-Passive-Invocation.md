# ADR-002: Active Retrieval vs Passive Invocation

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: P0/P1

---

## Context

When should the memory system retrieve relevant memories for the LLM?

Two paradigms exist:

1. **Passive Invocation**: Memory is written but not retrieved automatically. The LLM explicitly calls `memory.recall()` when needed.

2. **Active Retrieval**: Before each LLM call, the system proactively retrieves relevant memories based on the current context and query intent.

hermes-agent uses a hybrid approach: `MemoryProvider` has both active (prefetch hooks) and passive (explicit recall) modes.

---

## Decision

if2Ai implements a **phased approach**:

- **P0 (Passive)**: Memory is stored and can be recalled explicitly. No automatic retrieval.
- **P1 (Active Retrieval)**: Before each LLM call, the system proactively retrieves memories using intent-based weights.

### P0: Passive Invocation (Current)

```rust
// LLM explicitly calls recall
let memories = await memory.recall(
    query: "user preferences",
    category: Some("core"),
    limit: 10
);
```

### P1: Active Retrieval with Intent Routing

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryIntent {
    /// Code-related queries: semantic=0.5, episodic=0.3, working=0.2
    Code,
    /// Task-related queries: semantic=0.4, episodic=0.4, working=0.2
    Task,
    /// Factual queries: semantic=0.6, episodic=0.2, working=0.2
    Fact,
    /// Person-related queries: semantic=0.3, episodic=0.5, working=0.2
    Person,
    /// General queries: semantic=0.3, episodic=0.3, working=0.4
    General,
}

impl QueryIntent {
    pub fn retrieval_weights(&self) -> (f32, f32, f32) {
        match self {
            QueryIntent::Code => (0.5, 0.3, 0.2),
            QueryIntent::Task => (0.4, 0.4, 0.2),
            QueryIntent::Fact => (0.6, 0.2, 0.2),
            QueryIntent::Person => (0.3, 0.5, 0.2),
            QueryIntent::General => (0.3, 0.3, 0.4),
        }
    }
}

// Runtime retrieves before each LLM call
async fn pre_llm_call(context: &mut ChatContext) -> Result<(), RuntimeError> {
    let intent = classify_intent(&context.current_query);
    let (sem_weight, epi_weight, work_weight) = intent.retrieval_weights();

    // Parallel retrieval from each memory layer
    let (sem_results, epi_results, work_results) = tokio::join!(
        semantic.recall(context.current_query, None, 10),
        episodic.recall(context.current_query, None, 5),
        working.recall(context.current_query, None, 8),
    );

    // Weighted fusion
    let fused = weighted_fusion(sem_results, epi_results, work_results,
                                  sem_weight, epi_weight, work_weight);

    context.inject_memory(fused);
    Ok(())
}
```

---

## Rationale

### P0: Passive First

1. **Simplicity**: No intent classification needed for initial implementation.
2. **Predictability**: LLM controls what memories are relevant.
3. **No hallucination risk**: Active retrieval can inject irrelevant memories.
4. **Matches current architecture**: if2Ai's SessionManager works this way.

### P1: Active When Ready

1. **Better UX**: Relevant memories appear without explicit recall.
2. **Intent-based weights**: Different query types benefit from different memory layers.
3. **Proven pattern**: UClaw's three-layer architecture uses active retrieval.
4. **Hybrid fusion**: Combines multiple retrieval strategies for better recall.

### Why NOT Full Active from Start

1. **Intent classification** requires either ML model or rule-based heuristics.
2. **Weighted fusion** requires tuning weights per use case.
3. **Risk of irrelevant injection**: Too much context can hurt LLM performance.

---

## Consequences

### Positive
- P0 is simple and predictable
- P1 improves UX without breaking P0 compatibility
- Intent routing can be extended with learned weights

### Negative
- P1 adds complexity to runtime
- Intent classification is imperfect (rule-based at first)
- Too many memories can hurt LLM performance (context overflow)

### Neutral
- Memory still stored regardless of retrieval mode

---

## Implementation Notes

### P0 Implementation (Current)

The current `InMemoryMemoryProvider` (to be replaced by `SqliteMemoryProvider` per ADR-001) supports passive invocation:

```rust
#[async_trait]
impl MemoryProvider for SqliteMemoryProvider {
    async fn recall(&self, query: &str, category: Option<&str>, limit: usize) -> Result<Vec<MemoryEntry>, MemoryError> {
        let entries = self.fetch_entries(category).await?;
        let query_lower = query.to_lowercase();

        let filtered: Vec<MemoryEntry> = entries
            .into_iter()
            .filter(|e| {
                query.is_empty()
                    || e.key.to_lowercase().contains(&query_lower)
                    || e.content.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .collect();

        Ok(filtered)
    }
}
```

### P1: Intent Classification (Future)

Rule-based intent classification:

```rust
fn classify_intent(query: &str) -> QueryIntent {
    let query_lower = query.to_lowercase();

    if query_lower.contains("how do i") || query_lower.contains("fix") || query_lower.contains("bug") {
        return QueryIntent::Code;
    }
    if query_lower.contains("remember") || query_lower.contains("did we") {
        return QueryIntent::Task;
    }
    if query_lower.contains("who") || query_lower.contains("person") {
        return QueryIntent::Person;
    }
    if query_lower.contains("what is") || query_lower.contains("fact") {
        return QueryIntent::Fact;
    }

    QueryIntent::General
}
```

### P1: Weighted Fusion

```rust
fn weighted_fusion(
    sem: Vec<ScoredMemory>,
    epi: Vec<ScoredMemory>,
    work: Vec<ScoredMemory>,
    sem_w: f32,
    epi_w: f32,
    work_w: f32,
) -> Vec<ScoredMemory> {
    let mut scores: HashMap<String, f32> = HashMap::new();

    for (rank, item) in sem.iter().enumerate() {
        let score = 1.0 / (60 + rank) as f32 * sem_w;
        *scores.entry(item.key.clone()).or_insert(0.0) += score;
    }
    for (rank, item) in epi.iter().enumerate() {
        let score = 1.0 / (60 + rank) as f32 * epi_w;
        *scores.entry(item.key.clone()).or_insert(0.0) += score;
    }
    for (rank, item) in work.iter().enumerate() {
        let score = 1.0 / (60 + rank) as f32 * work_w;
        *scores.entry(item.key.clone()).or_insert(0.0) += score;
    }

    let mut results: Vec<_> = scores.into_iter()
        .map(|(key, score)| ScoredMemory { key, score })
        .collect();
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    results
}
```

---

## Review Checklist

- [ ] P0: `recall()` works without automatic retrieval
- [ ] P0: LLM can explicitly call `memory.recall()`
- [ ] P1: `QueryIntent` enum defined with 5 variants
- [ ] P1: `retrieval_weights()` returns correct weights per intent
- [ ] P1: Intent classification function implemented
- [ ] P1: `weighted_fusion()` combines results correctly
- [ ] P1: Runtime calls retrieval before LLM invocation
- [ ] P1: Memory injection respects token budget

---

## References

- [UClaw Context Budget Slots](../if2Ai-Memory-Autonomous-Learning-Architecture-Report.md#32-context-budget-slots)
- [hermes-agent MemoryProvider prefetch](https://github.com/1tius/hermes-agent/blob/main/agent/memory_provider.py)
- [HRR Retrieval](https://github.com/1tius/hermes-agent/blob/main/plugins/memory/holographic/retrieval.py)
