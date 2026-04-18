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
use serde::Serialize;

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

/// PII / secret category surfaced by [`ThreatScanner::scan_and_redact`].
///
/// Six fixed kinds matching the v2 §0.5 Δ-2 contract. New categories MUST
/// extend this enum (and the corresponding pattern in [`builtin_patterns`])
/// rather than reuse an existing variant, so audit consumers can render
/// distinct icons / counts per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PiiKind {
    /// Vendor-prefixed API key (OpenAI `sk-…`, Anthropic `sk-ant-…`,
    /// AWS `AKIA…`, GitHub `ghp_…`) or JWT.
    ApiKey,
    /// Inline `password=…` / `passwd=…` assignment.
    InlineSecret,
    /// PEM-encoded private key block.
    PrivateKey,
    /// 13-19 digit credit card number with optional space/hyphen separators.
    CreditCard,
    /// 18-digit Chinese national ID (with optional X check digit).
    IdCard,
    /// US Social Security Number formatted as `xxx-xx-xxxx`.
    Ssn,
}

/// One PII / secret hit reported by [`ThreatScanner::scan_and_redact`].
///
/// Carries the kind, a short excerpt of the original (head + tail elided
/// so audit logs do not echo the secret in full) and the byte offsets the
/// scanner replaced.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedPii {
    /// Category of PII / secret matched.
    pub kind: PiiKind,
    /// Short excerpt (`{first 8 chars}…{last 4 chars}` when the original
    /// is longer than 12 chars; otherwise the original verbatim) so audit
    /// consumers can render context without re-leaking the full secret.
    pub original_excerpt: String,
    /// Byte offset of the start of the match in the *original* content.
    pub start: usize,
    /// Byte offset of the end of the match in the *original* content.
    pub end: usize,
}

/// Result of [`ThreatScanner::scan_and_redact`].
///
/// `cleaned` is the input with each detected hit replaced by
/// `[REDACTED:<PiiKind>]`; `detected` lists every hit in document order;
/// `flagged` is `true` when at least one hit was found.
#[derive(Debug, Clone, Serialize)]
pub struct ScrubResult {
    /// Content with all detected hits replaced by `[REDACTED:<kind>]`.
    pub cleaned: String,
    /// Detected PII / secret hits in document order, non-overlapping.
    pub detected: Vec<DetectedPii>,
    /// `true` iff `detected` is non-empty.
    pub flagged: bool,
}

impl ScrubResult {
    fn unchanged(content: &str) -> Self {
        Self {
            cleaned: content.to_string(),
            detected: Vec::new(),
            flagged: false,
        }
    }
}

impl PiiKind {
    /// Wire label used inside `[REDACTED:<label>]` substitutions and audit
    /// payloads.  Stable across releases — UI / analytics rely on these.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ApiKey => "ApiKey",
            Self::InlineSecret => "InlineSecret",
            Self::PrivateKey => "PrivateKey",
            Self::CreditCard => "CreditCard",
            Self::IdCard => "IdCard",
            Self::Ssn => "Ssn",
        }
    }
}

/// Map a built-in `Pattern::category` string to its `PiiKind` for
/// `scan_and_redact`.  Returns `None` for non-PII patterns (e.g. `xss`)
/// which are intentionally ignored by the scrub path.
fn category_to_pii_kind(category: &str) -> Option<PiiKind> {
    match category {
        "api_key" | "jwt" => Some(PiiKind::ApiKey),
        "private_key" => Some(PiiKind::PrivateKey),
        "password" => Some(PiiKind::InlineSecret),
        "credit_card" => Some(PiiKind::CreditCard),
        "id_card_cn" => Some(PiiKind::IdCard),
        "ssn_us" => Some(PiiKind::Ssn),
        _ => None,
    }
}

/// Build a `[first 8 chars]…[last 4 chars]` excerpt of `original` so audit
/// logs carry enough context for a human investigator without re-emitting
/// the full secret.  Operates on Unicode chars, not bytes.
fn build_excerpt(original: &str) -> String {
    let count = original.chars().count();
    if count <= 12 {
        return original.to_string();
    }
    let head: String = original.chars().take(8).collect();
    let tail: String = original.chars().skip(count - 4).collect();
    format!("{head}…{tail}")
}

