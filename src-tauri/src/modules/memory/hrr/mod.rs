//! Holographic Reduced Representations (HRR)
//!
//! Vector Symbolic Architecture enabling algebraic memory operations:
//! bind, unbind, bundle, and similarity.
//!
//! Complements LanceDB for algebraic reasoning tasks.
//! Capacity is O(sqrt(dim)) — for 384d, ~700 items max.
//!
//! # `#![allow(dead_code)]` justification
//! This module provides the HRR algebraic reasoning layer for the memory system.
//! The re-exported types and functions will be consumed by the active retrieval
//! pipeline and the agent loop harness.

#![allow(dead_code)]

pub mod integration;
mod operations;
mod store;

#[allow(unused_imports)]
pub use operations::{bind, bundle, similarity, unbind, HRRVector};
#[allow(unused_imports)]
pub use store::HolographicStore;
