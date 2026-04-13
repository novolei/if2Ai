# ADR-008: Self-Learning Modules Independence

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: P2b

---

## Context

Autonomous learning capabilities (self-model, reflection, trust scoring, trajectory learning) are related but distinct from memory storage.

hermes-agent integrates learning into the memory provider system. if2Ai separates them.

**hermes-agent's approach**:
```python
# hermes-agent/agent/memory_manager.py
class MemoryManager:
    def __init__(self, memory_provider: MemoryProvider, trajectory: TrajectoryManager):
        self.memory = memory_provider
        self.trajectory = trajectory
        self.self_model = SelfModel()

    async def on_turn_end(self, turn):
        # Learning happens during memory sync
        await self.trajectory.record(turn)
        await self.self_model.update(turn)
```

---

## Decision

Implement self-learning modules as **independent from memory storage**:

```
┌─────────────────────────────────────────────────────────────────┐
│                     if2Ai Module Architecture                    │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐ │
│  │     Memory      │  │     Learning     │  │     Runtime     │ │
│  │   (Storage)     │  │   (Reasoning)    │  │   (Execution)   │ │
│  ├─────────────────┤  ├─────────────────┤  ├────────────────┤ │
│  │ SqliteMemory    │  │ SelfModel        │  │ ProviderMgr    │ │
│  │ LanceDBMemory   │  │ ReflectionEngine │  │ ToolRegistry   │ │
│  │ HRRStore        │  │ TrustTracker     │  │ SessionMgr     │ │
│  │                 │  │ TrajectoryManager │  │                │ │
│  └────────┬────────┘  └────────┬────────┘  └───────┬────────┘ │
│           │                     │                    │          │
│           └─────────────────────┼────────────────────┘          │
│                                 ▼                               │
│                    ┌─────────────────────┐                     │
│                    │   Cross-Module API   │                     │
│                    │   Memory → Learning  │                     │
│                    │   Learning → Runtime │                     │
│                    └─────────────────────┘                     │
└─────────────────────────────────────────────────────────────────┘
```

### Module Structure

```rust
// src-tauri/src/modules/learning/mod.rs

pub mod self_model;
pub mod reflection;
pub mod trust_tracker;
pub mod trajectory;

pub use self_model::{SelfModel, Capability, LearnedPattern, PerformanceMetrics};
pub use reflection::{ReflectionEngine, Reflection};
pub use trust_tracker::{TrustTracker, TrustFeedback};
pub use trajectory::{TrajectoryManager, Trajectory, TurnMetadata};
```

### SelfModel (Independent of Memory)

```rust
// src-tauri/src/modules/learning/self_model.rs

/// Agent's understanding of its own capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModel {
    /// What the agent can do
    pub capabilities: Vec<Capability>,
    /// What the agent cannot do
    pub limitations: Vec<Limitation>,
    /// Patterns learned from experience
    pub learned_patterns: Vec<LearnedPattern>,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Last updated
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub description: String,
    pub confidence: f32,  // 0.0-1.0
    pub last_used: DateTime<Utc>,
    pub use_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub id: String,
    pub trigger: String,       // Context pattern
    pub action: String,       // What was done
    pub success_rate: f32,
    pub sample_count: u32,
    pub last_applied: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_turns: u64,
    pub successful_turns: u64,
    pub failed_turns: u64,
    pub average_response_time_ms: f64,
    pub tool_usage_stats: HashMap<String, u32>,
}

impl Default for SelfModel {
    fn default() -> Self {
        Self {
            capabilities: Vec::new(),
            limitations: Vec::new(),
            learned_patterns: Vec::new(),
            performance: PerformanceMetrics::default(),
            updated_at: chrono::Utc::now(),
        }
    }
}
```

### ReflectionEngine (Reads Memory, Updates SelfModel)

