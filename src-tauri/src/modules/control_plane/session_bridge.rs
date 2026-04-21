//! Session bridge helpers: convert AppSession ↔ RuntimeSession and
//! log context fingerprints for debugging.
//!
//! Extracted from `commands/agent.rs` in GFR-005e (pure structural
//! move; function bodies byte-identical).

use crate::modules::control_plane::session_context::SessionExecutionContext;
use crate::modules::runtime::session::Session as RuntimeSession;
use crate::modules::session::Session as AppSession;

/// Convert application session to runtime session.
///
/// The application session has extra metadata (id, title, etc.) that we don't need
/// for the runtime. We only need the messages.
pub(crate) fn app_session_to_runtime(app_session: &AppSession) -> RuntimeSession {
    RuntimeSession {
        version: 1,
        messages: app_session.messages.clone(),
    }
}

pub(crate) fn log_context_fingerprint(caller: &str, context: &SessionExecutionContext) {
    let fingerprint =
        crate::modules::tools::context::context_fingerprint(&context.session_id, &context.workdir);
    tracing::info!(
        "[{}] context fingerprint='{}', session_id='{}', workdir='{}', permission_mode='{}'",
        caller,
        fingerprint,
        context.session_id,
        context.workdir.display(),
        context.permission_mode.as_str()
    );
}
