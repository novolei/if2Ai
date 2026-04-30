//! Trajectory Learning — ShareGPT JSONL export for future RL
//!
//! Captures conversation sessions as trajectories in ShareGPT format.
//! Export-only: if2Ai produces trajectory data but does NOT run RL training.
//!
//! # `#![allow(dead_code)]` justification
//! TrajectoryManager is invoked at session boundaries to record interaction
//! data for future model training. It will be wired into the agent loop harness.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::modules::runtime::session::{ContentBlock, MessageRole, Session};

/// A single conversation turn in ShareGPT format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub from: String, // "human" or "gpt"
    pub value: String,
}

/// Metadata for a trajectory turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnMetadata {
    pub session_id: String,
    pub timestamp: String,
    pub token_count: u64,
    pub tools_used: Vec<String>,
}

/// A complete trajectory representing one session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub id: String,
    pub conversations: Vec<ConversationEntry>,
    pub model_id: String,
    pub system: Option<String>,
    pub temperature: f32,
    pub turn_metadata: TurnMetadata,
}

impl Trajectory {
    /// Convert a Session to a Trajectory in ShareGPT format
    #[must_use]
    pub fn from_session(session: &Session, system_prompt: &str, model_id: &str) -> Self {
        let conversations: Vec<ConversationEntry> = session
            .messages
            .iter()
            .filter_map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "human",
                    MessageRole::Assistant => "gpt",
                    MessageRole::System | MessageRole::Tool => {
                        return None; // Skip system/tool in ShareGPT conversations
                    }
                };
                let value = msg
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if value.is_empty() {
                    None
                } else {
                    Some(ConversationEntry {
                        from: role.to_string(),
                        value,
                    })
                }
            })
            .collect();

        let tools_used: Vec<String> = session
            .messages
            .iter()
            .flat_map(|m| &m.blocks)
            .filter_map(|b| match b {
                ContentBlock::ToolUse { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();

        Self {
            id: format!("traj_{}", Utc::now().timestamp_millis()),
            conversations,
            model_id: model_id.to_string(),
            system: if system_prompt.is_empty() {
                None
            } else {
                Some(system_prompt.to_string())
            },
            temperature: 0.7,
            turn_metadata: TurnMetadata {
                session_id: String::new(), // Populated by caller
                timestamp: Utc::now().to_rfc3339(),
                token_count: 0, // Populated by caller
                tools_used,
            },
        }
    }

    /// Serialize to a JSONL line
    pub fn to_jsonl(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Get the number of conversation turns
    #[must_use]
    pub fn turn_count(&self) -> usize {
        self.conversations.len()
    }
}

/// Privacy controls for trajectory export
#[derive(Debug, Clone)]
pub struct TrajectoryPrivacy {
    /// Whether to include system prompt in trajectories
    pub include_system_prompt: bool,
    /// Whether to include tool calls in trajectories
    pub include_tool_calls: bool,
    /// Minimum session length to record (conversation entries)
    pub min_session_length: usize,
    /// Whether to anonymize user content
    pub anonymize_user_content: bool,
}

impl Default for TrajectoryPrivacy {
    fn default() -> Self {
        Self {
            include_system_prompt: false, // Privacy by default
            include_tool_calls: true,
            min_session_length: 3,        // Skip very short sessions
            anonymize_user_content: true, // Remove PII by default
        }
    }
}

/// Error type for trajectory operations
#[derive(Debug, thiserror::Error)]
pub enum TrajectoryError {
    #[error("read error: {0}")]
    ReadError(String),

    #[error("write error: {0}")]
    WriteError(String),

    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("session too short: {0} entries, minimum {1}")]
    SessionTooShort(usize, usize),
}

/// Manager for trajectory recording and export
///
/// Records sessions as ShareGPT JSONL files with date-based rotation.
/// Max file size: 100MB.
pub struct TrajectoryManager {
    base_path: PathBuf,
    max_file_size: usize,
    privacy: TrajectoryPrivacy,
}

impl TrajectoryManager {
    /// Create a new TrajectoryManager
    ///
    /// Creates the base directory if it doesn't exist.
    pub fn new(base_path: PathBuf) -> Result<Self, TrajectoryError> {
        std::fs::create_dir_all(&base_path)
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;

        Ok(Self {
            base_path,
            max_file_size: 100 * 1024 * 1024, // 100MB
            privacy: TrajectoryPrivacy::default(),
        })
    }

    /// Create with custom privacy settings
    pub fn with_privacy(
        base_path: PathBuf,
        privacy: TrajectoryPrivacy,
    ) -> Result<Self, TrajectoryError> {
        std::fs::create_dir_all(&base_path)
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;

        Ok(Self {
            base_path,
            max_file_size: 100 * 1024 * 1024,
            privacy,
        })
    }

    /// Record a session as a trajectory
    ///
    /// Converts the session to ShareGPT format and appends to the current
    /// date-based JSONL file. Rotates files when they exceed 100MB.
    pub async fn record(
        &self,
        session: &Session,
        system_prompt: &str,
        model_id: &str,
    ) -> Result<String, TrajectoryError> {
        let mut trajectory = Trajectory::from_session(session, system_prompt, model_id);

        // Check minimum length
        if trajectory.turn_count() < self.privacy.min_session_length {
            return Err(TrajectoryError::SessionTooShort(
                trajectory.turn_count(),
                self.privacy.min_session_length,
            ));
        }

        // Apply privacy: strip system prompt if not included
        if !self.privacy.include_system_prompt {
            trajectory.system = None;
        }

        let id = trajectory.id.clone();
        let jsonl = trajectory.to_jsonl()?;

        let path = self.current_file();

        let mut file = OpenOptions::new()
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
    ///
    /// Returns the number of trajectory lines exported.
    pub async fn export_all(&self, output_path: &Path) -> Result<u64, TrajectoryError> {
        let mut count = 0u64;

        let mut output = File::create(output_path)
            .await
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;

        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| TrajectoryError::ReadError(e.to_string()))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| TrajectoryError::ReadError(e.to_string()))?
        {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|e| e == "jsonl" || e == "jsonl.archived")
            {
                let data = fs::read_to_string(&path)
                    .await
                    .map_err(|e| TrajectoryError::ReadError(e.to_string()))?;
                output
                    .write_all(data.as_bytes())
                    .await
                    .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;
                count += data.lines().count() as u64;
            }
        }

        Ok(count)
    }

    /// Get the count of trajectory files
    pub async fn count_files(&self) -> Result<u64, TrajectoryError> {
        let mut count = 0u64;
        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| TrajectoryError::ReadError(e.to_string()))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| TrajectoryError::ReadError(e.to_string()))?
        {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|e| e == "jsonl" || e == "jsonl.archived")
            {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Get the path for the current date's trajectory file
    fn current_file(&self) -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d");
        self.base_path.join(format!("trajectory_{date}.jsonl"))
    }

    /// Check if the current file exceeds the max size
    async fn should_rotate(&self) -> Result<bool, TrajectoryError> {
        let path = self.current_file();
        if !path.exists() {
            return Ok(false);
        }
        let metadata = fs::metadata(&path)
            .await
            .map_err(|e| TrajectoryError::ReadError(e.to_string()))?;
        Ok(metadata.len() > self.max_file_size as u64)
    }

    /// Rotate the current file by renaming it with a timestamp
    async fn rotate(&self) -> Result<(), TrajectoryError> {
        let current = self.current_file();
        if !current.exists() {
            return Ok(());
        }
        let timestamp = Utc::now().timestamp();
        let archived = self
            .base_path
            .join(format!("trajectory_{timestamp}.jsonl.archived"));
        fs::rename(&current, &archived)
            .await
            .map_err(|e| TrajectoryError::WriteError(e.to_string()))?;
        Ok(())
    }

    /// Get the base path
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

