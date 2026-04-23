//! Recoverable pending permission records.
//!
//! A permission prompt is still resolved through the live in-process
//! channel, but the prompt itself is persisted here so a refreshed
//! frontend can rehydrate the dialog from runtime state instead of
//! treating `session.json` as another fact source.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Runtime-persisted permission request currently awaiting a user decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PendingPermissionRecord {
    /// Stable id for this permission prompt.
    pub request_id: String,
    /// Session whose active run is blocked on the permission decision.
    pub session_id: String,
    /// Tool requesting elevated permission.
    pub tool_name: String,
    /// Required permission mode for the tool.
    pub permission_mode: String,
    /// Current permission mode when the request was raised.
    pub current_mode: String,
    /// User-facing prompt message.
    pub message: String,
    /// RFC3339 timestamp for audit and stale-prompt diagnosis.
    pub requested_at: String,
}

/// Persist a pending permission record for later frontend recovery.
pub fn write_pending_permission(
    app_data_dir: &Path,
    record: &PendingPermissionRecord,
) -> io::Result<()> {
    let path = pending_permission_path(app_data_dir, &record.session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(record).map_err(io::Error::other)?;
    fs::write(path, bytes)
}

/// Read the pending permission record for a session, if one exists.
pub fn read_pending_permission(
    app_data_dir: &Path,
    session_id: &str,
) -> io::Result<Option<PendingPermissionRecord>> {
    let path = pending_permission_path(app_data_dir, session_id);
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Clear a resolved pending permission record.
pub fn clear_pending_permission(app_data_dir: &Path, session_id: &str) -> io::Result<()> {
    let path = pending_permission_path(app_data_dir, session_id);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn pending_permission_path(app_data_dir: &Path, session_id: &str) -> PathBuf {
    app_data_dir
        .join("runtime")
        .join("pending-permissions")
        .join(format!("{}.json", safe_session_filename(session_id)))
}

fn safe_session_filename(session_id: &str) -> String {
    session_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(session_id: &str) -> PendingPermissionRecord {
        PendingPermissionRecord {
            request_id: "request-1".to_string(),
            session_id: session_id.to_string(),
            tool_name: "bash".to_string(),
            permission_mode: "dangerFullAccess".to_string(),
            current_mode: "readOnly".to_string(),
            message: "Tool 'bash' requires dangerFullAccess permission".to_string(),
            requested_at: "2026-04-23T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn pending_permission_write_read_clear_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let record = sample_record("session-42");

        write_pending_permission(dir.path(), &record).unwrap();
        assert_eq!(
            read_pending_permission(dir.path(), "session-42").unwrap(),
            Some(record)
        );

        clear_pending_permission(dir.path(), "session-42").unwrap();
        assert_eq!(
            read_pending_permission(dir.path(), "session-42").unwrap(),
            None
        );
    }

    #[test]
    fn pending_permission_session_id_is_sanitized_for_path() {
        let dir = tempfile::tempdir().unwrap();
        let record = sample_record("../session/42");

        write_pending_permission(dir.path(), &record).unwrap();

        let expected_path = dir
            .path()
            .join("runtime")
            .join("pending-permissions")
            .join("___session_42.json");
        assert!(expected_path.exists());
        assert_eq!(
            read_pending_permission(dir.path(), "../session/42").unwrap(),
            Some(record)
        );
    }
}
