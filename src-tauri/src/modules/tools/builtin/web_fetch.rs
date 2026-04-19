//! Web Fetch tool - fetches web page content
//!
//! Provides HTML content fetching with optional regex-based tag extraction.

use std::net::IpAddr;
use std::sync::Arc;

use regex::Regex;
use reqwest::Client;
use url::{Host, Url};

use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Maximum text content returned to the LLM (**bytes** after HTML stripping).
/// Using a byte limit (not char limit) prevents the off-by-one where 50 000
/// Unicode chars can exceed the same value interpreted as bytes by the registry.
const MAX_CONTENT_BYTES: usize = 48 * 1024; // 49 152 bytes — safely < broker limit

/// Maximum raw HTML bytes we read from the network before aborting.
/// HTML can be 5–10× the final text size; 2MB is a generous ceiling that still
/// protects against fetching huge binary files or endless streams.
const MAX_RAW_BYTES: usize = 2 * 1024 * 1024;

/// Default timeout for web requests
#[allow(dead_code)]
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Cloud metadata IP ranges that should be blocked to prevent SSRF attacks.
const BLOCKED_IP_RANGES: &[&str] = &[
    "169.254.0.0/16", // AWS/Azure/GCP/Alibaba Cloud metadata
];

/// Hostnames that should be blocked to prevent SSRF attacks.
const BLOCKED_HOSTS: &[&str] = &[
    "metadata.google.internal", // GCP metadata
    "metadata.goog",            // GCP alternative
    "169.254.169.254",          // Cloud metadata (all providers)
    "169.254.169.253",          // Azure DNS
    "100.100.100.200",          // Alibaba Cloud metadata
];

/// Truncate a UTF-8 string to at most `max_bytes` bytes without splitting a
/// multi-byte character boundary.
pub(crate) fn truncate_to_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_owned();
    }
    // Walk char boundaries until we exceed max_bytes.
    let mut byte_end = 0;
    for (idx, _) in s.char_indices() {
        if idx > max_bytes {
            break;
        }
        byte_end = idx;
    }
    s[..byte_end].to_owned()
}

/// Check if a URL attempts to access cloud metadata endpoints (SSRF protection).
fn check_ssrf(url: &Url) -> Option<String> {
    // Check hostname blocklist
    let host = url.host_str().unwrap_or("");
    let host_lower = host.to_lowercase();

    for blocked in BLOCKED_HOSTS {
        if host_lower == blocked.to_lowercase() {
            return Some(format!(
                "SSRF blocked: '{}' is a cloud metadata endpoint",
                host
            ));
        }
        if host_lower.ends_with(&blocked.to_lowercase()) && host_lower.len() > blocked.len() {
            return Some(format!(
                "SSRF blocked: '{}' resolves to a cloud metadata endpoint",
                host
            ));
        }
    }

    // Check if host is an IP address in blocked ranges
    if let Some(Host::Ipv4(ipv4)) = url.host() {
        let ipv4_addr = ipv4.octets();
        for range in BLOCKED_IP_RANGES {
            if let Some((IpAddr::V4(network_ip), prefix_len)) = parse_cidr(range) {
                let network_octets = network_ip.octets();
                if prefix_len <= 32 {
                    let mask = if prefix_len == 0 {
                        0u32
                    } else {
                        !0u32 << (32 - prefix_len)
                    };
                    let network_u32 = u32::from_be_bytes(network_octets);
                    let ip_u32 = u32::from_be_bytes(ipv4_addr);
                    if (ip_u32 & mask) == (network_u32 & mask) {
                        return Some(format!(
                            "SSRF blocked: '{}' is in blocked IP range {}",
                            ipv4, range
                        ));
                    }
                }
            }
        }
    }

    None
}

/// Parse a CIDR notation string into (IpAddr, prefix_len).
#[allow(clippy::unnecessary_wraps)]
fn parse_cidr(cidr: &str) -> Option<(IpAddr, u8)> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let ip_str = parts[0];
    let prefix_len: u8 = parts[1].parse().ok()?;

    let ip: IpAddr = if ip_str.contains(':') {
        ip_str.parse().ok()?
    } else {
        IpAddr::V4(ip_str.parse().ok()?)
    };
    Some((ip, prefix_len))
}

