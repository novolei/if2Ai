//! Onboarding state machine type definitions.
//!
//! Defines the core types for the 6-step onboarding flow:
//! - `OnboardingStep`: The 6 individual steps in the onboarding process
//! - `AppOnboardingState`: High-level application state (FirstLaunch/Onboarding/Ready)
//! - `OnboardingState`: Persisted state stored in `~/.if2ai/state.json`
//! - `OnboardingFailure`: Records of failures for recovery purposes

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Error type ──────────────────────────────────────────────────────────────

/// Error type for onboarding state machine operations.
#[derive(Debug, Error)]
pub enum OnboardingError {
    #[error("invalid step index: {0}, must be 1-6")]
    InvalidStepIndex(u8),

    #[error("cannot proceed: {0}")]
    CannotProceed(String),

    #[error("cannot go back: already at first step")]
    AtFirstStep,

    #[error("already completed")]
    AlreadyCompleted,

    #[error("state file I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("state deserialize error: {0}")]
    DeserializeError(String),
}

// ── OnboardingStep ──────────────────────────────────────────────────────────

/// The 6 individual steps in the onboarding process.
///
/// Each step has specific preconditions and completion criteria
/// as defined in ADR-014 Section 3.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStep {
    /// Step 1: Welcome page — user clicks "Start"
    Welcome,
    /// Step 2: System check — CPU/GPU/Node.js detection + embedded model download
    SystemCheck,
    /// Step 3: Security confirmation — user reads and confirms 8 risk items
    SecurityConfirm,
    /// Step 4: Provider setup — select and configure AI model provider
    ProviderSetup,
    /// Step 5: Channel setup — configure communication channels
    ChannelSetup,
    /// Step 6: Activation — wake the agent for the first time
    Activation,
}

impl OnboardingStep {
    /// Convert to 1-based step index.
    #[must_use]
    pub fn as_index(&self) -> u8 {
        match self {
            Self::Welcome => 1,
            Self::SystemCheck => 2,
            Self::SecurityConfirm => 3,
            Self::ProviderSetup => 4,
            Self::ChannelSetup => 5,
            Self::Activation => 6,
        }
    }

    /// Convert from 1-based step index. Returns `None` for invalid indices.
    #[must_use]
    pub fn from_index(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::Welcome),
            2 => Some(Self::SystemCheck),
            3 => Some(Self::SecurityConfirm),
            4 => Some(Self::ProviderSetup),
            5 => Some(Self::ChannelSetup),
            6 => Some(Self::Activation),
            _ => None,
        }
    }

    /// Human-readable label for the step.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::SystemCheck => "System Check",
            Self::SecurityConfirm => "Security Confirm",
            Self::ProviderSetup => "Provider Setup",
            Self::ChannelSetup => "Channel Setup",
            Self::Activation => "Activation",
        }
    }
}

impl fmt::Display for OnboardingStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ── AppOnboardingState ──────────────────────────────────────────────────────

/// High-level application onboarding state.
///
/// This enum represents the overall lifecycle from the app's perspective:
/// - `FirstLaunch`: Never started onboarding
/// - `Onboarding { step }`: In the middle of the 6-step flow
/// - `Ready`: Onboarding complete, user can access main app
///
/// Note: This is distinct from the Tauri `AppState` struct in `commands/mod.rs`
/// which handles dependency injection. This enum is specifically for onboarding
/// lifecycle management.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AppOnboardingState {
    FirstLaunch,
    Onboarding { step: u8 },
    Ready,
}

// ── OnboardingFailure ───────────────────────────────────────────────────────

/// Records a failure during onboarding for recovery purposes.
///
/// Part of the interrupt recovery mechanism (ADR-014 Section 18.5).
/// When the app restarts after a failure, the UI can pre-fill forms
/// from `draft_configs` and show the last failure reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingFailure {
    /// The step where the failure occurred (1-6).
    pub step: u8,
    /// Human-readable error description.
    pub error: String,
    /// Machine-readable error code, e.g. "provider_timeout", "provider_auth_error".
    pub error_code: Option<String>,
    /// Whether the user can retry the failed operation.
    pub retriable: bool,
    /// ISO 8601 timestamp when the failure occurred.
    pub occurred_at: String,
}

// ── OnboardingState ─────────────────────────────────────────────────────────

/// Persisted onboarding state, stored in `~/.if2ai/state.json`.
///
/// This struct is the source of truth for the onboarding flow's progress.
/// It tracks completion status, current step, completed steps, and
/// intermediate draft configurations for crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    /// Whether the entire onboarding flow has been completed.
    pub onboarding_completed: bool,
    /// Current step index (1-6). 0 means not yet started.
    pub current_step: u8,
    /// Steps that have been fully completed (in order).
    pub completed_steps: Vec<u8>,
    /// Per-step intermediate drafts for crash recovery.
    /// Key is step index (1-6), value is arbitrary JSON representing
    /// the user's partially entered data for that step.
    #[serde(default)]
    pub draft_configs: HashMap<u8, serde_json::Value>,
    /// Most recent failure record, if any.
    #[serde(default)]
    pub last_failure: Option<OnboardingFailure>,
}

