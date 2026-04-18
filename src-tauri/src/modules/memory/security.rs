//! Threat scanner for memory writes (M5).
//!
//! Scans candidate memory `key` / `content` strings for patterns that look
//! like sensitive credentials before they hit persistent storage.  The scanner
//! is intentionally regex-based and conservative: it favours false-positives
//! over silently leaking secrets to disk.
//!
//! Scope: only used in the write path (`memory_store` tool).  Recall paths
//! never re-scan because retrieved entries were already gated on ingest.
//!
//! Behaviour:
//! - Each `Pattern` carries a category (e.g. `api_key`, `private_key`,
//!   `password`) used in audit / policy reason codes.
//! - The scanner is `Arc`-friendly and cheap to clone (regexes are compiled
//!   once into a `Vec<Pattern>` shared by reference).
//! - Patterns live behind a single `OnceLock` so the regex compilation cost
//!   is amortised across all writes for the lifetime of the process.

use regex::Regex;

/// A single labelled threat pattern.
#[derive(Debug, Clone)]
pub struct Pattern {
    /// Short machine-readable category, e.g. `"api_key"`.
    pub category: &'static str,
    /// Human-readable description used in audit messages.
    pub description: &'static str,
    /// Compiled regex.  `unwrap` is safe at construction time because all
    /// built-in patterns are unit-tested below.
    pub regex: Regex,
}

/// Result of scanning a `(key, content)` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreatReport {
    /// `true` when at least one pattern matched.
    pub flagged: bool,
    /// Category of the **first** matching pattern (or empty when clean).
    pub category: String,
    /// Human-readable description of the **first** matching pattern.
    pub description: String,
}

impl ThreatReport {
    /// Construct an empty (clean) report.
    pub(crate) fn clean() -> Self {
        Self {
            flagged: false,
            category: String::new(),
            description: String::new(),
        }
    }
}

/// Stateless threat scanner over a fixed pattern set.
#[derive(Debug, Clone)]
pub struct ThreatScanner {
    patterns: &'static [Pattern],
}

impl Default for ThreatScanner {
    fn default() -> Self {
        Self::with_builtin_patterns()
    }
}

impl ThreatScanner {
    /// Scanner seeded with the built-in pattern set (API keys, private keys,
    /// passwords, JWTs, generic high-entropy tokens).
    #[must_use]
    pub fn with_builtin_patterns() -> Self {
        Self {
            patterns: builtin_patterns(),
        }
    }

    /// Scan a `(key, content)` pair.  Returns the first matching pattern's
    /// category / description, or a clean report if nothing matched.
    ///
    /// The `key` is scanned because some agents naively name memory keys
    /// after the secret itself (`"OPENAI_API_KEY"`).
    #[must_use]
    pub fn scan(&self, key: &str, content: &str) -> ThreatReport {
        for pattern in self.patterns {
            if pattern.regex.is_match(content) || pattern.regex.is_match(key) {
                return ThreatReport {
                    flagged: true,
                    category: pattern.category.to_string(),
                    description: pattern.description.to_string(),
                };
            }
        }
        ThreatReport::clean()
    }
}

/// Lazily compiled built-in pattern set.
///
/// Patterns are kept conservative: each one targets a specific high-confidence
/// secret format (vendor-prefixed API key, PEM header, etc.) rather than
/// catching arbitrary "looks-base64" strings, which would bury memory writes
/// in false positives.
fn builtin_patterns() -> &'static [Pattern] {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Pattern {
                category: "api_key",
                description: "OpenAI-style API key (sk-…)",
                regex: Regex::new(r"sk-[A-Za-z0-9]{20,}").expect("openai key regex compiles"),
            },
            Pattern {
                category: "api_key",
                description: "Anthropic API key (sk-ant-…)",
                regex: Regex::new(r"sk-ant-[A-Za-z0-9_-]{20,}")
                    .expect("anthropic key regex compiles"),
            },
            Pattern {
                category: "api_key",
                description: "AWS access key id (AKIA…)",
                regex: Regex::new(r"AKIA[0-9A-Z]{16}").expect("aws key regex compiles"),
            },
            Pattern {
                category: "api_key",
                description: "GitHub personal access token (ghp_…)",
                regex: Regex::new(r"ghp_[A-Za-z0-9]{30,}").expect("github pat regex compiles"),
            },
            Pattern {
                category: "private_key",
                description: "PEM-encoded private key block",
                regex: Regex::new(r"-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----")
                    .expect("pem regex compiles"),
            },
            Pattern {
                category: "password",
                description: "Inline password assignment (password=…)",
                regex: Regex::new(r"(?i)(?:password|passwd|pwd)\s*[:=]\s*\S{6,}")
                    .expect("password regex compiles"),
            },
            Pattern {
                category: "jwt",
                description: "JSON Web Token (3 dot-separated base64url segments)",
                regex: Regex::new(
                    r"eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
                )
                .expect("jwt regex compiles"),
            },
            Pattern {
                category: "xss",
                description: "Inline <script> tag (potential stored-XSS payload)",
                regex: Regex::new(r"(?i)<\s*script\b[^>]*>").expect("xss regex compiles"),
            },
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_is_not_flagged() {
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan("birthday", "Mom's birthday is in October.");
        assert!(!report.flagged);
        assert!(report.category.is_empty());
    }

    #[test]
    fn flags_openai_api_key_in_content() {
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan("note", "key=sk-1234567890ABCDEFGHIJ");
        assert!(report.flagged);
        assert_eq!(report.category, "api_key");
        assert!(report.description.contains("OpenAI"));
    }

    #[test]
    fn flags_anthropic_key() {
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan("k", "sk-ant-abc1234567890ABCDEFGHIJ-_");
        assert!(report.flagged);
        assert_eq!(report.category, "api_key");
    }

    #[test]
    fn flags_aws_access_key() {
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan("k", "creds: AKIAIOSFODNN7EXAMPLE end");
        assert!(report.flagged);
    }

    #[test]
    fn flags_github_pat() {
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan("k", "token=ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345");
        assert!(report.flagged);
    }

    #[test]
    fn flags_pem_private_key() {
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan(
            "k",
            "-----BEGIN RSA PRIVATE KEY-----\nMIIE…\n-----END RSA PRIVATE KEY-----",
        );
        assert!(report.flagged);
        assert_eq!(report.category, "private_key");
    }

    #[test]
    fn flags_inline_password_assignment() {
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan("note", "Password: hunter2-supersecret");
        assert!(report.flagged);
        assert_eq!(report.category, "password");
    }

    #[test]
    fn flags_jwt() {
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan(
            "tok",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        );
        assert!(report.flagged);
        assert_eq!(report.category, "jwt");
    }

    #[test]
    fn flags_secret_named_key_even_with_clean_value() {
        // Key itself contains the credential — not uncommon for naive agents.
        let s = ThreatScanner::with_builtin_patterns();
        let report = s.scan("sk-1234567890ABCDEFGHIJ", "remember this");
        assert!(report.flagged);
    }

    #[test]
    fn scanner_is_clone_and_share_friendly() {
        let s = ThreatScanner::with_builtin_patterns();
        let s2 = s.clone();
        assert!(!s2.scan("x", "hello").flagged);
    }
}
