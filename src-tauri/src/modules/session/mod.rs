//! Session Module
//!
//! Provides session management with JSON file persistence.

pub mod manager;

#[allow(unused_imports)]
pub use manager::{Session, SessionError, SessionManager, SessionMeta};
