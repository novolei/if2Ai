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

// Public surface for downstream consumers (Tauri commands, bootstrap).
// `#[allow(unused_imports)]` because not every re-export has an internal
// consumer in the lib half — they exist for the bin half + tests.
#[allow(unused_imports)]
pub use collector_trajectory_source::CollectorTrajectorySource;
#[allow(unused_imports)]
pub use config::{ConsolidationStrategy, DayDreamConfig};
#[allow(unused_imports)]
pub use report::{DayDreamReport, StepError, StepOutcome};

pub mod engine;
#[allow(unused_imports)]
pub use engine::{CycleError, DayDreamEngine, TrajectorySource};

pub mod coordinator;
#[allow(unused_imports)]
pub use coordinator::{spawn_poll_loop, DayDreamCoordinator};

pub mod trajectory_source;
#[allow(unused_imports)]
pub use trajectory_source::EmptyTrajectorySource;
