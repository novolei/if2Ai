//! P2-14 — Best-effort redaction for persisted diagnostics (event log, etc.).

use serde_json::Value;

/// Keys whose string values are replaced in logs.
pub const SENSITIVE_JSON_KEYS: &[&str] = &[
    "api_key",
    "authorization",
    "password",
    "token",
    "secret",
    "client_secret",
    "refresh_token",
    "access_token",
    "bearer",
];

#[must_use]
pub fn event_log_redaction_enabled() -> bool {
    !std::env::var("IF2AI_DISABLE_EVENT_LOG_REDACTION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn redact_string(s: &str) -> String {
    if s.len() <= 8 {
        "[REDACTED]".to_string()
    } else {
        format!("[REDACTED:{}]", s.len())
    }
}

/// P2-14 — Return a redacted, length-bounded summary of a tool input
/// payload suitable for observer / event-log payloads. Falls back to a
/// short placeholder when the value cannot be serialized.
///
/// - Sensitive keys are redacted via [`redact_value_in_place`].
/// - Output is truncated to `max_chars` (default 512) to keep the
///   observer surface cheap and bounded.
#[must_use]
pub fn redact_tool_args_summary(args: &Value, max_chars: usize) -> String {
    let mut clone = args.clone();
    redact_value_in_place(&mut clone);
    let raw = serde_json::to_string(&clone).unwrap_or_else(|_| "<unserializable>".to_string());
    if raw.len() <= max_chars {
        raw
    } else {
        let cut = raw.char_indices().take(max_chars).count();
        format!("{}…[+{}b]", &raw[..cut], raw.len() - cut)
    }
}

/// Recursively redact sensitive keys in a JSON value (mutates in place).
pub fn redact_value_in_place(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let kl = k.to_ascii_lowercase();
                if SENSITIVE_JSON_KEYS
                    .iter()
                    .any(|sk| kl.contains(&sk.to_ascii_lowercase()))
                {
                    *v = Value::String(redact_string(&v.to_string()));
                } else {
                    redact_value_in_place(v);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_value_in_place(item);
            }
        }
        _ => {}
    }
}
