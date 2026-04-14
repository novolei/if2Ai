//! HTTP Request tool - makes generic HTTP requests
//!
//! Provides flexible HTTP request functionality with method selection.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use reqwest::Client;
use serde_json::Value;
use url::{Host, Url};

use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Default timeout for HTTP requests
#[allow(dead_code)]
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum response size: 1MB
#[allow(dead_code)]
const MAX_RESPONSE_SIZE: usize = 1024 * 1024;

/// Cloud metadata IP ranges that should be blocked to prevent SSRF attacks.
///
/// These addresses are used by cloud providers to expose instance metadata:
/// - 169.254.0.0/16: AWS, Azure, GCP, Alibaba Cloud metadata
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

/// Check if a URL attempts to access cloud metadata endpoints.
///
/// Returns an error message if the URL is blocked, None if it's safe.
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
        // Check domain suffix match (e.g., *.metadata.google.internal)
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

            // SSRF protection: check if URL attempts to access cloud metadata
            let parsed_url = Url::parse(&url)
                .map_err(|e| ToolError::Handler(format!("invalid URL '{}': {}", url, e)))?;
            if let Some(blocked_msg) = check_ssrf(&parsed_url) {
                return Err(ToolError::Handler(blocked_msg));
            }

            let url_str = parsed_url.as_str().to_string();

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
                "GET" => client.get(&url_str),
                "POST" => client.post(&url_str),
                "PUT" => client.put(&url_str),
                "DELETE" => client.delete(&url_str),
                "PATCH" => client.patch(&url_str),
                "HEAD" => client.head(&url_str),
                "OPTIONS" => client.request(reqwest::Method::OPTIONS, &url_str),
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

    #[test]
    fn test_ssrf_blocks_metadata_ip() {
        let url = Url::parse("http://169.254.169.254/latest/meta-data/").unwrap();
        assert!(check_ssrf(&url).is_some());
    }

    #[test]
    fn test_ssrf_blocks_metadata_hostname() {
        let url = Url::parse("http://metadata.google.internal/computeMetadata/v1/").unwrap();
        assert!(check_ssrf(&url).is_some());
    }

    #[test]
    fn test_ssrf_blocks_alibaba_metadata() {
        let url = Url::parse("http://100.100.100.200/latest/meta-data/").unwrap();
        assert!(check_ssrf(&url).is_some());
    }

    #[test]
    fn test_ssrf_allows_normal_url() {
        let url = Url::parse("https://www.example.com/api/data").unwrap();
        assert!(check_ssrf(&url).is_none());
    }

    #[test]
    fn test_ssrf_allows_github() {
        let url = Url::parse("https://api.github.com/repos/example").unwrap();
        assert!(check_ssrf(&url).is_none());
    }
}
