//! LLM call resilience: stream start retries, circuit breaker, optional failover,
//! and a small completion cache for tool-free `send_message` calls.
//!
//! Configuration is primarily via environment variables (see [`LlmResilienceConfig::from_env`]).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use tokio::time::sleep;

use crate::modules::api::{
    ApiError, MessageRequest, MessageResponse, MessageStream, ProviderClient,
};

/// Tunables for retries + circuit breaker + cache.
#[derive(Debug, Clone)]
pub struct LlmResilienceConfig {
    pub circuit_enabled: bool,
    pub circuit_failure_threshold: u32,
    pub circuit_recovery: Duration,
    pub stream_start_max_retries: u32,
    pub stream_start_backoff_ms: u64,
    pub completion_cache_enabled: bool,
    pub completion_cache_ttl: Duration,
    pub completion_cache_max_entries: usize,
}

impl Default for LlmResilienceConfig {
    fn default() -> Self {
        Self {
            circuit_enabled: true,
            circuit_failure_threshold: 5,
            circuit_recovery: Duration::from_secs(30),
            stream_start_max_retries: 3,
            stream_start_backoff_ms: 400,
            completion_cache_enabled: true,
            completion_cache_ttl: Duration::from_secs(3600),
            completion_cache_max_entries: 256,
        }
    }
}

impl LlmResilienceConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let mut c = Self::default();
        if std::env::var("IF2AI_LLM_CIRCUIT_ENABLED")
            .map(|v| v == "0")
            .unwrap_or(false)
        {
            c.circuit_enabled = false;
        }
        if let Ok(v) = std::env::var("IF2AI_LLM_CIRCUIT_FAILURE_THRESHOLD") {
            if let Ok(n) = v.parse::<u32>() {
                c.circuit_failure_threshold = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("IF2AI_LLM_CIRCUIT_RECOVERY_SECS") {
            if let Ok(n) = v.parse::<u64>() {
                c.circuit_recovery = Duration::from_secs(n.max(1));
            }
        }
        if let Ok(v) = std::env::var("IF2AI_LLM_STREAM_START_MAX_RETRIES") {
            if let Ok(n) = v.parse::<u32>() {
                c.stream_start_max_retries = n;
            }
        }
        if let Ok(v) = std::env::var("IF2AI_LLM_STREAM_START_BACKOFF_MS") {
            if let Ok(n) = v.parse::<u64>() {
                c.stream_start_backoff_ms = n.max(50);
            }
        }
        if std::env::var("IF2AI_LLM_COMPLETION_CACHE")
            .map(|v| v == "0")
            .unwrap_or(false)
        {
            c.completion_cache_enabled = false;
        }
        c
    }
}

/// Per-stream-task circuit state (consecutive provider failures).
#[derive(Debug, Default)]
pub struct StreamCircuitState {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

impl StreamCircuitState {
    pub fn check_and_clear_recovered(&mut self, cfg: &LlmResilienceConfig) -> Result<(), ApiError> {
        if !cfg.circuit_enabled {
            return Ok(());
        }
        let now = Instant::now();
        if let Some(until) = self.open_until {
            if now < until {
                let wait = until.saturating_duration_since(now);
                return Err(ApiError::CircuitBreakerOpen {
                    retry_after_secs: wait.as_secs().max(1),
                });
            }
            self.open_until = None;
        }
        Ok(())
    }

    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.open_until = None;
        // Module C (truth-loop iter-7) — mirror the success into the
        // process-wide `ProviderCircuitState` singleton so the
        // self-healing daemon's `ProviderCircuitProbe` sees a real
        // recovery signal. No-op until something else has consulted
        // `global_provider_circuit()` (lazy init).
        crate::modules::api::resilience::global_provider_circuit().record_success();
    }

    fn record_failure(&mut self, cfg: &LlmResilienceConfig) {
        if !cfg.circuit_enabled {
            return;
        }
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= cfg.circuit_failure_threshold {
            self.open_until = Some(Instant::now() + cfg.circuit_recovery);
            self.consecutive_failures = 0;
        }
        // Module C (truth-loop iter-7) — see `record_success`. We
        // mirror EVERY failure (independent of `circuit_enabled`'s
        // local recovery branch) so the daemon-side probe can still
        // surface `Degraded` even when the per-stream breaker is
        // configured off.
        crate::modules::api::resilience::global_provider_circuit().record_failure();
    }
}

fn cache_key_for_request(req: &MessageRequest) -> String {
    let mut h = Sha256::new();
    h.update(req.model.as_bytes());
    h.update(b"\n");
    if let Ok(bytes) = serde_json::to_vec(&req.messages) {
        h.update(&bytes);
    }
    if let Some(sys) = &req.system {
        h.update(sys.as_bytes());
    }
    format!("{:x}", h.finalize())
}

