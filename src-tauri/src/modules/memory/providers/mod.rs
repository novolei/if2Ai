//! Memory provider implementations
//!
//! - [`SqliteMemoryProvider`] — SQLite-backed (default, persistent)
//! - [`InMemoryMemoryProvider`] — In-memory (deprecated, for tests only)
//! - [`LanceDBMemory`] — LanceDB vector store (embedding + ANN search)
//! - [`VectorMemoryProvider`] — Vector-backed MemoryProvider (FastEmbed + LanceDB)

mod lancedb;
mod sqlite_provider;
mod vector_provider;

pub use sqlite_provider::SqliteMemoryProvider;
