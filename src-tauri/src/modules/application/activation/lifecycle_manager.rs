//! License lifecycle background loop.
//!
//! Runs while the desktop host is alive:
//!
//! - Every [`REVOKE_CHECK_INTERVAL`] (default 1 min) call
//!   [`LicenseLifecycleService::revoke_check`].  Server-side revoke /
//!   expiry / refresh-rotation is reflected in the returned snapshot
//!   (which also rewrites `~/.if2ai/activation/license.json` when
//!   needed).
//! - When the snapshot's `kind` or `allows_main_shell` differs from
//!   the previous tick, emit a Tauri event [`ACTIVATION_STATUS_EVENT`]
//!   with the full [`ActivationSnapshot`] payload so the frontend
//!   projection can refresh and the activation gate can pop.
//!
//! Failures (network down, etc.) are logged and skipped; the next
//! tick retries.

use std::time::Duration;

use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Runtime};

use crate::modules::application::license_lifecycle_service::LicenseLifecycleService;
use crate::modules::runtime::contracts::activation::{ActivationSnapshot, ActivationStatusKind};

/// Tauri event name carrying [`ActivationSnapshot`] when the
/// background revoke / refresh tick observes a state change.
pub const ACTIVATION_STATUS_EVENT: &str = "activation_status_changed";

/// How often the background loop polls the activation server for
/// revoke / refresh.  1 minute keeps remote revoke detection within
/// a minute while staying well below any sane rate limit.
pub const REVOKE_CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// Spawn the activation lifecycle loop.  Returns the [`JoinHandle`] so
/// the caller can `abort()` it on shutdown if desired (the desktop
/// host currently lets it ride along for the process lifetime).
pub fn spawn_activation_lifecycle<R: Runtime>(app: AppHandle<R>) -> JoinHandle<()> {
    spawn_activation_lifecycle_with_interval(app, REVOKE_CHECK_INTERVAL)
}

/// Test-friendly variant.
pub fn spawn_activation_lifecycle_with_interval<R: Runtime>(
    app: AppHandle<R>,
    interval: Duration,
) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let service = LicenseLifecycleService::new();
        let mut last_signature: Option<(ActivationStatusKind, bool)> = None;

        loop {
            tokio::time::sleep(interval).await;

            match service.revoke_check().await {
                Ok(snapshot) => {
                    let sig = (snapshot.status.kind, snapshot.allows_main_shell);
                    let changed = last_signature != Some(sig);
                    last_signature = Some(sig);
                    // Emit on any change AND whenever the gate is
                    // currently blocked, so a freshly-revoked snapshot
                    // observed on the very first tick (when we have no
                    // baseline) still wakes the frontend up.
                    if changed || !snapshot.allows_main_shell {
                        emit_snapshot(&app, &snapshot);
                    }
                }
                Err(err) => {
                    tracing::info!(
                        target: "if2ai::activation",
                        "lifecycle revoke-check deferred: {err}"
                    );
                }
            }
        }
    })
}

fn emit_snapshot<R: Runtime>(app: &AppHandle<R>, snapshot: &ActivationSnapshot) {
    if let Err(err) = app.emit(ACTIVATION_STATUS_EVENT, snapshot) {
        tracing::warn!(
            target: "if2ai::activation",
            "failed to emit {ACTIVATION_STATUS_EVENT}: {err}"
        );
    } else {
        tracing::info!(
            target: "if2ai::activation",
            kind = ?snapshot.status.kind,
            allows_main_shell = snapshot.allows_main_shell,
            "lifecycle: emitted {ACTIVATION_STATUS_EVENT}"
        );
    }
}
