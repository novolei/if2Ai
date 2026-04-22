//! Session commands - list_sessions, delete_session, create_session, list_project_sessions
//!
//! Provides session management commands for Tauri.

use tauri::State;

use crate::commands::AppState;
use crate::modules::identity::{
    apply_identity_customization_pack, read_identity_customization_pack, IdentityRegistry,
    SessionIdentityOverride,
};
use crate::modules::session::{Session, SessionMeta};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentityInput {
    pub soul_id: Option<String>,
    pub persona_id: Option<String>,
}

fn normalized_identity_override(
    identity: Option<SessionIdentityInput>,
) -> Result<Option<SessionIdentityOverride>, String> {
    let Some(identity) = identity else {
        return Ok(None);
    };

    // Validate against the **effective** registry (built-in + custom
    // personas from identity-pack.json) so a session can pick a
    // user-created persona without hitting "unknown persona id".
    // Best-effort pack read: a malformed pack falls back to built-in
    // only — better to reject the override than to corrupt session state.
    let builtin = IdentityRegistry::builtin();
    let pack = read_identity_customization_pack().unwrap_or_default();
    let registry = apply_identity_customization_pack(&builtin, &pack);
    let mut soul_id = identity
        .soul_id
        .and_then(|value| (!value.trim().is_empty()).then_some(value));
    let persona_id = identity
        .persona_id
        .and_then(|value| (!value.trim().is_empty()).then_some(value));

    if let Some(ref soul) = soul_id {
        if registry.soul(soul).is_none() {
            return Err(format!("unknown soul id: {soul}"));
        }
    }

    if let Some(ref persona) = persona_id {
        let persona_def = registry
            .persona(persona)
            .ok_or_else(|| format!("unknown persona id: {persona}"))?;
        match soul_id.as_deref() {
            Some(soul) if soul != persona_def.soul_id => {
                return Err(format!(
                    "persona `{persona}` does not belong to soul `{soul}`"
                ));
            }
            None => {
                soul_id = Some(persona_def.soul_id.clone());
            }
            _ => {}
        }
    }

    Ok(Some(SessionIdentityOverride {
        soul_id,
        persona_id,
    }))
}

/// Create a new session.
///
/// If project_id is empty, creates a legacy session (no project).
/// If project_id is non-empty, creates a session within that project.
#[tauri::command]
#[allow(dead_code)]
pub async fn create_session(
    state: State<'_, AppState>,
    project_id: String,
    title: String,
    identity: Option<SessionIdentityInput>,
) -> Result<Session, String> {
    let normalized_identity = normalized_identity_override(identity)?;
    if project_id.is_empty() {
        match normalized_identity {
            Some(identity) => state
                .session_manager
                .create_session_with_identity(
                    title,
                    String::new(),
                    identity.soul_id,
                    identity.persona_id,
                )
                .await
                .map_err(|e| e.to_string()),
            None => state
                .session_manager
                .create_session(title)
                .await
                .map_err(|e| e.to_string()),
        }
    } else {
        match normalized_identity {
            Some(identity) => state
                .session_manager
                .create_session_with_identity(
                    title,
                    project_id,
                    identity.soul_id,
                    identity.persona_id,
                )
                .await
                .map_err(|e| e.to_string()),
            None => state
                .session_manager
                .create_session_for_project(&project_id, title)
                .await
                .map_err(|e| e.to_string()),
        }
    }
}

/// List all sessions (legacy path only).
#[tauri::command]
#[allow(dead_code)]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionMeta>, String> {
    state
        .session_manager
        .list_sessions()
        .await
        .map_err(|e| e.to_string())
}

/// List sessions within a specific project.
#[tauri::command]
#[allow(dead_code)]
pub async fn list_project_sessions(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<SessionMeta>, String> {
    state
        .session_manager
        .list_project_sessions(&project_id)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a session by ID.
#[tauri::command]
#[allow(dead_code)]
pub async fn delete_session(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .session_manager
        .delete_session(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Rename a session in place.
#[tauri::command]
#[allow(dead_code)]
pub async fn rename_session(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<SessionMeta, String> {
    state
        .session_manager
        .rename_session(&id, title)
        .await
        .map(|session| SessionMeta::from_session(&session))
        .map_err(|e| e.to_string())
}

/// Set the pinned state of a session.
#[tauri::command]
#[allow(dead_code)]
pub async fn set_session_pinned(
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> Result<Session, String> {
    state
        .session_manager
        .set_session_pinned(&id, pinned)
        .await
        .map_err(|e| e.to_string())
}

/// Toggle per-session memory on or off.
///
/// `enabled = false` silences the rolling summary / compile / fact-extract
/// pipelines for this session even when the master memory switch is on,
/// and stamps `memory_disabled_since = now`.  `enabled = true` re-enables
/// memory and stamps `memory_reenabled_at = now`.  Both timestamps are
/// later read by the compile pipeline (lands in 8B) to skip the silenced
/// window when aggregating summaries.  Phase 8A.4 / v2 §Sprint 1 / T-A4.
#[tauri::command]
#[allow(dead_code)]
pub async fn memory_session_set_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<Session, String> {
    state
        .session_manager
        .set_session_memory_enabled(&id, enabled)
        .await
        .map_err(|e| e.to_string())
}

/// Update session-level identity override.
#[tauri::command]
#[allow(dead_code)]
pub async fn set_session_identity(
    state: State<'_, AppState>,
    id: String,
    identity: SessionIdentityInput,
) -> Result<SessionMeta, String> {
    let normalized =
        normalized_identity_override(Some(identity))?.unwrap_or(SessionIdentityOverride {
            soul_id: None,
            persona_id: None,
        });
    state
        .session_manager
        .set_session_identity(&id, normalized.soul_id, normalized.persona_id)
        .await
        .map(|session| SessionMeta::from_session(&session))
        .map_err(|e| e.to_string())
}

/// Get a session with its messages (for restoring chat history).
#[tauri::command]
#[allow(dead_code)]
pub async fn get_session(state: State<'_, AppState>, id: String) -> Result<Session, String> {
    state
        .session_manager
        .restore_session(&id)
        .await
        .map_err(|e| e.to_string())
}
