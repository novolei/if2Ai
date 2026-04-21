//! Stream-cancel service — application-layer body for the
//! `stop_agent_stream` IPC command.
//!
//! Owns the lookup-and-fire behaviour over the shared
//! `stream_cancel_senders` map so the IPC adapter
//! (`commands::agent::stop_agent_stream`) stays a thin wrapper
//! that only forwards arguments. Centralising this keeps every
//! "who is allowed to cancel a streaming turn" decision behind
//! one application-layer seam (CHARTER §2.1).

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::oneshot;

/// Application-layer body for the `stop_agent_stream` IPC
/// command.
///
/// Removes the cancel sender registered for `stream_id` and
/// fires the oneshot. The underlying receiver lives inside the
/// spawned streaming task created by
/// [`crate::modules::application::TurnService::stream_turn`].
///
/// # Errors
///
/// - The shared cancel-senders mutex was poisoned by a
///   concurrent panic.
/// - No streaming task is registered for the given `stream_id`
///   (already completed or never started).
pub(crate) fn cancel_stream(
    stream_cancel_senders: &Mutex<HashMap<String, oneshot::Sender<()>>>,
    stream_id: String,
) -> Result<(), String> {
    let mut senders = stream_cancel_senders
        .lock()
        .map_err(|e| format!("Failed to lock cancel senders: {e}"))?;

    let sender = senders
        .remove(&stream_id)
        .ok_or_else(|| format!("No active stream found for stream_id: {stream_id}"))?;

    // Sending the cancel signal — if the receiver has been
    // dropped (task already completed), this is a silent no-op.
    let _ = sender.send(());
    tracing::info!(
        "[stop_agent_stream] Cancel signal sent for stream_id: {}",
        stream_id
    );
    Ok(())
}
