//! Commands Module
//! Migrated from /rust/crates/commands
//! Handles command processing

pub mod trajectory;

// Re-export the trajectory introspection commands (H6). Note: the legacy
// synchronous `export_trajectories` lives in `crate::commands::settings` and
// continues to own that name to avoid breaking the existing IPC surface.
#[allow(unused_imports)]
pub use trajectory::{get_trajectory_count, get_trajectory_path};
