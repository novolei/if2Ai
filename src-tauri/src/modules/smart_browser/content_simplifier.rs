//! FEAT-BR-001 — Browser content simplifier (pure-function HTML → trim).
//!
//! Strips noise (script/style/nav/aside/footer) and keeps interactive +
//! semantic elements (main/article/form/input/button/a). Output is hard
//! capped at [`DEFAULT_MAX_OUTPUT_CHARS`] (= 35 000) chars; the
//! `adaptive_simplify` helper translates a token budget into a
//! per-call char ceiling and re-runs the simplifier.
//!
//! Pure & sync — no chromium / CDP / network. The wire-up Pack will
//! call this from `smart_browser::runtime::execute_browser_use_mcp_action`
//! and `tools::builtin::browser_tool` to replace raw HTML returns.

#![allow(dead_code)]

use scraper::{ElementRef, Html};

use crate::modules::runtime::budget::estimate_tokens;

/// Hard cap on simplified-HTML output length. Every code path in this
/// module respects it; `adaptive_simplify` may select a smaller value.
pub const DEFAULT_MAX_OUTPUT_CHARS: usize = 35_000;

/// Simplifier configuration. `Default` matches the values referenced
/// in `.qoder/specs/if2ai-agent-evolution-report.md` §Module L.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimplifierConfig {
    pub max_output_chars: usize,
    pub filter_tags: Vec<String>,
    pub preserve_tags: Vec<String>,
    pub preserve_attrs: Vec<String>,
    pub text_only: bool,
}

impl Default for SimplifierConfig {
    fn default() -> Self {
        Self {
            max_output_chars: DEFAULT_MAX_OUTPUT_CHARS,
            filter_tags: vec![
                "script".to_string(),
                "style".to_string(),
                "nav".to_string(),
                "aside".to_string(),
                "footer".to_string(),
                "noscript".to_string(),
                "iframe".to_string(),
                "svg".to_string(),
                "path".to_string(),
            ],
            preserve_tags: vec![
                "main".to_string(),
                "article".to_string(),
                "section".to_string(),
                "form".to_string(),
                "input".to_string(),
                "button".to_string(),
                "a".to_string(),
                "label".to_string(),
                "select".to_string(),
                "textarea".to_string(),
                "h1".to_string(),
                "h2".to_string(),
                "h3".to_string(),
                "p".to_string(),
                "ul".to_string(),
                "ol".to_string(),
                "li".to_string(),
                "table".to_string(),
                "tr".to_string(),
                "td".to_string(),
                "th".to_string(),
            ],
            preserve_attrs: vec![
                "id".to_string(),
                "class".to_string(),
                "name".to_string(),
                "type".to_string(),
                "href".to_string(),
                "src".to_string(),
                "value".to_string(),
                "placeholder".to_string(),
                "role".to_string(),
                "aria-label".to_string(),
            ],
            text_only: false,
        }
    }
}

/// Output of [`simplify_html`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimplifiedContent {
    pub html: String,
    pub text: String,
    pub token_estimate: usize,
    pub original_chars: usize,
    pub simplified_chars: usize,
    /// `simplified_chars / original_chars` — `1.0` when input was
    /// empty or already under-budget (no compression performed).
    pub compression_ratio: f64,
    pub key_elements: Vec<KeyElement>,
}

/// One interactive element extracted for agent-side targeting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyElement {
    pub tag: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub element_type: Option<String>,
    pub text_content: String,
    pub href: Option<String>,
}

/// Run the static-rules simplifier. Pure / sync / `O(n)` over input
/// length. Empty input → empty output (compression_ratio = 1.0,
/// no panic).
#[must_use]
pub fn simplify_html(raw_html: &str, config: &SimplifierConfig) -> SimplifiedContent {
    let original_chars = raw_html.len();
    if raw_html.trim().is_empty() {
        return SimplifiedContent {
            compression_ratio: 1.0,
            ..SimplifiedContent::default()
        };
    }

    let document = Html::parse_document(raw_html);

    // 1. Collect text blocks + key elements via a single descendant walk.
    let mut text_buf = String::new();
    let mut html_buf = String::new();
    let mut key_elements: Vec<KeyElement> = Vec::new();
    let filter_set: std::collections::HashSet<&str> =
        config.filter_tags.iter().map(String::as_str).collect();
    let preserve_set: std::collections::HashSet<&str> =
        config.preserve_tags.iter().map(String::as_str).collect();

    walk_node(
        document.root_element(),
        &filter_set,
        &preserve_set,
        config,
        &mut text_buf,
        &mut html_buf,
        &mut key_elements,
        config.max_output_chars,
    );

    let cap = config.max_output_chars.max(1);
    let text = truncate_to_chars(&text_buf, cap);
    let html_payload = if config.text_only {
        text.clone()
    } else {
        truncate_to_chars(&html_buf, cap)
    };
    let simplified_chars = html_payload.chars().count();
    let token_estimate = estimate_tokens(&html_payload);
    let compression_ratio = if original_chars == 0 {
        1.0
    } else {
        (simplified_chars as f64 / original_chars as f64).min(1.0)
    };

    SimplifiedContent {
        html: html_payload,
        text,
        token_estimate,
        original_chars,
        simplified_chars,
        compression_ratio,
        key_elements,
    }
}

