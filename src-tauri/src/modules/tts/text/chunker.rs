//! Voice clone 文本切块（Phase TTS-B.1 三段式升级）。
//!
//! 长文本送 TTS 前必须按 token 预算切成 chunks，每 chunk 独立 voice clone 合成
//! 后再拼接。本模块完整移植 Python `OnnxTtsRuntime.split_voice_clone_text`
//! (`onnx_tts_runtime.py:384-440`) 三段式算法：
//!
//! 1. **预处理**：CJK 感知补齐句末标点 / 短英文加 padding；折叠多空格。
//! 2. **三段式划分**（按优先级递降）：
//!    a. 按"句末标点" `。！？.!?；;` split → 拿到句子粒度。
//!    b. 单句 token 数仍超预算 → 按"子句标点" `,，、；;：:` split → 子句粒度。
//!    c. 子句仍超预算 → token 预算二分查找，回溯找最近的 punctuation 边界（25 字
//!       范围内）。
//! 3. **CJK 感知拼接**：相邻 chunks 合并时 CJK 直接相连，非 CJK 用空格分隔。
//! 4. **回退保证**：若最终只切出 1 chunk → 返回原文（与 Python 行为一致）。
//!
//! 还提供 [`estimate_inter_chunk_pause_seconds`]：估算 chunk 之间的停顿，
//! 短 chunk（≤4 词）用 0.40s，长 chunk 用 0.24s。

#![allow(dead_code)]

use crate::modules::tts::error::TtsError;
use crate::modules::tts::text::tokenizer::TtsTokenizer;

/// 句末终止标点（中英日通用）。
pub const SENTENCE_END_PUNCTUATION: &[char] = &['。', '！', '？', '.', '!', '?', '；', ';'];

/// 子句切分标点（次级停顿）。
pub const CLAUSE_SPLIT_PUNCTUATION: &[char] = &[',', '，', '、', '；', ';', '：', ':'];

/// 闭合性标点（应跟在句末标点之后保持配对）。
pub const CLOSING_PUNCTUATION: &[char] = &[
    '"', '\'', '”', '’', ')', ']', '}', '）', '】', '》', '」', '』',
];

/// chunk 间默认停顿：短 chunk 用长停顿（语句感）。
pub const PAUSE_SHORT_SECONDS: f32 = 0.40;
/// chunk 间默认停顿：长 chunk 用短停顿（连贯感）。
pub const PAUSE_LONG_SECONDS: f32 = 0.24;

/// 旧版入口（保留兼容签名）。新代码请用 [`split_voice_clone_text`]。
pub fn split_text_into_chunks(
    tokenizer: &TtsTokenizer,
    text: &str,
    max_tokens: usize,
) -> Result<Vec<String>, TtsError> {
    split_voice_clone_text(tokenizer, text, max_tokens)
}

/// CJK / 中文 / 日文 / 韩文字符判定。
pub fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk_char)
}

fn is_cjk_char(c: char) -> bool {
    matches!(c,
        '\u{4e00}'..='\u{9fff}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4dbf}' // CJK Extension A
        | '\u{3040}'..='\u{30ff}' // Hiragana + Katakana
        | '\u{ac00}'..='\u{d7af}' // Hangul Syllables
    )
}

/// 估算 chunk 之间的停顿秒数（按词数判断）。
///
/// 镜像 Python `estimate_voice_clone_inter_chunk_pause_seconds`
/// (`onnx_tts_runtime.py:434-440`)。
pub fn estimate_inter_chunk_pause_seconds(chunk_text: &str) -> f32 {
    let word_count = chunk_text.split_whitespace().count();
    if word_count <= 4 {
        PAUSE_SHORT_SECONDS
    } else {
        PAUSE_LONG_SECONDS
    }
}

