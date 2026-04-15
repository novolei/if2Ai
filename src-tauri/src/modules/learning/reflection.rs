//! Reflection Engine — analyzes sessions and updates self-model
//!
//! The reflection engine observes agent interactions and extracts
//! patterns about tool usage, success/failure, and topic clusters.
//! These patterns feed into the self-model for continuous improvement.
//!
//! # `#![allow(dead_code)]` justification
//! ReflectionEngine is the core learning mechanism. It will be invoked
//! at turn boundaries in the agent loop to enable self-improvement.

#![allow(dead_code)]

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;

use crate::modules::memory::SharedMemoryProvider;
use crate::modules::runtime::session::{ContentBlock, Session};

use super::self_model::{LearnedPattern, SelfModel};
use super::LearningResult;

/// A single reflection insight from session analysis
#[derive(Debug, Clone)]
pub struct Reflection {
    /// Pattern that was observed (e.g., "tool_X often followed by tool_Y")
    pub pattern: String,
    /// Insight derived from the pattern
    pub insight: String,
    /// Confidence in this reflection (0.0-1.0)
    pub confidence: f32,
    /// Source session ID
    pub source_session: String,
    /// When this reflection was generated
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Trait for reflection engines
#[async_trait]
pub trait ReflectionEngine: Send + Sync {
    /// Analyze a session and generate reflections
    async fn analyze_session(&self, session: &Session) -> LearningResult<Vec<Reflection>>;

    /// Update self-model based on reflections
    async fn update_self_model(&self, reflections: Vec<Reflection>) -> LearningResult<()>;

    /// Get current self-model
    async fn get_self_model(&self) -> LearningResult<SelfModel>;
}

/// Standard reflection engine implementation
///
/// Analyzes tool sequences, outcomes, and topics from sessions.
/// Reads from memory provider for context but doesn't modify it.
pub struct StandardReflectionEngine {
    self_model: RwLock<SelfModel>,
    memory: SharedMemoryProvider,
}

impl StandardReflectionEngine {
    /// Create a new reflection engine with the given memory provider
    pub fn new(memory: SharedMemoryProvider) -> Self {
        Self {
            self_model: RwLock::new(SelfModel::default()),
            memory,
        }
    }

