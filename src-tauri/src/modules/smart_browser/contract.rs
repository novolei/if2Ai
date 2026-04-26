//! Smart Browser DTOs shared by backend adapters, policy, and projection.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable session identifier for a Smart Browser instance.
///
/// Today this mirrors the chat `session_id`; keeping a newtype makes future
/// multi-browser sessions explicit instead of overloading raw strings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SmartBrowserSessionId(pub String);

impl SmartBrowserSessionId {
    /// Create a new Smart Browser session id.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Browser backend selected by Smart Browser policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartBrowserBackend {
    LocalRustCdp,
    BrowserUseMcp,
    BrowserUseCloud,
}

impl SmartBrowserBackend {
    /// Stable backend label used in logs, projection, and policy.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalRustCdp => "local_rust_cdp",
            Self::BrowserUseMcp => "browser_use_mcp",
            Self::BrowserUseCloud => "browser_use_cloud",
        }
    }
}

impl Default for SmartBrowserBackend {
    fn default() -> Self {
        Self::LocalRustCdp
    }
}

/// Canonical Smart Browser command names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartBrowserCommandKind {
    Start,
    Stop,
    Navigate,
    State,
    Screenshot,
    Click,
    TypeText,
    Scroll,
    Select,
    Key,
    Wait,
    Evaluate,
    Extract,
    Tabs,
    SwitchTab,
    CloseTab,
    Downloads,
    Console,
    Network,
    HandoffToHuman,
    ReleaseHuman,
}

impl SmartBrowserCommandKind {
    /// Return the stable snake_case label for the command.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Navigate => "navigate",
            Self::State => "state",
            Self::Screenshot => "screenshot",
            Self::Click => "click",
            Self::TypeText => "type_text",
            Self::Scroll => "scroll",
            Self::Select => "select",
            Self::Key => "key",
            Self::Wait => "wait",
            Self::Evaluate => "evaluate",
            Self::Extract => "extract",
            Self::Tabs => "tabs",
            Self::SwitchTab => "switch_tab",
            Self::CloseTab => "close_tab",
            Self::Downloads => "downloads",
            Self::Console => "console",
            Self::Network => "network",
            Self::HandoffToHuman => "handoff_to_human",
            Self::ReleaseHuman => "release_human",
        }
    }
}

/// Smart Browser command envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartBrowserCommand {
    pub command_id: String,
    pub session_id: SmartBrowserSessionId,
    pub backend: SmartBrowserBackend,
    pub kind: SmartBrowserCommandKind,
    #[serde(default)]
    pub input: Value,
}

/// Risk flags attached to a command or observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartBrowserRisk {
    Login,
    Payment,
    PersonalDataSubmit,
    FileUpload,
    CookieProfileSync,
    CrossOriginNavigation,
    CloudEscalation,
}

/// Browser-use/cloud escalation state projected into the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartBrowserEscalationState {
    None,
    Suggested,
    ApprovalRequired,
    Approved,
    Active,
    Blocked,
}

impl Default for SmartBrowserEscalationState {
    fn default() -> Self {
        Self::None
    }
}

/// Canonical observation returned by any Smart Browser backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartBrowserObservation {
    pub session_id: SmartBrowserSessionId,
    pub backend: SmartBrowserBackend,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_mime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_base64: Option<String>,
    #[serde(default)]
    pub risk_flags: Vec<SmartBrowserRisk>,
    #[serde(default)]
    pub escalation_state: SmartBrowserEscalationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_summary: Option<Value>,
}

/// Lifecycle states emitted by Smart Browser execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartBrowserEventKind {
    Queued,
    Running,
    Observed,
    Completed,
    Failed,
    Blocked,
    TakeoverStarted,
    TakeoverReleased,
    Escalated,
}

/// Canonical Smart Browser event envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartBrowserEvent {
    pub event_id: String,
    pub session_id: SmartBrowserSessionId,
    pub backend: SmartBrowserBackend,
    pub kind: SmartBrowserEventKind,
    pub command_kind: SmartBrowserCommandKind,
    pub occurred_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<SmartBrowserObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[cfg(test)]
pub mod tests {
    use serde_json::json;

    use super::{
        SmartBrowserBackend, SmartBrowserCommandKind, SmartBrowserEscalationState,
        SmartBrowserEvent, SmartBrowserEventKind, SmartBrowserObservation, SmartBrowserRisk,
        SmartBrowserSessionId,
    };

    #[test]
    fn serializes_core_events() {
        let event = SmartBrowserEvent {
            event_id: "event-1".to_string(),
            session_id: SmartBrowserSessionId::new("session-1"),
            backend: SmartBrowserBackend::LocalRustCdp,
            kind: SmartBrowserEventKind::Observed,
            command_kind: SmartBrowserCommandKind::Navigate,
            occurred_at_ms: 42,
            observation: Some(SmartBrowserObservation {
                session_id: SmartBrowserSessionId::new("session-1"),
                backend: SmartBrowserBackend::LocalRustCdp,
                url: Some("https://example.com".to_string()),
                title: Some("Example".to_string()),
                text_state: Some("[1] Link".to_string()),
                screenshot_mime: None,
                screenshot_base64: None,
                risk_flags: vec![SmartBrowserRisk::CrossOriginNavigation],
                escalation_state: SmartBrowserEscalationState::None,
                raw_summary: Some(json!({"source":"test"})),
            }),
            message: Some("observed".to_string()),
        };

        let encoded = serde_json::to_value(&event).expect("serialize smart browser event");
        assert_eq!(encoded["backend"], "local_rust_cdp");
        assert_eq!(encoded["kind"], "observed");
        assert_eq!(encoded["command_kind"], "navigate");
        assert_eq!(
            encoded["observation"]["risk_flags"][0],
            "cross_origin_navigation"
        );
    }
}