```rust
// src-tauri/src/modules/learning/reflection.rs

/// Reflection engine analyzes sessions and updates self-model
#[async_trait]
pub trait ReflectionEngine: Send + Sync {
    /// Analyze a session and generate reflections
    async fn analyze_session(&self, session: &Session) -> Result<Vec<Reflection>>;

    /// Update self-model based on reflections
    async fn update_self_model(&self, reflections: Vec<Reflection>) -> Result<()>;

    /// Get current self-model
    async fn get_self_model(&self) -> Result<SelfModel>;
}

pub struct Reflection {
    pub pattern: String,      // "tool_X often followed by tool_Y"
    pub insight: String,       // "Users prefer quick tools first"
    pub confidence: f32,       // 0.0-1.0
    pub source_session: String,
    pub timestamp: DateTime<Utc>,
}

/// Standard reflection implementation
pub struct StandardReflectionEngine {
    self_model: RwLock<SelfModel>,
    memory: Arc<dyn MemoryProvider>,
}

impl StandardReflectionEngine {
    pub fn new(memory: Arc<dyn MemoryProvider>) -> Self {
        Self {
            self_model: RwLock::new(SelfModel::default()),
            memory,
        }
    }
}

#[async_trait]
impl ReflectionEngine for StandardReflectionEngine {
    async fn analyze_session(&self, session: &Session) -> Result<Vec<Reflection>> {
        let mut reflections = Vec::new();

        // Pattern 1: Tool sequences
        reflections.extend(self.analyze_tool_sequences(session).await?);

        // Pattern 2: Success/failure patterns
        reflections.extend(self.analyze_outcomes(session).await?);

        // Pattern 3: Topic clusters
        reflections.extend(self.analyze_topics(session).await?);

        Ok(reflections)
    }

    async fn update_self_model(&self, reflections: Vec<Reflection>) -> Result<()> {
        let mut model = self.self_model.write().await;

        for reflection in reflections {
            // Update learned patterns
            if let Some(existing) = model.learned_patterns.iter_mut().find(|p| p.trigger == reflection.pattern) {
                // Blend with existing
                let weight = reflection.confidence / (existing.sample_count as f32 + 1.0);
                existing.success_rate = existing.success_rate * (1.0 - weight) + reflection.confidence * weight;
                existing.sample_count += 1;
            } else {
                model.learned_patterns.push(LearnedPattern {
                    id: uuid::Uuid::new_v4().to_string(),
                    trigger: reflection.pattern,
                    action: reflection.insight,
                    success_rate: reflection.confidence,
                    sample_count: 1,
                    last_applied: chrono::Utc::now(),
                });
            }
        }

        model.updated_at = chrono::Utc::now();
        Ok(())
    }

    async fn get_self_model(&self) -> Result<SelfModel> {
        Ok(self.self_model.read().await.clone())
    }
}
```

### TrustTracker (Independent of SelfModel)

```rust
// src-tauri/src/modules/learning/trust_tracker.rs

/// Trust scoring for memory entries
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TrustFeedback {
    Helpful,
    Unhelpful,
    Neutral,
}

pub struct TrustTracker {
    scores: RwLock<HashMap<String, f32>>,
}

impl TrustTracker {
    pub fn new() -> Self {
        Self {
            scores: RwLock::new(HashMap::new()),
        }
    }

    pub async fn adjust(&self, key: &str, feedback: TrustFeedback) {
        let delta = match feedback {
            TrustFeedback::Helpful => 0.05,
            TrustFeedback::Unhelpful => -0.10,
            TrustFeedback::Neutral => 0.0,
        };

        let mut scores = self.scores.write().await;
        let entry = scores.entry(key.to_string()).or_insert(0.0);
        *entry = (*entry + delta).clamp(-1.0, 1.0);
    }

    pub async fn get(&self, key: &str) -> f32 {
        self.scores.read().await.get(key).copied().unwrap_or(0.0)
    }
}

impl Default for TrustTracker {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## Rationale

### Why Separate from Memory?

1. **Single responsibility**: Memory stores data; learning reasons about it
2. **Independent evolution**: Memory can upgrade (SQLite → LanceDB) without touching learning
3. **Testing**: Easier to unit test learning without database complexity
4. **Performance**: Learning runs async, doesn't block memory operations
5. **Different persistence**: Learning data (patterns, metrics) has different schema than memory entries

### Why NOT Follow hermes-agent?

hermes-agent's integrated approach has issues:

1. **Tight coupling**: Memory provider changes break learning
2. **Lifecycle hooks**: 11 hooks create complex dependencies
3. **Testing difficulty**: Can't test learning without full memory provider
4. **Evolution problems**: Adding learning features requires memory refactoring

### Interface Design

```rust
/// Clean interface between memory and learning
pub trait MemoryToLearning {
    async fn get_recent_entries(&self, limit: usize) -> Result<Vec<MemoryEntry>>;
    async fn get_by_category(&self, category: &str) -> Result<Vec<MemoryEntry>>;
}

