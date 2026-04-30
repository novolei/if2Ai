//! Unified agentic loop algorithm shared by both the streaming and sync agent paths.
//!
//! Relocated from `application::turn_service::*` in Phase 3 T1 to fix the layering
//! inversion (runtime → application). Both delegates can now `use crate::modules::
//! runtime::agent_loop::{LoopDelegate, run_agentic_loop, AgenticLoopConfig, ...}`
//! without depending on the application layer.

pub mod config;
pub mod loop_runner;

pub use config::AgenticLoopConfig;
pub use loop_runner::{
    run_agentic_loop, LoopContext, LoopDelegate, LoopOutcome, LoopSignal, RespondResult, TextAction,
};
