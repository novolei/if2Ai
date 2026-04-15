//! Memory provider implementations
//!
//! - [`SqliteMemoryProvider`] — SQLite-backed (default, persistent)
//! - [`InMemoryMemoryProvider`] — In-memory (deprecated, for tests only)

mod sqlite_provider;

pub use sqlite_provider::SqliteMemoryProvider;
