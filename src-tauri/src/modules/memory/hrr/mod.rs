//! Holographic Reduced Representations (HRR)
//!
//! Vector Symbolic Architecture enabling algebraic memory operations:
//! bind, unbind, bundle, and similarity.
//!
//! Complements LanceDB for algebraic reasoning tasks.
//! Capacity is O(sqrt(dim)) — for 384d, ~700 items max.
//!
//! # Status (Memory Audit P0/P1)
//!
//! **Currently NOT wired into the active retrieval pipeline.**
//! The module compiles and is internally consistent (operations + store)
//! but no caller in `application::*` or `commands::*` depends on it.
//!
//! See [ADR-016 HRR Keep, Wire, or Remove](../../../../../docs/design-docs/postCLI/ADR/ADR-016-HRR-Keep-Or-Remove.md)
//! for the active keep / wire / remove decision (deadline 2026-05-31).
//! Until that ADR is resolved, the module stays dead-on-arrival to
//! avoid silent dependency creep.

#![allow(dead_code)]

pub mod integration;
mod operations;
mod store;

// Re-exports kept for the eventual integration entry-point and tests.
// `#[allow(unused_imports)]` is intentional until the HRR ADR is resolved.
#[allow(unused_imports)]
pub use operations::{bind, bundle, similarity, unbind, HRRVector};
#[allow(unused_imports)]
pub use store::HolographicStore;
