//! Tool-routing guidance injected into the system prompt (Phase 7C, slice 7C.4).
//!
//! Background
//! ----------
//!
//! After Phase 7B / 7C the agent has *three* web-access tools:
//! `web_search`, `web_fetch`, `browser`.  Without explicit guidance the
//! LLM tends to default to `browser` (the most expressive surface), which
//! is wrong for almost every "what's the weather?" / "find me a paper"
//! style query — `browser` cold-start is 3-8s and may trip Cloudflare's
//! bot detection on Anthropic-protected sites.
//!
//! [`web_tools_routing_block`] returns a Markdown section that:
//!
//! 1. Establishes a cheap-→expensive escalation order.
//! 2. Lists the *narrow* situations where `browser` actually wins.
//! 3. Lists explicit anti-patterns ("DO NOT launch the browser when …").
//! 4. Reminds the model how to react to the "user has taken over" error
//!    that slice 7C.3 surfaces from the tool layer.
//!
//! The block is *only* injected when at least two of the three web tools
//! are registered.  If only one (or none) is available the LLM has no
//! choice to make and the routing prompt would just waste tokens.

/// The names this module recognises as "web tools" for routing decisions.
const WEB_TOOL_NAMES: &[&str] = &["web_search", "web_fetch", "browser"];

/// Build the web-access routing prompt block, or `None` when fewer than
/// two web tools are registered (in which case the LLM has nothing to
/// route between).
///
/// `registered` is the list of tool names produced by
/// [`crate::modules::tools::ToolRegistry::tool_names`].
#[must_use]
pub fn web_tools_routing_block(registered: &[String]) -> Option<String> {
    let has_search = registered.iter().any(|n| n == "web_search");
    let has_fetch = registered.iter().any(|n| n == "web_fetch");
    let has_browser = registered.iter().any(|n| n == "browser");

    let count = [has_search, has_fetch, has_browser]
        .iter()
        .filter(|x| **x)
        .count();
    if count < 2 {
        return None;
    }

    let mut block = String::with_capacity(1024);
    block.push_str("# Web Access Tool Routing\n\n");
    block.push_str(
        "You have multiple web-access tools.  ESCALATE FROM CHEAP TO EXPENSIVE — \
         pick the lowest-cost tool that can answer the user.\n\n",
    );

    let mut step = 1;
    if has_search {
        block.push_str(&format!(
            "{step}. **web_search** — DEFAULT for finding information or discovering \
             URLs.  Cost: one provider call, ~1-2s.  Use first when the user asks \
             a factual question, wants links, or you don't yet know which page \
             holds the answer.\n",
        ));
        step += 1;
    }
    if has_fetch {
        block.push_str(&format!(
            "{step}. **web_fetch** — when you have a SPECIFIC URL and want clean \
             text content.  Cost: one HTTP request, ~1-3s.  Use after `web_search` \
             surfaces a promising link, or when the user gives you the URL.\n",
        ));
        step += 1;
    }
    if has_browser {
        block.push_str(&format!(
            "{step}. **browser** — LAST RESORT.  Use ONLY when:\n   \
             • the site requires login or session cookies (e.g. GitHub \
               notifications, Gmail, internal dashboards),\n   \
             • you need to fill a form, click a button, or interact with UI,\n   \
             • web_fetch returned empty or incomplete content because the page \
               is a SPA / JS-rendered (Twitter, Notion, modern e-commerce),\n   \
             • the user explicitly asked you to *see* or *operate* the page.\n",
        ));
    }

    block.push('\n');
    block.push_str("RULES:\n");
    if has_browser {
        block.push_str(
            "  • Browser cold-start is 3-8s and may trigger bot detection on \
             Cloudflare / Akamai sites — never use it speculatively.\n",
        );
        if has_fetch {
            block.push_str(
                "  • If `web_fetch` returns substantial text, do NOT escalate to \
                 `browser` unless the user explicitly asks for visual or \
                 interactive work.\n",
            );
        }
        if has_search || has_fetch {
            block.push_str(
                "  • If `browser` navigation hits a CAPTCHA / Cloudflare challenge, \
                 IMMEDIATELY fall back to `web_search` or `web_fetch` — do not \
                 retry click.\n",
            );
        }
        block.push_str(
            "  • If the `browser` tool returns an error containing 'User has taken \
             over the browser; AI tools are paused', the human is currently \
             interacting with the page (e.g. logging in or solving a CAPTCHA).  \
             STOP issuing browser tool calls and wait for the next user message.\n",
        );
    }

    Some(block)
}

/// Public read-only view of the names this module routes between.  Kept
/// here so call sites that want to assert "the routing guide knows about
/// tool X" don't have to duplicate the literal.  Currently consumed only
/// by tests; left `pub` so external observers (harness scripts, future
/// dashboard renderers) can call it without touching `WEB_TOOL_NAMES`.
#[allow(dead_code)]
#[must_use]
pub const fn web_tool_names() -> &'static [&'static str] {
    WEB_TOOL_NAMES
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn returns_none_when_no_web_tools_registered() {
        assert!(web_tools_routing_block(&names(&[])).is_none());
        assert!(web_tools_routing_block(&names(&["bash", "file_read"])).is_none());
    }

    #[test]
    fn returns_none_when_only_one_web_tool_registered() {
        assert!(web_tools_routing_block(&names(&["web_search"])).is_none());
        assert!(web_tools_routing_block(&names(&["web_fetch"])).is_none());
        assert!(web_tools_routing_block(&names(&["browser"])).is_none());
    }

    #[test]
    fn returns_some_when_two_web_tools_registered() {
        let block = web_tools_routing_block(&names(&["web_search", "web_fetch"]))
            .expect("block should be emitted");
        assert!(block.contains("web_search"));
        assert!(block.contains("web_fetch"));
        assert!(!block.contains("**browser**"));
    }

    #[test]
    fn returns_some_with_full_routing_when_all_three_registered() {
        let block = web_tools_routing_block(&names(&["web_search", "web_fetch", "browser"]))
            .expect("block should be emitted");
        assert!(block.contains("# Web Access Tool Routing"));
        assert!(block.contains("**web_search**"));
        assert!(block.contains("**web_fetch**"));
        assert!(block.contains("**browser**"));
    }

    #[test]
    fn always_warns_about_browser_costs_when_browser_present() {
        let block = web_tools_routing_block(&names(&["web_fetch", "browser"]))
            .expect("block should be emitted");
        assert!(block.contains("Browser cold-start is 3-8s"));
        assert!(block.contains("never use it speculatively"));
    }

    #[test]
    fn surfaces_takeover_paused_handling_when_browser_present() {
        let block = web_tools_routing_block(&names(&["web_search", "browser"]))
            .expect("block should be emitted");
        assert!(block.contains("User has taken over the browser"));
        assert!(block.contains("STOP issuing browser tool calls"));
    }

    #[test]
    fn skips_takeover_warning_when_browser_not_registered() {
        let block = web_tools_routing_block(&names(&["web_search", "web_fetch"]))
            .expect("block should be emitted");
        assert!(!block.contains("User has taken over"));
    }

    #[test]
    fn web_tool_names_is_stable() {
        assert_eq!(web_tool_names(), &["web_search", "web_fetch", "browser"]);
    }

    #[test]
    fn ignores_unrelated_tool_names() {
        let block = web_tools_routing_block(&names(&[
            "bash",
            "file_read",
            "web_search",
            "memory_store",
            "browser",
        ]))
        .expect("block should be emitted");
        assert!(block.contains("**web_search**"));
        assert!(block.contains("**browser**"));
    }
}
