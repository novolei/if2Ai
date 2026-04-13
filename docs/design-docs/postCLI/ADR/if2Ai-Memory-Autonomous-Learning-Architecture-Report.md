# if2Ai Memory and Autonomous Learning Architecture Report

**Version**: 2.0
**Date**: 2026-04-13
**Status**: Ratified
**Supersedes**: v1.0 (2026-01-15)

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Architecture Overview](#2-architecture-overview)
3. [Memory System Architecture](#3-memory-system-architecture)
4. [Autonomous Learning Framework](#4-autonomous-learning-framework)
5. [Implementation Phases](#5-implementation-phases)
6. [ADR Index](#6-adr-index)
7. [Differences from hermes-agent](#7-differences-from-hermes-agent)
8. [Security Considerations](#8-security-considerations)
9. [Migration Guide](#9-migration-guide)

---

## 1. Executive Summary

This document describes the integrated architecture for if2Ai's memory system and autonomous learning capabilities, synthesized from three sources:

1. **if2Ai current implementation** — Tauri 2 + React desktop application with Rust backend
2. **UClaw (iClaw)** — Three-layer memory architecture (Working/Episodic/Semantic) with HRR algebraic reasoning
3. **hermes-agent** — Trajectory learning, self-model reflection, and RL training infrastructure

### Key Architectural Decisions

| Decision | Rationale |
|----------|-----------|
| SQLite FTS5 as P0 persistence | Reliable CRUD + FTS without external dependencies; correct for desktop app |
| FastEmbed + LanceDB as P1 vector search | Offline multilingual-e5-small (384d); no external API calls |
| HRR as P2a complement (not replacement) | Algebraic reasoning for multi-hop queries; O(√dim) capacity limit |
| Self-learning modules independent of memory | Clean separation of concerns; memory is storage, learning is reasoning |
| Trajectory learning as P3 | ShareGPT JSONL format; feeds future RL pipeline without coupling |
| Skills Hub not introduced | Desktop single-user model; security risks of marketplace outweigh benefits |

### What We Do NOT Adopt from hermes-agent

| Rejected Component | Reason |
|-------------------|--------|
| Tinker-Atropos RL training | Requires GPU infrastructure, separate ML platform — orthogonal to Tauri desktop app |
| 8-source Skills Hub | Security risk (341 malicious skills in 2026-02 ClawHub incident); if2Ai ToolRegistry sufficient |
| Full RL training loop | Requires dedicated training infrastructure outside if2Ai scope |

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        if2Ai Desktop App                         │
│                    (Tauri 2 + React + TypeScript)                │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐ │
│  │  React UI    │  │ Tauri IPC    │  │   Rust Backend       │ │
│  │  (Frontend)  │  │  Bridge      │  │   (src-tauri/)      │ │
│  └──────────────┘  └──────────────┘  └──────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                     Module Architecture                          │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌───────────┐  │
│  │  Session   │ │  Memory    │ │  Runtime   │ │   Tools   │  │
│  │  Manager   │ │  Provider  │ │   Core     │ │  Registry │  │
│  └────────────┘ └────────────┘ └────────────┘ └───────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Layer Structure

```
Layer 0: Tauri Process & IPC
Layer 1: SQLite Persistence (P0)         ← replace InMemory HashMap
Layer 2: FastEmbed + LanceDB (P1)        ← optional vector search
Layer 3: HRR Algebraic Reasoning (P2a)   ← optional, complement to LanceDB
Layer 4: Self-Model + Reflection (P2b)  ← optional, independent of memory
Layer 5: Trajectory Learning (P3)        ← optional, JSONL persistence
```

---

## 3. Memory System Architecture

### 3.1 Three-Layer Memory Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    Context Budget Slots (4000 tokens)            │
├─────────────┬─────────────┬─────────────┬─────────────────────┤
│   System    │  Episodic   │  Semantic   │      Working        │
│   (10%)     │   (20%)     │   (30%)     │       (40%)         │
│   400 tok   │   800 tok   │   1200 tok  │      1600 tok       │
├─────────────┼─────────────┼─────────────┼─────────────────────┤
│ Frozen      │ Rolling     │ LanceDB     │ Sliding Window      │
│ Snapshot    │ LLM Summary │ Vector + FTS│ (8 turns)           │
├─────────────┼─────────────┼─────────────┼─────────────────────┤
│ Compact     │ Importance  │ Weibull     │ Priority Queue      │
│ on load     │ decay       │ Decay       │ + token budget      │
└─────────────┴─────────────┴─────────────┴─────────────────────┘
```

### 3.2 Memory Categories

```rust
pub enum MemoryCategory {
    Core,          // Long-term facts about user preferences, projects
    Daily,         // Per-day interaction summaries
    Conversation,  // Individual conversation entries
    Custom(String), // User-defined categories
}
```

### 3.3 Active Retrieval vs Passive Invocation

**Active Retrieval** (P1+): Memory is queried proactively before each LLM call using intent-based retrieval weights.

```rust
pub enum QueryIntent {
    Code,      // weights: semantic=0.5, episodic=0.3, working=0.2
    Task,      // weights: semantic=0.4, episodic=0.4, working=0.2
    Fact,      // weights: semantic=0.6, episodic=0.2, working=0.2
    Person,    // weights: semantic=0.3, episodic=0.5, working=0.2
    General,   // weights: semantic=0.3, episodic=0.3, working=0.4
}
```

**Passive Invocation** (P0): Memory is written but not automatically retrieved. LLM can call `memory.recall()` explicitly.

### 3.4 Importance Scoring

```rust
pub struct MemoryEntry {
    pub key: String,
    pub content: String,
    pub category: MemoryCategory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub importance: f32,           // Initial importance (0.0-1.0)
    pub access_count: u32,         // Number of times accessed
    pub trust_score: f32,          // Feedback-adjusted trust (-1.0 to 1.0)
}

impl MemoryEntry {
    pub fn compute_importance(&self, age_hours: f32) -> f32 {
        let base = self.importance + (self.access_count as f32 * 0.01);
        let trust_boost = (self.trust_score + 1.0) * 0.05;
        let decay = (-age_hours / (24.0 * 7.0)).exp(); // e^(-age/7days)
        (base + trust_boost) * decay
    }
}
```

### 3.5 Weibull Decay for Semantic Memory

```rust
fn weibull_decay(age_hours: f32, initial_importance: f32, lambda: f32, k: f32) -> f32 {
    let shape = 1.2;  // k: controls early decay rate
    let scale = 24.0 * 7.0;  // lambda: 7-day scale
    initial_importance * (k / lambda) * (age_hours / lambda).powf(k - 1.0) * (-(age_hours / lambda).powf(k)).exp()
}
```

### 3.6 Hybrid Retrieval (P1+)

When vector search is enabled, retrieval uses Reciprocal Rank Fusion (RRF):

```rust
fn rrf_fusion(results: Vec<Vec<ScoredMemory>>, k: u32) -> Vec<ScoredMemory> {
    let mut scores: HashMap<String, f32> = HashMap::new();
    for ranked_list in results {
        for (rank, item) in ranked_list.iter().enumerate() {
            let score = 1.0 / (k + rank as u32) as f32;
            *scores.entry(item.key.clone()).or_insert(0.0) += score;
        }
    }
    scores.into_iter().map(|(k, v)| ScoredMemory { key: k, score: v }).collect()
}
```

### 3.7 Context Compaction Algorithm

The compaction algorithm runs when episodic memory exceeds budget:

```rust
async fn compact_episodic(session: &mut Session, budget_tokens: usize) -> Result<()> {
    // 1. Sort entries by importance (Weibull decay + trust + access count)
    // 2. Select top-N entries fitting within budget
    // 3. Generate LLM summary if entries removed
    // 4. Emit summary as new episodic entry
    // 5. Prune original entries
}
```

---

## 4. Autonomous Learning Framework

### 4.1 Self-Model

The Self-Model represents the agent's understanding of its own capabilities:

```rust
pub struct SelfModel {
    pub capabilities: Vec<Capability>,
    pub limitations: Vec<Limitation>,
    pub learned_patterns: Vec<LearnedPattern>,
    pub performance_metrics: PerformanceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub id: String,
    pub trigger: String,          // Context pattern
    pub action: String,          // What was done successfully
    pub success_rate: f32,       // Historical success rate
    pub sample_count: u32,
    pub last_applied: DateTime<Utc>,
}
```

### 4.2 Reflection Engine

The Reflection Engine analyzes past experiences to update the Self-Model:

```rust
#[async_trait]
pub trait ReflectionEngine: Send + Sync {
    async fn analyze_session(&self, session: &Session) -> Result<Vec<Reflection>>;
    async fn update_self_model(&self, reflections: Vec<Reflection>) -> Result<()>;
}

pub struct Reflection {
    pub pattern: String,
    pub insight: String,
    pub confidence: f32,
    pub source_session: String,
}
```

### 4.3 Trajectory Learning

Trajectory data is persisted in ShareGPT JSONL format:

```json
{
  "id": "traj_abc123",
  "conversations": [
    {"from": "human", "value": "How do I fix the memory leak?"},
    {"from": "gpt", "value": "The issue is in the RwLock usage..."}
  ],
  "model_id": "claude-opus-4-6",
  "system": "You are a helpful assistant...",
  "temperature": 0.7,
  "turn_metadata": {
    "session_id": "sess_xyz",
    "timestamp": "2026-04-13T10:30:00Z",
    "token_count": 2048,
    "tools_used": ["memory.recall", "bash.run"]
  }
}
```

### 4.4 Trust Scoring

Feedback adjusts memory trust scores:

```rust
fn adjust_trust_score(current: f32, feedback: TrustFeedback) -> f32 {
    match feedback {
        TrustFeedback::Helpful => (current + 0.05).min(1.0),
        TrustFeedback::Unhelpful => (current - 0.10).max(-1.0),
        TrustFeedback::Neutral => current,
    }
}
```

### 4.5 What We DO NOT Implement

#### Tinker-Atropos RL Training

**hermes-agent's approach**: Integrates with Tinker-Atropos for RL-based agent training using trajectory data.

**if2Ai's decision**: NOT ADOPTED

**Rationale**:
1. **Infrastructure mismatch**: RL training requires GPU compute, long-running training jobs, and model deployment pipelines. These are orthogonal to if2Ai's desktop application architecture.
2. **Use case difference**: if2Ai is a human-in-the-loop assistant; users interact with it directly. RL training is for training model weights at scale.
3. **Future-proofing**: Trajectory data can be exported to feed a future RL pipeline when if2Ai has such infrastructure.

#### 8-Source Skills Hub

**hermes-agent's approach**: Multi-source skill discovery from 8 different skill repositories.

**if2Ai's decision**: NOT ADOPTED

**Rationale**:
1. **Security incident**: 2026-02 ClawHub incident (341 malicious skills discovered) demonstrated significant risks in external skill marketplaces.
2. **Single-user model**: if2Ai is a desktop single-user application. Skills are installed locally via ToolRegistry, not discovered from external marketplaces.
3. **Trust model mismatch**: hermes-agent's community/trusted/builtin trust model is designed for multi-tenant server-side agents, not desktop applications.

#### Full RL Training Loop

**hermes-agent's approach**: Provides rl_training_tool.py that orchestrates trajectory collection, compression, and Tinker-Atropos RL training.

**if2Ai's decision**: NOT ADOPTED

**Rationale**:
1. **Scope boundary**: RL training is a separate ML platform concern. if2Ai produces trajectory data but does not consume it for training.
2. **Infrastructure requirements**: Training requires experiment tracking (WandB), GPU cluster, model registry, and deployment pipeline — completely outside if2Ai scope.
3. **Architecture separation**: Trajectory learning (P3) is designed to be an **optional export path** for future RL systems, not a coupled training loop.

---

## 5. Implementation Phases

### Phase 0 (P0) — Foundation

**Goal**: Replace InMemory HashMap with SQLite persistence.

**Files modified**:
- `src-tauri/src/modules/memory/mod.rs`

**Deliverables**:
- [x] SQLite table creation
- [x] CRUD operations (store/recall/delete/export)
- [x] Category-based filtering
- [x] JSON file-based session persistence (already done)

**Acceptance criteria**:
```
✓ memory.store() persists data across process restarts
✓ memory.recall() returns matching entries
✓ memory.delete() removes entries
✓ SessionManager survives process restarts
```

### Phase 1 (P1) — Vector Search

**Goal**: Add FastEmbed + LanceDB for semantic memory retrieval.

**Files to create/modify**:
- `src-tauri/src/modules/memory/vector_provider.rs` (new)
- `src-tauri/src/modules/memory/providers/lancedb.rs` (new)
- `src-tauri/src/modules/memory/providers/sqlite_fts.rs` (new)

**Deliverables**:
- [ ] FastEmbed offline embedding (multilingual-e5-small, 384d)
- [ ] LanceDB table with FTS support
- [ ] Hybrid retrieval with RRF fusion
- [ ] Intent-based retrieval weights

**Acceptance criteria**:
```
✓ Embedding generation works offline
✓ LanceDB stores and retrieves vectors
✓ RRF fusion combines FTS5 + vector + graph results
✓ Intent routing selects appropriate weights
```

### Phase 2a (P2a) — HRR Algebraic Reasoning

**Goal**: Add HRR as a complement to LanceDB for multi-hop reasoning.

**Files to create**:
- `src-tauri/src/modules/memory/hrr/` (new directory)
- `src-tauri/src/modules/memory/hrr/mod.rs`
- `src-tauri/src/modules/memory/hrr/operations.rs`
- `src-tauri/src/modules/memory/hrr/store.rs`

**Deliverables**:
- [ ] Phase encoding (circular convolution)
- [ ] bind/unbind operations
- [ ] bundle operation (superposition)
- [ ] similarity computation (cosine)
- [ ] SQLite fact store with trust scoring

**Architecture decision**: HRR complements LanceDB — handles algebraic reasoning while LanceDB handles FTS + vector retrieval.

**Acceptance criteria**:
```
✓ HRR capacity: O(√dim) = ~700 items for 384d
✓ bind/unbind correctly retrieves wrapped values
✓ bundle combines multiple values
✓contradict detects opposition (cosine ≈ -1)
```

### Phase 2b (P2b) — Self-Model and Reflection

**Goal**: Implement self-model and reflection engine.

**Files to create**:
- `src-tauri/src/modules/learning/self_model.rs` (new)
- `src-tauri/src/modules/learning/reflection.rs` (new)
- `src-tauri/src/modules/learning/trust_tracker.rs` (new)

**Deliverables**:
- [ ] SelfModel with capabilities/limitations/patterns
- [ ] ReflectionEngine trait
- [ ] Trust scoring with feedback adjustment
- [ ] Session analysis for pattern extraction

**Architecture decision**: Self-learning modules are independent of memory storage — clean separation.

**Acceptance criteria**:
```
✓ SelfModel updates based on session history
✓ ReflectionEngine identifies patterns
✓ Trust scores adjust based on user feedback
```

### Phase 3 (P3) — Trajectory Learning

**Goal**: Implement trajectory persistence for future RL training data.

**Files to create**:
- `src-tauri/src/modules/learning/trajectory.rs` (new)
- `src-tauri/src/modules/learning/compressor.rs` (new)

**Deliverables**:
- [ ] ShareGPT JSONL format
- [ ] TrajectoryManager for persistence
- [ ] Session-to-trajectory conversion
- [ ] Compressor for training data optimization

**Architecture decision**: Trajectory is an **optional export path** — if2Ai produces data but does not run RL training.

**Acceptance criteria**:
```
✓ Sessions serialize to ShareGPT JSONL
✓ TrajectoryManager persists trajectory files
✓ Compression reduces trajectory size
✓ Export endpoint for future RL pipeline
```

---

## 6. ADR Index

| ADR | Title | Status | Phase |
|-----|-------|--------|-------|
| ADR-001 | SQLite P0 Persistence | Accepted | P0 |
| ADR-002 | Active Retrieval vs Passive Invocation | Accepted | P0/P1 |
| ADR-003 | FastEmbed + LanceDB Selection | Accepted | P1 |
| ADR-004 | Token Budget Allocation | Accepted | P0 |
| ADR-005 | Relationship with Upstream claw-cli | Accepted | All |
| ADR-006 | Security Design | Accepted | P0 |
| ADR-007 | HRR Introduction Timing (P2a) | Accepted | P2a |
| ADR-008 | Self-Learning Modules Independence | Accepted | P2b |
| ADR-009 | Trajectory Learning Timing (P3) | Accepted | P3 |
| ADR-010 | Skills Hub Not Introduced | Accepted | N/A |

---

## 7. Differences from hermes-agent

### 7.1 Memory System

| Aspect | hermes-agent | if2Ai |
|--------|-------------|-------|
| P0 persistence | JSON file + SQLite | SQLite (replacing HashMap) |
| Vector search | LanceDB only | LanceDB + FTS5 hybrid |
| HRR integration | Parallel with vector | Complement to vector (P2a) |
| Memory providers | ABC with 11 hooks | Simple trait (6 methods) |
| Threat scanning | Full path scanning | File path validation only |
| Frozen snapshot | On every write | On memory load only |

### 7.2 Learning Framework

| Aspect | hermes-agent | if2Ai |
|--------|-------------|-------|
| RL training | Tinker-Atropos integration | NOT ADOPTED |
| Skills Hub | 8-source marketplace | NOT ADOPTED |
| Self-model | Integrated with memory | Independent module |
| Trajectory | Full RL loop | Export-only (P3) |

### 7.3 Key Architectural Differences

1. **MemoryProvider complexity**: hermes-agent's ABC has 11 lifecycle hooks; if2Ai's trait has 6 methods. Simpler for desktop use case.

2. **HRR positioning**: hermes-agent uses HRR as primary vector store; if2Ai uses HRR as complement to LanceDB for algebraic reasoning.

3. **RL training**: hermes-agent integrates with Tinker-Atropos; if2Ai produces trajectory data for future external pipeline.

4. **Skills Hub**: hermes-agent supports multi-source skill discovery; if2Ai relies on local ToolRegistry.

---

## 8. Security Considerations

### 8.1 Threat Model

| Threat | Mitigation |
|--------|------------|
| Memory injection | Input sanitization + length limits |
| Path traversal | Validated paths + sandboxed directories |
| Malicious skills | ToolRegistry signing (future) |
| Data exfiltration | User-controlled export only |

### 8.2 Atomic Writes

All session and memory writes use atomic operations:

```rust
async fn atomic_write(path: &Path, data: &Session) -> Result<(), SessionError> {
    let temp_path = path.with_extension("tmp");
    let contents = serde_json::to_string_pretty(data)?;
    let mut file = fs::File::create(&temp_path).await?;
    file.write_all(contents.as_bytes()).await?;
    file.sync_all().await?;
    fs::rename(&temp_path, path).await?;
    Ok(())
}
```

### 8.3 Frozen Snapshot Integrity

System prompt snapshots are verified on load:

```rust
fn verify_frozen_snapshot(snapshot: &FrozenSnapshot, current_prompt: &str) -> bool {
    snapshot.prompt_hash == compute_hash(current_prompt)
}
```

---

## 9. Migration Guide

### P0 → P1 Migration

1. Add `fastembed` and `lancedb` to `Cargo.toml`
2. Create `vector_provider.rs` implementing `MemoryProvider`
3. Update `MemoryProvider::recall()` to use RRF fusion
4. Add intent classification to `SessionManager`

### P1 → P2a Migration

1. Add `hrr` module with phase encoding
2. Implement `bind/unbind/bundle` operations
3. Create HRR store with trust scoring
4. Add HRR query endpoint for multi-hop reasoning

### P2a → P2b Migration

1. Add `learning` module with `self_model.rs`
2. Implement `ReflectionEngine` trait
3. Update session analysis to extract patterns
4. Connect trust scores to memory importance

### P2b → P3 Migration

1. Add `trajectory.rs` with ShareGPT format
2. Implement `TrajectoryManager`
3. Add session-to-trajectory conversion
4. Create export endpoint for external RL pipeline

---

## Appendix A: Current if2Ai Memory Implementation

### Critical Bug: In-Memory HashMap

```rust
// CURRENT (BUG): Data lost on process restart
pub struct InMemoryMemoryProvider {
    entries: RwLock<HashMap<String, MemoryEntry>>,  // ← 进程内存
}

// P0 FIX: SQLite-backed persistence
pub struct SqliteMemoryProvider {
    conn: SqlitePool,
}
```

### SessionManager (Already Persistent)

The `SessionManager` already uses JSON file persistence with dual-path support:
- Legacy: `~/.if2ai/sessions/<id>.json`
- Project-scoped: `~/.if2ai/projects/<project_id>/sessions/<id>.json`

---

## Appendix B: UClaw Context Budget Reference

```
Total context: 4000 tokens (configurable)

System:     10% = 400 tokens   → Frozen snapshot on load
Episodic:   20% = 800 tokens   → LLM rolling summary
Semantic:   30% = 1200 tokens  → Vector + FTS retrieval
Working:    40% = 1600 tokens  → Sliding window (8 turns)
```

---

## Appendix C: hermes-agent Components NOT Adopted

### C.1 Tinker-Atropos RL Training

```python
# hermes-agent/tools/rl_training_tool.py
class RLTrainingTool(BaseTool):
    def __init__(self, atropos_config: AtroposConfig):
        self.atropos = AtroposClient(atropos_config)

    async def execute(self, session_id: str, **kwargs):
        # Collect trajectories → Compress → Send to Tinker-Atropos → Get trained model
```

**if2Ai decision**: NOT ADOPTED. Requires GPU infrastructure, separate ML platform.

### C.2 8-Source Skills Hub

```python
# hermes-agent/tools/skills_hub.py
class SkillsHub:
    SOURCES = [
        "claw_plugins", "claw_hub", "pypi", "github",
        "custom1", "custom2", "builtin", "enterprise"
    ]
```

**if2Ai decision**: NOT ADOPTED. Security risk + single-user desktop model.

### C.3 Full RL Training Loop

```python
# hermes-agent/agent/trajectory.py
class TrajectoryManager:
    async def collect(self, session: Session) -> Trajectory:
        # Convert session to ShareGPT format

    async def compress(self, trajectories: List[Trajectory]) -> CompressedTrajectory:
        # Deduplicate + filter low-quality trajectories

    def export_for_rl(self, compressed: CompressedTrajectory) -> bytes:
        # Export to Tinker-Atropos format
```

**if2Ai decision**: NOT ADOPTED. if2Ai exports trajectory data only; actual RL training is external.

---

*Document version 2.0 — Ratified 2026-04-13*