impl OnboardingState {
    /// Create a fresh `OnboardingState` for a first-time user.
    #[must_use]
    pub fn new() -> Self {
        Self {
            onboarding_completed: false,
            current_step: 0,
            completed_steps: Vec::new(),
            draft_configs: HashMap::new(),
            last_failure: None,
        }
    }

    /// Convert to the high-level `AppOnboardingState` enum.
    #[must_use]
    pub fn to_app_state(&self) -> AppOnboardingState {
        if self.onboarding_completed {
            return AppOnboardingState::Ready;
        }
        if self.current_step == 0 {
            return AppOnboardingState::FirstLaunch;
        }
        AppOnboardingState::Onboarding {
            step: self.current_step,
        }
    }
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onboarding_step_as_index() {
        assert_eq!(OnboardingStep::Welcome.as_index(), 1);
        assert_eq!(OnboardingStep::SystemCheck.as_index(), 2);
        assert_eq!(OnboardingStep::SecurityConfirm.as_index(), 3);
        assert_eq!(OnboardingStep::ProviderSetup.as_index(), 4);
        assert_eq!(OnboardingStep::ChannelSetup.as_index(), 5);
        assert_eq!(OnboardingStep::Activation.as_index(), 6);
    }

    #[test]
    fn test_onboarding_step_from_index() {
        assert_eq!(OnboardingStep::from_index(1), Some(OnboardingStep::Welcome));
        assert_eq!(
            OnboardingStep::from_index(6),
            Some(OnboardingStep::Activation)
        );
        assert_eq!(OnboardingStep::from_index(0), None);
        assert_eq!(OnboardingStep::from_index(7), None);
    }

    #[test]
    fn test_onboarding_step_roundtrip() {
        for i in 1..=6 {
            let step = OnboardingStep::from_index(i).unwrap();
            assert_eq!(step.as_index(), i);
        }
    }

    #[test]
    fn test_onboarding_state_new() {
        let state = OnboardingState::new();
        assert!(!state.onboarding_completed);
        assert_eq!(state.current_step, 0);
        assert!(state.completed_steps.is_empty());
        assert!(state.draft_configs.is_empty());
        assert!(state.last_failure.is_none());
    }

    #[test]
    fn test_onboarding_state_to_app_state() {
        let fresh = OnboardingState::new();
        assert!(matches!(
            fresh.to_app_state(),
            AppOnboardingState::FirstLaunch
        ));

        let mut in_progress = OnboardingState::new();
        in_progress.current_step = 3;
        assert!(matches!(
            in_progress.to_app_state(),
            AppOnboardingState::Onboarding { step: 3 }
        ));

        let mut completed = OnboardingState::new();
        completed.onboarding_completed = true;
        assert!(matches!(
            completed.to_app_state(),
            AppOnboardingState::Ready
        ));
    }

    #[test]
    fn test_onboarding_state_serialization() {
        let state = OnboardingState::new();
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("onboarding_completed"));
        assert!(json.contains("current_step"));
        assert!(json.contains("completed_steps"));

        let deserialized: OnboardingState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.current_step, state.current_step);
    }

    #[test]
    fn test_onboarding_state_serialization_with_draft() {
        let mut state = OnboardingState::new();
        state.current_step = 4;
        state.draft_configs.insert(
            4,
            serde_json::json!({
                "selected_provider": "ollama",
                "base_url": "http://localhost:11434"
            }),
        );
        state.last_failure = Some(OnboardingFailure {
            step: 4,
            error: "Connection timeout".to_string(),
            error_code: Some("provider_timeout".to_string()),
            retriable: true,
            occurred_at: "2026-04-16T10:30:00Z".to_string(),
        });

        let json = serde_json::to_string_pretty(&state).unwrap();
        let restored: OnboardingState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.current_step, 4);
        assert!(restored.draft_configs.contains_key(&4));
        assert!(restored.last_failure.is_some());
        let failure = restored.last_failure.unwrap();
        assert_eq!(failure.error_code, Some("provider_timeout".to_string()));
    }

    #[test]
    fn test_app_onboarding_state_serialization() {
        let first = AppOnboardingState::FirstLaunch;
        let json = serde_json::to_string(&first).unwrap();
        assert!(json.contains("first_launch"));

        let onboarding = AppOnboardingState::Onboarding { step: 3 };
        let json = serde_json::to_string(&onboarding).unwrap();
        assert!(json.contains("onboarding"));
        assert!(json.contains("3"));
    }
}
