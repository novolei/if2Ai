//! Shared HTTP client for provider API calls.
//!
//! A single `reqwest::Client` with connection pooling and keepalive
//! is reused across all provider HTTP requests (testing, model listing).
//! This avoids the overhead of creating a new client (TLS context,
//! connection pool) on every call.

use once_cell::sync::Lazy;
use reqwest::Client;

use std::time::Duration;

/// Shared HTTP client with connection pooling and keepalive.
/// Built once on first use, reused for all provider requests.
pub static PROVIDER_HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(4)
        .build()
        .expect("Failed to build shared HTTP client")
});
