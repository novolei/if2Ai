//! Browser control subsystem.
//!
//! Provides AI-driven browser automation via chromiumoxide (Chrome DevTools
//! Protocol). The public surface is:
//!
//! - [`chrome_finder`] — binary discovery
//! - [`cold_state`] — URL persistence across app restarts
//! - [`errors`] — shared error type
//! - [`registry`] — multi-session registry
//! - [`session`] — single-session lifecycle and actions
//! - [`snapshot`] — DOM AXTree snapshot algorithm
//! - [`events`] — Tauri event payloads and emission helpers
//!
//! # Dead-code suppression
//!
//! The `snapshot` and `session` internals are consumed via the tool layer; the
//! `allow` below suppresses warnings for items not yet wired at the binary
//! level. Remove once all slices are complete.
// snapshot module items are consumed transitively via session; allow dead_code
// until all slices are wired.
#![allow(dead_code, unused_imports)]

pub mod chrome_finder;
pub mod cold_state;
pub mod errors;
pub mod events;
pub mod registry;
pub mod session;
pub mod snapshot;

pub use chrome_finder::{find_chrome_binary, ChromeStatus};
pub use errors::BrowserError;
pub use events::{emit_browser_status, BrowserStatusEvent};
pub use registry::{BrowserRegistry, BrowserStatusEntry};
pub use session::{ActionLogEntry, BrowserSession, NavigateResult, ScrollDir};
