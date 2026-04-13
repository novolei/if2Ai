# ADR-007: HRR Introduction Timing (P2a)

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: P2a

---

## Context

Holographic Reduced Representations (HRR) is a Vector Symbolic Architecture (VSA) that enables algebraic memory operations. hermes-agent uses HRR as its primary vector store.

**Key HRR Operations**:
- `bind(a, b)`: Wrap b into a's key (a ⊙ b)
- `unbind(composite, a)`: Retrieve b from composite (a ⊙ b ⊙ a⁻¹)
- `bundle(v1, v2, ...)`: Superpose multiple values (v1 ⊕ v2 ⊕ ...)
- `similarity(a, b)`: Cosine similarity [-1, 1]

**hermes-agent's HRR**:
```python
# hermes-agent/plugins/memory/holographic/holographic.py
class HRRMemory:
    def bind(self, key, value):
        # Phase encoding with circular convolution
        return circular_convolution(key, value)

    def unbind(self, composite, key):
        # Inverse for retrieval
        return circular_convolution_inverse(composite, key)
```

---

## Decision

Introduce HRR in **P2a as a complement to LanceDB**, not a replacement.

### Architecture

```
                    ┌─────────────────────────────────────┐
                    │          Retrieval Query             │
                    └─────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
            ┌───────────┐   ┌───────────┐   ┌───────────────┐
            │  FTS5     │   │  LanceDB  │   │     HRR       │
            │ (keyword) │   │ (vector)  │   │  (algebraic)  │
            └───────────┘   └───────────┘   └───────────────┘
                    │               │               │
                    └───────────────┼───────────────┘
                                    ▼
                         ┌─────────────────┐
                         │   RRF Fusion    │
                         │  (Reciprocal    │
                         │  Rank Fusion)   │
                         └─────────────────┘
                                    │
                                    ▼
                          ┌─────────────────┐
                          │  Final Results  │
                          └─────────────────┘
```

### HRR Module Structure

```rust
// src-tauri/src/modules/memory/hrr/mod.rs

mod operations;
mod store;

pub use operations::{HRRVector, bind, unbind, bundle, similarity};
pub use store::HolographicStore;

/// HRR vector using phase encoding
/// Dimension: 384 (matches FastEmbed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HRRVector(pub Vec<f32>);

impl HRRVector {
    pub fn new(dimension: usize) -> Self {
        Self(vec![0.0; dimension])
    }

    pub fn from_text(text: &str, embedder: &FastEmbedProvider) -> Result<Self, HRRError> {
        let embedding = embedder.embed(text)?;
        Ok(Self(embedding))
    }

    pub fn dimension(&self) -> usize {
        self.0.len()
    }

    pub fn into_inner(self) -> Vec<f32> {
        self.0
    }
}
```

### HRR Operations

```rust
// src-tauri/src/modules/memory/hrr/operations.rs

use std::f32::consts::TAU;

/// Phase encode a value into a vector using circular convolution
pub fn bind(key: &HRRVector, value: &HRRVector) -> HRRVector {
    circular_convolution(key, value)
}

/// Decode a value from a composite using inverse circular convolution
pub fn unbind(composite: &HRRVector, key: &HRRVector) -> HRRVector {
    circular_convolution_inverse(composite, key)
}

/// Superpose multiple vectors (weighted sum in phase space)
pub fn bundle(vectors: &[HRRVector], weights: Option<&[f32]>) -> HRRVector {
    let dim = vectors.first().map(|v| v.dimension()).unwrap_or(0);
    let weights = weights.unwrap_or(&vec![1.0; vectors.len()]);

    let mut result = vec![0.0; dim];
    for (v, w) in vectors.iter().zip(weights.iter()) {
        for i in 0..dim {
            result[i] += v.0[i] * w;
        }
    }

    // Normalize to unit length
    let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        result.iter_mut().for_each(|x| *x /= norm);
    }

    HRRVector(result)
}

/// Cosine similarity between two HRR vectors
pub fn similarity(a: &HRRVector, b: &HRRVector) -> f32 {
    let dot: f32 = a.0.iter().zip(b.0.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.0.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.0.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Circular convolution (bind operation)
fn circular_convolution(a: &HRRVector, b: &HRRVector) -> HRRVector {
    let dim = a.dimension();
    let mut result = vec![0.0; dim];

    for i in 0..dim {
        for j in 0..dim {
            result[i] += a.0[j] * b.0[(i - j + dim) % dim];
        }
    }

    // Normalize
    let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        result.iter_mut().for_each(|x| *x /= norm);
    }

    HRRVector(result)
}

/// Inverse circular convolution (unbind operation)
/// For HRR: inverse = conjugate = reverse phase
fn circular_convolution_inverse(composite: &HRRVector, key: &HRRVector) -> HRRVector {
    let dim = composite.dimension();
    let mut result = vec![0.0; dim];

    // Key inversion: reverse and conjugate
    let key_inv: Vec<f32> = (0..dim)
        .map(|i| key.0[(dim - i) % dim])
        .collect();

    for i in 0..dim {
        for j in 0..dim {
            result[i] += composite.0[j] * key_inv[(i - j + dim) % dim];
        }
    }

    HRRVector(result)
}
```

### HRR Store

