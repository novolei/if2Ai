# ADR-003: FastEmbed + LanceDB Selection

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: P1

---

## Context

For P1 vector search, if2Ai needs two components:

1. **Embedding model**: Convert text to vectors
2. **Vector database**: Store and search vectors

Options considered:

| Option | Embedding | Vector DB | Offline | Notes |
|--------|-----------|-----------|---------|-------|
| OpenAI API | text-embedding-3 | External | No | API key, latency, privacy |
| Claude API | N/A | N/A | N/A | No embedding endpoint |
| FastEmbed + LanceDB | FastEmbed | LanceDB | Yes | Offline, 384d |
| SQLite FTS5 | N/A | N/A | N/A | Text-only, no vectors |

---

## Decision

Select **FastEmbed + LanceDB** as the P1 vector search stack.

### Why NOT Pure SQLite FTS5

SQLite FTS5 is excellent for full-text search but cannot:
- Compute semantic similarity between queries and documents
- Support vector nearest-neighbor queries
- Enable multi-hop reasoning with HRR (P2a)

### Why NOT External API

- **Privacy**: User data stays on-device
- **Latency**: No network round-trips
- **Offline**: Works without internet
- **Cost**: No API usage fees

### FastEmbed Configuration

```rust
// Model: multilingual-e5-small (384 dimensions)
// - Supports 100+ languages
// - Fast inference (CPU-friendly)
// - Offline model files

pub struct FastEmbedProvider {
    model: FastEmbed,
    dimension: usize,  // 384
}

impl FastEmbedProvider {
    pub fn new() -> Result<Self, EmbeddingError> {
        // Downloads model on first use, caches locally
        let model = FastEmbed::from_model("multilingual-e5-small")?;
        Ok(Self { model, dimension: 384 })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.model.encode(vec![text])?.pop()
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.model.encode(texts.to_vec())
    }
}
```

### LanceDB Configuration

```rust
// LanceDB table schema
// - Native FTS support
// - Vector column with IVF-PQ index
// - ACID transactions

use lancedb::connect;

pub struct LanceDBMemory {
    db: Database,
    table: Table,
}

impl LanceDBMemory {
    pub async fn new(db_path: PathBuf) -> Result<Self, LanceDBError> {
        let db = connect(&db_path).execute().await?;
        let table = db
            .create_table("semantic_memory")
            .schema(&Schema::new([
                field("key", DataType::Utf8),
                field("content", DataType::Utf8),
                field("category", DataType::Utf8),
                field("embedding", DataType::FixedSizeList(Box::new(DataType::Float32), 384)),
                field("created_at", DataType::Timestamp),
            ]))
            .execute()
            .await?;

        Ok(Self { db, table })
    }

    pub async fn insert(&self, entry: &MemoryEntry, embedding: &[f32]) -> Result<(), LanceDBError> {
        self.table.insert([
            ["key", entry.key.clone()],
            ["content", entry.content.clone()],
            ["category", entry.category.as_str().to_string()],
            ["embedding", serde_json::to_string(embedding)?],
            ["created_at", entry.created_at.to_rfc3339()],
        ]).execute().await?;
        Ok(())
    }

    pub async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<ScoredMemory>, LanceDBError> {
        let results = self.table
            .vector_search("embedding", query_embedding)
            .limit(limit)
            .execute()
            .await?;

        Ok(results.into_iter().map(|r| ScoredMemory {
            key: r["key"].to_string(),
            score: r["_score"],
        }).collect())
    }
}
```

---

## Rationale

### FastEmbed Advantages

1. **Offline**: Model files cached locally, no API calls
2. **Multilingual**: Supports 100+ languages (important for international users)
3. **CPU-friendly**: No GPU required
4. **Fast**: Optimized inference, suitable for desktop
5. **Small**: 384 dimensions = compact storage

### LanceDB Advantages

1. **Embedded**: No separate server process
2. **Native FTS**: Full-text search built-in
3. **ACID**: Transaction support prevents corruption
4. **Rust**: Native Rust client (important for if2Ai architecture)
5. **IVF-PQ index**: Fast approximate nearest-neighbor search

