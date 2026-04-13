# ADR-009: Trajectory Learning Timing (P3)

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: P3

---

## Context

Trajectory learning captures conversation sessions for future model training. hermes-agent integrates with Tinker-Atropos for full RL training.

**hermes-agent's trajectory system**:
```python
# hermes-agent/agent/trajectory.py
class TrajectoryManager:
    async def collect(self, session: Session) -> Trajectory:
        # Convert session to ShareGPT format

    async def compress(self, trajectories: List[Trajectory]) -> CompressedTrajectory:
        # Deduplicate + filter

    def export_for_rl(self, compressed: CompressedTrajectory) -> bytes:
        # Export to Tinker-Atropos format
```

---

## Decision

Implement trajectory learning in **P3 as an export-only system** — if2Ai produces trajectory data but does NOT run RL training.

### Key Difference from hermes-agent

| Aspect | hermes-agent | if2Ai |
|--------|-------------|-------|
| Trajectory collection | Yes | Yes |
| RL training | Tinker-Atropos integration | NOT ADOPTED |
| Trajectory export | To Tinker-Atropos | To JSONL file |
| Training loop | Full loop | Export only |

### Trajectory Format (ShareGPT JSONL)

```rust
// src-tauri/src/modules/learning/trajectory.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub id: String,
    pub conversations: Vec<ConversationEntry>,
    pub model_id: String,
    pub system: String,
    pub temperature: f32,
    pub turn_metadata: TurnMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub from: String,  // "human" or "gpt"
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnMetadata {
    pub session_id: String,
    pub timestamp: String,
    pub token_count: u64,
    pub tools_used: Vec<String>,
}

impl Trajectory {
    pub fn from_session(session: &Session, system_prompt: &str, model_id: &str) -> Self {
        let conversations: Vec<ConversationEntry> = session
            .messages
            .iter()
            .flat_map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "human",
                    MessageRole::Assistant => "gpt",
                    MessageRole::System => return vec![],  // Skip system in conversations
                };
                let value = msg.blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if value.is_empty() {
                    vec![]
                } else {
                    vec![ConversationEntry { from: role.to_string(), value }]
                }
            })
            .collect();

        let tools_used: Vec<String> = session
            .messages
            .iter()
            .filter_map(|m| m.tool_name.clone())
            .collect();

        Self {
            id: format!("traj_{}", session.id),
            conversations,
            model_id: model_id.to_string(),
            system: system_prompt.to_string(),
            temperature: 0.7,
            turn_metadata: TurnMetadata {
                session_id: session.id.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                token_count: session.token_count,
                tools_used,
            },
        }
    }

    pub fn to_jsonl(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}
```

### TrajectoryManager

```rust
pub struct TrajectoryManager {
    base_path: PathBuf,
    max_file_size: usize,  // Rotate files at this size
}

impl TrajectoryManager {
    pub fn new(base_path: PathBuf) -> Result<Self, TrajectoryError> {
        std::fs::create_dir_all(&base_path)?;
        Ok(Self {
            base_path,
            max_file_size: 100 * 1024 * 1024,  // 100MB
        })
    }

    /// Record a session as a trajectory
    pub async fn record(&self, session: &Session, system_prompt: &str, model_id: &str) -> Result<String> {
        let trajectory = Trajectory::from_session(session, system_prompt, model_id);
        let id = trajectory.id.clone();

        let path = self.current_file();
        let jsonl = trajectory.to_jsonl();

        // Append to current file
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;

        file.write_all(jsonl.as_bytes())
            .await
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;
        file.write_all(b"\n")
            .await
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;

        // Check rotation
        if self.should_rotate().await? {
            self.rotate().await?;
        }

        Ok(id)
    }

    /// Export all trajectories to a single file for external RL pipeline
    pub async fn export_all(&self, output_path: &Path) -> Result<u64> {
        let mut count = 0u64;

        let mut output = fs::File::create(output_path)
            .await
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;

        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| TrajectoryError::ReadError(e.to_string()))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| TrajectoryError::ReadError(e.to_string()))? {
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let data = fs::read_to_string(&path)
                    .await
                    .map_err(|e| TrajectoryError::ReadError(e.to_string()))?;
                output.write_all(data.as_bytes())
                    .await
                    .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;
                count += data.lines().count() as u64;
            }
        }

        Ok(count)
    }

    fn current_file(&self) -> PathBuf {
        let date = chrono::Utc::now().format("%Y-%m-%d");
        self.base_path.join(format!("trajectory_{}.jsonl", date))
    }

    async fn should_rotate(&self) -> Result<bool, TrajectoryError> {
        let path = self.current_file();
        if !path.exists() {
            return Ok(false);
        }
        let metadata = fs::metadata(&path).await
            .map_err(|e| TrajectoryError::ReadError(e.to_string()))?;
        Ok(metadata.len() > self.max_file_size)
    }

    async fn rotate(&self) -> Result<(), TrajectoryError> {
        let current = self.current_file();
        let timestamp = chrono::Utc::now().timestamp();
        let archived = self.base_path.join(format!("archive_{}.jsonl", timestamp));
        fs::rename(&current, &archived)
            .await
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TrajectoryError {
    #[error("read error: {0}")]
    ReadError(String),
    #[error("write error: {0}")]
    WriteError(String),
}
```

