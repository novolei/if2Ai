//! Browser control subsystem.
//!
//! Provides AI-driven browser automation via chromiumoxide (Chrome DevTools
//! Protocol). The public surface is:
//!
//! - [`chrome_finder`] — binary discovery
//! - [`errors`] — shared error type
//!
//! Additional modules (session, registry, snapshot, events, cold_state) are
//! added in subsequent slices (7B.2 – 7B.7).
//!
//! # Dead-code suppression
//!
//! The items in this module are wired into the command and tool layers in
//! slices 7B.2–7B.5. Until then they appear unused to `rustc` when the binary
//! is compiled; the `dead_code` allow avoids spurious warnings during the
//! incremental build.
// Items are consumed in slices 7B.2–7B.5; suppress unused warnings until wired.
#![allow(dead_code, unused_imports)]

pub mod chrome_finder;
pub mod errors;
pub mod registry;
pub mod session;
pub mod snapshot;

pub use chrome_finder::{find_chrome_binary, ChromeStatus};
pub use errors::BrowserError;
pub use registry::{BrowserRegistry, BrowserStatusEntry};
pub use session::{ActionLogEntry, BrowserSession, NavigateResult, ScrollDir};