    /// Analyze tool sequence patterns in a session
    ///
    /// Identifies frequently occurring tool pairs (A -> B) and
    /// generates reflections for patterns appearing 3+ times.
    async fn analyze_tool_sequences(&self, session: &Session) -> LearningResult<Vec<Reflection>> {
        let mut sequences: HashMap<String, u32> = HashMap::new();

        // Count tool pairs from tool use/result blocks
        let tool_names: Vec<String> = session
            .messages
            .iter()
            .flat_map(|msg| &msg.blocks)
            .filter_map(|block| match block {
                ContentBlock::ToolUse { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();

        for window in tool_names.windows(2) {
            let pair = format!("{} -> {}", window[0], window[1]);
            *sequences.entry(pair).or_insert(0) += 1;
        }

        // Generate reflections for frequent patterns
        let reflections = sequences
            .into_iter()
            .filter(|(_, count)| *count >= 3)
            .map(|(pattern, count)| Reflection {
                pattern: format!("Tool sequence: {pattern}"),
                insight: format!("Observed {count} times"),
                confidence: (count as f32 / 10.0).min(1.0),
                source_session: String::new(), // Session ID from context
                timestamp: Utc::now(),
            })
            .collect();

        Ok(reflections)
    }

    /// Analyze success/failure patterns in a session
    async fn analyze_outcomes(&self, session: &Session) -> LearningResult<Vec<Reflection>> {
        let mut error_tools: HashMap<String, u32> = HashMap::new();
        let mut total_tools: u32 = 0;

        for block in session.messages.iter().flat_map(|m| &m.blocks) {
            if let ContentBlock::ToolResult {
                tool_name,
                is_error,
                ..
            } = block
            {
                total_tools += 1;
                if *is_error {
                    *error_tools.entry(tool_name.clone()).or_insert(0) += 1;
                }
            }
        }

        let reflections = error_tools
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(tool, errors)| Reflection {
                pattern: format!("Tool errors: {tool}"),
                insight: format!("{errors} errors in this session"),
                confidence: (errors as f32 / total_tools.max(1) as f32).min(1.0),
                source_session: String::new(),
                timestamp: Utc::now(),
            })
            .collect();

        Ok(reflections)
    }

    /// Analyze topic patterns in a session
    async fn analyze_topics(&self, session: &Session) -> LearningResult<Vec<Reflection>> {
        // Simple topic detection: look for file extensions mentioned in tool use
        let mut file_types: HashMap<String, u32> = HashMap::new();

        for block in session.messages.iter().flat_map(|m| &m.blocks) {
            if let ContentBlock::ToolUse { input, .. } = block {
                // Look for file extensions in tool input
                for word in input.split_whitespace() {
                    if let Some(dot_pos) = word.rfind('.') {
                        let ext = &word[dot_pos..];
                        if ext.len() <= 6 {
                            *file_types.entry(ext.to_string()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        let reflections = file_types
            .into_iter()
            .filter(|(_, count)| *count >= 3)
            .map(|(ext, count)| Reflection {
                pattern: format!("File type: {ext}"),
                insight: format!("Mentioned {count} times"),
                confidence: (count as f32 / 10.0).min(1.0),
                source_session: String::new(),
                timestamp: Utc::now(),
            })
            .collect();

        Ok(reflections)
    }
}

#[async_trait]
impl ReflectionEngine for StandardReflectionEngine {
    async fn analyze_session(&self, session: &Session) -> LearningResult<Vec<Reflection>> {
        let mut reflections = Vec::new();

        reflections.extend(self.analyze_tool_sequences(session).await?);
        reflections.extend(self.analyze_outcomes(session).await?);
        reflections.extend(self.analyze_topics(session).await?);

        Ok(reflections)
    }

    async fn update_self_model(&self, reflections: Vec<Reflection>) -> LearningResult<()> {
        let mut model = self.self_model.write().await;

        for reflection in reflections {
            if let Some(existing) = model
                .learned_patterns
                .iter_mut()
                .find(|p| p.trigger == reflection.pattern)
            {
                // Blend with existing pattern
                let weight = reflection.confidence / (existing.sample_count as f32 + 1.0);
                existing.success_rate =
                    existing.success_rate * (1.0 - weight) + reflection.confidence * weight;
                existing.sample_count += 1;
                existing.last_applied = Utc::now();
            } else {
                model.learned_patterns.push(LearnedPattern {
                    id: uuid::Uuid::new_v4().to_string(),
                    trigger: reflection.pattern,
                    action: reflection.insight,
                    success_rate: reflection.confidence,
                    sample_count: 1,
                    last_applied: Utc::now(),
                });
            }
        }

        model.updated_at = Utc::now();
        Ok(())
    }

    async fn get_self_model(&self) -> LearningResult<SelfModel> {
        Ok(self.self_model.read().await.clone())
    }
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)]

    use std::sync::Arc;

    use super::*;
    use crate::modules::memory::InMemoryMemoryProvider;
    use crate::modules::runtime::session::ConversationMessage;

    fn make_session_with_tools(tool_sequence: &[(&str, bool)]) -> Session {
        let mut session = Session::new();
        for (name, is_error) in tool_sequence {
            let mut msg = ConversationMessage {
                role: crate::modules::runtime::session::MessageRole::Assistant,
                blocks: Vec::new(),
                usage: None,
                thinking: None,
                task_outcome: None,
                degraded_reason: None,
                resume_available: None,
                resume_cursor: None,
                request_id: None,
            };
            msg.blocks.push(ContentBlock::ToolUse {
                id: format!("tool-{name}"),
                name: name.to_string(),
                input: String::new(),
            });
            msg.blocks.push(ContentBlock::ToolResult {
                tool_use_id: format!("tool-{name}"),
                tool_name: name.to_string(),
                output: String::new(),
                is_error: *is_error,
            });
            session.messages.push(msg);
        }
        session
    }

    #[allow(deprecated)]
    fn make_engine() -> StandardReflectionEngine {
        StandardReflectionEngine::new(Arc::new(InMemoryMemoryProvider::new()))
    }

    #[tokio::test]
    async fn analyze_empty_session_returns_no_reflections() {
        let engine = make_engine();
        let session = Session::new();
        let reflections = engine.analyze_session(&session).await.unwrap();
        assert!(reflections.is_empty());
    }

    #[tokio::test]
    async fn tool_sequence_detection_requires_minimum_count() {
        let engine = make_engine();
        // Only 2 occurrences of "read -> bash" — below threshold of 3
        let session = make_session_with_tools(&[
            ("read_file", false),
            ("bash", false),
            ("read_file", false),
            ("bash", false),
        ]);
        let reflections = engine.analyze_session(&session).await.unwrap();
        // No reflections should be generated (need 3+)
        let seq_reflections: Vec<_> = reflections
            .iter()
            .filter(|r| r.pattern.starts_with("Tool sequence:"))
            .collect();
        assert!(
            seq_reflections.is_empty(),
            "expected no sequence reflections, got {:?}",
            seq_reflections
        );
    }

    #[tokio::test]
    async fn tool_sequence_detection_fires_with_enough_occurrences() {
        let engine = make_engine();
        // 4 occurrences of "read_file -> bash" — above threshold
        let session = make_session_with_tools(&[
            ("read_file", false),
            ("bash", false),
            ("read_file", false),
            ("bash", false),
            ("read_file", false),
            ("bash", false),
            ("read_file", false),
            ("bash", false),
        ]);
        let reflections = engine.analyze_session(&session).await.unwrap();
        let seq_reflections: Vec<_> = reflections
            .iter()
            .filter(|r| r.pattern.starts_with("Tool sequence:"))
            .collect();
        // Both "read_file -> bash" (4x) and "bash -> read_file" (3x) exceed threshold
        assert_eq!(seq_reflections.len(), 2);
        assert!(seq_reflections.iter().any(|r| r.pattern.contains("read_file") && r.pattern.contains("bash")));
    }

    #[tokio::test]
    async fn error_detection_fires_for_repeated_errors() {
        let engine = make_engine();
        let session = make_session_with_tools(&[("bash", true), ("bash", true), ("bash", false)]);
        let reflections = engine.analyze_session(&session).await.unwrap();
        let error_reflections: Vec<_> = reflections
            .iter()
            .filter(|r| r.pattern.starts_with("Tool errors:"))
            .collect();
        assert_eq!(error_reflections.len(), 1);
        assert!(error_reflections[0].pattern.contains("bash"));
    }

    #[tokio::test]
    async fn update_self_model_adds_patterns() {
        let engine = make_engine();
        let reflections = vec![Reflection {
            pattern: "test pattern".to_string(),
            insight: "test insight".to_string(),
            confidence: 0.8,
            source_session: "session-1".to_string(),
            timestamp: Utc::now(),
        }];

        engine.update_self_model(reflections).await.unwrap();
        let model = engine.get_self_model().await.unwrap();
        assert_eq!(model.learned_patterns.len(), 1);
        assert_eq!(model.learned_patterns[0].trigger, "test pattern");
    }

    #[tokio::test]
    async fn update_self_model_blends_existing_patterns() {
        let engine = make_engine();
        // First update
        engine
            .update_self_model(vec![Reflection {
                pattern: "same pattern".to_string(),
                insight: "first".to_string(),
                confidence: 0.5,
                source_session: String::new(),
                timestamp: Utc::now(),
            }])
            .await
            .unwrap();

        // Second update with same pattern
        engine
            .update_self_model(vec![Reflection {
                pattern: "same pattern".to_string(),
                insight: "second".to_string(),
                confidence: 0.8,
                source_session: String::new(),
                timestamp: Utc::now(),
            }])
            .await
            .unwrap();

        let model = engine.get_self_model().await.unwrap();
        assert_eq!(model.learned_patterns.len(), 1);
        assert_eq!(model.learned_patterns[0].sample_count, 2);
    }
}
