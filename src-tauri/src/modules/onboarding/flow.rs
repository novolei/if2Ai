//! Onboarding flow state machine driver.
//!
//! Implements the step transition logic defined in ADR-014 Section 3.3:
//! - `next_step()`: Advance to the next step (with precondition checks)
//! - `prev_step()`: Go back to the previous step (boundary-safe)
//! - `can_proceed()`: Check if a step's completion criteria are met
//!
//! The state machine enforces sequential progression — no skipping steps.

use crate::modules::onboarding::state::{
    AppOnboardingState, OnboardingError, OnboardingState, OnboardingStep,
};

/// Stateless onboarding flow driver.
///
/// All methods take and return `OnboardingState`, leaving persistence
/// to the caller (see `store` module). This design keeps the flow
/// logic pure and testable.
pub struct OnboardingFlow;

impl OnboardingFlow {
    /// Advance to the next step.
    ///
    /// # Rules
    /// - If already completed, returns `AlreadyCompleted`
    /// - If at step 0 (FirstLaunch), advances to step 1 (Welcome)
    /// - Otherwise advances from current step to current + 1
    /// - Marks the current step as completed before advancing
    pub fn next_step(state: &OnboardingState) -> Result<OnboardingState, OnboardingError> {
        if state.onboarding_completed {
            return Err(OnboardingError::AlreadyCompleted);
        }

        let next_step_index = if state.current_step == 0 {
            1 // FirstLaunch → Welcome
        } else {
            let next = state.current_step + 1;
            if next > 6 {
                return Err(OnboardingError::CannotProceed(
                    "already at final step (Activation)".to_string(),
                ));
            }
            next
        };

        let next_step = OnboardingStep::from_index(next_step_index)
            .ok_or(OnboardingError::InvalidStepIndex(next_step_index))?;

        let mut new_state = state.clone();

        // Mark the previous step as completed (if we were past step 0)
        if new_state.current_step > 0
            && !new_state.completed_steps.contains(&new_state.current_step)
        {
            new_state.completed_steps.push(new_state.current_step);
        }

        new_state.current_step = next_step.as_index();

        Ok(new_state)
    }

    /// Go back to the previous step.
    ///
    /// Returns `AtFirstStep` if already at step 1 (Welcome).
    pub fn prev_step(state: &OnboardingState) -> Result<OnboardingState, OnboardingError> {
        if state.onboarding_completed {
            return Err(OnboardingError::CannotProceed(
                "onboarding already completed, cannot go back".to_string(),
            ));
        }

        if state.current_step <= 1 {
            return Err(OnboardingError::AtFirstStep);
        }

        let prev_index = state.current_step - 1;
        let prev_step = OnboardingStep::from_index(prev_index)
            .ok_or(OnboardingError::InvalidStepIndex(prev_index))?;

        let mut new_state = state.clone();
        new_state.current_step = prev_step.as_index();

        Ok(new_state)
    }

    /// Check if the current step's completion criteria are met.
    ///
    /// This is a placeholder that returns `true` for all steps.
    /// In practice, this will be wired to actual checks:
    /// - Step 2 (SystemCheck): all checks passed + model downloaded
    /// - Step 3 (SecurityConfirm): user confirmed security
    /// - Step 4 (ProviderSetup): provider configured + model selected
    /// - Step 5 (ChannelSetup): at least one channel configured
    /// - Step 6 (Activation): first message sent successfully
    ///
    /// For now, the frontend controls step advancement via Tauri commands.
    #[must_use]
    pub fn can_proceed(_step: u8, _state: &OnboardingState) -> bool {
        // TODO: Wire to actual precondition checks in later slices
        // Step 2: check system_check report
        // Step 3: check security_confirmed flag
        // Step 4: check active_provider and active_model in config
        // Step 5: check at least one configured channel
        // Step 6: check activation checklist
        true
    }

    /// Mark onboarding as fully completed.
    ///
    /// Sets `onboarding_completed = true`, adds step 6 to completed_steps,
    /// and clears draft_configs (no longer needed after completion).
    pub fn complete(state: &OnboardingState) -> Result<OnboardingState, OnboardingError> {
        if state.onboarding_completed {
            return Err(OnboardingError::AlreadyCompleted);
        }

        if state.current_step != 6 {
            return Err(OnboardingError::CannotProceed(format!(
                "cannot complete onboarding from step {} (must be at step 6: Activation)",
                state.current_step
            )));
        }

        let mut new_state = state.clone();
        new_state.onboarding_completed = true;
        new_state.current_step = 6;
        if !new_state.completed_steps.contains(&6) {
            new_state.completed_steps.push(6);
        }
        // Clear draft configs — no longer needed after completion
        new_state.draft_configs.clear();
        new_state.last_failure = None;

        Ok(new_state)
    }

    /// Save a draft configuration for a specific step.
    ///
    /// Used for crash recovery — if the app closes unexpectedly,
    /// the UI can pre-fill forms from the saved draft.
    pub fn save_draft_config(
        state: &OnboardingState,
        step: u8,
        config: &serde_json::Value,
    ) -> Result<OnboardingState, OnboardingError> {
        if step == 0 || step > 6 {
            return Err(OnboardingError::InvalidStepIndex(step));
        }

        let mut new_state = state.clone();
        new_state.draft_configs.insert(step, config.clone());
        new_state.current_step = step.max(new_state.current_step);

        Ok(new_state)
    }

