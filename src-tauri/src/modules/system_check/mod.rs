//! System check module.
//!
//! Provides hardware and runtime detection for the onboarding flow:
//! - CPU, GPU, and Node.js detection
//! - Embedded model download with progress tracking
//! - Full system check report

#![allow(dead_code)]

pub mod env;
pub mod model_download;
pub mod types;
