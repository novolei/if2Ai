//! DayDream — background memory consolidation cycle.
//!
//! See `docs/superpowers/specs/2026-05-08-a2-daydream-design.md`.

pub mod config;
pub mod report;
pub mod prune;

pub use config::{ConsolidationStrategy, DayDreamConfig};
pub use report::{DayDreamReport, StepError, StepOutcome};