/// Re-run [`simplify_html`] with a max-char ceiling derived from
/// `available_tokens` (≈ 4 chars per token, capped at the global
/// [`DEFAULT_MAX_OUTPUT_CHARS`]).
#[must_use]
pub fn adaptive_simplify(raw_html: &str, available_tokens: usize) -> SimplifiedContent {
    let mut config = SimplifierConfig::default();
    let derived_chars = available_tokens.saturating_mul(4);
    config.max_output_chars = derived_chars.clamp(64, DEFAULT_MAX_OUTPUT_CHARS);

    let mut content = simplify_html(raw_html, &config);
    if content.token_estimate > available_tokens && available_tokens > 0 {
        // Defensive second pass — char/token ratio drift on tag-heavy
        // pages can push us over budget. Trim further until safe.
        let trimmed = truncate_to_chars(
            &content.html,
            available_tokens.saturating_mul(3).max(64),
        );
        let token_estimate = estimate_tokens(&trimmed);
        let simplified_chars = trimmed.chars().count();
        content = SimplifiedContent {
            html: trimmed,
            simplified_chars,
            token_estimate,
            ..content
        };
    }
    content
}

#[allow(clippy::too_many_arguments)]
fn walk_node(
    node: ElementRef<'_>,
    filter_set: &std::collections::HashSet<&str>,
    preserve_set: &std::collections::HashSet<&str>,
    config: &SimplifierConfig,
    text_buf: &mut String,
    html_buf: &mut String,
    key_elements: &mut Vec<KeyElement>,
    char_budget: usize,
) {
    if html_buf.len() >= char_budget {
        return;
    }
    let tag_name = node.value().name().to_lowercase();
    if filter_set.contains(tag_name.as_str()) {
        return;
    }

    let is_preserved = preserve_set.contains(tag_name.as_str());

    if is_preserved && !config.text_only {
        let attrs = render_preserved_attrs(node, config);
        if attrs.is_empty() {
            html_buf.push_str(&format!("<{tag_name}>"));
        } else {
            html_buf.push_str(&format!("<{tag_name} {attrs}>"));
        }
    }

    let kelem = maybe_collect_key_element(node, &tag_name);
    if let Some(k) = kelem {
        key_elements.push(k);
    }

    for child in node.children() {
        if html_buf.len() >= char_budget {
            break;
        }
        if let Some(text) = child.value().as_text() {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            text_buf.push_str(trimmed);
            text_buf.push(' ');
            if !config.text_only {
                html_buf.push_str(trimmed);
                html_buf.push(' ');
            }
        } else if let Some(child_el) = ElementRef::wrap(child) {
            walk_node(
                child_el,
                filter_set,
                preserve_set,
                config,
                text_buf,
                html_buf,
                key_elements,
                char_budget,
            );
        }
    }

    if is_preserved && !config.text_only && html_buf.len() < char_budget {
        html_buf.push_str(&format!("</{tag_name}>"));
    }
}

fn render_preserved_attrs(node: ElementRef<'_>, config: &SimplifierConfig) -> String {
    let mut parts: Vec<String> = Vec::new();
    let allow: std::collections::HashSet<&str> =
        config.preserve_attrs.iter().map(String::as_str).collect();
    for (name, value) in node.value().attrs() {
        let lname = name.to_lowercase();
        if allow.contains(lname.as_str()) {
            let cleaned = value.replace('"', "'");
            parts.push(format!("{lname}=\"{cleaned}\""));
        }
    }
    parts.join(" ")
}

fn maybe_collect_key_element(node: ElementRef<'_>, tag: &str) -> Option<KeyElement> {
    matches!(
        tag,
        "input" | "button" | "a" | "select" | "textarea" | "form"
    )
    .then(|| {
        let attr = |name: &str| node.value().attr(name).map(|s| s.to_string());
        KeyElement {
            tag: tag.to_string(),
            id: attr("id"),
            name: attr("name"),
            element_type: attr("type"),
            href: attr("href"),
            text_content: node
                .text()
                .collect::<String>()
                .trim()
                .chars()
                .take(120)
                .collect(),
        }
    })
}

fn truncate_to_chars(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    input.chars().take(max_chars).collect()
}
