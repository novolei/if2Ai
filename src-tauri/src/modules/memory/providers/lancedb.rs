//! LanceDB vector memory store
//!
//! Embedded vector database using LanceDB with Arrow schema.
//! Stores embeddings with 384 dimensions and supports ANN vector search.
//!
//! # `#![allow(dead_code)]` justification
//! These types are infrastructure for the vector memory pipeline.
//! They will be consumed by the full MemoryProvider integration
//! and the agent loop (Phase 6E+).

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::{
    Array, FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use lancedb::query::{ExecutableQuery, QueryBase};
use serde::{Deserialize, Serialize};

use crate::modules::memory::MemoryEntry;

/// Error type for LanceDB operations
#[derive(Debug, thiserror::Error)]
pub enum LanceDBError {
    #[error("lancedb operation failed: {0}")]
    OperationFailed(String),

    #[error("key not found: {0}")]
    KeyNotFound(String),

    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("schema error: {0}")]
    SchemaError(String),
}

impl From<lancedb::Error> for LanceDBError {
    fn from(e: lancedb::Error) -> Self {
        LanceDBError::OperationFailed(e.to_string())
    }
}

/// A memory entry with an associated similarity score from vector search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMemory {
    pub key: String,
    pub content: String,
    pub category: String,
    pub score: f32,
}

/// The expected embedding dimension for LanceDB vectors
const EMBEDDING_DIM: i32 = 384;

/// Returns the Arrow schema for the semantic_memory table
fn memory_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("key", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new("category", DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIM,
            ),
            false,
        ),
        Field::new("created_at", DataType::Utf8, false),
    ]))
}

/// Convert a single entry + embedding into an Arrow RecordBatch
fn entry_to_batch(entry: &MemoryEntry, embedding: &[f32]) -> Result<RecordBatch, String> {
    let schema = memory_schema();

    let keys = StringArray::from(vec![entry.key.clone()]);
    let contents = StringArray::from(vec![entry.content.clone()]);
    let categories = StringArray::from(vec![entry.category.as_str().to_string()]);
    let timestamps = StringArray::from(vec![entry.created_at.to_rfc3339()]);

    let embedding_values = Float32Array::from(embedding.to_vec());
    let embeddings = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        EMBEDDING_DIM,
        Arc::new(embedding_values),
        None,
    )
    .map_err(|e| format!("embedding array creation failed: {e}"))?;

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(keys),
            Arc::new(contents),
            Arc::new(categories),
            Arc::new(embeddings),
            Arc::new(timestamps),
        ],
    )
    .map_err(|e| format!("batch creation failed: {e}"))
}

/// LanceDB vector memory store
///
/// Stores memory entries with their 384-dimensional embeddings,
/// and supports approximate nearest-neighbor (ANN) vector search.
pub struct LanceDBMemory {
    db: lancedb::Connection,
    table: lancedb::Table,
}

impl LanceDBMemory {
    /// Create or open a LanceDB memory store at the given path
    ///
    /// Creates the `semantic_memory` table if it doesn't exist.
    pub async fn new(db_path: PathBuf) -> Result<Self, LanceDBError> {
        let uri = db_path
            .to_str()
            .ok_or_else(|| LanceDBError::SchemaError(format!("invalid path: {:?}", db_path)))?
            .to_string();

        let db = lancedb::connect(&uri)
            .execute()
            .await
            .map_err(|e| LanceDBError::OperationFailed(format!("connect failed: {e}")))?;

        // Try to open existing table, or create a new one
        let table = match db.open_table("semantic_memory").execute().await {
            Ok(table) => table,
            Err(_) => {
                // Create empty table with schema
                let schema = memory_schema();
                let empty_iter: std::vec::IntoIter<Result<RecordBatch, _>> = vec![].into_iter();
                let data = Box::new(RecordBatchIterator::new(empty_iter, schema.clone()));
                db.create_table("semantic_memory", data).execute().await?
            }
        };

        Ok(Self { db, table })
    }