```rust
// src-tauri/src/modules/memory/hrr/store.rs

pub struct HolographicStore {
    vectors: RwLock<HashMap<String, HRRVector>>,
    dimension: usize,
    max_capacity: usize,  // O(√dim) = ~700 for 384d
}

impl HolographicStore {
    pub fn new(dimension: usize) -> Self {
        // Capacity = O(√dim) from theoretical bound
        let max_capacity = ((dimension as f32).sqrt() * 100.0) as usize;

        Self {
            vectors: RwLock::new(HashMap::new()),
            dimension,
            max_capacity,
        }
    }

    pub async fn store(&self, key: &str, value: &HRRVector) -> Result<(), HRRError> {
        if value.dimension() != self.dimension {
            return Err(HRRError::DimensionMismatch);
        }

        let mut vectors = self.vectors.write().await;
        if vectors.len() >= self.max_capacity {
            return Err(HRRError::CapacityExceeded(self.max_capacity));
        }

        vectors.insert(key.to_string(), value.clone());
        Ok(())
    }

    pub async fn probe(&self, query: &HRRVector) -> Vec<(String, f32)> {
        let vectors = self.vectors.read().await;

        let mut results: Vec<_> = vectors
            .iter()
            .map(|(k, v)| (k.clone(), similarity(query, v)))
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results
    }

    pub async fn reason(&self, premises: &[HRRVector], conclusions: &[HRRVector]) -> Vec<(usize, usize, f32)> {
        // Check premise-conclusion similarity after binding
        let mut results = Vec::new();

        for (pi, premise) in premises.iter().enumerate() {
            for (ci, conclusion) in conclusions.iter().enumerate() {
                // If premise.bind(conclusion) ≈ identity, they are related
                let relatedness = self.check_relation(premise, conclusion);
                if relatedness > 0.8 {
                    results.push((pi, ci, relatedness));
                }
            }
        }

        results
    }

    pub async fn contradict(&self, a: &HRRVector, b: &HRRVector) -> bool {
        similarity(a, b) < -0.8
    }

    fn check_relation(&self, premise: &HRRVector, conclusion: &HRRVector) -> f32 {
        // Compose and check similarity
        let composite = bind(premise, conclusion);
        similarity(&composite, premise)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HRRError {
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("capacity exceeded: max {0}")]
    CapacityExceeded(usize),

    #[error("HRR operation failed: {0}")]
    OperationFailed(String),
}
```

---

## Rationale

### Why HRR at All?

HRR enables unique capabilities that FTS5 and vector search cannot provide:

1. **Algebraic reasoning**: `A.bind(B)` then `A.unbind()` retrieves B
2. **Multi-hop queries**: "What is the color of X?" → HRR traverses relations
3. **Compositional queries**: Complex queries as algebraic expressions
4. **No embedding model needed**: Pure phase encoding

### Why P2a (Not P0 or P1)?

| Phase | Why NOT HRR | Why NOT YET |
|-------|------------|-------------|
| P0 | SQLite foundation first | HRR needs FastEmbed |
| P1 | LanceDB FTS+vector sufficient for most queries | HRR adds complexity |
| P2a | LanceDB proven stable | HRR adds algebraic reasoning |
| P4+ | HRR capacity limits hit at scale | Scale testing needed |

### Why Complement (Not Replacement)?

HRR has a critical limitation: **O(√dim) capacity**.

For 384d vectors (FastEmbed):
- Max capacity: ~700 items
- With 500+ memories, HRR starts losing information

**Solution**: HRR handles algebraic reasoning tasks; LanceDB handles storage and retrieval.

| Task | Use |
|------|-----|
| "Find memories about X" | LanceDB vector search |
| "What is related to A and B?" | HRR reasoning |
| "Find contradictory facts" | HRR contradict |
| "Multi-hop: A→B→C" | HRR probe |

---

## Consequences

### Positive
- Algebraic reasoning for complex queries
- Multi-hop traversal enabled
- Compositional query support
- Contradiction detection

### Negative
- O(√dim) capacity limits storage
- Complexity increases maintenance burden
- Requires careful capacity management

### Neutral
- FastEmbed still needed for initial encoding
- LanceDB remains primary store

---

## Implementation Notes

### Capacity Management

```rust
impl HolographicStore {
    /// Evict lowest-importance entries when capacity exceeded
    pub async fn store_with_eviction(&self, key: &str, value: &HRRVector, importance: f32) -> Result<()> {
        {
            let vectors = self.vectors.read().await;
            if vectors.len() >= self.max_capacity {
                // Need eviction
            }
        }

        // Write new entry
        self.store(key, value).await?;

        // Evict if needed
        if self.len().await > self.max_capacity {
            self.evict_lowest_importance().await;
        }

        Ok(())
    }
}
```

### Trust Scoring (from hermes-agent)

```rust
pub struct TrustScoredStore {
    store: HolographicStore,
    trust_scores: RwLock<HashMap<String, f32>>,
}

impl TrustScoredStore {
    pub async fn adjust_trust(&self, key: &str, delta: f32) {
        let mut scores = self.trust_scores.write().await;
        let entry = scores.entry(key.to_string()).or_insert(0.0);
        *entry = (*entry + delta).clamp(-1.0, 1.0);
    }
}
```

---

## Review Checklist

- [ ] HRRVector wraps 384d f32 vector
- [ ] `bind()` correctly implements circular convolution
- [ ] `unbind()` correctly implements inverse convolution
- [ ] `bundle()` correctly superposes with optional weights
- [ ] `similarity()` returns cosine similarity [-1, 1]
- [ ] Capacity enforced at O(√dim)
- [ ] `probe()` returns ranked similar vectors
- [ ] `reason()` finds premise-conclusion relations
- [ ] `contradict()` detects opposition (similarity < -0.8)
- [ ] HRR used alongside LanceDB, not instead of

---

## References

- [hermes-agent HRR Implementation](https://github.com/1tius/hermes-agent/blob/main/plugins/memory/holographic/holographic.py)
- [HRR Paper (Plate, 1994)](https://www.sciencedirect.com/science/article/pii/S0304397596001491)
- [VSA Primer](https://arxiv.org/abs/2106.14806)
- [iClaw HRR](https://github.com/1tius/iClaw/blob/main/agent/hrr.py)
