//! Session Module
//!
//! Provides session management with JSON file persistence.

pub mod manager;
pub mod session_undo;

#[allow(unused_imports)]
pub use manager::{Session, SessionError, SessionManager, SessionMeta};
#[allow(unused_imports)]
pub use session_undo::{ConversationUndoStatus, SessionUndoRegistry};
