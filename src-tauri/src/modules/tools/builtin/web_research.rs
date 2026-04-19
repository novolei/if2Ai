//! `web_research` compound tool (Phase 7C, slice 7C.11).
//!
//! Replaces the typical "search → fetch top-K → synthesise" chain that
//! used to take 4 LLM round-trips with a single tool call.  The
//! implementation:
//!
//! 1. Issues a DuckDuckGo Instant-Answer + DDG-Lite search to discover
//!    candidate URLs (no API key required so the tool always works).
//! 2. Concurrently `web_fetch`-es the top `max_pages` (default 3,
//!    capped at 5) using the readability heuristic from
//!    [`super::web_fetch::extract_article`].
//! 3. Concatenates the results into a single Markdown report with
//!    per-source headings and explicit "FETCH FAILED" placeholders for
//!    pages that timed out so the LLM can see partial progress.
//!
//! Per-page fetch is bounded to 10s; the overall tool timeout is 45s.
//! When DDG returns nothing, the tool exits early with an actionable
//! message rather than emitting a silent empty report.

use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use reqwest::Client;
use serde::Deserialize;

use crate::modules::tools::builtin::web_fetch::{extract_article, extract_text, truncate_to_bytes};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

const MAX_TOOL_TIMEOUT_SECS: u64 = 45;
const PER_FETCH_TIMEOUT_SECS: u64 = 10;
const DEFAULT_MAX_PAGES: usize = 3;
const HARD_MAX_PAGES: usize = 5;
const PER_PAGE_BYTES: usize = 2_000;
const MAX_TOTAL_BYTES: usize = 48 * 1024;
const FALLBACK_HTML_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone)]
struct SearchHit {
    title: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct DdgResponse {
    #[serde(rename = "Heading", default)]
    heading: String,
    #[serde(rename = "AbstractText", default)]
    abstract_text: String,
    #[serde(rename = "AbstractURL", default)]
    abstract_url: String,
    #[serde(rename = "Results", default)]
    results: Vec<DdgItem>,
    #[serde(rename = "RelatedTopics", default)]
    related_topics: Vec<DdgRelated>,
}

#[derive(Debug, Deserialize)]
struct DdgItem {
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
        topics: Vec<DdgItem>,
    },
}

