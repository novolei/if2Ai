//! Self Model — agent's understanding of its own capabilities
//!
//! Tracks what the agent can do, what it cannot do, and patterns
//! learned from experience. Updated by the reflection engine.
//!
//! # `#![allow(dead_code)]` justification
//! SelfModel is populated by reflection and consumed by the agent loop.
//! It will be wired into the agent loop harness for self-awareness.

#![allow(dead_code)]

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent's understanding of its own capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModel {
    /// What the agent can do
    pub capabilities: Vec<Capability>,
    /// What the agent cannot do well yet
    pub limitations: Vec<Limitation>,
    /// Patterns learned from experience
    pub learned_patterns: Vec<LearnedPattern>,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Last updated
    pub updated_at: DateTime<Utc>,
}

impl Default for SelfModel {
    fn default() -> Self {
        Self {
            capabilities: Vec::new(),
            limitations: Vec::new(),
            learned_patterns: Vec::new(),
            performance: PerformanceMetrics::default(),
            updated_at: Utc::now(),
        }
    }
}

impl SelfModel {
    /// Add or update a capability
    pub fn update_capability(&mut self, capability: Capability) {
        if let Some(existing) = self.capabilities.iter_mut().find(|c| c.id == capability.id) {
            *existing = capability;
        } else {
            self.capabilities.push(capability);
        }
        self.updated_at = Utc::now();
    }

    /// Record a limitation
    pub fn add_limitation(&mut self, limitation: Limitation) {
        self.limitations.push(limitation);
        self.updated_at = Utc::now();
    }

    /// Update performance metrics after a turn
    pub fn record_turn(&mut self, success: bool, response_time_ms: f64) {
        self.performance.total_turns += 1;
        if success {
            self.performance.successful_turns += 1;
        } else {
            self.performance.failed_turns += 1;
        }
        // Running average for response time
        let n = self.performance.total_turns as f64;
        self.performance.average_response_time_ms =
            self.performance.average_response_time_ms * ((n - 1.0) / n) + response_time_ms / n;
        self.updated_at = Utc::now();
    }

    /// Compute overall success rate
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.performance.total_turns == 0 {
            return 1.0;
        }
        self.performance.successful_turns as f64 / self.performance.total_turns as f64
    }
}

/// A specific capability the agent has
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Confidence in this capability (0.0-1.0)
    pub confidence: f32,
    /// When this capability was last used
    pub last_used: DateTime<Utc>,
    /// How many times this capability has been used
    pub use_count: u32,
}

impl Capability {
    /// Create a new capability
    #[must_use]
    pub fn new(id: String, name: String, description: String) -> Self {
        Self {
            id,
            name,
            description,
            confidence: 0.5,
            last_used: Utc::now(),
            use_count: 0,
        }
    }

    /// Record a use of this capability
    pub fn record_use(&mut self, success: bool) {
        self.use_count += 1;
        self.last_used = Utc::now();
        // Adjust confidence based on success
        let delta = if success { 0.02 } else { -0.05 };
        self.confidence = (self.confidence + delta).clamp(0.0, 1.0);
    }
}

/// A known limitation of the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limitation {
    pub id: String,
    pub description: String,
    /// When this limitation was identified
    pub identified_at: DateTime<Utc>,
    /// How severe the limitation is (0.0-1.0)
    pub severity: f32,
}

impl Limitation {
    #[must_use]
    pub fn new(id: String, description: String, severity: f32) -> Self {
        Self {
            id,
            description,
            identified_at: Utc::now(),
            severity: severity.clamp(0.0, 1.0),
        }
    }
}

/// A pattern learned from experience
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub id: String,
    /// Context pattern that triggers this pattern
    pub trigger: String,
    /// What action was taken
    pub action: String,
    /// Success rate of this pattern
    pub success_rate: f32,
    /// Number of samples
    pub sample_count: u32,
    /// When this pattern was last applied
    pub last_applied: DateTime<Utc>,
}

impl LearnedPattern {
    /// Update the pattern with a new observation
    pub fn update(&mut self, success: bool) {
        let weight = 1.0 / (self.sample_count as f32 + 1.0);
        let outcome = if success { 1.0 } else { 0.0 };
        self.success_rate = self.success_rate * (1.0 - weight) + outcome * weight;
        self.sample_count += 1;
        self.last_applied = Utc::now();
    }
}

