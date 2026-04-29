//! Smart Browser bounded context.
//!
//! This module provides the stable contract that lets If2Ai route browser
//! work through different execution backends while preserving one product
//! surface, one policy layer, and one projection path.

pub mod agentic;
pub mod browser_use_mcp;
pub mod cloud;
pub mod contract;
pub mod local_adapter;
pub mod content_simplifier;
pub mod coordinate_strategy;
pub mod policy;
pub mod runtime;
pub mod session_health;
