//! Rust port of `tts_robust_normalizer_single_script.py`.
//!
//! Does robustness cleaning only — no semantic number/date expansion.
//! Protects high-risk tokens (URLs, emails, mentions, filenames).
//! Normalizes structural punctuation, spacing, and markdown.

#![allow(dead_code)]

use fancy_regex::Regex;
use once_cell::sync::Lazy;

// ── Constants ──────────────────────────────────────────────────────────

const CJK: &str = r"[\u{3400}-\u{4dbf}\u{4e00}-\u{9fff}\u{3040}-\u{30ff}]";
const PROT: &str = r"___PROT\d+___";

const TRAILING_CLOSERS: &str = "\"')]}）】》〉」』”'";

// ── Protected patterns ────────────────────────────────────────────────

static RE_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://[^\s\u{3000}，。！？；、）】》〉」』]+").expect("invalid url regex")
});
static RE_EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?<![\w.+\-])[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}(?![\w.\-])")
        .expect("invalid email regex")
});
static RE_MENTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?<![A-Za-z0-9_])@[A-Za-z0-9_]{1,32}").expect("invalid mention regex")
});
static RE_REDDIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?<![A-Za-z0-9_])(?:u|r)/[A-Za-z0-9_]+").expect("invalid reddit regex")
});
static RE_HASHTAG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?<![A-Za-z0-9_])#(?!\s)[^\s#]+").expect("invalid hashtag regex"));
static RE_DOT_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?<![A-Za-z0-9_])\.(?=[A-Za-z0-9._\-]*[A-Za-z0-9])[A-Za-z0-9._\-]+")
        .expect("invalid dot-token regex")
});
static RE_FILELIKE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?<![A-Za-z0-9_])(?=[A-Za-z0-9._/+:\-]*[A-Za-z])(?=[A-Za-z0-9._/+:\-]*[./+:\-])[A-Za-z0-9][A-Za-z0-9._/+:\-]*(?![A-Za-z0-9_])")
        .expect("invalid filelike regex")
});
static RE_PROT: Lazy<Regex> = Lazy::new(|| Regex::new(PROT).expect("invalid prot regex"));
static RE_ZERO_WIDTH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[\u{200b}-\u{200d}\u{feff}]").expect("invalid zero-width regex"));
static RE_LATINISH: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"(?:{PROT}|(?=[A-Za-z0-9._/+:\-]*[A-Za-z])[A-Za-z0-9][A-Za-z0-9._/+:\-]*)"
    ))
    .expect("invalid latinish regex")
});

// ── Public API ─────────────────────────────────────────────────────────

/// Normalize TTS input text — mirrors `normalize_tts_text()` from Python.
#[must_use]
pub fn normalize_tts_text(text: &str) -> String {
    let mut text = base_cleanup(text);
    text = normalize_markdown_and_lines(&text);
    text = normalize_flow_arrows(&text);
    let (mut text, protected) = protect_spans(&text);
    text = normalize_visible_underscores(&text);
    text = normalize_spaces(&text);
    text = normalize_structural_punctuation(&text);
    text = normalize_repeated_punctuation(&text);
    text = normalize_spaces(&text);
    text = restore_spans(&text, &protected);
    text = text.trim().to_string();
    ensure_terminal_punctuation_by_line(&text)
}

// ── Rule implementations ───────────────────────────────────────────────

fn base_cleanup(text: &str) -> String {
    let text = text
        .replace("\r\n", "\n")
        .replace("\r", "\n")
        .replace("\u{3000}", " ");
    let text = RE_ZERO_WIDTH.replace_all(&text, "").to_string();
    text.chars()
        .filter(|ch| {
            if matches!(*ch, '\n' | '\t' | ' ') {
                return true;
            }
            let cat = unicode_category(ch);
            !cat.starts_with('C')
        })
        .collect()
}