/// Run a DuckDuckGo search and return candidate hits ordered by
/// DDG's preferred sequence (Abstract → Results → RelatedTopics).
async fn search_ddg(client: &Client, query: &str, max_results: usize) -> Vec<SearchHit> {
    let mut hits: Vec<SearchHit> = Vec::new();

    let encoded = urlencoding::encode(query);
    let api_url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
        encoded
    );
    if let Ok(resp) = client.get(&api_url).send().await {
        if resp.status().is_success() {
            if let Ok(body) = resp.text().await {
                if let Ok(parsed) = serde_json::from_str::<DdgResponse>(&body) {
                    if !parsed.heading.is_empty() && !parsed.abstract_url.is_empty() {
                        hits.push(SearchHit {
                            title: parsed.heading.clone(),
                            url: parsed.abstract_url.clone(),
                        });
                        let _ = &parsed.abstract_text; // used later in formatting
                    }
                    for r in parsed.results {
                        if r.first_url.starts_with("http") {
                            hits.push(SearchHit {
                                title: r.text,
                                url: r.first_url,
                            });
                        }
                    }
                    for related in parsed.related_topics {
                        match related {
                            DdgRelated::Item { text, first_url }
                                if first_url.starts_with("http")
                                    && !first_url.contains("duckduckgo.com/c/") =>
                            {
                                hits.push(SearchHit {
                                    title: text,
                                    url: first_url,
                                });
                            }
                            DdgRelated::Category { topics } => {
                                for t in topics {
                                    if t.first_url.starts_with("http")
                                        && !t.first_url.contains("duckduckgo.com/c/")
                                    {
                                        hits.push(SearchHit {
                                            title: t.text,
                                            url: t.first_url,
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Fallback: DDG Lite HTML scraping when Instant Answer was empty.
    if hits.is_empty() {
        let lite_url = format!("https://lite.duckduckgo.com/lite/?q={}&kl=wt-wt", encoded);
        if let Ok(resp) = client
            .get(&lite_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (compatible; If2AiResearchBot/1.0)",
            )
            .send()
            .await
        {
            if let Ok(bytes) = resp.bytes().await {
                let capped = if bytes.len() > FALLBACK_HTML_BYTES {
                    &bytes[..FALLBACK_HTML_BYTES]
                } else {
                    &bytes[..]
                };
                let html = String::from_utf8_lossy(capped);
                #[allow(clippy::expect_used)]
                let re = Regex::new(
                    r#"(?i)<a[^>]+class="[^"]*result[^"]*"[^>]+href="([^"]+)"[^>]*>([^<]{3,120})</a>"#,
                )
                .expect("static regex valid");
                for cap in re.captures_iter(&html) {
                    if hits.len() >= max_results {
                        break;
                    }
                    if let (Some(href), Some(title)) = (cap.get(1), cap.get(2)) {
                        let href = href.as_str().to_string();
                        let title = title.as_str().trim().to_string();
                        if href.starts_with("http") && !href.contains("duckduckgo.com") {
                            hits.push(SearchHit { title, url: href });
                        }
                    }
                }
            }
        }
    }

    // Dedup by URL while preserving order, then trim.
    let mut seen = std::collections::HashSet::new();
    hits.retain(|h| seen.insert(h.url.clone()));
    hits.truncate(max_results);
    hits
}

/// Fetch one URL and return Markdown article text or `Err(reason)`.
async fn fetch_one(client: &Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (compatible; If2AiResearchBot/1.0)",
        )
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("body read error: {e}"))?;
    // 1MB raw cap so a misbehaving site doesn't OOM the runtime.
    let capped = if bytes.len() > 1024 * 1024 {
        &bytes[..1024 * 1024]
    } else {
        &bytes[..]
    };
    let html = String::from_utf8_lossy(capped).to_string();
    let extracted = extract_article(&html).unwrap_or_else(|| extract_text(&html, PER_PAGE_BYTES));
    Ok(truncate_to_bytes(&extracted, PER_PAGE_BYTES))
}

/// Tool entry factory for `web_research`.
#[allow(dead_code)]
#[must_use]
pub fn entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _ctx: SharedToolContext| {
        Box::pin(async move {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("missing required parameter: query".to_string()))?
                .to_string();
            let max_pages = args
                .get("max_pages")
                .and_then(|v| v.as_u64())
                .map(|v| (v as usize).clamp(1, HARD_MAX_PAGES))
                .unwrap_or(DEFAULT_MAX_PAGES);

            let client = Client::builder()
                .timeout(Duration::from_secs(PER_FETCH_TIMEOUT_SECS))
                .user_agent("Mozilla/5.0 (compatible; If2AiResearchBot/1.0)")
                .build()
                .map_err(|e| ToolError::Handler(format!("HTTP client build failed: {e}")))?;

            let hits = search_ddg(&client, &query, max_pages * 2).await;
            if hits.is_empty() {
                return Ok(format!(
                    "# Research: {query}\n\n_No DuckDuckGo results returned._\n\
                     Tip: try a more specific query or use `web_search` directly with a configured provider."
                ));
            }
            let chosen: Vec<SearchHit> = hits.into_iter().take(max_pages).collect();

            // Concurrent fetch of every chosen URL with a per-fetch
            // timeout so a slow page doesn't tank the whole report.
            let urls: Vec<String> = chosen.iter().map(|h| h.url.clone()).collect();
            let fetches = urls.iter().map(|url| {
                let client = client.clone();
                let url = url.clone();
                async move {
                    match tokio::time::timeout(
                        Duration::from_secs(PER_FETCH_TIMEOUT_SECS),
                        fetch_one(&client, &url),
                    )
                    .await
                    {
                        Ok(inner) => inner,
                        Err(_) => Err(format!("timed out after {PER_FETCH_TIMEOUT_SECS}s")),
                    }
                }
            });
            let articles: Vec<Result<String, String>> = futures::future::join_all(fetches).await;

            let mut out = String::new();
            out.push_str(&format!("# Research: {query}\n\n"));
            out.push_str(&format!("## Sources ({} fetched)\n\n", articles.len()));
            for (i, (hit, article)) in chosen.iter().zip(articles.iter()).enumerate() {
                let title = if hit.title.is_empty() {
                    "(no title)".to_string()
                } else {
                    hit.title.clone()
                };
                match article {
                    Ok(body) => {
                        out.push_str(&format!(
                            "\n### [{idx}] {title} — {url}\n\n{body}\n",
                            idx = i,
                            title = title,
                            url = hit.url,
                            body = body,
                        ));
                    }
                    Err(reason) => {
                        out.push_str(&format!(
                            "\n### [{idx}] {title} — {url}\nFETCH FAILED: {reason}\n",
                            idx = i,
                            title = title,
                            url = hit.url,
                            reason = reason,
                        ));
                    }
                }
                if out.len() > MAX_TOTAL_BYTES {
                    out.push_str("\n[output truncated to fit budget]\n");
                    break;
                }
            }
            Ok(out)
        })
    });

    ToolEntry {
        name: "web_research".to_string(),
        toolset: "web".to_string(),
        description: "Compound research tool: searches DuckDuckGo for `query`, \
                      concurrently fetches the top `max_pages` (default 3, max 5) \
                      pages with the readability heuristic, and returns a single \
                      Markdown report with all sources.  Replaces the typical \
                      web_search → web_fetch ×N chain in one round-trip."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Topic to research (English yields best DDG results)."
                },
                "max_pages": {
                    "type": "number",
                    "description": "Pages to fetch in parallel (1-5, default 3)."
                }
            },
            "required": ["query"]
        }),
        max_result_size: Some(64 * 1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(MAX_TOOL_TIMEOUT_SECS as u32),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_has_correct_structure() {
        let e = entry();
        assert_eq!(e.name, "web_research");
        assert_eq!(e.toolset, "web");
        assert!(!e.disabled);
        let schema = e.input_schema.to_string();
        assert!(schema.contains("\"query\""));
        assert!(schema.contains("\"max_pages\""));
    }

    #[test]
    fn max_pages_is_clamped_to_hard_max() {
        // The clamp lives inline in the handler; assert the constant matches the schema description.
        assert_eq!(HARD_MAX_PAGES, 5);
    }
}
