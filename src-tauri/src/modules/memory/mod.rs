//! Memory module - provides long-term memory storage and retrieval
//!
//! This module defines the MemoryProvider trait and related types for
//! storing and retrieving persistent memory across sessions.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory entry stored in the memory system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub content: String,
    pub category: MemoryCategory,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Memory category for organizing memory entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryCategory {
    Core,
    Daily,
    Conversation,
    Custom(String),
}

impl MemoryCategory {
    pub fn as_str(&self) -> &str {
        match self {
            MemoryCategory::Core => "core",
            MemoryCategory::Daily => "daily",
            MemoryCategory::Conversation => "conversation",
            MemoryCategory::Custom(s) => s,
        }
    }
}

/// Error type for memory operations
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum MemoryError {
    #[error("memory error: {0}")]
    Generic(String),
    #[error("key not found: {0}")]
    KeyNotFound(String),
    #[error("category not found: {0}")]
    CategoryNotFound(String),
}

/// Trait for memory storage providers
/// Implement this trait to provide different storage backends (in-memory, file-based, etc.)
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    /// Store a memory entry
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError>;

    /// Recall memory entries matching a query
    async fn recall(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError>;

    /// Delete a specific memory entry by key
    async fn delete(&self, key: &str) -> Result<(), MemoryError>;

    /// Purge all entries in a category
    async fn purge_category(&self, category: &str) -> Result<(), MemoryError>;

    /// Export entries, optionally filtered by category
    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError>;
}

/// In-memory implementation of MemoryProvider
#[derive(Debug, Default)]
pub struct InMemoryMemoryProvider {
    entries: RwLock<HashMap<String, MemoryEntry>>,
}

impl InMemoryMemoryProvider {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MemoryProvider for InMemoryMemoryProvider {
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError> {
        let now = chrono::Utc::now();
        let entry = MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: category.clone(),
            created_at: now,
            updated_at: now,
        };
        let mut entries = self.entries.write().await;
        entries.insert(key.to_string(), entry);
        Ok(())
    }

    async fn recall(
        &self,
        query: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let entries = self.entries.read().await;
        let query_lower = query.to_lowercase();

        let filtered: Vec<MemoryEntry> = entries
            .values()
            .filter(|e| {
                // Check category filter
                let category_match = category.map(|c| e.category.as_str() == c).unwrap_or(true);
                // Check query match (in key or content)
                let query_match = query.is_empty()
                    || e.key.to_lowercase().contains(&query_lower)
                    || e.content.to_lowercase().contains(&query_lower);
                category_match && query_match
            })
            .take(limit)
            .cloned()
            .collect();

        Ok(filtered)
    }

    async fn delete(&self, key: &str) -> Result<(), MemoryError> {
        let mut entries = self.entries.write().await;
        if entries.remove(key).is_none() {
            return Err(MemoryError::KeyNotFound(key.to_string()));
        }
        Ok(())
    }

    async fn purge_category(&self, category: &str) -> Result<(), MemoryError> {
        let mut entries = self.entries.write().await;
        entries.retain(|_, e| e.category.as_str() != category);
        Ok(())
    }

    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError> {
        let entries = self.entries.read().await;
        let filtered: Vec<MemoryEntry> = entries
            .values()
            .filter(|e| category.map(|c| e.category.as_str() == c).unwrap_or(true))
            .cloned()
            .collect();
        Ok(filtered)
    }
}

/// Global memory provider instance
pub type SharedMemoryProvider = Arc<dyn MemoryProvider>;

/// Default in-memory memory provider
pub fn default_memory_provider() -> SharedMemoryProvider {
    Arc::new(InMemoryMemoryProvider::new())
}