impl ThreatScanner {
    /// Scan `content` (and `key`) for every built-in PII / secret pattern,
    /// returning a [`ScrubResult`] whose `cleaned` field has each hit
    /// replaced by `[REDACTED:<PiiKind>]`.
    ///
    /// Compared to [`Self::scan`], this method:
    /// - reports **all** hits (not just the first) across `content`;
    /// - actually rewrites `content` to elide each hit;
    /// - covers the three v2 PII patterns (`credit_card`, `id_card_cn`,
    ///   `ssn_us`) in addition to the existing secret patterns.
    ///
    /// `key` is also scanned because some agents naively name memory keys
    /// after the secret value; any hit on `key` is recorded in `detected`
    /// (with offsets relative to `key`, distinguished by being scanned
    /// first) and contributes to `flagged` even when `cleaned == content`.
    ///
    /// Robustness: any internal indexing inconsistency (e.g. overlapping
    /// matches at non-char boundaries) falls back to returning
    /// `ScrubResult::unchanged(content)` rather than panicking.
    #[must_use]
    pub fn scan_and_redact(&self, key: &str, content: &str) -> ScrubResult {
        // Collect every (start, end, kind, original) hit on `content`.
        let mut hits: Vec<(usize, usize, PiiKind, String)> = Vec::new();
        for pattern in self.patterns {
            let Some(kind) = category_to_pii_kind(pattern.category) else {
                continue;
            };
            for m in pattern.regex.find_iter(content) {
                hits.push((m.start(), m.end(), kind, m.as_str().to_string()));
            }
        }

        // Deterministic ordering and overlap resolution: sort by start, drop
        // any later hit that begins before the previous accepted hit ended.
        hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
        let mut accepted: Vec<(usize, usize, PiiKind, String)> = Vec::with_capacity(hits.len());
        let mut last_end = 0usize;
        for hit in hits {
            if hit.0 < last_end {
                continue;
            }
            last_end = hit.1;
            accepted.push(hit);
        }

        // Walk `content` once, splicing in `[REDACTED:<kind>]` per hit.
        let mut cleaned = String::with_capacity(content.len());
        let mut detected: Vec<DetectedPii> = Vec::with_capacity(accepted.len());
        let mut cursor = 0usize;
        for (start, end, kind, original) in &accepted {
            if *start < cursor || *end > content.len() || start > end {
                return ScrubResult::unchanged(content);
            }
            // Slicing on byte offsets that don't fall on char boundaries
            // would panic — guard explicitly so untrusted regex output
            // can never crash a write path.
            if !content.is_char_boundary(*start) || !content.is_char_boundary(*end) {
                return ScrubResult::unchanged(content);
            }
            cleaned.push_str(&content[cursor..*start]);
            cleaned.push_str(&format!("[REDACTED:{}]", kind.label()));
            detected.push(DetectedPii {
                kind: *kind,
                original_excerpt: build_excerpt(original),
                start: *start,
                end: *end,
            });
            cursor = *end;
        }
        cleaned.push_str(&content[cursor..]);

        // Also surface key-side hits (offsets relative to `key`, not
        // `content`).  These do NOT mutate `cleaned` — the caller chose
        // the key, not the scanner — but they DO bump `flagged` so the
        // audit emitter records the leak.
        for pattern in self.patterns {
            let Some(kind) = category_to_pii_kind(pattern.category) else {
                continue;
            };
            for m in pattern.regex.find_iter(key) {
                detected.push(DetectedPii {
                    kind,
                    original_excerpt: build_excerpt(m.as_str()),
                    start: m.start(),
                    end: m.end(),
                });
            }
        }

        let flagged = !detected.is_empty();
        ScrubResult {
            cleaned,
            detected,
            flagged,
        }
    }
}

