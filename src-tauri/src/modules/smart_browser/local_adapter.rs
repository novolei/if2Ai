//! Adapter helpers between the legacy `browser` tool and Smart Browser.

use super::contract::SmartBrowserCommandKind;

/// Mapping between the current `browser` tool action string and the canonical
/// Smart Browser command kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserToolActionMapping {
    pub browser_action: &'static str,
    pub command_kind: SmartBrowserCommandKind,
}

/// Convert a legacy `browser` tool action into a Smart Browser command kind.
#[must_use]
pub fn browser_tool_action_to_command_kind(action: &str) -> Option<SmartBrowserCommandKind> {
    Some(match action {
        "start" => SmartBrowserCommandKind::Start,
        "stop" => SmartBrowserCommandKind::Stop,
        "navigate" => SmartBrowserCommandKind::Navigate,
        "snapshot" => SmartBrowserCommandKind::State,
        "screenshot" => SmartBrowserCommandKind::Screenshot,
        "click" => SmartBrowserCommandKind::Click,
        "type" => SmartBrowserCommandKind::TypeText,
        "scroll" => SmartBrowserCommandKind::Scroll,
        "select" => SmartBrowserCommandKind::Select,
        "key" => SmartBrowserCommandKind::Key,
        "wait" => SmartBrowserCommandKind::Wait,
        "evaluate" => SmartBrowserCommandKind::Evaluate,
        "tabs" => SmartBrowserCommandKind::Tabs,
        "switch_tab" => SmartBrowserCommandKind::SwitchTab,
        "close_tab" => SmartBrowserCommandKind::CloseTab,
        "downloads" => SmartBrowserCommandKind::Downloads,
        "console" => SmartBrowserCommandKind::Console,
        "network" => SmartBrowserCommandKind::Network,
        _ => return None,
    })
}

/// Convert a Smart Browser command kind into the existing `browser` tool action.
#[must_use]
pub fn smart_browser_command_to_browser_action(
    kind: SmartBrowserCommandKind,
) -> Option<&'static str> {
    Some(match kind {
        SmartBrowserCommandKind::Start => "start",
        SmartBrowserCommandKind::Stop => "stop",
        SmartBrowserCommandKind::Navigate => "navigate",
        SmartBrowserCommandKind::State => "snapshot",
        SmartBrowserCommandKind::Screenshot => "screenshot",
        SmartBrowserCommandKind::Click => "click",
        SmartBrowserCommandKind::TypeText => "type",
        SmartBrowserCommandKind::Scroll => "scroll",
        SmartBrowserCommandKind::Select => "select",
        SmartBrowserCommandKind::Key => "key",
        SmartBrowserCommandKind::Wait => "wait",
        SmartBrowserCommandKind::Evaluate => "evaluate",
        SmartBrowserCommandKind::Tabs => "tabs",
        SmartBrowserCommandKind::SwitchTab => "switch_tab",
        SmartBrowserCommandKind::CloseTab => "close_tab",
        SmartBrowserCommandKind::Downloads => "downloads",
        SmartBrowserCommandKind::Console => "console",
        SmartBrowserCommandKind::Network => "network",
        SmartBrowserCommandKind::Extract
        | SmartBrowserCommandKind::HandoffToHuman
        | SmartBrowserCommandKind::ReleaseHuman => return None,
    })
}

/// Return the stable legacy action mapping table.
#[must_use]
pub fn browser_tool_action_mappings() -> Vec<BrowserToolActionMapping> {
    [
        "start",
        "stop",
        "navigate",
        "snapshot",
        "screenshot",
        "click",
        "type",
        "scroll",
        "select",
        "key",
        "wait",
        "evaluate",
        "tabs",
        "switch_tab",
        "close_tab",
        "downloads",
        "console",
        "network",
    ]
    .iter()
    .filter_map(|action| {
        browser_tool_action_to_command_kind(action).map(|command_kind| BrowserToolActionMapping {
            browser_action: action,
            command_kind,
        })
    })
    .collect()
}

#[cfg(test)]
pub mod tests {
    use super::{
        browser_tool_action_mappings, browser_tool_action_to_command_kind,
        smart_browser_command_to_browser_action,
    };
    use crate::modules::smart_browser::contract::SmartBrowserCommandKind;

    #[test]
    fn maps_browser_actions_without_renaming() {
        assert_eq!(
            browser_tool_action_to_command_kind("navigate"),
            Some(SmartBrowserCommandKind::Navigate)
        );
        assert_eq!(
            browser_tool_action_to_command_kind("snapshot"),
            Some(SmartBrowserCommandKind::State)
        );
        assert_eq!(
            smart_browser_command_to_browser_action(SmartBrowserCommandKind::TypeText),
            Some("type")
        );
        assert_eq!(
            smart_browser_command_to_browser_action(SmartBrowserCommandKind::Extract),
            None
        );

        for mapping in browser_tool_action_mappings() {
            assert_eq!(
                smart_browser_command_to_browser_action(mapping.command_kind),
                Some(mapping.browser_action)
            );
        }
    }
}
