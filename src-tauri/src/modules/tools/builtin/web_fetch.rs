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
fn truncate_to_bytes(s: &str, max_bytes: usize) -> String {
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

            let result = if let Some(sel) = selector {
                // Convert simple CSS selector to regex pattern
                // Supports: tag, tag.class, tag#id, tag[attr=value]
                let tag_pattern = if sel.contains('.') {
                    // class selector
                    let parts: Vec<&str> = sel.split('.').collect();
                    let tag = parts[0];
                    let class = parts[1];
                    format!(r#"<{}[^>]*class="{}"[^>]*>([^<]*)"#, tag, class)
                } else if sel.contains('#') {
                    // id selector
                    let parts: Vec<&str> = sel.split('#').collect();
                    let tag = parts[0];
                    let id = parts[1];
                    format!(r#"<{}[^>]*id="{}"[^>]*>([^<]*)"#, tag, id)
                } else if sel.starts_with('<') {
                    // tag only - convert to regex
                    let tag = sel.trim_matches('<').trim_matches('>');
                    format!(r"<{}([^<]*)", tag)
                } else {
                    // Treat as generic tag pattern
                    format!(r"<{}([^<]*)", sel)
                };

                let re = Regex::new(&tag_pattern).map_err(|e| {
                    ToolError::Handler(format!("invalid selector '{}': {}", sel, e))
                })?;

                let mut results: Vec<String> = Vec::new();
                for cap in re.captures_iter(&html) {
                    if let Some(text) = cap.get(1) {
                        let content = text.as_str().trim().to_string();
                        if !content.is_empty() {
                            results.push(content);
                        }
                    }
                }
                results.join("\n")
            } else {
                // Return plain text (strip HTML tags) up to max_length bytes.
                // We truncate by UTF-8 byte count (not char count) so the result
                // always fits within the broker's max_result_size byte limit even
                // for pages with multibyte characters (CJK, emoji, etc.).
                // SAFETY: This regex pattern is a static string literal that is always valid.
                // Regex::new() can only fail with an invalid pattern, which cannot happen here.
                #[allow(clippy::expect_used)]
                let tag_re = Regex::new(r"<[^>]+>").expect("regex pattern is valid static string");
                let text = tag_re.replace_all(&html, " ");
                let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
                truncate_to_bytes(&normalized, max_length)
            };

            Ok(result)
        })
    });

    ToolEntry {
        name: "web_fetch".to_string(),
        toolset: "web".to_string(),
        description: "Fetch web page content".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector or tag name to extract specific elements"
                },
                "max_length": {
                    "type": "number",
                    "description": "Maximum content length (default: 51200)"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn web_fetch_tool_entry_has_correct_structure() {
        let entry = entry();
        assert_eq!(entry.name, "web_fetch");
        assert_eq!(entry.toolset, "web");
        assert!(!entry.disabled);
    }
}