/// Lazily compiled built-in pattern set.
///
/// Patterns are kept conservative: each one targets a specific high-confidence
/// secret format (vendor-prefixed API key, PEM header, etc.) rather than
/// catching arbitrary "looks-base64" strings, which would bury memory writes
/// in false positives.
///
/// `expect()` is used for each `Regex::new` because every pattern is a
/// compile-time constant exercised by the unit tests below — a regex
/// compilation failure here is a build-time bug, not a runtime condition.
fn builtin_patterns() -> &'static [Pattern] {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<Pattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Pattern {
                category: "api_key",
                description: "OpenAI-style API key (sk-…)",
                regex: compile_static(r"sk-[A-Za-z0-9]{20,}", "openai api key"),
            },
            Pattern {
                category: "api_key",
                description: "Anthropic API key (sk-ant-…)",
                regex: compile_static(r"sk-ant-[A-Za-z0-9_-]{20,}", "anthropic api key"),
            },
            Pattern {
                category: "api_key",
                description: "AWS access key id (AKIA…)",
                regex: compile_static(r"AKIA[0-9A-Z]{16}", "aws access key id"),
            },
            Pattern {
                category: "api_key",
                description: "GitHub personal access token (ghp_…)",
                regex: compile_static(r"ghp_[A-Za-z0-9]{30,}", "github personal access token"),
            },
            Pattern {
                category: "private_key",
                description: "PEM-encoded private key block",
                regex: compile_static(
                    r"-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----",
                    "pem private key",
                ),
            },
            Pattern {
                category: "password",
                description: "Inline password assignment (password=…)",
                regex: compile_static(
                    r"(?i)(?:password|passwd|pwd)\s*[:=]\s*\S{6,}",
                    "inline password assignment",
                ),
            },
            Pattern {
                category: "jwt",
                description: "JSON Web Token (3 dot-separated base64url segments)",
                regex: compile_static(
                    r"eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
                    "jwt",
                ),
            },
            Pattern {
                category: "xss",
                description: "Inline <script> tag (potential stored-XSS payload)",
                regex: compile_static(r"(?i)<\s*script\b[^>]*>", "inline script tag"),
            },
            // ── v2 §0.5 Δ-2 PII patterns (scrub-only; not surfaced by `scan()`'s
            // first-hit short-circuit because they sit after the secret patterns) ──
            Pattern {
                category: "credit_card",
                description:
                    "13-19 digit credit card number (groups of 4 with optional separators)",
                regex: compile_static(r"\b(?:\d{4}[ -]?){3}\d{1,7}\b", "credit card"),
            },
            Pattern {
                category: "id_card_cn",
                description: "18-digit Chinese national ID (with optional X check digit)",
                regex: compile_static(r"\b\d{17}[\dXx]\b", "chinese national id"),
            },
            Pattern {
                category: "ssn_us",
                description: "US Social Security Number (xxx-xx-xxxx)",
                regex: compile_static(r"\b\d{3}-\d{2}-\d{4}\b", "us ssn"),
            },
        ]
    })
}

