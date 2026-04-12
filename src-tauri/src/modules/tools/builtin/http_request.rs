//! HTTP Request tool - makes generic HTTP requests
//!
//! Provides flexible HTTP request functionality with method selection.

use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Client;
use serde_json::Value;

use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Default timeout for HTTP requests
#[allow(dead_code)]
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum response size: 1MB
#[allow(dead_code)]
const MAX_RESPONSE_SIZE: usize = 1024 * 1024;

/// Creates the http_request tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _context: SharedToolContext| {
        Box::pin(async move {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("missing required parameter: url".to_string()))?
                .to_string();

            let method = args
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("GET")
                .to_uppercase();

            let headers: Option<HashMap<String, String>> =
                args.get("headers").and_then(|v| v.as_object()).map(|h| {
                    h.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect()
                });

            let body = args.get("body").and_then(|v| v.as_str());

            // Create HTTP client
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
                .build()
                .map_err(|e| ToolError::Handler(format!("failed to create HTTP client: {}", e)))?;

            let mut request = match method.as_str() {
                "GET" => client.get(&url),
                "POST" => client.post(&url),
                "PUT" => client.put(&url),
                "DELETE" => client.delete(&url),
                "PATCH" => client.patch(&url),
                "HEAD" => client.head(&url),
                "OPTIONS" => client.request(reqwest::Method::OPTIONS, &url),
                _ => {
                    return Err(ToolError::Handler(format!(
                        "unsupported HTTP method: {}",
                        method
                    )));
                }
            };

            // Add headers
            if let Some(h) = headers {
                for (key, value) in h {
                    request = request.header(&key, &value);
                }
            }

            // Add body
            if let Some(b) = body {
                request = request.body(b.to_string());
            }

            // Send request
            let response = request
                .send()
                .await
                .map_err(|e| ToolError::Handler(format!("HTTP request failed: {}", e)))?;

            let status = response.status().as_u16();
            let resp_headers: Value = response
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        Value::String(v.to_str().unwrap_or("").to_string()),
                    )
                })
                .collect();

            let body = response
                .text()
                .await
                .map_err(|e| ToolError::Handler(format!("failed to read response body: {}", e)))?;

            let truncated_body = if body.len() > MAX_RESPONSE_SIZE {
                format!(
                    "{}...(truncated {} bytes)",
                    &body[..MAX_RESPONSE_SIZE],
                    body.len() - MAX_RESPONSE_SIZE
                )
            } else {
                body
            };

            Ok(serde_json::json!({
                "status": status,
                "headers": resp_headers,
                "body": truncated_body
            })
            .to_string())
        })
    });

    ToolEntry {
        name: "http_request".to_string(),
        toolset: "web".to_string(),
        description: "Make HTTP requests".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to request"
                },
                "method": {
                    "type": "string",
                    "description": "HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)"
                },
                "headers": {
                    "type": "object",
                    "description": "HTTP headers"
                },
                "body": {
                    "type": "string",
                    "description": "Request body"
                }
            },
            "required": ["url"]
        }),
        max_result_size: Some(1024 * 1024),
        timeout_secs: Some(30),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn http_request_tool_entry_has_correct_structure() {
        let entry = entry();
        assert_eq!(entry.name, "http_request");
        assert_eq!(entry.toolset, "web");
        assert!(!entry.disabled);
    }
}
