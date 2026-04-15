//! SQLite-backed memory provider
//!
//! Persists memory entries to a SQLite database, surviving process restarts.
//! Uses rusqlite (sync) wrapped in spawn_blocking for async compatibility.

use async_trait::async_trait;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::modules::memory::{MemoryCategory, MemoryEntry, MemoryError, MemoryProvider};

/// SQLite-backed implementation of [`MemoryProvider`]
///
/// Stores memory entries in a SQLite database with proper indexing
/// for category-based queries and full-text search support.
pub struct SqliteMemoryProvider {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteMemoryProvider {
    /// Create a new SQLite provider, initializing the schema if needed.
    pub fn new(db_path: PathBuf) -> Result<Self, MemoryError> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| MemoryError::Generic(format!("Failed to create directory: {e}")))?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| MemoryError::Generic(format!("Failed to open database: {e}")))?;

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_entries (
                key TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                category TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                access_count INTEGER DEFAULT 0,
                trust_score REAL DEFAULT 0.0
            )",
            [],
        )
        .map_err(|e| MemoryError::Generic(format!("Failed to create schema: {e}")))?;

        // Create indexes (idempotent)
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_category ON memory_entries(category)",
            [],
        )
        .map_err(|e| MemoryError::Generic(format!("Failed to create index: {e}")))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_created_at ON memory_entries(created_at)",
            [],
        )
        .map_err(|e| MemoryError::Generic(format!("Failed to create index: {e}")))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Parse a database row into a MemoryEntry
    fn row_to_entry(row: &rusqlite::Row<'_>) -> Result<MemoryEntry, rusqlite::Error> {
        let key: String = row.get(0)?;
        let content: String = row.get(1)?;
        let category_str: String = row.get(2)?;
        let created_at_str: String = row.get(3)?;
        let updated_at_str: String = row.get(4)?;
        let importance: f64 = row.get(5).unwrap_or(0.5);
        let access_count: i64 = row.get(6).unwrap_or(0);
        let trust_score: f64 = row.get(7).unwrap_or(0.0);

        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    4,
                    "created_at".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;

        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    5,
                    "updated_at".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;

        Ok(MemoryEntry {
            key,
            content,
            category: parse_category(&category_str),
            created_at,
            updated_at,
            importance,
            access_count: access_count as u64,
            trust_score,
        })
    }
}

/// Parse a category string, handling both known and custom categories
fn parse_category(s: &str) -> MemoryCategory {
    match s {
        "core" => MemoryCategory::Core,
        "daily" => MemoryCategory::Daily,
        "conversation" => MemoryCategory::Conversation,
        other => MemoryCategory::Custom(other.to_string()),
    }
}

#[async_trait]
impl MemoryProvider for SqliteMemoryProvider {
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError> {
        let key = key.to_string();
        let content = content.to_string();
        let category_str = category.as_str().to_string();

        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().map_err(|e| MemoryError::Generic(e.to_string()))?;
            c.execute(
                "INSERT INTO memory_entries (key, content, category, created_at, updated_at, importance, access_count, trust_score)
                 VALUES ($1, $2, $3, $4, $4, 0.5, 0, 0.0)
                 ON CONFLICT(key) DO UPDATE SET
                     content = excluded.content,
                     category = excluded.category,
                     updated_at = excluded.updated_at",
                params![
                    key,
                    content,
                    category_str,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|e| MemoryError::Generic(format!("Failed to store entry: {e}")))?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn recall(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let query_str = query.to_string();
        let category_str = category.map(String::from);
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().map_err(|e| MemoryError::Generic(e.to_string()))?;

            let mut stmt = match &category_str {
                Some(_) => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance, access_count, trust_score
                     FROM memory_entries
                     WHERE category = ?1
                     ORDER BY updated_at DESC
                     LIMIT ?2",
                )?,
                None => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance, access_count, trust_score
                     FROM memory_entries
                     ORDER BY updated_at DESC
                     LIMIT ?1",
                )?,
            };

            let rows = match &category_str {
                Some(cat) => stmt.query_map(params![cat, limit as i64], Self::row_to_entry)?,
                None => stmt.query_map(params![limit as i64], Self::row_to_entry)?,
            };

            let query_lower = query_str.to_lowercase();
            let mut result = Vec::new();
            for row in rows {
                let entry = row.map_err(|e| MemoryError::Generic(e.to_string()))?;

                // Client-side query filtering
                if !query_str.is_empty() {
                    let query_match = entry.key.to_lowercase().contains(&query_lower)
                        || entry.content.to_lowercase().contains(&query_lower);
                    if !query_match {
                        continue;
                    }
                }

                result.push(entry);
            }

            Ok(result)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn delete(&self, key: &str) -> Result<(), MemoryError> {
        let key = key.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            let affected = c
                .execute("DELETE FROM memory_entries WHERE key = ?1", params![key])
                .map_err(|e| MemoryError::Generic(format!("Failed to delete entry: {e}")))?;

            if affected == 0 {
                return Err(MemoryError::KeyNotFound(key));
            }

            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn purge_category(&self, category: &str) -> Result<(), MemoryError> {
        let category = category.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn
                .lock()
                .map_err(|e| MemoryError::Generic(e.to_string()))?;
            c.execute(
                "DELETE FROM memory_entries WHERE category = ?1",
                params![category],
            )
            .map_err(|e| MemoryError::Generic(format!("Failed to purge category: {e}")))?;
            Ok(())
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }

    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError> {
        let category_str = category.map(String::from);
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().map_err(|e| MemoryError::Generic(e.to_string()))?;

            let mut stmt = match &category_str {
                Some(_) => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance, access_count, trust_score
                     FROM memory_entries
                     WHERE category = ?1
                     ORDER BY updated_at DESC",
                )?,
                None => c.prepare(
                    "SELECT key, content, category, created_at, updated_at, importance, access_count, trust_score
                     FROM memory_entries
                     ORDER BY updated_at DESC",
                )?,
            };

            let rows = match &category_str {
                Some(cat) => stmt.query_map(params![cat], Self::row_to_entry)?,
                None => stmt.query_map([], Self::row_to_entry)?,
            };

            let mut result = Vec::new();
            for row in rows {
                result.push(row.map_err(|e| MemoryError::Generic(e.to_string()))?);
            }

            Ok(result)
        })
        .await
        .map_err(|e| MemoryError::Generic(format!("Task panicked: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_provider() -> (SqliteMemoryProvider, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_memory.db");
        let provider = SqliteMemoryProvider::new(db_path).unwrap();
        (provider, temp_dir)
    }

    #[tokio::test]
    async fn store_and_recall() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("test_key", "Hello world", MemoryCategory::Core)
            .await
            .unwrap();

        let results = provider.recall("test_key", None, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "test_key");
        assert_eq!(results[0].content, "Hello world");
        assert_eq!(results[0].importance, 0.5);
        assert_eq!(results[0].access_count, 0);
    }

