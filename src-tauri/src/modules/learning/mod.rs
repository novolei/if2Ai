//! Learning module — self-model, reflection, and trust tracking
//!
//! Independent from memory storage. Learning reasons about memory data
//! but doesn't replace or modify it directly.
//!
//! # `#![allow(dead_code)]` justification
//! This module provides autonomous learning capabilities for the agent.
//! It will be wired into the agent loop harness to enable self-improvement.

#![allow(dead_code)]

pub mod active_overlay;
pub mod candidate_evaluator;
pub mod failure_clustering;
pub mod failure_taxonomy;
pub mod promotion_gate;
pub mod reflection;
pub mod reflection_generator;
pub mod reflection_note;
pub mod self_model;
pub mod strategy_registry;
pub mod strategy_registry_service;
pub mod strategy_registry_store;
pub mod strategy_rollout;
pub mod trajectory;
pub mod trajectory_score;
pub mod trust_tracker;

#[allow(unused_imports)]
pub use active_overlay::{
    project_effect, ActiveStrategyEffect, ActiveStrategyOverlay, ActiveStrategyOverlayResolver,
    ACTIVE_STRATEGY_OVERLAY_VERSION,
};
#[allow(unused_imports)]
pub use candidate_evaluator::{
    CandidateEvaluationOutcome, CandidateEvaluator, CandidateEvaluatorError,
    CandidateSuiteEvaluationOutcome, EvaluationProgress, EvaluationStep,
};
#[allow(unused_imports)]
pub use failure_clustering::{
    cluster_failures, ClusteredFailureSet, FailureCluster, FailureSignature,
    FAILURE_CLUSTERING_VERSION,
};
#[allow(unused_imports)]
pub use failure_taxonomy::{classify_blocking_failure, FailureCategory, FAILURE_TAXONOMY_VERSION};
#[allow(unused_imports)]
pub use promotion_gate::{
    check_eligibility, check_eligibility_with_basis, PromotionBasis, PromotionDecision,
    PromotionEligibility, PromotionGateError, PromotionGateOutcome, PromotionGateService,
    PROMOTION_GATE_VERSION,
};
#[allow(unused_imports)]
pub use reflection::{Reflection, ReflectionEngine};
#[allow(unused_imports)]
pub use reflection_generator::{
    generate_reflection_notes, ReflectionGeneration, REFLECTION_GENERATOR_VERSION,
};
#[allow(unused_imports)]
pub use reflection_note::{
    ReflectionEvidenceRef, ReflectionIssueType, ReflectionNote, StrategyProposal,
    REFLECTION_NOTE_VERSION,
};
#[allow(unused_imports)]
pub use self_model::{Capability, LearnedPattern, PerformanceMetrics, SelfModel};
#[allow(unused_imports)]
pub use strategy_registry::{
    source_from_reflection, ActivationAudit, CandidateStrategy, CompareRef, CompareTarget,
    RecommendationRef, RollbackAudit, RollbackTarget, RolloutState, StrategyDefinition,
    StrategyIdentity, StrategySource, SuiteEvaluationRef, SupersedeRecord,
    STRATEGY_REGISTRY_VERSION,
};
#[allow(unused_imports)]
pub use strategy_registry_service::{
    RegisterFromReflectionOpts, RegisterManualOpts, StrategyRegistryError, StrategyRegistryService,
};
#[allow(unused_imports)]
pub use strategy_registry_store::{
    RegistryPersistenceError, StrategyIndexEntry, StrategyRegistryStore,
    STRATEGY_REGISTRY_PERSISTENCE_VERSION,
};
#[allow(unused_imports)]
pub use strategy_rollout::{
    ActivateInput, RollbackInput, RolloutOutcome, StrategyRolloutError, StrategyRolloutService,
    STRATEGY_ROLLOUT_VERSION,
};
#[allow(unused_imports)]
pub use trajectory::{Trajectory, TrajectoryCompressor, TrajectoryManager, TrajectoryPrivacy};
#[allow(unused_imports)]
pub use trajectory_score::{
    score_compare, score_run_report, AxisBreakdown, CompareScore, TrajectoryAxis, TrajectoryScore,
    TRAJECTORY_SCORE_VERSION,
};
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