/// Creates the web_fetch tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _context: SharedToolContext| {
        Box::pin(async move {
            let url_str = args
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("missing required parameter: url".to_string()))?
                .to_string();

            let selector = args.get("selector").and_then(|v| v.as_str());

            let max_length = args
                .get("max_length")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(MAX_CONTENT_BYTES);

            // Validate URL
            let url = Url::parse(&url_str)
                .map_err(|e| ToolError::Handler(format!("invalid URL '{}': {}", url_str, e)))?;

            // SSRF protection: check if URL attempts to access cloud metadata
            if let Some(blocked_msg) = check_ssrf(&url) {
                return Err(ToolError::Handler(blocked_msg));
            }

            // Create HTTP client
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
                .build()
                .map_err(|e| ToolError::Handler(format!("failed to create HTTP client: {}", e)))?;

            // Fetch content
            let response = client
                .get(url.as_str())
                .header("User-Agent", "Mozilla/5.0 (compatible; If2Ai/1.0)")
                .send()
                .await
                .map_err(|e| ToolError::Handler(format!("failed to fetch URL: {}", e)))?;

            // Only abort early for truly unreasonable sizes (binary files, etc.).
            // Normal HTML pages can be 200–500KB but strip down to 20–50KB of
            // text.  Checking Content-Length here and rejecting anything > 100KB
            // was too aggressive — removed in favour of a streaming byte limit.
            let declared_len = response.content_length().unwrap_or(0);
            if declared_len > MAX_RAW_BYTES as u64 {
                return Err(ToolError::Handler(format!(
                    "response too large ({} bytes); use a more specific URL or selector",
                    declared_len
                )));
            }

            // Read the body (capped later at MAX_RAW_BYTES by truncation).
            // We read all bytes first because reqwest 0.11 doesn't expose
            // a simple incremental read without the `stream` feature.
            let raw_bytes = response
                .bytes()
                .await
                .map_err(|e| ToolError::Handler(format!("failed to read response body: {}", e)))?;
            let capped = if raw_bytes.len() > MAX_RAW_BYTES {
                &raw_bytes[..MAX_RAW_BYTES]
            } else {
                &raw_bytes[..]
            };
            let html = String::from_utf8_lossy(capped).into_owned();

            // Phase 7C, slice 7C.10 — `mode` selects extraction strategy.
            //   selector  : CSS selector via the scraper crate
            //   article   : lightweight readability heuristic
            //   text      : strip-all-tags fallback (legacy behaviour)
            //   auto      : article when content-type is HTML, else text
            let mode = args
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("auto")
                .to_ascii_lowercase();
            let mode = if selector.is_some() && mode == "auto" {
                "selector".to_string()
            } else {
                mode
            };

            let result = match mode.as_str() {
                "selector" => extract_with_selector(&html, selector.unwrap_or(""))?,
                "article" => {
                    extract_article(&html).unwrap_or_else(|| extract_text(&html, max_length))
                }
                "text" => extract_text(&html, max_length),
                "auto" | _ => {
                    // Default: try article first; fall back to text.
                    extract_article(&html).unwrap_or_else(|| extract_text(&html, max_length))
                }
            };

            Ok(truncate_to_bytes(&result, max_length))
        })
    });

    ToolEntry {
        name: "web_fetch".to_string(),
        toolset: "web".to_string(),
        description: "Fetch web page content. \
                      mode='auto' (default) extracts the main article via a readability heuristic; \
                      mode='article' forces the heuristic; \
                      mode='selector' returns text matching a real CSS selector (Phase 7C uses scraper); \
                      mode='text' strips all tags (legacy fallback)."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch"
                },
                "mode": {
                    "type": "string",
                    "enum": ["auto", "article", "selector", "text"],
                    "description": "Extraction strategy. Default: 'auto' (article heuristic + text fallback)."
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for action='selector' (e.g. 'main article p', '.post-body')."
                },
                "max_length": {
                    "type": "number",
                    "description": "Maximum content length in UTF-8 bytes (default: 49152)."
                }
            },
            "required": ["url"]
        }),
        max_result_size: Some(64 * 1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(30),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

// ── Phase 7C, slice 7C.10: extraction strategies ─────────────────────────────

/// Strip every HTML tag and collapse whitespace.  Same algorithm as
/// pre-7C.10 `web_fetch` (legacy behaviour), retained as the safety
/// net for `mode='text'` and as fallback when `article` heuristic
/// finds nothing useful.
pub(crate) fn extract_text(html: &str, max_length: usize) -> String {
    #[allow(clippy::expect_used)]
    let tag_re = Regex::new(r"<[^>]+>").expect("regex pattern is valid static string");
    let text = tag_re.replace_all(html, " ");
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_to_bytes(&normalized, max_length)
}

/// Real CSS-selector extraction via `scraper`.  Replaces the pre-7C.10
/// regex-based selector that broke on nested elements / quoted
/// attribute values.
fn extract_with_selector(html: &str, sel: &str) -> Result<String, ToolError> {
    use scraper::{Html, Selector};
    if sel.is_empty() {
        return Err(ToolError::Handler(
            "mode='selector' requires a non-empty 'selector' field".to_string(),
        ));
    }
    let document = Html::parse_document(html);
    let parsed = Selector::parse(sel)
        .map_err(|e| ToolError::Handler(format!("invalid CSS selector '{}': {:?}", sel, e)))?;
    let mut out: Vec<String> = Vec::new();
    for el in document.select(&parsed).take(50) {
        let text: String = el.text().collect::<Vec<_>>().join(" ");
        let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !trimmed.is_empty() {
            out.push(trimmed);
        }
    }
    Ok(out.join("\n\n"))
}

/// Lightweight Readability-style article extraction.
///
/// Strategy: walk every `<article>` / `<main>` / `<section>` / `<div>`,
/// score = direct text length minus 50 × link-text length, return the
/// highest-scored node's plain text.  Returns `None` when no candidate
/// scored above zero (caller falls back to `extract_text`).
///
/// This is intentionally minimal — a full Mozilla Readability port
/// would be ~3000 lines of JS-equivalent Rust; this 80-line version
/// covers Wikipedia, blog posts, Substack, news articles, and most
/// other content sites well enough for an LLM tool.
pub(crate) fn extract_article(html: &str) -> Option<String> {
    use scraper::{Html, Selector};
    let document = Html::parse_document(html);
    // Strip noisy elements first.
    #[allow(clippy::expect_used)]
    let strip_sel = Selector::parse("script, style, nav, footer, aside, noscript")
        .expect("static selector is valid");
    let mut html_clean = html.to_string();
    for el in document.select(&strip_sel) {
        // Mark with a placeholder we can wipe via regex.
        // (scraper does not let us mutate; this is an approximation.)
        let outer = el.html();
        if !outer.is_empty() {
            html_clean = html_clean.replace(&outer, "");
        }
    }
    let cleaned = Html::parse_document(&html_clean);

    #[allow(clippy::expect_used)]
    let candidate_sel =
        Selector::parse("article, main, section, div").expect("static selector is valid");
    #[allow(clippy::expect_used)]
    let link_sel = Selector::parse("a").expect("static selector is valid");

    let mut best_score: i64 = 0;
    let mut best_text: Option<String> = None;
    for el in cleaned.select(&candidate_sel) {
        let text: String = el.text().collect::<Vec<_>>().join(" ");
        let text_len = text.chars().count() as i64;
        if text_len < 200 {
            // Ignore short navigation snippets / sidebars.
            continue;
        }
        // Bonus for class/id hint that this is article content.
        let attr_bonus = el
            .value()
            .attr("class")
            .or_else(|| el.value().attr("id"))
            .map(|s| s.to_ascii_lowercase())
            .map(|s| {
                let mut bonus = 0i64;
                for hint in ["article", "content", "main", "post", "entry", "story"] {
                    if s.contains(hint) {
                        bonus += 200;
                    }
                }
                for penalty in ["sidebar", "comment", "footer", "related", "ad"] {
                    if s.contains(penalty) {
                        bonus -= 200;
                    }
                }
                bonus
            })
            .unwrap_or(0);
        let link_text_len: i64 = el
            .select(&link_sel)
            .map(|a| a.text().collect::<String>().chars().count() as i64)
            .sum();
        let score = text_len + attr_bonus - link_text_len * 2;
        if score > best_score {
            best_score = score;
            best_text = Some(text);
        }
    }

    let raw = best_text?;
    let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn web_fetch_tool_entry_has_correct_structure() {
        let entry = entry();
        assert_eq!(entry.name, "web_fetch");
        assert_eq!(entry.toolset, "web");
        assert!(!entry.disabled);
        // Phase 7C, slice 7C.10 — schema must include `mode` enum.
        let schema = entry.input_schema.to_string();
        assert!(schema.contains("\"mode\""));
        assert!(schema.contains("\"article\""));
        assert!(schema.contains("\"selector\""));
    }

    #[test]
    fn extract_with_selector_returns_text_for_real_css() {
        let html = r#"<html><body>
            <main><p class="x">Hello</p><p class="x">World</p></main>
        </body></html>"#;
        let out = extract_with_selector(html, "p.x").unwrap();
        assert!(out.contains("Hello"));
        assert!(out.contains("World"));
    }

    #[test]
    fn extract_with_selector_rejects_invalid_selector() {
        let err = extract_with_selector("<html></html>", "$$$bogus").unwrap_err();
        assert!(matches!(err, ToolError::Handler(_)));
    }

    #[test]
    fn extract_article_picks_largest_content_node() {
        let html = r#"<html><body>
            <nav><a href='#'>Skip</a></nav>
            <article class="post-body">
                <p>This is the main story body that contains substantially more text than any other section in the document so the heuristic should pick it up clearly and easily.</p>
                <p>It even has a second paragraph with more substantive prose to push the score over the threshold of 200 characters required by extract_article.</p>
            </article>
            <aside><a href='#'>Sidebar link</a></aside>
        </body></html>"#;
        let out = extract_article(html).expect("article should be detected");
        assert!(out.contains("main story body"));
        assert!(!out.contains("Sidebar"));
    }

    #[test]
    fn extract_article_returns_none_for_text_too_short() {
        let html = "<html><body><p>Hi</p></body></html>";
        assert!(extract_article(html).is_none());
    }

    #[test]
    fn extract_text_strips_tags() {
        let html = "<html><body><p>Hello</p><script>bad()</script></body></html>";
        let out = extract_text(html, 1024);
        assert!(out.contains("Hello"));
        assert!(!out.contains("<p>"));
    }
}
