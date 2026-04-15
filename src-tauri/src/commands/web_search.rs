//! Tauri commands for managing web search provider configuration.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::modules::tools::builtin::web_search_config::{self, WebSearchProvider};

/// Serialisable provider entry returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    pub name: String,
    /// Redacted key shown in the UI (e.g. "tvly-abc…xyz").
    pub key_preview: Option<String>,
    /// Base URL for self-hosted instances like SearXNG.
    pub base_url: Option<String>,
    pub enabled: bool,
}

impl ProviderEntry {
    fn from_provider(p: &WebSearchProvider) -> Self {
        let key_preview = p.api_key.as_deref().map(redact_key);
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            key_preview,
            base_url: p.base_url.clone(),
            enabled: p.enabled,
        }
    }
}

/// Returns the last 4 chars of a key and redacts the middle.
fn redact_key(key: &str) -> String {
    if key.len() <= 8 {
        return "•".repeat(key.len());
    }
    let prefix: String = key.chars().take(8).collect();
    let suffix: String = key
        .chars()
        .rev()
        .take(3)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{}…{}", prefix, suffix)
}

/// Return all configured providers (keys are redacted).
#[tauri::command]
pub fn get_web_search_config() -> Vec<ProviderEntry> {
    web_search_config::load()
        .providers
        .iter()
        .map(ProviderEntry::from_provider)
        .collect()
}

/// Request body for `set_web_search_config`.
#[derive(Debug, Deserialize)]
pub struct SetProviderRequest {
    pub id: String,
    pub name: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub enabled: Option<bool>,
}

/// Add or update a provider entry.  If `id` already exists the entry is
/// replaced; otherwise it is appended.
#[tauri::command]
pub fn upsert_web_search_provider(
    provider: SetProviderRequest,
) -> Result<Vec<ProviderEntry>, String> {
    let mut cfg = web_search_config::load();

    let new_entry = WebSearchProvider {
        id: provider.id.clone(),
        name: provider.name.clone(),
        api_key: provider.api_key,
        base_url: provider.base_url,
        enabled: provider.enabled.unwrap_or(true),
    };

    if let Some(existing) = cfg.providers.iter_mut().find(|p| p.id == provider.id) {
        *existing = new_entry;
    } else {
        cfg.providers.push(new_entry);
    }

    web_search_config::save(&cfg)?;
    Ok(cfg
        .providers
        .iter()
        .map(ProviderEntry::from_provider)
        .collect())
}

/// Remove a provider by id.
#[tauri::command]
pub fn remove_web_search_provider(id: String) -> Result<Vec<ProviderEntry>, String> {
    let mut cfg = web_search_config::load();
    cfg.providers.retain(|p| p.id != id);
    web_search_config::save(&cfg)?;
    Ok(cfg
        .providers
        .iter()
        .map(ProviderEntry::from_provider)
        .collect())
}

/// Reorder providers by supplying the new ordered list of ids.
#[tauri::command]
pub fn reorder_web_search_providers(
    ordered_ids: Vec<String>,
) -> Result<Vec<ProviderEntry>, String> {
    let mut cfg = web_search_config::load();
    let mut new_order: Vec<WebSearchProvider> = Vec::with_capacity(cfg.providers.len());
    for id in &ordered_ids {
        if let Some(idx) = cfg.providers.iter().position(|p| &p.id == id) {
            new_order.push(cfg.providers.remove(idx));
        }
    }
    // Append any that weren't mentioned in ordered_ids.
    new_order.append(&mut cfg.providers);
    cfg.providers = new_order;
    web_search_config::save(&cfg)?;
    Ok(cfg
        .providers
        .iter()
        .map(ProviderEntry::from_provider)
        .collect())
}

/// Validate an API key / base URL for the given provider by making a live
/// test request.  Returns `Ok(())` on success or a human-readable error.
#[tauri::command]
pub async fn validate_web_search_key(
    provider_id: String,
    api_key: Option<String>,
    base_url: Option<String>,
) -> Result<String, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; If2Ai/1.0)")
        .build()
        .map_err(|e| e.to_string())?;

    match provider_id.as_str() {
        "tavily" => {
            let key = api_key.ok_or("Tavily requires an API key")?;
            let body = serde_json::json!({
                "api_key": key,
                "query": "test",
                "max_results": 1,
                "search_depth": "basic"
            });
            let resp = client
                .post("https://api.tavily.com/search")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("连接 Tavily 失败: {}", e))?;
            if resp.status().is_success() {
                Ok("Tavily API Key 验证成功".to_string())
            } else {
                let status = resp.status().as_u16();
                let msg = resp.text().await.unwrap_or_default();
                Err(format!("Tavily 返回 {}: {}", status, msg))
            }
        }
        "brave" => {
            let key = api_key.ok_or("Brave 需要 API Key")?;
            let resp = client
                .get("https://api.search.brave.com/res/v1/web/search?q=test&count=1")
                .header("Accept", "application/json")
                .header("Accept-Encoding", "gzip")
                .header("X-Subscription-Token", &key)
                .send()
                .await
                .map_err(|e| format!("连接 Brave 失败: {}", e))?;
            if resp.status().is_success() {
                Ok("Brave API Key 验证成功".to_string())
            } else {
                Err(format!("Brave 返回 {}", resp.status()))
            }
        }
        "serper" => {
            let key = api_key.ok_or("Serper 需要 API Key")?;
            let resp = client
                .post("https://google.serper.dev/search")
                .header("X-API-KEY", &key)
                .json(&serde_json::json!({"q": "test", "num": 1}))
                .send()
                .await
                .map_err(|e| format!("连接 Serper 失败: {}", e))?;
            if resp.status().is_success() {
                Ok("Serper API Key 验证成功".to_string())
            } else {
                Err(format!("Serper 返回 {}", resp.status()))
            }
        }
        "searxng" => {
            let base = base_url.ok_or("SearXNG 需要填写实例 URL")?;
            let base = base.trim_end_matches('/');
            let test_url = format!("{}/search?q=test&format=json", base);
            let resp = client
                .get(&test_url)
                .header("Accept", "application/json")
                .send()
                .await
                .map_err(|e| format!("连接 SearXNG 失败: {}", e))?;
            if resp.status().is_success() {
                Ok(format!("SearXNG 实例 {} 连通性验证通过", base))
            } else {
                Err(format!("SearXNG 返回 {}", resp.status()))
            }
        }
        other => Err(format!("未知服务商: {}", other)),
    }
}