fn unicode_category(ch: &char) -> String {
    use unicode_categories::UnicodeCategories;
    // C category: control, format, private use. Surrogates can't be Rust chars.
    if ch.is_other_control() {
        return "Cc".to_string();
    }
    if ch.is_other_format() {
        return "Cf".to_string();
    }
    if ch.is_other_private_use() {
        return "Co".to_string();
    }

    // L category
    if ch.is_letter_lowercase() {
        return "Ll".to_string();
    }
    if ch.is_letter_modifier() {
        return "Lm".to_string();
    }
    if ch.is_letter_other() {
        return "Lo".to_string();
    }
    if ch.is_letter_titlecase() {
        return "Lt".to_string();
    }
    if ch.is_letter_uppercase() {
        return "Lu".to_string();
    }

    // M category
    if ch.is_mark_spacing_combining() {
        return "Mc".to_string();
    }
    if ch.is_mark_enclosing() {
        return "Me".to_string();
    }
    if ch.is_mark_nonspacing() {
        return "Mn".to_string();
    }

    // N category
    if ch.is_number_decimal_digit() {
        return "Nd".to_string();
    }
    if ch.is_number_letter() {
        return "Nl".to_string();
    }
    if ch.is_number_other() {
        return "No".to_string();
    }

    // P category
    if ch.is_punctuation_connector() {
        return "Pc".to_string();
    }
    if ch.is_punctuation_dash() {
        return "Pd".to_string();
    }
    if ch.is_punctuation_close() {
        return "Pe".to_string();
    }
    if ch.is_punctuation_final_quote() {
        return "Pf".to_string();
    }
    if ch.is_punctuation_initial_quote() {
        return "Pi".to_string();
    }
    if ch.is_punctuation_other() {
        return "Po".to_string();
    }
    if ch.is_punctuation_open() {
        return "Ps".to_string();
    }

    // S category
    if ch.is_symbol_currency() {
        return "Sc".to_string();
    }
    if ch.is_symbol_modifier() {
        return "Sk".to_string();
    }
    if ch.is_symbol_math() {
        return "Sm".to_string();
    }
    if ch.is_symbol_other() {
        return "So".to_string();
    }

    // Z category
    if ch.is_separator_line() {
        return "Zl".to_string();
    }
    if ch.is_separator_paragraph() {
        return "Zp".to_string();
    }
    if ch.is_separator_space() {
        return "Zs".to_string();
    }

    // Unassigned (Cn) — anything not covered above
    "Cn".to_string()
}

fn normalize_markdown_and_lines(text: &str) -> String {
    let re = Regex::new(r"\[([^\[\]]+?)\]\((https?://[^)\s]+)\)").expect("invalid regex pattern");
    let text = re.replace_all(text, "$1 $2");

    let lines: Vec<String> = text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.is_empty() {
        return String::new();
    }

    let mut merged: Vec<String> = vec![strip_list_prefix(&lines[0])];
    for line in &lines[1..] {
        let last = merged.len() - 1;
        merged[last] = ensure_terminal_punctuation(&merged[last]);
        merged.push(strip_list_prefix(line));
    }
    merged.concat()
}

fn strip_list_prefix(line: &str) -> String {
    static RE_HEADING: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^#{1,6}\s+").expect("invalid heading regex"));
    static RE_QUOTE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^>\s+").expect("invalid blockquote regex"));
    static RE_ULIST: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[-*+]\s+").expect("invalid ulist regex"));
    static RE_OLIST: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^\d+[.)]\s+").expect("invalid olist regex"));

    let s = RE_HEADING.replace(line, "");
    let s = RE_QUOTE.replace(&s, "");
    let s = RE_ULIST.replace(&s, "");
    let s = RE_OLIST.replace(&s, "");
    s.to_string()
}

fn protect_spans(text: &str) -> (String, Vec<String>) {
    let mut protected: Vec<String> = Vec::new();

    let patterns: &[&Lazy<Regex>] = &[
        &RE_URL,
        &RE_EMAIL,
        &RE_MENTION,
        &RE_REDDIT,
        &RE_HASHTAG,
        &RE_DOT_TOKEN,
        &RE_FILELIKE,
    ];

    let mut text = text.to_string();
    for pat in patterns {
        text = pat
            .replace_all(&text, |caps: &fancy_regex::Captures| {
                let idx = protected.len();
                protected.push(caps[0].to_string());
                format!("___PROT{idx}___")
            })
            .to_string();
    }

    (text, protected)
}

fn restore_spans(text: &str, protected: &[String]) -> String {
    let mut text = text.to_string();
    for (idx, original) in protected.iter().enumerate() {
        text = text.replace(&format!("___PROT{idx}___"), original);
    }
    text
}

fn normalize_visible_underscores(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for caps in RE_PROT.find_iter(text).flatten() {
        let unprotected = &text[last_end..caps.start()];
        result.push_str(&unprotected.replace('_', " "));
        result.push_str(caps.as_str());
        last_end = caps.end();
    }
    let remaining = &text[last_end..];
    result.push_str(&remaining.replace('_', " "));
    result
}

fn normalize_flow_arrows(text: &str) -> String {
    static RE_ARROW: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\s*(?:<[-=]+>|[-=]+>|<[-=]+|[→←↔⇒⇐⇔⟶⟵⟷⟹⟸⟺↦↤↪↩])\s*")
            .expect("invalid regex pattern")
    });
    RE_ARROW.replace_all(text, "，").to_string()
}