/// Performance metrics for the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_turns: u64,
    pub successful_turns: u64,
    pub failed_turns: u64,
    pub average_response_time_ms: f64,
    pub tool_usage_stats: HashMap<String, u32>,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_turns: 0,
            successful_turns: 0,
            failed_turns: 0,
            average_response_time_ms: 0.0,
            tool_usage_stats: HashMap::new(),
        }
    }
}

impl PerformanceMetrics {
    /// Record a tool usage
    pub fn record_tool_use(&mut self, tool_name: &str) {
        *self
            .tool_usage_stats
            .entry(tool_name.to_string())
            .or_insert(0) += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_self_model_is_empty() {
        let model = SelfModel::default();
        assert!(model.capabilities.is_empty());
        assert!(model.limitations.is_empty());
        assert!(model.learned_patterns.is_empty());
        assert_eq!(model.performance.total_turns, 0);
    }

    #[test]
    fn capability_records_use() {
        let mut cap = Capability::new(
            "test".to_string(),
            "Test Cap".to_string(),
            "A test".to_string(),
        );
        assert_eq!(cap.use_count, 0);
        assert!((cap.confidence - 0.5).abs() < 0.001);

        cap.record_use(true);
        assert_eq!(cap.use_count, 1);
        assert!(cap.confidence > 0.5);

        cap.record_use(false);
        assert_eq!(cap.use_count, 2);
        // One success (+0.02) then one failure (-0.05) = 0.5 + 0.02 - 0.05 = 0.47
        assert!(cap.confidence < 0.52);
    }

    #[test]
    fn self_model_records_turns() {
        let mut model = SelfModel::default();
        model.record_turn(true, 100.0);
        model.record_turn(true, 200.0);
        model.record_turn(false, 150.0);

        assert_eq!(model.performance.total_turns, 3);
        assert_eq!(model.performance.successful_turns, 2);
        assert_eq!(model.performance.failed_turns, 1);
        assert!((model.success_rate() - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn success_rate_defaults_to_one() {
        let model = SelfModel::default();
        assert!((model.success_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn update_capability_adds_new() {
        let mut model = SelfModel::default();
        let cap = Capability::new("a".to_string(), "A".to_string(), "Test".to_string());
        model.update_capability(cap);
        assert_eq!(model.capabilities.len(), 1);
    }

    #[test]
    fn update_capability_replaces_existing() {
        let mut model = SelfModel::default();
        let cap1 = Capability::new("a".to_string(), "A".to_string(), "Test".to_string());
        model.update_capability(cap1);

        let cap2 = Capability::new("a".to_string(), "A".to_string(), "Updated".to_string());
        model.update_capability(cap2);

        assert_eq!(model.capabilities.len(), 1);
        assert_eq!(model.capabilities[0].description, "Updated");
    }

    #[test]
    fn learned_pattern_updates_success_rate() {
        let mut pattern = LearnedPattern {
            id: "p1".to_string(),
            trigger: "edit".to_string(),
            action: "search first".to_string(),
            success_rate: 0.5,
            sample_count: 1,
            last_applied: Utc::now(),
        };

        pattern.update(true);
        assert_eq!(pattern.sample_count, 2);
        // After success: 0.5 * (1 - 1/2) + 1.0 * (1/2) = 0.25 + 0.5 = 0.75
        assert!((pattern.success_rate - 0.75).abs() < 0.001);
    }

    #[test]
    fn limitation_clamps_severity() {
        let lim = Limitation::new("l1".to_string(), "Test limitation".to_string(), 1.5);
        assert!((lim.severity - 1.0).abs() < 0.001);

        let lim2 = Limitation::new("l2".to_string(), "Test limitation".to_string(), -0.5);
        assert!((lim2.severity - 0.0).abs() < 0.001);
    }

    #[test]
    fn performance_records_tool_usage() {
        let mut metrics = PerformanceMetrics::default();
        metrics.record_tool_use("read_file");
        metrics.record_tool_use("read_file");
        metrics.record_tool_use("bash");

        assert_eq!(metrics.tool_usage_stats["read_file"], 2);
        assert_eq!(metrics.tool_usage_stats["bash"], 1);
    }
}
