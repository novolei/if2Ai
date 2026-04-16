//! Onboarding state persistence layer.
//!
//! Reads and writes `OnboardingState` to `~/.if2ai/state.json`.
//! Handles directory creation, atomic writes, and graceful error handling.

use std::path::{Path, PathBuf};

use crate::modules::onboarding::state::{OnboardingError, OnboardingState};

/// Returns the path to `~/.if2ai/state.json`.
fn state_file_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".if2ai").join("state.json")
}

/// Returns the path to `~/.if2ai/` directory.
fn if2ai_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".if2ai")
}

/// Ensure the `~/.if2ai/` directory exists, creating it if necessary.
fn ensure_if2ai_dir() -> Result<(), std::io::Error> {
    let dir = if2ai_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    // Set directory permissions to 0700 (owner only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dir)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&dir, perms)?;
    }
    Ok(())
}

/// Load onboarding state from `~/.if2ai/state.json`.
///
/// If the file does not exist, returns a fresh `OnboardingState::new()`.
/// If the file exists but is corrupt, returns an error.
pub async fn load_state() -> Result<OnboardingState, OnboardingError> {
    load_state_from_path(&state_file_path()).await
}

/// Save onboarding state to `~/.if2ai/state.json`.
///
/// Creates the `~/.if2ai/` directory if it doesn't exist.
/// Uses atomic write (write to temp file, then rename) to prevent
/// corruption on crash during write.
pub async fn save_state(state: &OnboardingState) -> Result<(), OnboardingError> {
    ensure_if2ai_dir()?;
    save_state_to_path(state, &state_file_path()).await
}

/// Delete the onboarding state file.
///
/// Used during `config_reset_onboarding()` to clear all progress.
pub async fn delete_state() -> Result<(), OnboardingError> {
    let path = state_file_path();
    if path.exists() {
        tokio::fs::remove_file(&path)
            .await
            .map_err(OnboardingError::IoError)?;
    }
    Ok(())
}

/// Internal: load from a specific path (used by both public API and tests).
async fn load_state_from_path(path: &Path) -> Result<OnboardingState, OnboardingError> {
    if !path.exists() {
        return Ok(OnboardingState::new());
    }

    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(OnboardingError::IoError)?;

    let state: OnboardingState = serde_json::from_str(&content).map_err(|e| {
        OnboardingError::DeserializeError(format!("failed to parse state.json: {e}"))
    })?;

    Ok(state)
}

/// Internal: save to a specific path (used by both public API and tests).
async fn save_state_to_path(state: &OnboardingState, path: &Path) -> Result<(), OnboardingError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(OnboardingError::IoError)?;
    }

    let content = serde_json::to_string_pretty(state).map_err(|e| {
        OnboardingError::DeserializeError(format!("failed to serialize state: {e}"))
    })?;

    // Atomic write: write to temp file, then rename
    let temp_path = path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, content)
        .await
        .map_err(OnboardingError::IoError)?;

    // Set file permissions to 0600 (owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_path)
            .map_err(OnboardingError::IoError)?
            .permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&temp_path, perms).map_err(OnboardingError::IoError)?;
    }

    tokio::fs::rename(&temp_path, path)
        .await
        .map_err(OnboardingError::IoError)?;

    Ok(())
}

/// Get the path to the state file (exposed for testing).
#[cfg(test)]
pub fn test_state_path() -> PathBuf {
    state_file_path()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_file_path_ends_with_state_json() {
        let path = test_state_path();
        assert!(
            path.to_string_lossy().ends_with("state.json"),
            "Expected path to end with state.json, got: {:?}",
            path
        );
    }

    #[test]
    fn test_state_file_path_contains_if2ai() {
        let path = test_state_path();
        assert!(
            path.to_string_lossy().contains(".if2ai"),
            "Expected path to contain .if2ai, got: {:?}",
            path
        );
    }

    /// Helper: create a temp directory with a state.json path.
    fn temp_state_path() -> PathBuf {
        let dir = tempfile::tempdir().unwrap();
        dir.path().join("state.json")
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let path = temp_state_path();
        let mut state = OnboardingState::new();
        state.current_step = 3;
        state.completed_steps = vec![1, 2];

        save_state_to_path(&state, &path).await.unwrap();
        let loaded = load_state_from_path(&path).await.unwrap();

        assert_eq!(loaded.current_step, 3);
        assert_eq!(loaded.completed_steps, vec![1, 2]);
    }

    #[tokio::test]
    async fn test_load_fresh_returns_new_state() {
        let path = temp_state_path();
        // Ensure no state file exists
        assert!(!path.exists());

        let state = load_state_from_path(&path).await.unwrap();
        assert!(!state.onboarding_completed);
        assert_eq!(state.current_step, 0);
    }

    #[tokio::test]
    async fn test_delete_removes_file() {
        let path = temp_state_path();
        let mut state = OnboardingState::new();
        state.current_step = 1;
        save_state_to_path(&state, &path).await.unwrap();

        assert!(path.exists());

        tokio::fs::remove_file(&path).await.unwrap();
        assert!(!path.exists());
    }
}
