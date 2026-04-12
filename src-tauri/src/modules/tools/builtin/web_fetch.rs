//! Web Fetch tool - fetches web page content
//!
//! Provides HTML content fetching with optional regex-based tag extraction.

use std::sync::Arc;

use regex::Regex;
use reqwest::Client;
use url::Url;

use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Maximum content length: 50KB
#[allow(dead_code)]
const MAX_CONTENT_LENGTH: usize = 50 * 1024;

/// Default timeout for web requests
#[allow(dead_code)]
const DEFAULT_TIMEOUT_SECS: u64 = 30;

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
                .unwrap_or(MAX_CONTENT_LENGTH);

            // Validate URL
            let url = Url::parse(&url_str)
                .map_err(|e| ToolError::Handler(format!("invalid URL '{}': {}", url_str, e)))?;

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

            let content_length = response.content_length().unwrap_or(0);
            if content_length > (max_length * 2) as u64 {
                return Err(ToolError::Handler(format!(
                    "content length {} exceeds limit",
                    content_length
                )));
            }

            let html = response
                .text()
                .await
                .map_err(|e| ToolError::Handler(format!("failed to read response: {}", e)))?;

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
                // Return plain text (strip HTML tags) up to max_length
                let tag_re = Regex::new(r"<[^>]+>").unwrap();
                let text = tag_re.replace_all(&html, " ");
                let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
                normalized.chars().take(max_length).collect()
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
        max_result_size: Some(50 * 1024),
        timeout_secs: Some(30),
        disabled: false,
        handler,
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
