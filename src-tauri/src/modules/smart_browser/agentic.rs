//! Agentic browser-use policy helpers.

/// Raw browser-use agent retry tool. It must stay hidden behind Smart Browser
/// policy instead of being exposed directly to the main LLM tool registry.
pub const RAW_BROWSER_USE_AGENT_TOOL: &str = "retry_with_browser_use_agent";

/// Return true when a browser-use MCP tool must not be exposed raw.
#[must_use]
pub fn hides_raw_agent_tool(tool_name: &str) -> bool {
    tool_name == RAW_BROWSER_USE_AGENT_TOOL
}

/// Return true when a browser-use MCP tool can be exposed as a direct primitive.
#[must_use]
pub fn allows_direct_mcp_tool(tool_name: &str) -> bool {
    !hides_raw_agent_tool(tool_name)
}

#[cfg(test)]
pub mod tests {
    use super::{allows_direct_mcp_tool, hides_raw_agent_tool};

    #[test]
    fn hides_raw_agent_tool_from_direct_exposure() {
        assert!(hides_raw_agent_tool("retry_with_browser_use_agent"));
        assert!(!hides_raw_agent_tool("browser_navigate"));
        assert!(!allows_direct_mcp_tool("retry_with_browser_use_agent"));
    }
}
