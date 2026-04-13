# ADR-005: Relationship with Upstream claw-cli

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: All

---

## Context

if2Ai and claw-cli share common ancestry. Both are built on similar principles for memory and session management. The question is: **How should if2Ai relate to upstream claw-cli?**

Key similarities:
- SQLite-backed memory (both)
- Session persistence with JSON files (both)
- Similar memory entry schema
- Both use chrono for timestamps

Key differences:
- **Platform**: claw-cli is CLI; if2Ai is Tauri desktop
- **Providers**: claw-cli uses JSON files; if2Ai uses SQLite + LanceDB
- **Scope**: claw-cli is CLI tool; if2Ai is full agent platform
- **Learning**: hermes-agent has trajectory learning; if2Ai does not

---

## Decision

if2Ai maintains **selective alignment** with claw-cli upstream:

1. **Import useful patterns** from claw-cli/UClaw when they improve if2Ai
2. **Do not fork** the codebase — if2Ai is a separate project
3. **Document deviations** in ADRs (this document)
4. **Adopt compatible schemas** where possible for future interoperability

### Shared Concepts (Aligned)

| Concept | claw-cli/UClaw | if2Ai | Notes |
|---------|---------------|-------|-------|
| Memory schema | JSON/SQLite | SQLite | Compatible |
| Session format | JSON | JSON | Compatible |
| Memory categories | Core/Daily/Conversation | Core/Daily/Conversation | Identical |
| Importance decay | Weibull | Weibull | Identical |
| Token budgets | 10/20/30/40% | 10/20/30/40% | Identical |

### Divergent Concepts (Documented)

| Concept | claw-cli/UClaw | if2Ai | Rationale |
|---------|---------------|-------|-----------|
| Vector search | External API | FastEmbed + LanceDB | Offline required |
| Embedding model | OpenAI | FastEmbed multilingual | Offline required |
| HRR | Primary store | P2a complement | Capacity limits |
| Tool registry | JSON files | Rust registry | Different architecture |
| RL training | Tinker-Atropos | Export only | Scope difference |

### Schema Compatibility

if2Ai memory entries are compatible with claw-cli:

```rust
// if2Ai MemoryEntry (compatible with claw-cli)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub content: String,
    pub category: MemoryCategory,  // Core, Daily, Conversation, Custom
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Extended fields not in claw-cli (if2Ai-specific)
    pub importance: f32,          // Default 0.5
    pub access_count: u32,         // Default 0
    pub trust_score: f32,          // Default 0.0
}

pub enum MemoryCategory {
    Core,
    Daily,
    Conversation,
    Custom(String),
}
```

```python
# claw-cli memory entry (Python, equivalent structure)
@dataclass
class MemoryEntry:
    key: str
    content: str
    category: str  # "core", "daily", "conversation"
    created_at: str  # ISO timestamp
    updated_at: str
    # No importance/access_count/trust_score in claw-cli base
```

### Session Format Compatibility

```rust
// if2Ai Session (compatible with claw-cli)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,        // if2Ai extension
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: String,
    pub updated_at: String,
    pub token_count: u64,
    pub pinned: bool,
}
```

```python
# claw-cli Session (Python, equivalent structure)
@dataclass
class Session:
    id: str
    title: str
    messages: List[Message]
    created_at: str
    updated_at: str
    token_count: int
    pinned: bool = False
    # No project_id in claw-cli
```

---

## Rationale

### Why Not Fork?

1. **Different platforms**: CLI vs desktop have different constraints
2. **Different priorities**: Offline vs API-first
3. **Different scope**: Single tool vs full platform
4. **Maintenance burden**: Fork divergence is costly

### Why Import Patterns?

1. **Proven designs**: UClaw's three-layer memory is well-tested
2. **Schema compatibility**: Enables future data sharing
3. **Reduced R&D**: Don't reinvent what's already good

### Why Document Deviations?

1. **Future integration**: If if2Ai and claw-cli need to share data, schemas are compatible
2. **Clear boundaries**: Developers understand what if2Ai-specific code is
3. **ADR-driven decisions**: All deviations have documented rationale

---

## Consequences

### Positive
- Clear relationship with upstream
- Schema compatibility enables future data exchange
- Can import patterns without full fork

### Negative
- Must maintain documentation of differences
- Schema extensions (importance, trust_score) may not be upstream compatible

### Neutral
- if2Ai and claw-cli evolve independently
- hermes-agent integration is separate concern

---

## Implementation Notes

### Importing Patterns

When importing from claw-cli/UClaw:

1. **Read the source**: Understand the original implementation
2. **Document the ADR**: Record the decision in this ADR index
3. **Adapt to Rust**: Translate Python patterns to idiomatic Rust
4. **Test equivalence**: Verify behavior matches original

### Example: Compaction Algorithm

if2Ai's compaction is identical to upstream:

```rust
// src-tauri/src/modules/runtime/compact.rs
//
// This implementation is adapted from:
// - iClaw/agent/context_compressor.py (UClaw)
// - hermes-agent/agent/context_compressor.py
//
// Changes from upstream:
// - Rust async/await instead of Python asyncio
// - Token counting using tiktoken-rs instead of tiktoken
// - SessionManager integration for persistence
```

### Data Export for claw-cli

if2Ai can export memory in claw-cli-compatible format:

```rust
pub fn export_for_clawcli(&self, path: PathBuf) -> Result<(), SessionError> {
    let entries = self.memory.export(None)?;
    let claw_entries: Vec<ClawCliMemoryEntry> = entries
        .into_iter()
        .map(|e| ClawCliMemoryEntry {
            key: e.key,
            content: e.content,
            category: e.category.as_str().to_string(),
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        })
        .collect();

    let json = serde_json::to_string_pretty(&claw_entries)?;
    std::fs::write(path, json)?;
    Ok(())
}
```

---

## Review Checklist

- [ ] MemoryEntry schema is claw-cli compatible
- [ ] Session schema is claw-cli compatible
- [ ] MemoryCategory enum matches claw-cli
- [ ] Compaction algorithm documented as adapted from upstream
- [ ] Schema extensions (importance, trust_score) documented
- [ ] Data export to claw-cli format supported
- [ ] Divergent decisions documented in ADRs

---

## References

- [iClaw/UClaw Context Compressor](https://github.com/1tius/iClaw/blob/main/agent/context_compressor.py)
- [hermes-agent Context Compressor](https://github.com/1tius/hermes-agent/blob/main/agent/context_compressor.py)
- [if2Ai Session Manager](../src-tauri/src/modules/session/manager.rs)
- [if2Ai Memory Module](../src-tauri/src/modules/memory/mod.rs)