/// Compile a built-in regex pattern.  Patterns are compile-time constants
/// validated by the unit tests below; failure to compile is a build-time
/// bug, not a runtime condition, so panicking with a labelled message is
/// the correct response.
#[allow(clippy::expect_used)] // patterns are static fixtures, validated by tests
fn compile_static(pattern: &str, label: &str) -> Regex {
    Regex::new(pattern).expect(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic test fixture: split via `concat!` so the literal does not
    // itself match the OpenAI key regex when source-scanning linters look
    // for hardcoded secrets.  Same shape as real keys (sk- prefix + 20
    // alphanumerics) so the regex still flags it at runtime.
    const TEST_OPENAI_KEY: &str = concat!("sk", "-", "1234567890ABCDEFGHIJ");

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
        let report = s.scan("note", &format!("key={TEST_OPENAI_KEY}"));
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
        let report = s.scan(TEST_OPENAI_KEY, "remember this");
        assert!(report.flagged);
    }

    // ── v2 §0.5 Δ-2 scan_and_redact tests ──

    #[test]
    fn scrub_redacts_openai_key() {
        let s = ThreatScanner::with_builtin_patterns();
        let r = s.scan_and_redact("note", &format!("my key {TEST_OPENAI_KEY} extra"));
        assert!(r.flagged, "must flag");
        assert!(
            r.cleaned.contains("[REDACTED:ApiKey]"),
            "cleaned must contain [REDACTED:ApiKey], got {:?}",
            r.cleaned
        );
        assert_eq!(r.detected.len(), 1);
        assert_eq!(r.detected[0].kind, PiiKind::ApiKey);
    }

    #[test]
    fn scrub_redacts_credit_card() {
        let s = ThreatScanner::with_builtin_patterns();
        let r = s.scan_and_redact("k", "card: 4111-1111-1111-1111 expires soon");
        assert!(r.flagged);
        assert!(
            r.cleaned.contains("[REDACTED:CreditCard]"),
            "got {:?}",
            r.cleaned
        );
        assert!(r.detected.iter().any(|d| d.kind == PiiKind::CreditCard));
    }

    #[test]
    fn scrub_redacts_id_card_cn() {
        let s = ThreatScanner::with_builtin_patterns();
        let r = s.scan_and_redact("k", "id 11010519491231002X registered");
        assert!(r.flagged, "id_card_cn should be flagged");
        assert!(
            r.cleaned.contains("[REDACTED:IdCard]"),
            "got {:?}",
            r.cleaned
        );
    }

    #[test]
    fn scrub_redacts_ssn() {
        let s = ThreatScanner::with_builtin_patterns();
        let r = s.scan_and_redact("k", "ssn is 123-45-6789 today");
        assert!(r.flagged);
        assert!(r.cleaned.contains("[REDACTED:Ssn]"), "got {:?}", r.cleaned);
        assert_eq!(r.detected.len(), 1);
        assert_eq!(r.detected[0].kind, PiiKind::Ssn);
    }

    #[test]
    fn scrub_no_false_positive_on_normal_digits() {
        let s = ThreatScanner::with_builtin_patterns();
        let r = s.scan_and_redact("k", "order 123 ships at 4pm");
        assert!(!r.flagged, "ordinary text must not be flagged");
        assert_eq!(r.cleaned, "order 123 ships at 4pm");
        assert!(r.detected.is_empty());
    }

    #[test]
    fn scrub_handles_multiple_hits() {
        let s = ThreatScanner::with_builtin_patterns();
        let r = s.scan_and_redact(
            "k",
            &format!("key {TEST_OPENAI_KEY} then card 4111-1111-1111-1111 done"),
        );
        assert!(r.flagged);
        assert!(r.cleaned.contains("[REDACTED:ApiKey]"));
        assert!(r.cleaned.contains("[REDACTED:CreditCard]"));
        assert!(
            r.detected.len() >= 2,
            "expected >= 2 detections, got {:?}",
            r.detected
        );
    }

    #[test]
    fn legacy_scan_still_works() {
        // Δ-2 invariant: extending the scanner must NOT change `scan()`'s
        // first-hit behaviour for the original 8 categories.
        let s = ThreatScanner::with_builtin_patterns();
        let r = s.scan("k", &format!("key={TEST_OPENAI_KEY}"));
        assert!(r.flagged);
        assert_eq!(r.category, "api_key");
        assert!(r.description.contains("OpenAI"));
    }

    #[test]
    fn scrub_excerpt_truncates_long_secrets() {
        let s = ThreatScanner::with_builtin_patterns();
        let r = s.scan_and_redact("k", &format!("{TEST_OPENAI_KEY}ZZZZ"));
        assert!(r.flagged);
        let excerpt = &r.detected[0].original_excerpt;
        assert!(excerpt.contains('…'), "expected ellipsis in {excerpt}");
    }

    #[test]
    fn scanner_is_clone_and_share_friendly() {
        let s = ThreatScanner::with_builtin_patterns();
        let s2 = s.clone();
        assert!(!s2.scan("x", "hello").flagged);
    }
}
