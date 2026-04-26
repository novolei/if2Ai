//! browser-use cloud escalation contracts.

use super::contract::{SmartBrowserBackend, SmartBrowserEscalationState, SmartBrowserRisk};
use super::policy::{evaluate_cloud_escalation, SmartBrowserPolicyDecision};
use crate::modules::runtime::permissions::{
    smart_browser_cloud_required_mode, PermissionMode, PermissionOutcome, PermissionPolicy,
};

/// Request to move a Smart Browser session to a cloud backend.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CloudEscalationRequest {
    pub session_id: String,
    pub reason: String,
    #[serde(default)]
    pub risks: Vec<SmartBrowserRisk>,
}

/// Decision recorded for the UI timeline after cloud escalation policy runs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CloudEscalationDecision {
    pub backend: SmartBrowserBackend,
    pub state: SmartBrowserEscalationState,
    pub reason: String,
}

/// Evaluate a cloud escalation request without performing any network work.
#[must_use]
pub fn decide_cloud_escalation(
    request: &CloudEscalationRequest,
    user_approved: bool,
) -> CloudEscalationDecision {
    match evaluate_cloud_escalation(SmartBrowserBackend::BrowserUseCloud, user_approved) {
        SmartBrowserPolicyDecision::Allow => CloudEscalationDecision {
            backend: SmartBrowserBackend::BrowserUseCloud,
            state: SmartBrowserEscalationState::Approved,
            reason: request.reason.clone(),
        },
        SmartBrowserPolicyDecision::RequiresApproval { reason } => CloudEscalationDecision {
            backend: SmartBrowserBackend::BrowserUseCloud,
            state: SmartBrowserEscalationState::ApprovalRequired,
            reason,
        },
        SmartBrowserPolicyDecision::Block { reason } => CloudEscalationDecision {
            backend: SmartBrowserBackend::BrowserUseCloud,
            state: SmartBrowserEscalationState::Blocked,
            reason,
        },
    }
}

/// Authorize a cloud escalation through the shared permission policy model.
#[must_use]
pub fn authorize_cloud_escalation(
    current_mode: PermissionMode,
    user_approved: bool,
) -> PermissionOutcome {
    let policy = PermissionPolicy::new(current_mode)
        .with_tool_requirement("smart_browser.cloud", smart_browser_cloud_required_mode());
    if !user_approved {
        return PermissionOutcome::Deny {
            reason: "user has not approved Smart Browser cloud escalation".to_string(),
        };
    }
    policy.authorize("smart_browser.cloud", "browser_use_cloud", None)
}

#[cfg(test)]
mod tests {
    use super::{authorize_cloud_escalation, decide_cloud_escalation, CloudEscalationRequest};
    use crate::modules::runtime::permissions::{PermissionMode, PermissionOutcome};
    use crate::modules::smart_browser::contract::{SmartBrowserEscalationState, SmartBrowserRisk};

    #[test]
    fn cloud_escalation_records_approval_required_and_approved_states() {
        let request = CloudEscalationRequest {
            session_id: "session-1".to_string(),
            reason: "captcha handoff".to_string(),
            risks: vec![SmartBrowserRisk::CloudEscalation],
        };

        assert_eq!(
            decide_cloud_escalation(&request, false).state,
            SmartBrowserEscalationState::ApprovalRequired
        );
        assert_eq!(
            decide_cloud_escalation(&request, true).state,
            SmartBrowserEscalationState::Approved
        );
    }

    #[test]
    fn cloud_escalation_uses_permission_policy() {
        assert!(matches!(
            authorize_cloud_escalation(PermissionMode::Prompt, false),
            PermissionOutcome::Deny { reason } if reason.contains("not approved")
        ));
        assert_eq!(
            authorize_cloud_escalation(PermissionMode::Allow, true),
            PermissionOutcome::Allow
        );
    }
}
