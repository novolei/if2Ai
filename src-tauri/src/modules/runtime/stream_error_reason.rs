//! Classify stream errors into a stable `kind: raw` reason string.
//!
//! Extracted from `commands/agent.rs` in GFR-005b (pure structural
//! move; function bodies byte-identical). The 5 reason-kind prefixes
//! produced here form the agreed protocol consumed by
//! `harness/event_bus.rs` / `harness/run_report.rs` / `harness/trace_aggregator.rs`.

pub(crate) fn format_stream_error_reason(error: &impl std::fmt::Display) -> String {
    let raw = error.to_string();
    let lower = raw.to_ascii_lowercase();
    let kind = if lower.contains("timed out") || lower.contains("timeout") {
        "network_timeout"
    } else if lower.contains("429") || lower.contains("too many requests") || lower.contains("rate limit") {
        "rate_limited"
    } else if lower.contains("invalid_request_error")
        || lower.contains("invalid params")
        || lower.contains("bad request")
    {
        "request_validation_error"
    } else if lower.contains("permission")
        || lower.contains("forbidden")
        || lower.contains("denied")
    {
        "permission_error"
    } else if lower.contains("connection") || lower.contains("broken pipe") || lower.contains("eof")
    {
        "network_transport_error"
    } else {
        "model_stream_error"
    };
    format!("{kind}: {raw}")
}

pub(crate) fn is_network_timeout_reason(reason: &str) -> bool {
    reason.to_ascii_lowercase().contains("network_timeout:")
}

/// Returns `true` when the reason string was classified as rate-limited (429).
pub(crate) fn is_rate_limited_reason(reason: &str) -> bool {
    reason.to_ascii_lowercase().contains("rate_limited:")
}
