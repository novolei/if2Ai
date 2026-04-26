//! Smart Browser backend policy helpers.

use super::contract::{SmartBrowserBackend, SmartBrowserRisk};

/// Error returned when policy cannot resolve a backend label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendPolicyError {
    UnknownBackend(String),
    ApprovalRequired { reason: String },
    Blocked { reason: String },
}

impl std::fmt::Display for BackendPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownBackend(label) => write!(f, "unknown Smart Browser backend: {label}"),
            Self::ApprovalRequired { reason } => write!(f, "approval required: {reason}"),
            Self::Blocked { reason } => write!(f, "blocked: {reason}"),
        }
    }
}

impl std::error::Error for BackendPolicyError {}

/// Resolve an optional backend label into a concrete backend.
pub fn resolve_backend_label(
    label: Option<&str>,
) -> Result<SmartBrowserBackend, BackendPolicyError> {
    match label.unwrap_or("local_rust_cdp") {
        "local_rust_cdp" | "local" | "rust_cdp" => Ok(SmartBrowserBackend::LocalRustCdp),
        "browser_use_mcp" | "browser-use-mcp" => Ok(SmartBrowserBackend::BrowserUseMcp),
        "browser_use_cloud" | "browser-use-cloud" => Ok(SmartBrowserBackend::BrowserUseCloud),
        other => Err(BackendPolicyError::UnknownBackend(other.to_string())),
    }
}

/// Return true when a risk flag requires explicit human approval.
#[must_use]
pub const fn risk_requires_approval(risk: SmartBrowserRisk) -> bool {
    matches!(
        risk,
        SmartBrowserRisk::Login
            | SmartBrowserRisk::Payment
            | SmartBrowserRisk::PersonalDataSubmit
            | SmartBrowserRisk::FileUpload
            | SmartBrowserRisk::CookieProfileSync
            | SmartBrowserRisk::CloudEscalation
    )
}

/// Decision returned by Smart Browser risk policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartBrowserPolicyDecision {
    Allow,
    RequiresApproval { reason: String },
    Block { reason: String },
}

/// Evaluate sensitive action risk flags.
#[must_use]
pub fn evaluate_sensitive_action(
    risks: &[SmartBrowserRisk],
    user_approved: bool,
) -> SmartBrowserPolicyDecision {
    if risks.is_empty() {
        return SmartBrowserPolicyDecision::Allow;
    }
    if risks
        .iter()
        .any(|risk| *risk == SmartBrowserRisk::CookieProfileSync)
        && !user_approved
    {
        return SmartBrowserPolicyDecision::RequiresApproval {
            reason: "cookie/profile sync requires explicit user approval".to_string(),
        };
    }
    if risks.iter().any(|risk| risk_requires_approval(*risk)) && !user_approved {
        return SmartBrowserPolicyDecision::RequiresApproval {
            reason: "sensitive browser action requires explicit user approval".to_string(),
        };
    }
    SmartBrowserPolicyDecision::Allow
}

/// Evaluate whether Smart Browser may move to a cloud backend.
#[must_use]
pub fn evaluate_cloud_escalation(
    requested_backend: SmartBrowserBackend,
    user_approved: bool,
) -> SmartBrowserPolicyDecision {
    if requested_backend != SmartBrowserBackend::BrowserUseCloud {
        return SmartBrowserPolicyDecision::Allow;
    }
    if user_approved {
        SmartBrowserPolicyDecision::Allow
    } else {
        SmartBrowserPolicyDecision::RequiresApproval {
            reason: "browser-use cloud escalation requires explicit user approval".to_string(),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::{
        evaluate_cloud_escalation, evaluate_sensitive_action, resolve_backend_label,
        BackendPolicyError, SmartBrowserPolicyDecision,
    };
    use crate::modules::smart_browser::contract::{SmartBrowserBackend, SmartBrowserRisk};

    #[test]
    fn defaults_and_rejects_unknown_backend() {
        assert_eq!(
            resolve_backend_label(None).expect("default backend"),
            SmartBrowserBackend::LocalRustCdp
        );
        assert_eq!(
            resolve_backend_label(Some("browser_use_mcp")).expect("mcp backend"),
            SmartBrowserBackend::BrowserUseMcp
        );
        assert!(matches!(
            resolve_backend_label(Some("selenium")),
            Err(BackendPolicyError::UnknownBackend(label)) if label == "selenium"
        ));
    }

    #[test]
    fn cloud_escalation_requires_approval() {
        assert!(matches!(
            evaluate_cloud_escalation(SmartBrowserBackend::BrowserUseCloud, false),
            SmartBrowserPolicyDecision::RequiresApproval { reason }
                if reason.contains("cloud escalation")
        ));
        assert_eq!(
            evaluate_cloud_escalation(SmartBrowserBackend::BrowserUseCloud, true),
            SmartBrowserPolicyDecision::Allow
        );
    }

    #[test]
    fn sensitive_actions_are_gated() {
        let risks = [
            SmartBrowserRisk::Login,
            SmartBrowserRisk::Payment,
            SmartBrowserRisk::FileUpload,
        ];

        assert!(matches!(
            evaluate_sensitive_action(&risks, false),
            SmartBrowserPolicyDecision::RequiresApproval { reason }
                if reason.contains("sensitive browser action")
        ));
        assert_eq!(
            evaluate_sensitive_action(&risks, true),
            SmartBrowserPolicyDecision::Allow
        );
        assert!(matches!(
            evaluate_sensitive_action(&[SmartBrowserRisk::CookieProfileSync], false),
            SmartBrowserPolicyDecision::RequiresApproval { reason }
                if reason.contains("cookie/profile sync")
        ));
    }
}