### Trajectory Compressor (Optional Enhancement)

```rust
/// Compress trajectories for training efficiency
pub struct TrajectoryCompressor {
    min_success_rate: f32,
    max_length: usize,
}

impl TrajectoryCompressor {
    pub fn compress(&self, trajectories: Vec<Trajectory>) -> Vec<Trajectory> {
        trajectories
            .into_iter()
            .filter(|t| self.is_high_quality(t))
            .map(|t| self.truncate_if_needed(t))
            .collect()
    }

    fn is_high_quality(&self, trajectory: &Trajectory) -> bool {
        // Filter out low-success trajectories
        let success_rate = trajectory.success_rate();
        success_rate >= self.min_success_rate
    }

    fn truncate_if_needed(&self, mut trajectory: Trajectory) -> Trajectory {
        // Truncate very long trajectories
        if trajectory.conversations.len() > self.max_length {
            trajectory.conversations.truncate(self.max_length);
        }
        trajectory
    }
}
```

---

## Rationale

### Why Export-Only?

1. **Scope boundary**: RL training is a separate ML platform concern
2. **Infrastructure mismatch**: Training requires GPU, experiment tracking, model registry
3. **Privacy**: User trajectory data stays local until explicitly exported
4. **Flexibility**: Users can choose their own RL pipeline

### Why NOT Tinker-Atropos?

Tinker-Atropos integration requires:
- GPU infrastructure
- Long-running training jobs
- Experiment tracking (WandB)
- Model deployment pipeline

These are orthogonal to if2Ai's desktop application architecture.

### Why P3 (Not P0 or P1)?

| Phase | Priority | Trajectory |
|-------|----------|------------|
| P0 | SQLite persistence | Out of scope |
| P1 | Vector search | Out of scope |
| P2a | HRR reasoning | Out of scope |
| P2b | Self-model | Learning data feeds trajectory |
| P3 | Trajectory | Now we have sessions to record |

---

## Consequences

### Positive
- Trajectory data available for future RL
- Export-only keeps architecture clean
- JSONL format is industry standard
- Rotation prevents unbounded growth

### Negative
- Users must build own RL pipeline
- Trajectory collection has storage cost

### Neutral
- hermes-agent integration requires adapter
- Full RL loop is external

---

## Implementation Notes

### Session Integration

```rust
// In SessionManager, after each turn:
pub async fn add_message(&self, session_id: &str, msg: ConversationMessage) -> Result<()> {
    // ... existing code ...

    // Record to trajectory if enabled
    if self.trajectory_manager.is_enabled() {
        let session = self.restore_session(session_id).await?;
        let system_prompt = self.get_frozen_snapshot();
        self.trajectory_manager.record(&session, &system_prompt, "claude-opus-4-6").await?;
    }

    Ok(())
}
```

### Export Command

```rust
/// Tauri command for trajectory export
#[tauri::command]
pub async fn export_trajectories(
    output_path: String,
) -> Result<u64, String> {
    let manager = TrajectoryManager::new(PathBuf::from(&output_path).parent().unwrap())
        .map_err(|e| e.to_string())?;

    manager.export_all(&output_path)
        .await
        .map_err(|e| e.to_string())
}
```

### Privacy Controls

```rust
pub struct TrajectoryPrivacy {
    pub include_system_prompt: bool,
    pub include_tool_calls: bool,
    pub min_session_length: usize,
    pub anonymize_user_content: bool,
}

impl Default for TrajectoryPrivacy {
    fn default() -> Self {
        Self {
            include_system_prompt: false,  // Privacy by default
            include_tool_calls: true,
            min_session_length: 3,        // Skip very short sessions
            anonymize_user_content: true,  // Remove PII
        }
    }
}
```

---

## Review Checklist

- [ ] Trajectory struct matches ShareGPT format
- [ ] `from_session()` converts Session to Trajectory
- [ ] `to_jsonl()` serializes correctly
- [ ] TrajectoryManager records to files
- [ ] File rotation at 100MB
- [ ] `export_all()` combines files
- [ ] Export command available via Tauri IPC
- [ ] Privacy controls (opt-in system prompt)
- [ ] RL training NOT integrated

---

## References

- [hermes-agent TrajectoryManager](https://github.com/1tius/hermes-agent/blob/main/agent/trajectory.py)
- [ShareGPT Format](https://sharegpt.com)
- [Tinker-Atropos](https://github.com/1tius/tinker-atropos) — external RL framework
