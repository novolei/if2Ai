//! Web Search tool — searches the web for information.
//!
//! Provider priority:
//!   1. First enabled provider in `~/.if2ai/web-search-config.json`
//!      (Tavily → SearXNG → Brave → Serper → Gemini)
//!   2. DuckDuckGo Instant Answer JSON API (no key required)
//!   3. DuckDuckGo Lite HTML scraping fallback
//!
//! When no provider is configured the tool still works via DDG but returns
//! a notice so the frontend can prompt the user to add an API key.

use std::sync::Arc;

use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

use crate::modules::tools::builtin::web_search_config;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

const DEFAULT_TIMEOUT_SECS: u64 = 20;
const MAX_RESULTS: usize = 10;
/// Max raw bytes read from DDG Lite HTML fallback page.
const FALLBACK_MAX_RAW: usize = 512 * 1024;
/// Tag prepended to results when no API key is configured.
const NO_KEY_NOTICE: &str = "[web_search: 当前使用 DuckDuckGo 免费搜索，结果质量有限。\
                              建议在「设置 → Web Search」中配置 Tavily / Brave 等服务商以获取更准确的结果。]\n\n";

// ── DDG structs ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DdgResponse {
    #[serde(rename = "Heading", default)]
    heading: String,
    #[serde(rename = "AbstractText", default)]
    abstract_text: String,
    #[serde(rename = "AbstractURL", default)]
    abstract_url: String,
    #[serde(rename = "AbstractSource", default)]
    abstract_source: String,
    #[serde(rename = "OfficialWebsite", default)]
    official_website: String,
    #[serde(rename = "Results", default)]
    results: Vec<DdgResult>,
    #[serde(rename = "RelatedTopics", default)]
    related_topics: Vec<DdgRelated>,
}

#[derive(Debug, Deserialize)]
struct DdgResult {
    #[serde(rename = "Text", default)]
    text: String,
    #[serde(rename = "FirstURL", default)]
    first_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DdgRelated {
    Item {
        #[serde(rename = "Text", default)]
        text: String,
        #[serde(rename = "FirstURL", default)]
        first_url: String,
    },
    Category {
        #[serde(rename = "Topics", default)]
        topics: Vec<DdgResult>,
    },
}

// ── Tavily structs ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyResult>,
    #[serde(default)]
    answer: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    score: f64,
}

// ── Brave structs ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct BraveResponse {
    web: Option<BraveWeb>,
}

#[derive(Debug, Deserialize)]
struct BraveWeb {
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
}

// ── Serper structs ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SerperResponse {
    #[serde(rename = "organic", default)]
    organic: Vec<SerperResult>,
    #[serde(rename = "answerBox")]
    answer_box: Option<SerperAnswerBox>,
}

#[derive(Debug, Deserialize)]
struct SerperResult {
    title: String,
    link: String,
    #[serde(default)]
    snippet: String,
}

#[derive(Debug, Deserialize)]
struct SerperAnswerBox {
    #[serde(default)]
    snippet: String,
}

// ── SearXNG structs ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SearxResponse {
    results: Vec<SearxResult>,
}

#[derive(Debug, Deserialize)]
struct SearxResult {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
}

// ── DDG formatters ───────────────────────────────────────────────────────────

