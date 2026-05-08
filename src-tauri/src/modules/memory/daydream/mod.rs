//! DayDream — background memory consolidation cycle.
//!
//! See `docs/superpowers/specs/2026-05-08-a2-daydream-design.md`.

pub mod collector_trajectory_source;
pub mod config;
pub mod merge;
pub mod prune;
pub mod reflect;
pub mod refresh;
pub mod report;

pub use collector_trajectory_source::CollectorTrajectorySource;
pub use config::{ConsolidationStrategy, DayDreamConfig};
pub use report::{DayDreamReport, StepError, StepOutcome};

pub mod engine;
pub use engine::{CycleError, DayDreamEngine, TrajectorySource};

pub mod coordinator;
pub use coordinator::{spawn_poll_loop, DayDreamCoordinator};

pub mod trajectory_source;
pub use trajectory_source::EmptyTrajectorySource;