pub trait LearningToRuntime {
    async fn get_suggested_tools(&self, context: &str) -> Result<Vec<String>>;
    async fn get_retry_strategy(&self, failure: &Failure) -> Result<RetryStrategy>;
}
```

---

## Consequences

### Positive
- Clean separation of concerns
- Independent module evolution
- Easier testing and mocking
- Different persistence strategies per module
- Clear interfaces

### Negative
- More modules to maintain
- Cross-module communication overhead
- Need to coordinate schema changes

### Neutral
- hermes-agent integration requires adapter
- Trajectory learning still optional (P3)

---

## Implementation Notes

### Module Initialization

```rust
// src-tauri/src/modules/learning/mod.rs

pub async fn init_learning(memory: Arc<dyn MemoryProvider>) -> Result<LearningModule> {
    let trust_tracker = Arc::new(TrustTracker::new());
    let reflection_engine = Arc::new(StandardReflectionEngine::new(memory.clone()));
    let trajectory_manager = Arc::new(TrajectoryManager::new()?);

    Ok(LearningModule {
        self_model: RwLock::new(SelfModel::default()),
        trust_tracker,
        reflection_engine,
        trajectory_manager,
    })
}

pub struct LearningModule {
    pub self_model: RwLock<SelfModel>,
    pub trust_tracker: Arc<TrustTracker>,
    pub reflection_engine: Arc<dyn ReflectionEngine>,
    pub trajectory_manager: Arc<TrajectoryManager>,
}
```

### Memory Integration

```rust
// Learning reads from memory, doesn't replace it
impl StandardReflectionEngine {
    async fn analyze_tool_sequences(&self, session: &Session) -> Result<Vec<Reflection>> {
        let mut sequences: HashMap<String, u32> = HashMap::new();

        // Count tool pairs
        for window in session.messages.windows(2) {
            if let (Some(a), Some(b)) = (window.first(), window.get(1)) {
                let pair = format!("{} → {}", a.tool_name, b.tool_name);
                *sequences.entry(pair).or_insert(0) += 1;
            }
        }

        // Generate reflections for frequent patterns
        sequences
            .into_iter()
            .filter(|(_, count)| count >= 3)
            .map(|(pattern, count)| Reflection {
                pattern: format!("Tool sequence: {}", pattern),
                insight: format!("Observed {} times", count),
                confidence: (count as f32 / 10.0).min(1.0),
                source_session: session.id.clone(),
                timestamp: chrono::Utc::now(),
            })
            .collect()
    }
}
```

---

## Review Checklist

- [ ] SelfModel defined with capabilities/patterns/metrics
- [ ] ReflectionEngine trait with analyze_session/update_self_model
- [ ] StandardReflectionEngine implements trait
- [ ] TrustTracker maintains per-entry trust scores
- [ ] Learning module is separate from memory module
- [ ] MemoryToLearning interface defined
- [ ] Cross-module calls use interfaces (not concrete types)
- [ ] Trust scores don't affect memory storage directly
- [ ] Reflection updates don't modify memory entries

---

## References

- [hermes-agent SelfModel](https://github.com/1tius/hermes-agent/blob/main/agent/self_model.py)
- [hermes-agent Reflection](https://github.com/1tius/hermes-agent/blob/main/agent/reflection.py)
- [hermes-agent Trust Scoring](https://github.com/1tius/hermes-agent/blob/main/plugins/memory/holographic/store.py)
