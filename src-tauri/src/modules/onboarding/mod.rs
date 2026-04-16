//! Onboarding module — 6-step state machine for first-time user setup.
//!
//! This module implements the onboarding platform defined in ADR-014:
//! a backend-driven state machine that guides users through
//! environment check, security confirmation, provider configuration,
//! channel configuration, and agent activation.
//!
//! ## Submodules
//! - `state`: Core types (`OnboardingStep`, `OnboardingState`, `AppOnboardingState`, `OnboardingFailure`)
//! - `store`: Persistence layer (`~/.if2ai/state.json` read/write)
//! - `flow`: State machine transitions (`next_step`, `prev_step`, `complete`)

// Allow dead_code: types defined here will be consumed by Tauri commands
// in slice 6g.7 (Onboarding Commands). See ADR-014 Section 5.
#![allow(dead_code)]

pub mod flow;
pub mod state;
pub mod store;

// Re-export key types for convenience
#[allow(unused_imports)]
pub use flow::OnboardingFlow;
#[allow(unused_imports)]
pub use state::{AppOnboardingState, OnboardingError, OnboardingState, OnboardingStep};