/// 主入口：voice clone 三段式切块。
///
/// 镜像 Python `split_voice_clone_text` (`onnx_tts_runtime.py:384-432`)。
pub fn split_voice_clone_text(
    tokenizer: &TtsTokenizer,
    text: &str,
    max_tokens: usize,
) -> Result<Vec<String>, TtsError> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let safe_max = max_tokens.max(1);
    let prepared = prepare_for_chunking(normalized);

    // Layer 1: 句末标点切分
    let sentence_candidates = split_by_punctuation(&prepared, SENTENCE_END_PUNCTUATION);
    let sentence_candidates = if sentence_candidates.is_empty() {
        vec![prepared.trim().to_string()]
    } else {
        sentence_candidates
    };

    // 把每个句子按 max_tokens 进一步细化
    let mut sentence_slices: Vec<(usize, String)> = Vec::new();
    for sentence in sentence_candidates {
        let s = sentence.trim();
        if s.is_empty() {
            continue;
        }
        let s_tokens = tokenizer.count_tokens(s)?;
        if s_tokens <= safe_max {
            sentence_slices.push((s_tokens, s.to_string()));
            continue;
        }

        // Layer 2: 句子超预算 → 子句切分
        let clause_candidates = {
            let cand = split_by_punctuation(s, CLAUSE_SPLIT_PUNCTUATION);
            if cand.len() <= 1 {
                vec![s.to_string()]
            } else {
                cand
            }
        };
        for clause in clause_candidates {
            let c = clause.trim();
            if c.is_empty() {
                continue;
            }
            let c_tokens = tokenizer.count_tokens(c)?;
            if c_tokens <= safe_max {
                sentence_slices.push((c_tokens, c.to_string()));
                continue;
            }
            // Layer 3: 子句仍超预算 → token 二分 + 边界回溯
            for piece in split_text_by_token_budget(tokenizer, c, safe_max)? {
                let p = piece.trim();
                if !p.is_empty() {
                    sentence_slices.push((tokenizer.count_tokens(p)?, p.to_string()));
                }
            }
        }
    }

    // 贪心拼接：相邻 sentence_slices 在不超预算时合并，CJK 相连，非 CJK 加空格
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_tokens: usize = 0;

    for (tokens, sentence) in sentence_slices {
        if current.is_empty() {
            current = sentence;
            current_tokens = tokens;
            continue;
        }
        if current_tokens + tokens > safe_max {
            chunks.push(std::mem::take(&mut current).trim().to_string());
            current = sentence;
            current_tokens = tokens;
        } else {
            current = join_cjk_aware(&current, &sentence);
            current_tokens = tokenizer.count_tokens(&current)?;
        }
    }
    if !current.is_empty() {
        chunks.push(current.trim().to_string());
    }

    // 回退保证：单 chunk 时返回原文（避免对短文本不必要切分）
    if chunks.len() <= 1 {
        return Ok(vec![normalized.to_string()]);
    }
    Ok(chunks)
}

/// CJK 感知拼接：两侧任一含 CJK → 直接相连；否则加空格。
fn join_cjk_aware(left: &str, right: &str) -> String {
    if left.is_empty() {
        return right.to_string();
    }
    if right.is_empty() {
        return left.to_string();
    }
    if contains_cjk(left) || contains_cjk(right) {
        format!("{left}{right}")
    } else {
        format!("{left} {right}")
    }
}

/// 文本预处理：折叠空格 + CJK 句末补齐 + 短英文 padding。
///
/// 镜像 Python `_prepare_text_for_sentence_chunking` (`onnx_tts_runtime.py:187-204`)。
fn prepare_for_chunking(text: &str) -> String {
    let mut s = text.replace(['\r', '\n'], " ");
    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
    let s = s.trim().to_string();
    if s.is_empty() {
        return s;
    }

    if contains_cjk(&s) {
        // CJK 末尾若不是句末标点 → 补 "。"
        let last = s.chars().last().unwrap();
        if !SENTENCE_END_PUNCTUATION.contains(&last) {
            return format!("{s}。");
        }
        return s;
    }

    // 非 CJK：首字母大写 + 末尾补 "."
    let mut chars: Vec<char> = s.chars().collect();
    if let Some(first) = chars.first_mut() {
        if first.is_lowercase() {
            *first = first.to_ascii_uppercase();
        }
    }
    let mut s2: String = chars.into_iter().collect();
    if let Some(last) = s2.chars().last() {
        if last.is_alphanumeric() {
            s2.push('.');
        }
    }
    // 短英文（< 5 词）→ 加 padding 让模型有 lead-in 空间
    let word_count = s2.split_whitespace().count();
    if word_count < 5 {
        s2 = format!("        {s2}");
    }
    s2
}