    /// Record a failure for recovery purposes.
    pub fn record_failure(
        state: &OnboardingState,
        step: u8,
        error: &str,
        error_code: Option<&str>,
        retriable: bool,
    ) -> Result<OnboardingState, OnboardingError> {
        if step == 0 || step > 6 {
            return Err(OnboardingError::InvalidStepIndex(step));
        }

        let mut new_state = state.clone();
        new_state.last_failure = Some(crate::modules::onboarding::state::OnboardingFailure {
            step,
            error: error.to_string(),
            error_code: error_code.map(String::from),
            retriable,
            occurred_at: chrono::Utc::now().to_rfc3339(),
        });

        Ok(new_state)
    }

    /// Get the current high-level app state from an `OnboardingState`.
    #[must_use]
    pub fn to_app_state(state: &OnboardingState) -> AppOnboardingState {
        state.to_app_state()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_step_from_first_launch() {
        let state = OnboardingState::new();
        let result = OnboardingFlow::next_step(&state).unwrap();
        assert_eq!(result.current_step, 1);
        assert!(result.completed_steps.is_empty());
    }

    #[test]
    fn test_next_step_sequential() {
        let mut state = OnboardingState::new();
        state.current_step = 1;

        for expected in 2..=6 {
            state = OnboardingFlow::next_step(&state).unwrap();
            assert_eq!(state.current_step, expected);
            assert!(state.completed_steps.contains(&(expected - 1)));
        }
    }

    #[test]
    fn test_next_step_from_final_step_errors() {
        let mut state = OnboardingState::new();
        state.current_step = 6;
        state.completed_steps = vec![1, 2, 3, 4, 5];

        let err = OnboardingFlow::next_step(&state).unwrap_err();
        assert!(matches!(err, OnboardingError::CannotProceed(_)));
    }

    #[test]
    fn test_next_step_when_completed_errors() {
        let mut state = OnboardingState::new();
        state.onboarding_completed = true;

        let err = OnboardingFlow::next_step(&state).unwrap_err();
        assert!(matches!(err, OnboardingError::AlreadyCompleted));
    }

    #[test]
    fn test_prev_step_from_step_3() {
        let mut state = OnboardingState::new();
        state.current_step = 3;

        let result = OnboardingFlow::prev_step(&state).unwrap();
        assert_eq!(result.current_step, 2);
    }

    #[test]
    fn test_prev_step_from_first_errors() {
        let mut state = OnboardingState::new();
        state.current_step = 1;

        let err = OnboardingFlow::prev_step(&state).unwrap_err();
        assert!(matches!(err, OnboardingError::AtFirstStep));
    }

    #[test]
    fn test_prev_step_from_zero_errors() {
        let state = OnboardingState::new(); // current_step = 0

        let err = OnboardingFlow::prev_step(&state).unwrap_err();
        assert!(matches!(err, OnboardingError::AtFirstStep));
    }

    #[test]
    fn test_complete_marks_done_and_clears_drafts() {
        let mut state = OnboardingState::new();
        state.current_step = 6;
        state
            .draft_configs
            .insert(6, serde_json::json!({ "test": "data" }));

        let result = OnboardingFlow::complete(&state).unwrap();
        assert!(result.onboarding_completed);
        assert!(result.completed_steps.contains(&6));
        assert!(result.draft_configs.is_empty());
        assert!(result.last_failure.is_none());
    }

    #[test]
    fn test_complete_from_wrong_step_errors() {
        let state = OnboardingState::new(); // at step 0
        assert!(OnboardingFlow::complete(&state).is_err());

        let mut state = OnboardingState::new();
        state.current_step = 3;
        assert!(OnboardingFlow::complete(&state).is_err());
    }

    #[test]
    fn test_can_proceed_always_true_for_now() {
        let state = OnboardingState::new();
        for step in 1..=6 {
            assert!(OnboardingFlow::can_proceed(step, &state));
        }
        // TODO: This will change when precondition checks are wired
    }

    #[test]
    fn test_to_app_state_transitions() {
        let fresh = OnboardingState::new();
        assert!(matches!(
            OnboardingFlow::to_app_state(&fresh),
            AppOnboardingState::FirstLaunch
        ));

        let mut mid = OnboardingState::new();
        mid.current_step = 4;
        assert!(matches!(
            OnboardingFlow::to_app_state(&mid),
            AppOnboardingState::Onboarding { step: 4 }
        ));

        let mut done = OnboardingState::new();
        done.onboarding_completed = true;
        assert!(matches!(
            OnboardingFlow::to_app_state(&done),
            AppOnboardingState::Ready
        ));
    }

    #[test]
    fn test_save_draft_config() {
        let state = OnboardingState::new();
        let draft = serde_json::json!({ "api_key": "sk-test" });
        let result = OnboardingFlow::save_draft_config(&state, 4, &draft).unwrap();
        assert_eq!(result.current_step, 4);
        assert!(result.draft_configs.contains_key(&4));
    }

    #[test]
    fn test_record_failure() {
        let state = OnboardingState::new();
        let result = OnboardingFlow::record_failure(
            &state,
            4,
            "Connection timeout",
            Some("provider_timeout"),
            true,
        )
        .unwrap();
        let failure = result.last_failure.unwrap();
        assert_eq!(failure.step, 4);
        assert_eq!(failure.error, "Connection timeout");
        assert_eq!(failure.error_code, Some("provider_timeout".to_string()));
        assert!(failure.retriable);
    }
}