    #[tokio::test]
    async fn store_upserts_existing() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("key", "First value", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("key", "Second value", MemoryCategory::Core)
            .await
            .unwrap();

        let results = provider.recall("", None, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Second value");
    }

    #[tokio::test]
    async fn delete_entry() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("to_delete", "value", MemoryCategory::Core)
            .await
            .unwrap();

        provider.delete("to_delete").await.unwrap();

        let results = provider.recall("to_delete", None, 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn delete_missing_key_returns_error() {
        let (provider, _temp) = create_test_provider();

        let err = provider.delete("nonexistent").await.unwrap_err();
        assert!(matches!(err, MemoryError::KeyNotFound(_)));
    }

    #[tokio::test]
    async fn purge_category() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("a", "1", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("b", "2", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("c", "3", MemoryCategory::Daily)
            .await
            .unwrap();

        provider.purge_category("core").await.unwrap();

        let core = provider.recall("", Some("core"), 10).await.unwrap();
        assert!(core.is_empty());

        let daily = provider.recall("", Some("daily"), 10).await.unwrap();
        assert_eq!(daily.len(), 1);
    }

    #[tokio::test]
    async fn filter_by_category() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("x", "1", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("y", "2", MemoryCategory::Daily)
            .await
            .unwrap();

        let core = provider.recall("", Some("core"), 10).await.unwrap();
        assert_eq!(core.len(), 1);
        assert_eq!(core[0].key, "x");

        let all = provider.recall("", None, 10).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn export_all() {
        let (provider, _temp) = create_test_provider();

        provider
            .store("a", "1", MemoryCategory::Core)
            .await
            .unwrap();
        provider
            .store("b", "2", MemoryCategory::Daily)
            .await
            .unwrap();

        let exported = provider.export(None).await.unwrap();
        assert_eq!(exported.len(), 2);

        let exported_core = provider.export(Some("core")).await.unwrap();
        assert_eq!(exported_core.len(), 1);
        assert_eq!(exported_core[0].key, "a");
    }

    #[tokio::test]
    async fn recall_with_limit() {
        let (provider, _temp) = create_test_provider();

        for i in 0..5 {
            provider
                .store(
                    &format!("entry_{i}"),
                    &format!("value_{i}"),
                    MemoryCategory::Core,
                )
                .await
                .unwrap();
        }

        let results = provider.recall("", None, 2).await.unwrap();
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    async fn custom_category() {
        let (provider, _temp) = create_test_provider();

        provider
            .store(
                "custom_key",
                "value",
                MemoryCategory::Custom("my_proj".to_string()),
            )
            .await
            .unwrap();

        let results = provider.recall("", Some("my_proj"), 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "custom_key");
    }

    #[tokio::test]
    async fn persistence_survives_restart() {
        // Create provider, store data, drop it
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("persist_test.db");

        {
            let provider = SqliteMemoryProvider::new(db_path.clone()).unwrap();
            provider
                .store("persist_key", "persist_value", MemoryCategory::Core)
                .await
                .unwrap();
        } // Provider dropped here, connection closed

        // Reopen the same database file
        let provider2 = SqliteMemoryProvider::new(db_path).unwrap();
        let results = provider2.recall("persist_key", None, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "persist_value");
    }
}
