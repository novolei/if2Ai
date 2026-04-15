//! Input validation for memory entries
//!
//! Validates keys and content to prevent injection attacks
//! before data is stored in SQLite.

/// Validate and sanitize memory entry inputs
pub fn validate_memory_entry(key: &str, content: &str) -> Result<(), ValidationError> {
    // Key validation
    if key.is_empty() {
        return Err(ValidationError::EmptyKey);
    }
    if key.len() > 256 {
        return Err(ValidationError::KeyTooLong(key.len()));
    }
    if !key
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(ValidationError::InvalidKeyFormat);
    }

    // Content validation
    let content_len = content.len();
    if content_len > 1_000_000 {
        return Err(ValidationError::ContentTooLarge(content_len));
    }

    // Check for injection patterns
    if contains_injection_pattern(content) {
        return Err(ValidationError::InjectionDetected);
    }

    Ok(())
}

fn contains_injection_pattern(content: &str) -> bool {
    let suspicious = [
        "<script",
        "javascript:",
        "data:text/html",
        "{{.",
        "{{=",
        "${",
        "#{",
        "\\x00",
    ];
    let lower = content.to_lowercase();
    suspicious.iter().any(|p| lower.contains(p))
}

/// Error types for input validation
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("key must not be empty")]
    EmptyKey,

    #[error("key too long: {0} bytes (max 256)")]
    KeyTooLong(usize),

    #[error("invalid key format: only alphanumeric, '_', '-', '.' allowed")]
    InvalidKeyFormat,

    #[error("content too large: {0} bytes (max 1_000_000)")]
    ContentTooLarge(usize),

    #[error("injection pattern detected in content")]
    InjectionDetected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_key_and_content() {
        assert!(validate_memory_entry("my_key", "Hello, world!").is_ok());
        assert!(validate_memory_entry("test.user.v1", "Simple content").is_ok());
    }

    #[test]
    fn rejects_empty_key() {
        let err = validate_memory_entry("", "content").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyKey));
    }

    #[test]
    fn rejects_long_key() {
        let long_key = "a".repeat(300);
        let err = validate_memory_entry(&long_key, "content").unwrap_err();
        assert!(matches!(err, ValidationError::KeyTooLong(300)));
    }

    #[test]
    fn rejects_invalid_key_chars() {
        let err = validate_memory_entry("my key!", "content").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidKeyFormat));
        let err = validate_memory_entry("my/key", "content").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidKeyFormat));
    }

    #[test]
    fn rejects_large_content() {
        let large = "x".repeat(1_000_001);
        let err = validate_memory_entry("key", &large).unwrap_err();
        assert!(matches!(err, ValidationError::ContentTooLarge(1_000_001)));
    }

    #[test]
    fn detects_xss_patterns() {
        let patterns = [
            "<script>alert('xss')</script>",
            "javascript:alert(1)",
            "data:text/html,<script>",
        ];
        for pattern in patterns {
            let err = validate_memory_entry("key", pattern).unwrap_err();
            assert!(
                matches!(err, ValidationError::InjectionDetected),
                "expected InjectionDetected for: {pattern}"
            );
        }
    }

    #[test]
    fn detects_template_injection() {
        let patterns = ["{{.}}", "${DANGEROUS}", "#{system('ls')}"];
        for pattern in patterns {
            let err = validate_memory_entry("key", pattern).unwrap_err();
            assert!(
                matches!(err, ValidationError::InjectionDetected),
                "expected InjectionDetected for: {pattern}"
            );
        }
    }

    #[test]
    fn case_insensitive_detection() {
        let err = validate_memory_entry("key", "<SCRIPT>alert(1)</SCRIPT>").unwrap_err();
        assert!(matches!(err, ValidationError::InjectionDetected));
    }
}
