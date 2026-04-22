//! Desktop host composition for the Tauri shell.
//!
//! Keeps the native desktop host focused on launcher concerns:
//! plugin wiring, managed native state, tray/menu behavior, and
//! top-level window lifecycle. Business commands remain in
//! `crate::commands`, but `main.rs` no longer owns their
//! registration details directly.

pub mod builder;
pub mod setup;

pub use builder::attach_native_host;
pub use setup::setup_desktop_host;