fn format_ddg_response(resp: &DdgResponse, max_results: usize) -> String {
    let mut lines: Vec<String> = Vec::new();

    if !resp.heading.is_empty() && !resp.abstract_text.is_empty() {
        lines.push(format!("## {}", resp.heading));
        lines.push(resp.abstract_text.clone());
        if !resp.abstract_url.is_empty() {
            lines.push(format!(
                "Source: {} ({})",
                resp.abstract_source, resp.abstract_url
            ));
        }
        lines.push(String::new());
    }

    if !resp.official_website.is_empty() {
        lines.push(format!("Official website: {}", resp.official_website));
    }

    let mut result_count = 0;

    for r in &resp.results {
        if result_count >= max_results {
            break;
        }
        if !r.text.is_empty() {
            lines.push(format!("- {} ({})", r.text, r.first_url));
            result_count += 1;
        }
    }

    for related in &resp.related_topics {
        if result_count >= max_results {
            break;
        }
        match related {
            DdgRelated::Item { text, first_url }
                if !text.is_empty() && !first_url.contains("duckduckgo.com/c/") =>
            {
                lines.push(format!("- {} ({})", text, first_url));
                result_count += 1;
            }
            DdgRelated::Category { topics } => {
                for t in topics {
                    if result_count >= max_results {
                        break;
                    }
                    if !t.text.is_empty() && !t.first_url.contains("duckduckgo.com/c/") {
                        lines.push(format!("- {} ({})", t.text, t.first_url));
                        result_count += 1;
                    }
                }
            }
            _ => {}
        }
    }

    if lines.is_empty() {
        return "No results found. The query may be too specific or about a very new/unknown \
                topic. Try web_fetch on a specific URL directly."
            .to_string();
    }

    lines.join("\n")
}

/// DDG Lite HTML scraping fallback.
async fn fetch_ddg_html_fallback(
    client: &Client,
    query: &str,
    max_results: usize,
) -> Result<String, ToolError> {
    let encoded = urlencoding::encode(query);
    let url = format!("https://lite.duckduckgo.com/lite/?q={}&kl=wt-wt", encoded);
    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .header("Accept", "text/html,application/xhtml+xml")
        .send()
        .await
        .map_err(|e| ToolError::Handler(e.to_string()))?;

    let raw_bytes = response
        .bytes()
        .await
        .map_err(|e| ToolError::Handler(e.to_string()))?;
    let capped = if raw_bytes.len() > FALLBACK_MAX_RAW {
        &raw_bytes[..FALLBACK_MAX_RAW]
    } else {
        &raw_bytes[..]
    };
    let html = String::from_utf8_lossy(capped);

    let re = Regex::new(
        r#"(?i)<a[^>]+class="[^"]*result[^"]*"[^>]+href="([^"]+)"[^>]*>([^<]{3,120})</a>"#,
    )
    .map_err(|e| ToolError::Handler(e.to_string()))?;

    let mut lines: Vec<String> = Vec::new();
    for cap in re.captures_iter(&html) {
        if lines.len() >= max_results {
            break;
        }
        if let (Some(href), Some(title)) = (cap.get(1), cap.get(2)) {
            let href = href.as_str();
            let title = title.as_str().trim();
            if href.starts_with("http") && !href.contains("duckduckgo.com") {
                lines.push(format!("- {} ({})", title, href));
            }
        }
    }

    Ok(lines.join("\n"))
}

// ── Per-provider search functions ────────────────────────────────────────────