### Why NOT Alternatives

| Alternative | Reason for Rejection |
|------------|----------------------|
| OpenAI embeddings | API key, internet required, privacy concerns |
| ChromaDB | Python-first, not Rust-native |
| Qdrant | Requires separate server process |
| pgvector | Requires PostgreSQL server |
| Pinecone | External service, API key required |

---

## Consequences

### Positive
- Fully offline vector search
- User privacy preserved
- Fast approximate nearest-neighbor
- Native FTS for keyword search
- Rust-native integration

### Negative
- 384d embeddings larger than FTS5-only storage
- Model download on first use (~100MB)
- IVF-PQ index build time for large datasets

### Neutral
- Requires P0 SQLite foundation
- Complements HRR (P2a) rather than replacing it

---

## Implementation Notes

### Initialization

```rust
pub async fn init_vector_memory(base_path: PathBuf) -> Result<LanceDBMemory, MemoryError> {
    let db_path = base_path.join("vector_memory");
    let memory = LanceDBMemory::new(db_path).await?;

    // Ensure FastEmbed model is cached
    let embedder = FastEmbedProvider::new()
        .map_err(|e| MemoryError::Generic(e.to_string()))?;

    Ok(memory)
}
```

### Hybrid Search with RRF

```rust
pub async fn hybrid_recall(
    &self,
    query: &str,
    category: Option<&str>,
    limit: usize,
) -> Result<Vec<ScoredMemory>, MemoryError> {
    // 1. Get query embedding
    let query_embedding = self.embedder.embed(query)?;

    // 2. Parallel search: FTS5 + vector
    let (fts_results, vec_results) = tokio::join!(
        self.fts_search(query, category, limit),
        self.vector_search(&query_embedding, limit),
    );

    // 3. RRF fusion (k=60 per ADR-002)
    let fused = rrf_fusion(vec![fts_results, vec_results], 60);

    Ok(fused.into_iter().take(limit).collect())
}

fn rrf_fusion(results: Vec<Vec<ScoredMemory>>, k: u32) -> Vec<ScoredMemory> {
    let mut scores: HashMap<String, f32> = HashMap::new();

    for ranked_list in results {
        for (rank, item) in ranked_list.iter().enumerate() {
            let score = 1.0 / (k + rank as u32) as f32;
            *scores.entry(item.key.clone()).or_insert(0.0) += score;
        }
    }

    let mut fused: Vec<_> = scores.into_iter()
        .map(|(key, score)| ScoredMemory { key, score })
        .collect();
    fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    fused
}
```

### Schema Migration

LanceDB schema versioning:

```rust
const SCHEMA_VERSION: i32 = 1;

async fn migrate_if_needed(&self) -> Result<(), LanceDBError> {
    let current_version = self.get_schema_version().await?;

    if current_version < SCHEMA_VERSION {
        // Apply migrations
        self.add_missing_columns().await?;
        self.rebuild_index().await?;
        self.set_schema_version(SCHEMA_VERSION).await?;
    }

    Ok(())
}
```

---

## Review Checklist

- [ ] FastEmbed model downloads and caches on first use
- [ ] FastEmbed::encode() returns 384d vectors
- [ ] LanceDB table created with correct schema
- [ ] LanceDB insert stores entry + embedding
- [ ] LanceDB search returns nearest neighbors
- [ ] Hybrid search combines FTS5 + vector with RRF
- [ ] RRF fusion uses k=60
- [ ] Category filtering works
- [ ] Schema migration handles version upgrades

---

## References

- [FastEmbed](https://github.com/aspida/fastembed) — Rust embedding library
- [LanceDB](https://lancedb.com/) — vector database
- [multilingual-e5-small](https://huggingface.co/intfloat/multilingual-e5-small) — embedding model
- [RRF Fusion](../if2Ai-Memory-Autonomous-Learning-Architecture-Report.md#36-hybrid-retrieval-p1)