fn normalize_spaces(text: &str) -> String {
    let mut text = Regex::new(r"[ \t\r\f\v]+")
        .expect("invalid regex pattern")
        .replace_all(text, " ")
        .to_string();

    // CJK internal: remove spaces
    let re = Regex::new(&format!("({CJK})\\s+(?={CJK})")).expect("invalid regex pattern");
    text = re.replace_all(&text, "$1").to_string();

    // CJK vs digits: remove spaces
    let re = Regex::new(&format!("({CJK})\\s+(?=\\d)")).expect("invalid regex pattern");
    text = re.replace_all(&text, "$1").to_string();
    let re = Regex::new(&format!(r"(\d)\s+(?={CJK})")).expect("invalid regex pattern");
    text = re.replace_all(&text, "$1").to_string();

    // CJK vs Latin: add space
    let re = Regex::new(&format!("({CJK})(?=(?:{}))", r"___PROT\d+___|(?<![A-Za-z0-9])(?=[A-Za-z0-9._/+:\-]*[A-Za-z])[A-Za-z0-9][A-Za-z0-9._/+:\-]*")).expect("invalid regex pattern");
    text = re.replace_all(&text, "$1 ").to_string();
    let re = Regex::new(&format!("(?:(?:{}))(?={CJK})", r"___PROT\d+___|(?<![A-Za-z0-9])(?=[A-Za-z0-9._/+:\-]*[A-Za-z])[A-Za-z0-9][A-Za-z0-9._/+:\-]*")).expect("invalid regex pattern");
    text = re.replace_all(&text, "$0 ").to_string();

    // Collapse multiple spaces
    text = Regex::new(r" {2,}")
        .expect("invalid regex pattern")
        .replace_all(&text, " ")
        .to_string();

    // No space after Chinese punctuation (excluding ASCII quotes which may wrap Latin text)
    text = Regex::new(r#"\s+([，。！？；：、'」』】）》])"#)
        .expect("invalid regex pattern")
        .replace_all(&text, "$1")
        .to_string();
    text = Regex::new(r"([（【「『《])\s+")
        .expect("invalid regex pattern")
        .replace_all(&text, "$1")
        .to_string();
    text = Regex::new(r"([，。！？；：、])\s*")
        .expect("invalid regex pattern")
        .replace_all(&text, "$1")
        .to_string();

    // No space before ASCII punctuation
    text = Regex::new(r"\s+([,.;!?])")
        .expect("invalid regex pattern")
        .replace_all(&text, "$1")
        .to_string();

    // No space between closing ASCII quote and following CJK (only when quote follows CJK directly)
    text = Regex::new(&format!(r#"({CJK})"\s+({CJK})"#))
        .expect("invalid regex pattern")
        .replace_all(&text, r#"$1"$2"#)
        .to_string();

    text = Regex::new(r" {2,}")
        .expect("invalid regex pattern")
        .replace_all(&text, " ")
        .to_string();
    text.trim().to_string()
}

fn normalize_structural_punctuation(text: &str) -> String {
    let mut text = text.to_string();

    // Brackets -> double quotes
    text = Regex::new(r"\[\s*([^\[\]]+?)\s*\]")
        .expect("invalid regex pattern")
        .replace_all(&text, r#""$1""#)
        .to_string();
    text = Regex::new(r"\{\s*([^{}]+?)\s*\}")
        .expect("invalid regex pattern")
        .replace_all(&text, r#""$1""#)
        .to_string();
    text = Regex::new(r"[【〖『「]\s*([^】〗』」]+?)\s*[】〗』」]")
        .expect("invalid regex pattern")
        .replace_all(&text, r#""$1""#)
        .to_string();

    // 《》 standalone headlines only
    text = Regex::new(
        r"(^|[。！？!?；;]\s*)《([^》]+)》(?=\s*(?:___PROT\d+___|[—–―\-]{2,}|$|[。！？!?；;，,]))",
    )
    .expect("invalid regex pattern")
    .replace_all(&text, "$1$2")
    .to_string();

    // Flow arrows
    text = normalize_flow_arrows(&text);

    // Long dashes -> sentence boundary
    text = Regex::new(r"\s*(?:—|–|―|\-){2,}\s*")
        .expect("invalid regex pattern")
        .replace_all(&text, "。")
        .to_string();

    text
}

fn normalize_repeated_punctuation(text: &str) -> String {
    let mut text = text.to_string();

    // Ellipsis / consecutive dots
    text = Regex::new(r"(?:\.{3,}|…{2,}|……+)")
        .expect("invalid regex pattern")
        .replace_all(&text, "。")
        .to_string();

    // Repeated same-type punctuation
    text = Regex::new(r"[。．]{2,}")
        .expect("invalid regex pattern")
        .replace_all(&text, "。")
        .to_string();
    text = Regex::new(r"[，,]{2,}")
        .expect("invalid regex pattern")
        .replace_all(&text, "，")
        .to_string();
    text = Regex::new(r"[!！]{2,}")
        .expect("invalid regex pattern")
        .replace_all(&text, "！")
        .to_string();
    text = Regex::new(r"[?？]{2,}")
        .expect("invalid regex pattern")
        .replace_all(&text, "？")
        .to_string();

    // Mixed ?! -> ？！
    text = Regex::new(r"[!?！？]{2,}")
        .expect("invalid regex pattern")
        .replace_all(&text, |caps: &fancy_regex::Captures| {
            let s = &caps[0];
            let has_q = s.contains('?') || s.contains('？');
            let has_e = s.contains('!') || s.contains('！');
            if has_q && has_e {
                "？！"
            } else if has_q {
                "？"
            } else {
                "！"
            }
        })
        .to_string();

    text
}

fn ensure_terminal_punctuation(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut index = chars.len() - 1;

    // Skip trailing whitespace
    while index > 0 && chars[index].is_whitespace() {
        index -= 1;
    }

    // Skip trailing closers
    while index > 0 && TRAILING_CLOSERS.contains(chars[index]) {
        if index == 0 {
            break;
        }
        index -= 1;
    }

    if index == 0 && (chars[index].is_whitespace() || TRAILING_CLOSERS.contains(chars[index])) {
        return text.to_string();
    }

    let ch = chars[index];
    let cat = unicode_category(&ch);
    if cat.starts_with('P') {
        text.to_string()
    } else {
        format!("{text}。")
    }
}

fn ensure_terminal_punctuation_by_line(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    text.lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                ensure_terminal_punctuation(trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// All test cases from the Python reference `TEST_CASES`.
    const TEST_CASES: &[(&str, &str, &str)] = &[
        // 1) .map / dot-leading token / filenames / versions
        ("dot_map_sentence", "2026 年 3 月 31 日，安全研究员 Chaofan Shou (@Fried_rice) 发现 Anthropic 的 npm 包中暴露了 .map 文件，", "2026年3月31日，安全研究员 Chaofan Shou (@Fried_rice) 发现 Anthropic 的 npm 包中暴露了 .map 文件，"),
        ("dot_tokens", "别把 .env、.npmrc、.gitignore 提交上去。", "别把 .env、.npmrc、.gitignore 提交上去。"),
        ("file_names", "请检查 bundle.min.js、package.json 和 processing_moss_tts.py。", "请检查 bundle.min.js、package.json 和 processing_moss_tts.py。"),
        ("index_d_ts", "index.d.ts 里也有同样的问题。", "index.d.ts 里也有同样的问题。"),
        ("version_build", "Bug 的讨论可以精确到 v2.3.1 (Build 15)。", "Bug 的讨论可以精确到 v2.3.1 (Build 15)。"),
        ("version_rc", "3.0.0-rc.1 还不能上生产。", "3.0.0-rc.1 还不能上生产。"),
        ("jar_name", "fabric-api-0.91.3+1.20.2.jar 需要单独下载。", "fabric-api-0.91.3+1.20.2.jar 需要单独下载。"),
        // 2) URL / Email / mention / hashtag / Reddit
        ("url", "仓库地址是 https://github.com/instructkr/claude-code", "仓库地址是 https://github.com/instructkr/claude-code。"),
        ("email", "联系邮箱：ops+tts@example.ai", "联系邮箱：ops+tts@example.ai。"),
        ("mention", "@Fried_rice 说这是 source map 暴露。", "@Fried_rice 说这是 source map 暴露。"),
        ("reddit", "去 r/singularity 看讨论。", "去 r/singularity 看讨论。"),
        ("hashtag_chain", "#张雪峰#张雪峰[话题]#张雪峰事件", "#张雪峰#张雪峰[话题]#张雪峰事件。"),
        ("mention_hashtag_boundary", "关注@biscuit0228_并转发#thetime_tbs", "关注 @biscuit0228_ 并转发 #thetime_tbs。"),
        // 3) brackets -> double quotes
        ("speaker_bracket", "[S1]你好。[S2]收到。", "\"S1\"你好。\"S2\"收到。"),
        ("event_bracket", "请模仿 {whisper} 的语气说\"别出声\"。", "请模仿 \"whisper\" 的语气说\"别出声\"。"),
        ("order_bracket", "订单号：[AB-1234-XYZ]", "订单号：\"AB-1234-XYZ\"。"),
        // 4) structural symbols
        ("struct_headline", "〖重磅〗《新品发布》——现在开始！", "\"重磅\"《新品发布》。现在开始！"),
        ("struct_notice", "【公告】今天 20:00 维护——预计 30 分钟。", "\"公告\"今天20:00维护。预计30分钟。"),
        ("struct_quote_chain", "『特别提醒』「不要外传」", "\"特别提醒\"\"不要外传\"。"),
        ("struct_embedded_quote", "他说【重要通知】明天发布。", "他说\"重要通知\"明天发布。"),
        ("flow_arrow_chain", "请求接入 -> 身份与策略判定 -> 域服务处理", "请求接入，身份与策略判定，域服务处理。"),
        ("flow_arrow_no_space", "A->B", "A，B。"),
        ("flow_arrow_unicode", "配置中心→推理编排→运行时执行", "配置中心，推理编排，运行时执行。"),
        // 5) embedded titles: preserved
        ("embedded_title", "我喜欢《哈姆雷特》这本书。", "我喜欢《哈姆雷特》这本书。"),
        // 6) repeated punctuation / social noise
        ("noise_qe", "真的假的？？？！！！", "真的假的？！"),
        ("noise_ellipsis", "这个包把 app.js.map 也发上去了......太离谱了！！！", "这个包把 app.js.map 也发上去了。太离谱了！"),
        ("noise_ellipsis_cn", "【系统提示】请模仿{sad}低沉语气，说\"今天下雨了……\"", "\"系统提示\"请模仿\"sad\"低沉语气，说\"今天下雨了。\""),
        // 7) spacing rules
        ("english_spaces", "This   is   a   test.", "This is a test."),
        ("chinese_spaces", "这 是 一 段  含有多种空白的文本。", "这是一段含有多种空白的文本。"),
        ("mixed_spaces_1", "这是Anthropic的npm包", "这是 Anthropic 的 npm 包。"),
        ("mixed_spaces_2", "今天update到v2.3.1了", "今天 update 到 v2.3.1 了。"),
        ("mixed_spaces_3", "处理app.js.map文件", "处理 app.js.map 文件。"),
        ("underscore_plain_1", "foo_bar", "foo bar。"),
        ("underscore_plain_2", "中文_ABC", "中文 ABC。"),
        ("underscore_protected_mention", "关注@foo_bar", "关注 @foo_bar。"),
        // 8) Markdown / lists / newlines
        ("markdown_link", "详情见 [release note](https://github.com/example/release)", "详情见 release note https://github.com/example/release。"),
        ("markdown_heading", "# I made a free open source app to help with markdown files", "I made a free open source app to help with markdown files。"),
        ("list_lines", "- 修复 .map 泄露\n- 发布 v2.3.1", "修复 .map 泄露。发布 v2.3.1。"),
        ("numbered_lines", "1. 安装依赖\n2. 运行测试\n3. 发布 v2.3.1", "安装依赖。运行测试。发布 v2.3.1。"),
        ("newlines", "第一行\n第二行\n第三行", "第一行。第二行。第三行。"),
        // 9) terminal punctuation
        ("terminal_punct_plain", "今天发布", "今天发布。"),
        ("terminal_punct_quoted", "他说\"你好\"", "他说\"你好\"。"),
        ("terminal_punct_existing", "今天发布。", "今天发布。"),
        ("terminal_punct_newlines", "第一行\n第二行。", "第一行。第二行。"),
        ("terminal_punct_blank_lines", "第一行\n\n第二行", "第一行。第二行。"),
        // 10) zero-width chars / idempotence
        ("zero_width_url", "详见 https://x.com/\u{200b}Safety", "详见 https://x.com/Safety。"),
    ];

    #[test]
    fn all_python_test_cases_pass() {
        for (name, input, expected) in TEST_CASES {
            let actual = normalize_tts_text(input);
            assert_eq!(
                actual, *expected,
                "test case '{}' failed\n  input:    {:?}\n  expected: {:?}\n  actual:   {:?}",
                name, input, expected, actual
            );
        }
    }

    #[test]
    fn idempotence() {
        for (name, input, expected) in TEST_CASES {
            let first = normalize_tts_text(input);
            assert_eq!(&first, expected, "first pass failed for '{}'", name);
            let second = normalize_tts_text(&first);
            assert_eq!(
                second, first,
                "idempotence failed for '{}'\n  first:  {:?}\n  second: {:?}",
                name, first, second
            );
        }
    }
}