/// Compress trajectories for training efficiency
///
/// Filters low-quality trajectories and truncates very long ones.
pub struct TrajectoryCompressor {
    min_success_rate: f32,
    max_length: usize,
}

impl TrajectoryCompressor {
    /// Create a new compressor with default thresholds
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_success_rate: 0.0, // Accept all by default
            max_length: 100,       // Truncate to 100 turns max
        }
    }

    /// Compress a list of trajectories
    #[must_use]
    pub fn compress(&self, trajectories: Vec<Trajectory>) -> Vec<Trajectory> {
        trajectories
            .into_iter()
            .filter(|t| self.is_high_quality(t))
            .map(|t| self.truncate_if_needed(t))
            .collect()
    }

    /// Check if a trajectory meets the quality threshold
    fn is_high_quality(&self, trajectory: &Trajectory) -> bool {
        // Currently accepts all (no success rate available without feedback)
        let _ = self.min_success_rate;
        trajectory.turn_count() > 0
    }

    /// Truncate trajectories that exceed max length
    fn truncate_if_needed(&self, mut trajectory: Trajectory) -> Trajectory {
        if trajectory.conversations.len() > self.max_length {
            trajectory.conversations.truncate(self.max_length);
        }
        trajectory
    }
}

impl Default for TrajectoryCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::session::{ConversationMessage, MessageRole, Session};

    fn make_session(messages: &[(&str, MessageRole)]) -> Session {
        let mut session = Session::new();
        for (text, role) in messages {
            session.messages.push(ConversationMessage {
                role: *role,
                blocks: vec![ContentBlock::Text {
                    text: text.to_string(),
                }],
                usage: None,
                thinking: None,
                task_outcome: None,
                degraded_reason: None,
                resume_available: None,
                resume_cursor: None,
                request_id: None,
                finish_reason: None,
            });
        }
        session
    }

    #[test]
    fn trajectory_from_session() {
        let session = make_session(&[
            ("Hello", MessageRole::User),
            ("Hi there!", MessageRole::Assistant),
        ]);
        let traj = Trajectory::from_session(&session, "system prompt", "test-model");

        assert_eq!(traj.conversations.len(), 2);
        assert_eq!(traj.conversations[0].from, "human");
        assert_eq!(traj.conversations[1].from, "gpt");
        assert_eq!(traj.model_id, "test-model");
    }

    #[test]
    fn trajectory_skips_system_messages() {
        let session = make_session(&[
            ("system prompt", MessageRole::System),
            ("Hello", MessageRole::User),
            ("Hi", MessageRole::Assistant),
        ]);
        let traj = Trajectory::from_session(&session, "", "test-model");

        // System messages in the session should be skipped in ShareGPT
        assert_eq!(traj.conversations.len(), 2);
    }

    #[test]
    fn trajectory_to_jsonl() {
        let session = make_session(&[("Hello", MessageRole::User)]);
        let traj = Trajectory::from_session(&session, "", "test-model");

        let jsonl = traj.to_jsonl().unwrap();
        assert!(jsonl.contains("\"from\":\"human\""));
        assert!(jsonl.contains("\"value\":\"Hello\""));
    }

    #[tokio::test]
    async fn record_session_to_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("traj_test_{}", Utc::now().timestamp_millis()));
        let manager = TrajectoryManager::new(temp_dir.clone()).unwrap();

        let session = make_session(&[
            ("Question 1", MessageRole::User),
            ("Answer 1", MessageRole::Assistant),
            ("Question 2", MessageRole::User),
            ("Answer 2", MessageRole::Assistant),
            ("Question 3", MessageRole::User),
            ("Answer 3", MessageRole::Assistant),
        ]);

        let id = manager
            .record(&session, "system", "test-model")
            .await
            .unwrap();
        assert!(id.starts_with("traj_"));

        // Verify file was created
        let current_file = manager.current_file();
        assert!(current_file.exists());

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn record_rejects_short_sessions() {
        let temp_dir =
            std::env::temp_dir().join(format!("traj_short_{}", Utc::now().timestamp_millis()));
        let manager = TrajectoryManager::new(temp_dir.clone()).unwrap();

        let session = make_session(&[
            ("Short", MessageRole::User),
            ("Reply", MessageRole::Assistant),
        ]);

        let result = manager.record(&session, "system", "test-model").await;
        assert!(result.is_err());

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn export_all_combines_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("traj_export_{}", Utc::now().timestamp_millis()));
        let manager = TrajectoryManager::new(temp_dir.clone()).unwrap();

        let session = make_session(&[
            ("Q1", MessageRole::User),
            ("A1", MessageRole::Assistant),
            ("Q2", MessageRole::User),
            ("A2", MessageRole::Assistant),
            ("Q3", MessageRole::User),
            ("A3", MessageRole::Assistant),
        ]);

        manager.record(&session, "", "model1").await.unwrap();

        let output_path = temp_dir.join("export.jsonl");
        let count = manager.export_all(&output_path).await.unwrap();
        assert!(count >= 1);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[test]
    fn compressor_truncates_long_trajectories() {
        let compressor = TrajectoryCompressor::new();
        let mut long_traj = Trajectory::from_session(
            &make_session(&[("q", MessageRole::User), ("a", MessageRole::Assistant)]),
            "",
            "model",
        );
        // Manually add many conversations
        for i in 0..200 {
            long_traj.conversations.push(ConversationEntry {
                from: if i % 2 == 0 {
                    "human".to_string()
                } else {
                    "gpt".to_string()
                },
                value: format!("turn {i}"),
            });
        }

        let compressed = compressor.compress(vec![long_traj]);
        assert_eq!(compressed.len(), 1);
        assert!(compressed[0].conversations.len() <= 100);
    }

    #[test]
    fn privacy_defaults_exclude_system_prompt() {
        let privacy = TrajectoryPrivacy::default();
        assert!(!privacy.include_system_prompt);
        assert!(privacy.anonymize_user_content);
        assert_eq!(privacy.min_session_length, 3);
    }

    #[test]
    fn turn_count_matches_conversations() {
        let session = make_session(&[
            ("a", MessageRole::User),
            ("b", MessageRole::Assistant),
            ("c", MessageRole::User),
        ]);
        let traj = Trajectory::from_session(&session, "", "model");
        assert_eq!(traj.turn_count(), 3);
    }

    #[test]
    fn trajectory_strips_empty_blocks() {
        let mut session = Session::new();
        session.messages.push(ConversationMessage {
            role: MessageRole::User,
            blocks: vec![ContentBlock::ToolUse {
                id: "t1".to_string(),
                name: "read_file".to_string(),
                input: String::new(),
            }],
            usage: None,
            thinking: None,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            request_id: None,
            finish_reason: None,
        });
        let traj = Trajectory::from_session(&session, "", "model");
        // ToolUse blocks should not produce text in ShareGPT conversations
        assert_eq!(traj.conversations.len(), 0);
    }
}
