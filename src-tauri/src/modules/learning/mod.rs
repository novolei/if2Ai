//! Learning module — self-model, reflection, and trust tracking
//!
//! Independent from memory storage. Learning reasons about memory data
//! but doesn't replace or modify it directly.
//!
//! # `#![allow(dead_code)]` justification
//! This module provides autonomous learning capabilities for the agent.
//! It will be wired into the agent loop harness to enable self-improvement.

#![allow(dead_code)]

pub mod reflection;
pub mod self_model;
pub mod trajectory;
pub mod trust_tracker;

#[allow(unused_imports)]
pub use reflection::{Reflection, ReflectionEngine};
#[allow(unused_imports)]
pub use self_model::{Capability, LearnedPattern, PerformanceMetrics, SelfModel};
#[allow(unused_imports)]
pub use trajectory::{Trajectory, TrajectoryCompressor, TrajectoryManager, TrajectoryPrivacy};
#[allow(unused_imports)]
pub use trust_tracker::{TrustFeedback, TrustTracker};

/// Result type for learning operations
pub type LearningResult<T> = Result<T, LearningError>;

/// Error type for learning operations
#[derive(Debug, thiserror::Error)]
pub enum LearningError {
    #[error("learning operation failed: {0}")]
    Generic(String),

    #[error("memory provider error: {0}")]
    MemoryProvider(String),

    #[error("model not initialized")]
    NotInitialized,
}

impl From<MemoryError> for LearningError {
    fn from(e: MemoryError) -> Self {
        LearningError::MemoryProvider(e.to_string())
    }
}

use crate::modules::memory::MemoryError;

/// Learning module container
///
/// Holds the self-model, reflection engine, and trust tracker as
/// independent but coordinated components.
pub struct LearningModule {
    pub self_model: SelfModel,
    pub trust_tracker: TrustTracker,
    pub reflection_engine: reflection::StandardReflectionEngine,
}

impl LearningModule {
    /// Initialize the learning module with a memory provider
    pub async fn new(memory: crate::modules::memory::SharedMemoryProvider) -> LearningResult<Self> {
        let trust_tracker = TrustTracker::new();
        let reflection_engine = reflection::StandardReflectionEngine::new(memory);

        Ok(Self {
            self_model: SelfModel::default(),
            trust_tracker,
            reflection_engine,
        })
    }

    /// Get a reference to the self-model
    pub fn self_model(&self) -> &SelfModel {
        &self.self_model
    }

    /// Get a mutable reference to the self-model
    pub fn self_model_mut(&mut self) -> &mut SelfModel {
        &mut self.self_model
    }
}
