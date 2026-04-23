//! Security module — layered defense for memory operations
//!
//! Provides:
//! - Input validation (key/content sanitization)
//! - Path validation (traversal prevention)
//! - Atomic writes (crash-safe file operations)
//! - Memory access control (category-based permissions)

// These types are infrastructure for future consumers; allow dead_code until used.
#![allow(dead_code)]

pub mod access;
pub mod atomic_write;
pub mod path;
pub mod redaction;
pub mod safety;
pub mod validation;

// Re-export for external consumers; currently unused but reserved for future use.
#[allow(unused_imports)]
pub use access::MemoryAccessContext;
#[allow(unused_imports)]
pub use atomic_write::{atomic_json_write, atomic_write};
#[allow(unused_imports)]
pub use path::validate_safe_path;
#[allow(unused_imports)]
pub use validation::validate_memory_entry;