/// 按指定标点集 split 字符串，标点保留在前一段末尾，且任何紧跟的闭合标点
/// 一并归到该段。
fn split_by_punctuation(text: &str, punctuation: &[char]) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut sentences: Vec<String> = Vec::new();
    let mut buf: Vec<char> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        buf.push(c);
        if punctuation.contains(&c) {
            // lookahead 把所有闭合标点跟上
            let mut j = i + 1;
            while j < chars.len() && CLOSING_PUNCTUATION.contains(&chars[j]) {
                buf.push(chars[j]);
                j += 1;
            }
            let segment: String = buf.iter().collect();
            let trimmed = segment.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed.to_string());
            }
            buf.clear();
            // skip whitespace 到下一段
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    if !buf.is_empty() {
        let tail: String = buf.iter().collect();
        let t = tail.trim();
        if !t.is_empty() {
            sentences.push(t.to_string());
        }
    }
    sentences
}

/// 当一段文本即使切到子句仍超预算时，按 token 预算二分查找最大前缀；找到后
/// 优先回溯到最近 25 字内的 punctuation/space 边界，避免硬切单词。
///
/// 镜像 Python `split_text_by_token_budget` (`onnx_tts_runtime.py:342-382`)。
fn split_text_by_token_budget(
    tokenizer: &TtsTokenizer,
    text: &str,
    max_tokens: usize,
) -> Result<Vec<String>, TtsError> {
    let preferred_boundary: Vec<char> = SENTENCE_END_PUNCTUATION
        .iter()
        .chain(CLAUSE_SPLIT_PUNCTUATION.iter())
        .copied()
        .chain(std::iter::once(' '))
        .collect();

    let mut remaining: String = text.trim().to_string();
    let mut pieces: Vec<String> = Vec::new();
    while !remaining.is_empty() {
        if tokenizer.count_tokens(&remaining)? <= max_tokens {
            pieces.push(remaining);
            break;
        }
        // 二分找最大前缀长度（按 char 计数，因为 token 跨多字节）
        let chars: Vec<char> = remaining.chars().collect();
        let mut low = 1usize;
        let mut high = chars.len();
        let mut best = 1usize;
        while low <= high {
            let mid = (low + high) / 2;
            let candidate: String = chars[..mid].iter().collect();
            let c = candidate.trim();
            if c.is_empty() {
                low = mid + 1;
                continue;
            }
            if tokenizer.count_tokens(c)? <= max_tokens {
                best = mid;
                low = mid + 1;
            } else {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            }
        }
        // 回溯：在 best 前 25 字范围内找最近的 boundary 字符
        let mut cut = best;
        let scan_start = best.saturating_sub(25);
        for idx in (scan_start..best).rev() {
            if preferred_boundary.contains(&chars[idx]) {
                cut = idx + 1;
                break;
            }
        }
        if cut == 0 {
            cut = best;
        }
        let piece: String = chars[..cut].iter().collect();
        let p = piece.trim();
        if p.is_empty() {
            // 退化：硬切 best 长度避免死循环
            let fallback: String = chars[..best].iter().collect();
            pieces.push(fallback.trim().to_string());
            remaining = chars[best..].iter().collect::<String>().trim().to_string();
        } else {
            pieces.push(p.to_string());
            remaining = chars[cut..].iter().collect::<String>().trim().to_string();
        }
    }
    Ok(pieces)
}

