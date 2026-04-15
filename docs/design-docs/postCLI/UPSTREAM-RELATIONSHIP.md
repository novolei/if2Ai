# UPSTREAM-RELATIONSHIP.md

## if2Ai vs claw-cli / iClaw / hermes-agent

This document describes the relationship between if2Ai and its upstream relatives:
- **claw-cli / iClaw** — CLI agent with SQLite memory
- **hermes-agent** — Python agent framework with HRR memory

if2Ai is a **separate project**, not a fork. It selectively imports patterns from upstream while adapting them for the Tauri desktop platform.

---

## Schema Compatibility

### MemoryEntry

| Field | claw-cli | if2Ai | Compatible? |
|-------|----------|-------|-------------|
| `key` | String | String | Yes |
| `content` | String | String | Yes |
| `category` | String | Enum (Core/Daily/Conversation/Custom) | Yes (serialized as string) |
| `created_at` | ISO timestamp | DateTime<Utc> | Yes (RFC3339) |
| `updated_at` | ISO timestamp | DateTime<Utc> | Yes (RFC3339) |
| `importance` | — | f64 | No (if2Ai extension) |
| `access_count` | — | u64 | No (if2Ai extension) |
| `trust_score` | — | f64 | No (if2Ai extension) |

if2Ai can export to claw-cli-compatible format via `export_for_clawcli()`, which strips extended fields.

### Session

| Field | claw-cli | if2Ai | Compatible? |
|-------|----------|-------|-------------|
| `id` | String | UUID-based | Yes |
| `title` | String | String | Yes |
| `messages` | List[Message] | Vec<ConversationMessage> | Yes |
| `created_at` | String | DateTime | Yes |
| `updated_at` | String | DateTime | Yes |
| `token_count` | int | u64 (via TokenUsage) | Yes |
| `pinned` | bool | bool | Yes |
| `project_id` | — | String | No (if2Ai extension) |
| `version` | — | u32 | No (if2Ai extension) |

### Memory Categories

Both projects use identical category names:
- `core` — Core knowledge
- `daily` — Daily activity logs
- `conversation` — Conversation history

---

## Shared Design Decisions

### Token Budget Allocation (10/20/30/40%)

Both projects use the same budget split:
- System: 10%
- Episodic: 20%
- Semantic: 30%
- Working: 40%

### Weibull Decay

Both projects use Weibull decay for episodic memory:
- Lambda: 7 days
- k: 1.2

### Memory Provider Pattern

Both use a provider/abstraction pattern for storage, but:
- claw-cli: SQLite + JSON files
- if2Ai: SQLite + LanceDB (vector) + HRR (algebraic)

---

## Divergent Decisions

### Vector Search

| Aspect | claw-cli | if2Ai | Rationale |
|--------|----------|-------|-----------|
| Embedding | OpenAI API | FastEmbed (offline) | Desktop app requires offline capability |
| Vector store | External API | LanceDB (embedded) | No external service dependency |
| Model | — | multilingual-e5-small (384d) | Offline, multilingual support |

### HRR (Holographic Reduced Representations)

| Aspect | hermes-agent | if2Ai | Rationale |
|--------|-------------|-------|-----------|
| Role | Primary memory store | P2a complement to LanceDB | HRR capacity is O(√dim), too limited for primary storage |
| Integration | Direct | Via HybridMemoryProvider | Clean separation of concerns |

### Learning Modules

| Aspect | hermes-agent | if2Ai | Rationale |
|--------|-------------|-------|-----------|
| Self-model | Integrated in MemoryManager | Separate learning module | Independent evolution |
| RL training | Tinker-Atropos integration | Export only (ShareGPT JSONL) | Scope difference — RL training requires GPU infrastructure |
| Trajectory | Direct export | Export-only with privacy controls | User data stays local until explicitly exported |

### Platform

| Aspect | claw-cli | if2Ai |
|--------|----------|-------|
| Platform | CLI | Tauri 2 desktop app |
| UI | Terminal | React frontend |
| Commands | CLI args | Tauri IPC commands |
| Config | Config files | Settings UI |

---

## Code Provenance

| if2Ai File | Origin | Adaptation |
|-----------|--------|-----------|
| `modules/runtime/compact.rs` | iClaw/context_compressor.py, hermes-agent/context_compressor.py | Rust async, simplified token estimation |
| `modules/memory/mod.rs` (MemoryEntry) | claw-cli memory schema | Extended with importance, access_count, trust_score |
| `modules/runtime/budget.rs` (ContextBudget) | claw-cli budget allocation | Rust implementation with tiktoken-rs |
| `modules/learning/trajectory.rs` | hermes-agent trajectory | ShareGPT JSONL export, no RL training |

---

## Import Policy

When importing patterns from upstream:

1. **Read the source** — Understand the original implementation
2. **Document the ADR** — Record the decision with rationale
3. **Adapt to Rust** — Translate Python patterns to idiomatic Rust
4. **Test equivalence** — Verify behavior matches original
5. **Note provenance** — Add source comments at file headers

---

## Future Interoperability

if2Ai's schema compatibility with claw-cli enables:

- **Data migration**: Users can export if2Ai memories to claw-cli format
- **Shared tooling**: Both projects could use the same analysis tools
- **Cross-pollination**: New patterns from either project could be adopted by the other

The extended fields (importance, access_count, trust_score) are silently ignored by claw-cli, so backward compatibility is maintained.
