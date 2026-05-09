//! Evolution sub-module — trajectory collection and self-reflection.
//!
//! # Components
//!
//! - [`TrajectoryCollector`] — in-memory collector for agent execution traces.
//! - [`SelfReflector`] — rule-based engine that extracts [`Insight`]s from
//!   completed trajectories.
//!
//! The evolution module is the foundation of Phase 3 self-improvement:
//! it captures *what happened* (trajectories) and derives *what to learn*
//! (insights / patterns) without requiring LLM calls.

pub mod procedural;
pub mod reflector;
pub mod trajectory;

#[allow(unused_imports)]
pub use procedural::ProceduralMemoryManager;
#[allow(unused_imports)]
pub use reflector::{Insight, InsightCategory, Pattern, ReflectionReport, SelfReflector};
#[allow(unused_imports)]
pub use trajectory::{
    AgentAction, TaskOutcome, ToolCallRecord, Trajectory, TrajectoryCollector, TurnRecord,
};
