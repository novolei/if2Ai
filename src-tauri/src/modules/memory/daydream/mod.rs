//! DayDream — background memory consolidation cycle.
//!
//! See `docs/superpowers/specs/2026-05-08-a2-daydream-design.md`.

pub mod config;
pub mod merge;
pub mod report;
pub mod prune;
pub mod reflect;
pub mod refresh;

pub use config::{ConsolidationStrategy, DayDreamConfig};
pub use report::{DayDreamReport, StepError, StepOutcome};

pub mod engine;
pub use engine::{CycleError, DayDreamEngine, TrajectorySource};