// `split_by_token_budget` 暴露给单测用；非 pub 即可。

#[cfg(test)]
mod tests {
    use super::*;

    fn home_tokenizer() -> Option<TtsTokenizer> {
        let p = std::path::PathBuf::from(format!(
            "{}/.if2ai/models/tts/MOSS-TTS-Nano-100M-ONNX/tokenizer.model",
            std::env::var("HOME").unwrap_or_default()
        ));
        if !p.exists() {
            return None;
        }
        TtsTokenizer::load(&p).ok()
    }

    #[test]
    fn contains_cjk_basics() {
        assert!(contains_cjk("你好"));
        assert!(contains_cjk("こんにちは"));
        assert!(contains_cjk("안녕"));
        assert!(!contains_cjk("Hello world"));
    }

    #[test]
    fn join_cjk_aware_glues_chinese() {
        assert_eq!(join_cjk_aware("你好。", "世界。"), "你好。世界。");
        assert_eq!(join_cjk_aware("Hi.", "There."), "Hi. There.");
        assert_eq!(join_cjk_aware("你好。", "Hi."), "你好。Hi.");
    }

    #[test]
    fn split_by_punctuation_preserves_terminators() {
        let s = split_by_punctuation("Hello, world. How are you?", SENTENCE_END_PUNCTUATION);
        assert_eq!(s, vec!["Hello, world.", "How are you?"]);
    }

    #[test]
    fn split_by_punctuation_carries_closing_quotes() {
        let s = split_by_punctuation("She said \"hi.\" Then left.", SENTENCE_END_PUNCTUATION);
        assert_eq!(s, vec!["She said \"hi.\"", "Then left."]);
    }

    #[test]
    fn prepare_appends_zh_terminator_when_missing() {
        let p = prepare_for_chunking("你好世界");
        assert_eq!(p, "你好世界。");
    }

    #[test]
    fn prepare_keeps_existing_zh_terminator() {
        let p = prepare_for_chunking("你好世界。");
        assert_eq!(p, "你好世界。");
    }

    #[test]
    fn prepare_pads_short_english() {
        let p = prepare_for_chunking("hi");
        assert!(p.starts_with("        "));
        assert!(p.ends_with('.'));
    }

    #[test]
    fn prepare_normalizes_whitespace() {
        assert_eq!(prepare_for_chunking("a   b  c"), "        A b c.");
    }

    #[test]
    fn estimate_pause_short_vs_long() {
        assert!(
            (estimate_inter_chunk_pause_seconds("hello world") - PAUSE_SHORT_SECONDS).abs() < 1e-6
        );
        assert!(
            (estimate_inter_chunk_pause_seconds("one two three four five six")
                - PAUSE_LONG_SECONDS)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn split_short_text_returns_original_only() {
        let Some(tk) = home_tokenizer() else {
            eprintln!("[skip] tokenizer not available");
            return;
        };
        let chunks = split_voice_clone_text(&tk, "你好世界。", 75).unwrap();
        assert_eq!(chunks, vec!["你好世界。"]);
    }

    #[test]
    fn split_long_text_breaks_into_multiple_chunks() {
        let Some(tk) = home_tokenizer() else {
            eprintln!("[skip] tokenizer not available");
            return;
        };
        // 构造 ~6 句长文本
        let long = "人工智能正在改变世界。\
                    它影响着我们的生活。\
                    它改变了我们的工作方式。\
                    机器学习技术日新月异。\
                    深度学习模型越来越强大。\
                    自然语言处理也取得了突破。"
            .repeat(3);
        let chunks = split_voice_clone_text(&tk, &long, 30).unwrap();
        assert!(
            chunks.len() > 1,
            "expected multiple chunks, got {}",
            chunks.len()
        );
        for c in &chunks {
            let n = tk.count_tokens(c).unwrap();
            assert!(n <= 30 * 2, "chunk too long: {n} tokens, text={c:?}");
        }
    }
}
