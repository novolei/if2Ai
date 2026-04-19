//! Text chunker for long text splitting.
//!
//! Splits normalized text into chunks that respect a token budget,
//! suitable for voice clone mode where the model has a limited
//! context window.
//!
//! Mirrors `model._split_text_into_best_sentences()` from the Python
//! reference, which splits text at natural sentence boundaries and
//! groups sentences up to `max_tokens`.

// Justification: chunker functions are consumed by TTS-4.x voice clone
// text splitting and long text continuation mode slices.
#![allow(dead_code)]

/// Default sentence boundary punctuation marks.
///
/// Includes Chinese (。！？；), Japanese (。！？；), and Western (.!?) sentence terminators.
const SENTENCE_BOUNDARIES: &[char] = &['.', '!', '?', '。', '！', '？', '；', ';'];

/// Split text into chunks, each not exceeding the token budget.
///
/// The algorithm:
/// 1. Split text at sentence boundaries (。！？.!?)
/// 2. Group sentences greedily until adding the next sentence
///    would exceed `max_tokens`
/// 3. Each chunk is a self-contained unit of text
///
/// Mirrors `_split_text_into_best_sentences()` from the Python reference.
pub fn split_text_into_chunks(
    tokenizer: &crate::modules::tts::text::tokenizer::TtsTokenizer,
    text: &str,
    max_tokens: usize,
) -> Result<Vec<String>, crate::modules::tts::error::TtsError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    // Split into sentences at boundary punctuation
    let sentences = split_into_sentences(text);

    // Greedily group sentences up to max_tokens
    let mut chunks: Vec<String> = Vec::new();
    let mut current_chunk = String::new();
    let mut current_token_count: usize = 0;

    for sentence in sentences {
        let sentence_tokens = tokenizer.count_tokens(&sentence)?;

        // If a single sentence exceeds max_tokens, force it into its own chunk
        if sentence_tokens > max_tokens {
            // Push whatever we have accumulated
            if !current_chunk.is_empty() {
                chunks.push(std::mem::take(&mut current_chunk));
                current_token_count = 0;
            }
            // Force the long sentence as its own chunk
            chunks.push(sentence);
            continue;
        }

        // If adding this sentence would exceed the budget, start a new chunk
        if current_token_count + sentence_tokens > max_tokens && !current_chunk.is_empty() {
            chunks.push(std::mem::take(&mut current_chunk));
            current_token_count = 0;
        }

        if current_chunk.is_empty() {
            current_chunk = sentence;
            current_token_count = sentence_tokens;
        } else {
            current_chunk.push_str(&sentence);
            current_token_count += sentence_tokens;
        }
    }

    // Don't forget the last chunk
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    Ok(chunks)
}

/// Split text into sentences at boundary punctuation marks.
///
/// Preserves the boundary punctuation attached to each sentence.
/// Empty segments between consecutive boundaries are skipped.
fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = text.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if SENTENCE_BOUNDARIES.contains(&ch) {
            let end = i + 1;
            let sentence: String = chars[start..end].iter().collect();
            let trimmed = sentence.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed.to_string());
            }
            start = end;
        }
    }

    // Remaining text after the last boundary
    if start < chars.len() {
        let remainder: String = chars[start..].iter().collect();
        let trimmed = remainder.trim();
        if !trimmed.is_empty() {
            sentences.push(trimmed.to_string());
        }
    }

    if sentences.is_empty() && !text.trim().is_empty() {
        sentences.push(text.trim().to_string());
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_english_sentences() {
        let text = "Hello world. How are you? I'm fine!";
        let sentences = split_into_sentences(text);
        assert_eq!(sentences, vec!["Hello world.", "How are you?", "I'm fine!"]);
    }

    #[test]
    fn split_chinese_sentences() {
        let text = "你好，世界。欢迎使用。";
        let sentences = split_into_sentences(text);
        assert_eq!(sentences, vec!["你好，世界。", "欢迎使用。"]);
    }

    #[test]
    fn split_mixed_sentences() {
        let text = "你好。Hello world. 再见！";
        let sentences = split_into_sentences(text);
        assert_eq!(sentences, vec!["你好。", "Hello world.", "再见！"]);
    }

    #[test]
    fn split_no_boundaries() {
        let text = "hello world";
        let sentences = split_into_sentences(text);
        assert_eq!(sentences, vec!["hello world"]);
    }

    #[test]
    fn split_empty_text() {
        let sentences = split_into_sentences("");
        assert!(sentences.is_empty());
    }

    #[test]
    fn split_consecutive_boundaries() {
        let text = "Hi!!. Really?";
        let sentences = split_into_sentences(text);
        // Each boundary creates a separate sentence: "Hi!" then "!" then "." then "Really?"
        assert_eq!(sentences, vec!["Hi!", "!", ".", "Really?"]);
    }

    #[test]
    fn sentence_boundaries_constant() {
        assert!(SENTENCE_BOUNDARIES.contains(&'.'));
        assert!(SENTENCE_BOUNDARIES.contains(&'。'));
        assert!(SENTENCE_BOUNDARIES.contains(&'!'));
        assert!(SENTENCE_BOUNDARIES.contains(&'！'));
    }
}