    /// Insert a memory entry with its embedding
    pub async fn insert(&self, entry: &MemoryEntry, embedding: &[f32]) -> Result<(), LanceDBError> {
        if embedding.len() != EMBEDDING_DIM as usize {
            return Err(LanceDBError::DimensionMismatch {
                expected: EMBEDDING_DIM as usize,
                actual: embedding.len(),
            });
        }

        let batch = entry_to_batch(entry, embedding).map_err(LanceDBError::SchemaError)?;
        let schema = memory_schema();

        let batches: Vec<Result<RecordBatch, _>> = vec![Ok(batch)];
        let data = Box::new(RecordBatchIterator::new(batches.into_iter(), schema));

        self.table.add(data).execute().await?;
        Ok(())
    }

    /// Search for nearest neighbors by query embedding
    pub async fn search(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, LanceDBError> {
        let results = self
            .table
            .vector_search(query_embedding)
            .map_err(|e| LanceDBError::OperationFailed(format!("vector_search failed: {e}")))?
            .limit(limit)
            .execute()
            .await?;

        scored_memories_from_stream(results).await
    }

    /// Search with category filter
    pub async fn search_with_filter(
        &self,
        query_embedding: &[f32],
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, LanceDBError> {
        let mut builder = self
            .table
            .vector_search(query_embedding)
            .map_err(|e| LanceDBError::OperationFailed(format!("vector_search failed: {e}")))?
            .limit(limit);

        if let Some(cat) = category {
            builder = builder.only_if(format!("category = '{}'", cat));
        }

        let results = builder.execute().await?;
        scored_memories_from_stream(results).await
    }

    /// Delete entries by key
    pub async fn delete(&self, key: &str) -> Result<(), LanceDBError> {
        let filter = format!("key = '{}'", key);
        self.table.delete(&filter).await?;
        Ok(())
    }

    /// Full-text search on content column
    pub async fn fts_search(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, LanceDBError> {
        // Client-side content filtering (FTS index would need setup for production)
        // Get all rows up to limit and filter
        let results = self.table.query().limit(limit).execute().await?;

        // Filter client-side for content match (FTS would need index setup)
        let lower_query = query.to_lowercase();
        scored_memories_from_stream(results)
            .await
            .map(|mut entries| {
                entries.retain(|e| {
                    e.content.to_lowercase().contains(&lower_query)
                        || e.key.to_lowercase().contains(&lower_query)
                });
                entries
            })
            .map(|mut entries| {
                if let Some(cat) = category {
                    entries.retain(|e| e.category == cat);
                }
                entries
            })
    }

    /// Count entries in the table
    pub async fn count(&self) -> Result<usize, LanceDBError> {
        let count = self
            .table
            .count_rows(None)
            .await
            .map_err(|e| LanceDBError::OperationFailed(format!("count failed: {e}")))?;
        Ok(count)
    }

    /// Return a reference to the LanceDB connection
    pub fn connection(&self) -> &lancedb::Connection {
        &self.db
    }

    /// Return a reference to the table
    pub fn table(&self) -> &lancedb::Table {
        &self.table
    }
}

/// Extract ScoredMemory entries from a record batch stream
async fn scored_memories_from_stream(
    mut stream: lancedb::arrow::SendableRecordBatchStream,
) -> Result<Vec<ScoredMemory>, LanceDBError> {
    use futures::StreamExt;

    let mut results = Vec::new();

    while let Some(batch_result) = stream.next().await {
        let batch = batch_result
            .map_err(|e| LanceDBError::OperationFailed(format!("stream error: {e}")))?;
        results.extend(scored_memories_from_batch(&batch)?);
    }

    Ok(results)
}

/// Extract ScoredMemory entries from a single RecordBatch
fn scored_memories_from_batch(batch: &RecordBatch) -> Result<Vec<ScoredMemory>, LanceDBError> {
    if batch.num_rows() == 0 {
        return Ok(Vec::new());
    }

    // Get column indices by name
    let key_idx = batch
        .schema()
        .index_of("key")
        .map_err(|e| LanceDBError::SchemaError(format!("missing 'key' column: {e}")))?;
    let content_idx = batch
        .schema()
        .index_of("content")
        .map_err(|e| LanceDBError::SchemaError(format!("missing 'content' column: {e}")))?;
    let category_idx = batch
        .schema()
        .index_of("category")
        .map_err(|e| LanceDBError::SchemaError(format!("missing 'category' column: {e}")))?;

    let key_arr = batch
        .column(key_idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| LanceDBError::SchemaError("key is not StringArray".to_string()))?;

    let content_arr = batch
        .column(content_idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| LanceDBError::SchemaError("content is not StringArray".to_string()))?;

    let category_arr = batch
        .column(category_idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| LanceDBError::SchemaError("category is not StringArray".to_string()))?;

    // Try to get _distance column (added by vector search)
    let distance_arr = batch
        .schema()
        .index_of("_distance")
        .ok()
        .and_then(|idx| batch.column(idx).as_any().downcast_ref::<Float32Array>());

    let mut results = Vec::new();
    for i in 0..batch.num_rows() {
        let score = distance_arr.map(|arr| arr.value(i)).unwrap_or(0.0);

        results.push(ScoredMemory {
            key: key_arr.value(i).to_string(),
            content: content_arr.value(i).to_string(),
            category: category_arr.value(i).to_string(),
            score,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_entry(key: &str, content: &str) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: crate::modules::memory::MemoryCategory::Core,
            created_at: now,
            updated_at: now,
            importance: 0.5,
            access_count: 0,
            trust_score: 0.0,
        }
    }

    fn test_embedding(seed: f32) -> Vec<f32> {
        vec![seed; EMBEDDING_DIM as usize]
    }

    #[tokio::test]
    async fn create_and_open_existing_table() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let db_path = temp_dir.path().join("test_lancedb");

        // Create
        let mem = LanceDBMemory::new(db_path.clone())
            .await
            .expect("create lancedb");
        assert_eq!(mem.count().await.expect("count"), 0);

        // Open existing (should not fail)
        let mem2 = LanceDBMemory::new(db_path).await.expect("open existing");
        assert_eq!(mem2.count().await.expect("count"), 0);
    }

    #[tokio::test]
    async fn insert_and_count() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let mem = LanceDBMemory::new(temp_dir.path().join("test2"))
            .await
            .expect("create lancedb");

        let entry = test_entry("k1", "Hello world");
        let embedding = test_embedding(0.1);

        mem.insert(&entry, &embedding)
            .await
            .expect("insert should succeed");
        assert_eq!(mem.count().await.expect("count"), 1);
    }

    #[tokio::test]
    async fn search_returns_results_after_insert() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let mem = LanceDBMemory::new(temp_dir.path().join("test3"))
            .await
            .expect("create lancedb");

        let entry = test_entry("k1", "Hello world");
        let embedding = test_embedding(0.1);
        mem.insert(&entry, &embedding).await.expect("insert");

        let query_embedding = test_embedding(0.1);
        let results = mem.search(&query_embedding, 10).await.expect("search");
        assert!(!results.is_empty());
        assert_eq!(results[0].key, "k1");
    }

    #[tokio::test]
    async fn delete_removes_entry() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let mem = LanceDBMemory::new(temp_dir.path().join("test4"))
            .await
            .expect("create lancedb");

        let entry = test_entry("k1", "Hello world");
        let embedding = test_embedding(0.1);
        mem.insert(&entry, &embedding).await.expect("insert");
        assert_eq!(mem.count().await.expect("count"), 1);

        mem.delete("k1").await.expect("delete should succeed");
        assert_eq!(mem.count().await.expect("count"), 0);
    }

    #[tokio::test]
    async fn dimension_mismatch_error() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let mem = LanceDBMemory::new(temp_dir.path().join("test5"))
            .await
            .expect("create lancedb");

        let entry = test_entry("k1", "Hello");
        let bad_embedding = vec![0.1f32; 128]; // Wrong dimension

        let result = mem.insert(&entry, &bad_embedding).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn search_empty_table() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let mem = LanceDBMemory::new(temp_dir.path().join("test6"))
            .await
            .expect("create lancedb");

        let query = test_embedding(0.5);
        let results = mem.search(&query, 10).await.expect("search");
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn fts_search_empty_table() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let mem = LanceDBMemory::new(temp_dir.path().join("test7"))
            .await
            .expect("create lancedb");

        let results = mem.fts_search("hello", None, 10).await.expect("fts");
        assert!(results.is_empty());
    }
}