async fn search_tavily(
    client: &Client,
    api_key: &str,
    query: &str,
    max_results: usize,
) -> Result<String, ToolError> {
    let body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "max_results": max_results,
        "search_depth": "basic",
        "include_answer": true
    });

    let response = client
        .post("https://api.tavily.com/search")
        .json(&body)
        .send()
        .await
        .map_err(|e| ToolError::Handler(format!("Tavily request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let msg = response.text().await.unwrap_or_else(|_| status.to_string());
        return Err(ToolError::Handler(format!(
            "Tavily error {}: {}",
            status, msg
        )));
    }

    let resp: TavilyResponse = response
        .json()
        .await
        .map_err(|e| ToolError::Handler(format!("Tavily parse error: {}", e)))?;

    let mut lines: Vec<String> = Vec::new();
    if let Some(answer) = resp.answer.filter(|a| !a.is_empty()) {
        lines.push(format!("**Answer:** {}", answer));
        lines.push(String::new());
    }

    let mut sorted = resp.results;
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for r in sorted.into_iter().take(max_results) {
        let snippet = if r.content.is_empty() {
            String::new()
        } else {
            let s = r.content.chars().take(120).collect::<String>();
            format!(" — {}", s)
        };
        lines.push(format!("- **{}**{} ({})", r.title, snippet, r.url));
    }

    if lines.is_empty() {
        return Ok("No results found.".to_string());
    }
    Ok(lines.join("\n"))
}

async fn search_brave(
    client: &Client,
    api_key: &str,
    query: &str,
    max_results: usize,
) -> Result<String, ToolError> {
    let encoded = urlencoding::encode(query);
    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
        encoded, max_results
    );

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip")
        .header("X-Subscription-Token", api_key)
        .send()
        .await
        .map_err(|e| ToolError::Handler(format!("Brave request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(ToolError::Handler(format!("Brave error: {}", status)));
    }

    let resp: BraveResponse = response
        .json()
        .await
        .map_err(|e| ToolError::Handler(format!("Brave parse error: {}", e)))?;

    let results = resp.web.map(|w| w.results).unwrap_or_default();
    if results.is_empty() {
        return Ok("No results found.".to_string());
    }

    let lines: Vec<String> = results
        .into_iter()
        .take(max_results)
        .map(|r| {
            let snippet = if r.description.is_empty() {
                String::new()
            } else {
                format!(" — {}", r.description.chars().take(120).collect::<String>())
            };
            format!("- **{}**{} ({})", r.title, snippet, r.url)
        })
        .collect();
    Ok(lines.join("\n"))
}

async fn search_serper(
    client: &Client,
    api_key: &str,
    query: &str,
    max_results: usize,
) -> Result<String, ToolError> {
    let body = serde_json::json!({
        "q": query,
        "num": max_results
    });

    let response = client
        .post("https://google.serper.dev/search")
        .header("X-API-KEY", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ToolError::Handler(format!("Serper request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(ToolError::Handler(format!("Serper error: {}", status)));
    }

    let resp: SerperResponse = response
        .json()
        .await
        .map_err(|e| ToolError::Handler(format!("Serper parse error: {}", e)))?;

    let mut lines: Vec<String> = Vec::new();
    if let Some(ab) = resp.answer_box.filter(|a| !a.snippet.is_empty()) {
        lines.push(format!("**Answer:** {}", ab.snippet));
        lines.push(String::new());
    }

    for r in resp.organic.into_iter().take(max_results) {
        let snippet = if r.snippet.is_empty() {
            String::new()
        } else {
            format!(" — {}", r.snippet.chars().take(120).collect::<String>())
        };
        lines.push(format!("- **{}**{} ({})", r.title, snippet, r.link));
    }

    if lines.is_empty() {
        return Ok("No results found.".to_string());
    }
    Ok(lines.join("\n"))
}

async fn search_searxng(
    client: &Client,
    base_url: &str,
    query: &str,
    max_results: usize,
) -> Result<String, ToolError> {
    let base = base_url.trim_end_matches('/');
    let encoded = urlencoding::encode(query);
    let url = format!(
        "{}/search?q={}&format=json&categories=general",
        base, encoded
    );

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| ToolError::Handler(format!("SearXNG request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(ToolError::Handler(format!("SearXNG error: {}", status)));
    }

    let resp: SearxResponse = response
        .json()
        .await
        .map_err(|e| ToolError::Handler(format!("SearXNG parse error: {}", e)))?;

    let lines: Vec<String> = resp
        .results
        .into_iter()
        .take(max_results)
        .map(|r| {
            let snippet = if r.content.is_empty() {
                String::new()
            } else {
                format!(" — {}", r.content.chars().take(120).collect::<String>())
            };
            format!("- **{}**{} ({})", r.title, snippet, r.url)
        })
        .collect();

    if lines.is_empty() {
        return Ok("No results found.".to_string());
    }
    Ok(lines.join("\n"))
}

// ── Tool entry ────────────────────────────────────────────────────────────────

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

            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
                .user_agent("Mozilla/5.0 (compatible; If2Ai/1.0)")
                .build()
                .map_err(|e| ToolError::Handler(format!("HTTP client build failed: {}", e)))?;

            // Try configured providers first.
            if let Some(provider) = web_search_config::active_provider() {
                let result = match provider.id.as_str() {
                    "tavily" => {
                        if let Some(key) = &provider.api_key {
                            search_tavily(&client, key, &query, max_results).await
                        } else {
                            Err(ToolError::Handler("Tavily: missing api_key".into()))
                        }
                    }
                    "brave" => {
                        if let Some(key) = &provider.api_key {
                            search_brave(&client, key, &query, max_results).await
                        } else {
                            Err(ToolError::Handler("Brave: missing api_key".into()))
                        }
                    }
                    "serper" => {
                        if let Some(key) = &provider.api_key {
                            search_serper(&client, key, &query, max_results).await
                        } else {
                            Err(ToolError::Handler("Serper: missing api_key".into()))
                        }
                    }
                    "searxng" => {
                        let base = provider.base_url.as_deref().unwrap_or("https://searx.be");
                        search_searxng(&client, base, &query, max_results).await
                    }
                    other => Err(ToolError::Handler(format!("Unknown provider: {}", other))),
                };

                match result {
                    Ok(text) => return Ok(text),
                    Err(e) => {
                        tracing::warn!(
                            "web_search provider '{}' failed: {}; falling back to DDG",
                            provider.id,
                            e
                        );
                    }
                }
            }

            // DDG Instant Answer JSON API (no key required).
            let no_key_mode = web_search_config::active_provider().is_none();
            let encoded_query = urlencoding::encode(&query);
            let api_url = format!(
                "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
                encoded_query
            );

            let response =
                client.get(&api_url).send().await.map_err(|e| {
                    ToolError::Handler(format!("web_search DDG request failed: {}", e))
                })?;

            if !response.status().is_success() {
                return Err(ToolError::Handler(format!(
                    "web_search DDG HTTP error: {}",
                    response.status()
                )));
            }

            let body = response
                .text()
                .await
                .map_err(|e| ToolError::Handler(format!("failed to read DDG response: {}", e)))?;

            let ddg: DdgResponse = serde_json::from_str(&body)
                .map_err(|e| ToolError::Handler(format!("failed to parse DDG response: {}", e)))?;

            let primary = format_ddg_response(&ddg, max_results);

            let result = if primary.starts_with("No results found") {
                // Try DDG Lite HTML fallback for real-time / niche queries.
                match fetch_ddg_html_fallback(&client, &query, max_results).await {
                    Ok(fb) if !fb.is_empty() => fb,
                    _ => primary,
                }
            } else {
                primary
            };

            if no_key_mode {
                Ok(format!("{}{}", NO_KEY_NOTICE, result))
            } else {
                Ok(result)
            }
        })
    });

    ToolEntry {
        name: "web_search".to_string(),
        toolset: "web".to_string(),
        description: "Search the web for information. Uses configured provider (Tavily/Brave/\
                      Serper/SearXNG) or falls back to DuckDuckGo when no provider is set. \
                      For brand-new or niche topics with no results, use web_fetch on a \
                      specific URL directly."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (English yields best results for DDG fallback)"
                },
                "max_results": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            },
            "required": ["query"]
        }),
        max_result_size: Some(16 * 1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(25),
        disabled: false,
        handler,
        multimodal_handler: None,
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

    #[test]
    fn format_ddg_response_empty_returns_hint() {
        let resp = DdgResponse {
            heading: String::new(),
            abstract_text: String::new(),
            abstract_url: String::new(),
            abstract_source: String::new(),
            official_website: String::new(),
            results: vec![],
            related_topics: vec![],
        };
        let out = format_ddg_response(&resp, 10);
        assert!(out.contains("No results found"));
    }

    #[test]
    fn format_ddg_response_with_abstract() {
        let resp = DdgResponse {
            heading: "Apple".to_string(),
            abstract_text: "Apple Inc. is a technology company.".to_string(),
            abstract_url: "https://en.wikipedia.org/wiki/Apple".to_string(),
            abstract_source: "Wikipedia".to_string(),
            official_website: "https://apple.com".to_string(),
            results: vec![DdgResult {
                text: "Official site".to_string(),
                first_url: "https://apple.com".to_string(),
            }],
            related_topics: vec![],
        };
        let out = format_ddg_response(&resp, 10);
        assert!(out.contains("Apple"));
        assert!(out.contains("apple.com"));
    }
}
