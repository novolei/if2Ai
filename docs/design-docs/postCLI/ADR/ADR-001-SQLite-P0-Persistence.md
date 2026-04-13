# ADR-001: SQLite P0 Persistence

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: P0

---

## Context

The current if2Ai memory implementation uses `InMemoryMemoryProvider` with a `HashMap` stored in process memory. This design has a critical flaw: **all memory entries are lost when the process restarts**.

```rust
// CURRENT (BUGGY) IMPLEMENTATION
pub struct InMemoryMemoryProvider {
    entries: RwLock<HashMap<String, MemoryEntry>>,  // ← 进程内存, 进程终止即丢失
}
```

For a desktop application where users expect persistence across restarts, this is unacceptable. The SessionManager already uses JSON file persistence, but the memory system does not.

---

## Decision

Replace `InMemoryMemoryProvider` with a SQLite-backed implementation that persists data to disk.

### SQLite Schema

```sql
CREATE TABLE memory_entries (
    key TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    category TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    importance REAL DEFAULT 0.5,
    access_count INTEGER DEFAULT 0,
    trust_score REAL DEFAULT 0.0
);

CREATE INDEX idx_memory_category ON memory_entries(category);
CREATE INDEX idx_memory_created_at ON memory_entries(created_at);
```

### Provider Trait (Unchanged)

The `MemoryProvider` trait interface remains unchanged, ensuring backward compatibility:

```rust
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    async fn store(&self, key: &str, content: &str, category: MemoryCategory) -> Result<(), MemoryError>;
    async fn recall(&self, query: &str, category: Option<&str>, limit: usize) -> Result<Vec<MemoryEntry>, MemoryError>;
    async fn delete(&self, key: &str) -> Result<(), MemoryError>;
    async fn purge_category(&self, category: &str) -> Result<(), MemoryError>;
    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError>;
}
```

### New Implementation: SqliteMemoryProvider

```rust
pub struct SqliteMemoryProvider {
    pool: SqlitePool,
}

impl SqliteMemoryProvider {
    pub async fn new(db_path: PathBuf) -> Result<Self, MemoryError> {
        let pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
            .await
            .map_err(|e| MemoryError::Generic(e.to_string()))?;

        // Initialize schema
        pool.execute(
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
        .await
        .map_err(|e| MemoryError::Generic(e.to_string()))?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl MemoryProvider for SqliteMemoryProvider {
    async fn store(&self, key: &str, content: &str, category: MemoryCategory) -> Result<(), MemoryError> {
        let now = chrono::Utc::now().to_rfc3339();
        self.pool
            .execute(
                "INSERT OR REPLACE INTO memory_entries (key, content, category, created_at, updated_at)
                 VALUES ($1, $2, $3, COALESCE((SELECT created_at FROM memory_entries WHERE key = $1), $4), $4)",
                [&key, &content, category.as_str(), &now],
            )
            .await
            .map_err(|e| MemoryError::Generic(e.to_string()))?;
        Ok(())
    }

    async fn recall(&self, query: &str, category: Option<&str>, limit: usize) -> Result<Vec<MemoryEntry>, MemoryError> {
        // Query implementation (see full code in mod.rs)
    }

    async fn delete(&self, key: &str) -> Result<(), MemoryError> {
        self.pool
            .execute("DELETE FROM memory_entries WHERE key = $1", [&key])
            .await
            .map_err(|e| MemoryError::Generic(e.to_string()))?;
        Ok(())
    }

    async fn purge_category(&self, category: &str) -> Result<(), MemoryError> {
        self.pool
            .execute("DELETE FROM memory_entries WHERE category = $1", [&category])
            .await
            .map_err(|e| MemoryError::Generic(e.to_string()))?;
        Ok(())
    }

    async fn export(&self, category: Option<&str>) -> Result<Vec<MemoryEntry>, MemoryError> {
        // Export implementation
    }
}
```

---

## Rationale

1. **Persistence**: SQLite data survives process restarts, unlike HashMap in memory.
2. **Mature technology**: SQLite is battle-tested, with excellent Rust support (`sqlx`).
3. **No external dependencies**: Unlike LanceDB, SQLite is embedded and requires no server process.
4. **FTS support**: SQLite FTS5 provides full-text search when needed (P0 optional).
5. **ACID compliance**: Atomic writes prevent corruption on crashes.
6. **Backward compatible**: Trait unchanged, only implementation swaps.

---

## Consequences

### Positive
- Memory persists across restarts
- Query performance improves with B-tree indexing
- Foundation for FTS5 full-text search (P1)

### Negative
- Slight latency increase vs in-memory HashMap (mitigated by connection pooling)
- Database file management (mitigated by path in app data directory)

### Neutral
- SessionManager already uses similar JSON file approach (familiar pattern)

---

## Implementation Notes

### File Location
- Database path: `~/.if2ai/memory/memory.db`
- Create directory on first launch

### Migration Path
```rust
// In module initialization
pub fn default_memory_provider() -> SharedMemoryProvider {
    let db_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("memory")
        .join("memory.db");

    Arc::new(SqliteMemoryProvider::new(db_path).await)
}
```

### Error Handling
All errors map to `MemoryError`:
- `KeyNotFound` for missing keys on delete
- `CategoryNotFound` for purge on empty category
- `Generic` for SQL errors

---

## Review Checklist

- [ ] `InMemoryMemoryProvider` replaced with `SqliteMemoryProvider`
- [ ] `MemoryProvider` trait unchanged (backward compatible)
- [ ] SQLite schema created with proper indexes
- [ ] `store()` uses INSERT OR REPLACE (upsert)
- [ ] `recall()` filters by category and query
- [ ] `delete()` returns `KeyNotFound` on missing key
- [ ] `purge_category()` removes all in category
- [ ] `export()` returns all or filtered entries
- [ ] Error variants cover all failure modes

---

## References

- [SessionManager Implementation](../src-tauri/src/modules/session/manager.rs) — similar persistence pattern
- [SQLite FTS5](https://www.sqlite.org/fts5.html) — future full-text search
- [sqlx crate](https://github.com/launchbadge/sqlx) — async SQLite for Rust
