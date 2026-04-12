//! Web Search tool - searches the web for information
//!
//! Provides web search functionality using DuckDuckGo HTML.

use std::sync::Arc;

use regex::Regex;
use reqwest::Client;

use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Default timeout for web requests
#[allow(dead_code)]
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum search results
#[allow(dead_code)]
const MAX_RESULTS: usize = 10;

/// Creates the web_search tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _context: SharedToolContext| {
        Box::pin(async move {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("missing required parameter: query".to_string()))?
                .to_string();

            let max_results = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(MAX_RESULTS);

            // Build DuckDuckGo HTML search URL
            let encoded_query = urlencoding::encode(&query);
            let search_url = format!("https://html.duckduckgo.com/html/?q={}", encoded_query);

            // Create HTTP client
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
                .build()
                .map_err(|e| ToolError::Handler(format!("failed to create HTTP client: {}", e)))?;

            // Fetch search results
            let response = client
                .get(&search_url)
                .header("User-Agent", "Mozilla/5.0 (compatible; If2Ai/1.0)")
                .send()
                .await
                .map_err(|e| {
                    ToolError::Handler(format!("failed to fetch search results: {}", e))
                })?;

            let html = response
                .text()
                .await
                .map_err(|e| ToolError::Handler(format!("failed to read response: {}", e)))?;

            // Parse results using regex
            // DuckDuckGo HTML results have the pattern: <a class="result__a" href="URL">TITLE</a>
            let result_re = Regex::new(r#"<a class="result__a" href="([^"]+)">([^<]+)</a>"#)
                .map_err(|e| ToolError::Handler(format!("invalid regex: {}", e)))?;

            let mut results: Vec<String> = Vec::new();
            for (idx, cap) in result_re.captures_iter(&html).enumerate() {
                if idx >= max_results {
                    break;
                }
                if let (Some(href), Some(title)) = (cap.get(1), cap.get(2)) {
                    results.push(format!("{} - {}", title.as_str().trim(), href.as_str()));
                }
            }

            Ok(results.join("\n"))
        })
    });

    ToolEntry {
        name: "web_search".to_string(),
        toolset: "web".to_string(),
        description: "Search the web for information".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "max_results": {
                    "type": "number",
                    "description": "Maximum number of results (default: 10)"
                }
            },
            "required": ["query"]
        }),
        max_result_size: Some(10 * 1024),
        timeout_secs: Some(30),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn web_search_tool_entry_has_correct_structure() {
        let entry = entry();
        assert_eq!(entry.name, "web_search");
        assert_eq!(entry.toolset, "web");
        assert!(!entry.disabled);
    }
}