struct CacheEntry {
    inserted: Instant,
    response: MessageResponse,
}

static COMPLETION_CACHE: Lazy<Mutex<HashMap<String, CacheEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn completion_cache_get(key: &str, ttl: Duration) -> Option<MessageResponse> {
    let mut g = COMPLETION_CACHE.lock().ok()?;
    let ent = g.get(key)?;
    if ent.inserted.elapsed() > ttl {
        g.remove(key);
        return None;
    }
    Some(ent.response.clone())
}

fn completion_cache_put(
    key: String,
    _ttl: Duration,
    max_entries: usize,
    response: MessageResponse,
) {
    let Ok(mut g) = COMPLETION_CACHE.lock() else {
        return;
    };
    if g.len() >= max_entries {
        // crude LRU eviction: drop ~10% oldest by insertion time
        let mut pairs: Vec<_> = g.iter().map(|(k, v)| (k.clone(), v.inserted)).collect();
        pairs.sort_by_key(|(_, t)| *t);
        let drop_n = (max_entries / 10).max(1);
        for (k, _) in pairs.into_iter().take(drop_n) {
            g.remove(&k);
        }
    }
    g.insert(
        key,
        CacheEntry {
            inserted: Instant::now(),
            response,
        },
    );
}

fn request_cacheable(req: &MessageRequest) -> bool {
    !req.stream && req.tools.as_ref().is_none_or(Vec::is_empty) && req.tool_choice.is_none()
}

/// `send_message` with optional LRU-ish completion cache (tool-free completions only).
pub async fn send_message_resilient_cached(
    client: &ProviderClient,
    req: &MessageRequest,
    cfg: &LlmResilienceConfig,
) -> Result<MessageResponse, ApiError> {
    if cfg.completion_cache_enabled && request_cacheable(req) {
        let key = cache_key_for_request(req);
        if let Some(hit) = completion_cache_get(&key, cfg.completion_cache_ttl) {
            return Ok(hit);
        }
        let resp = client.send_message(req).await?;
        completion_cache_put(
            key,
            cfg.completion_cache_ttl,
            cfg.completion_cache_max_entries,
            resp.clone(),
        );
        return Ok(resp);
    }
    client.send_message(req).await
}

async fn try_stream_message_inner(
    client: &ProviderClient,
    req: &MessageRequest,
    cfg: &LlmResilienceConfig,
) -> Result<MessageStream, ApiError> {
    let mut last_err: Option<ApiError> = None;
    for attempt in 0..=cfg.stream_start_max_retries {
        match client.stream_message(req).await {
            Ok(s) => return Ok(s),
            Err(e) => {
                let retry = e.is_retryable() && attempt < cfg.stream_start_max_retries;
                last_err = Some(e);
                if !retry {
                    break;
                }
                sleep(Duration::from_millis(
                    cfg.stream_start_backoff_ms
                        .saturating_mul(attempt as u64 + 1),
                ))
                .await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        ApiError::Timeout("stream_message failed with no error captured".to_string())
    }))
}

/// Start a provider message stream: optional failover client, retries, and circuit accounting.
pub async fn stream_message_with_resilience(
    primary: &ProviderClient,
    fallback: Option<&ProviderClient>,
    req: &MessageRequest,
    circuit: &mut StreamCircuitState,
    cfg: &LlmResilienceConfig,
) -> Result<MessageStream, ApiError> {
    circuit.check_and_clear_recovered(cfg)?;

    match try_stream_message_inner(primary, req, cfg).await {
        Ok(s) => {
            circuit.record_success();
            crate::modules::observability::emit("llm.stream.start.ok", &req.model);
            Ok(s)
        }
        Err(primary_err) => {
            if let Some(fb) = fallback {
                match try_stream_message_inner(fb, req, cfg).await {
                    Ok(s) => {
                        tracing::warn!(
                            "[llm_resilience] primary stream failed; succeeded on failover provider"
                        );
                        circuit.record_success();
                        crate::modules::observability::emit(
                            "llm.stream.start.failover_ok",
                            &format!("model={} err={primary_err}", req.model),
                        );
                        Ok(s)
                    }
                    Err(fb_err) => {
                        tracing::error!(
                            "[llm_resilience] primary and failover stream start failed: primary={primary_err}; failover={fb_err}"
                        );
                        circuit.record_failure(cfg);
                        crate::modules::observability::emit(
                            "llm.stream.start.err",
                            &format!("model={} err={primary_err}", req.model),
                        );
                        Err(primary_err)
                    }
                }
            } else {
                if primary_err.is_retryable() {
                    circuit.record_failure(cfg);
                } else {
                    circuit.record_success();
                }
                crate::modules::observability::emit(
                    "llm.stream.start.err",
                    &format!("model={} err={primary_err}", req.model),
                );
                Err(primary_err)
            }
        }
    }
}
