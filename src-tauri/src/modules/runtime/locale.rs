// 8A.3 lays the foundation for 8B+ memory-prompt builders; consumers
// (rolling_summary_prompt, compile_today_prompt, diary_writer) ship in
// subsequent slices.
#![allow(dead_code)]

//! Locale helpers — single source of truth for the agent's UI / output
//! language.  Used by all memory-prompt builders (rolling summary,
//! compile_today / compile_week / compile_longterm / facts, diary writer)
//! so we do not fan out language detection across modules.
//!
//! See `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §0.5 Δ-13.
//!
//! This module deliberately depends on **nothing outside `std`** so that
//! every memory module — including ones that cannot afford the
//! `chrono` / `chrono-tz` dependency footprint — can call [`is_zh`] freely.

/// Default UI language tag used until [`crate::modules::runtime::config`]
/// gains a configurable `language` field.  Keeping this `"en-US"`
/// preserves the project's existing English-only default behaviour.
const DEFAULT_LANGUAGE: &str = "en-US";

/// Returns `true` when the active runtime UI language starts with `"zh"`.
///
/// TODO(8A.x): once `RuntimeConfig` exposes a `language: String` field,
/// route this through `config::current().language` instead of the
/// hard-coded [`DEFAULT_LANGUAGE`].  Until then this returns `false`,
/// which matches the project's shipping English UI.
#[must_use]
pub fn is_zh() -> bool {
    is_zh_for(DEFAULT_LANGUAGE)
}

/// Pure helper — returns `true` when `language` (a BCP-47 tag like
/// `"zh-CN"`, `"zh-Hant-TW"`, `"en-US"`) starts with the `"zh"` prefix.
///
/// Kept separate from [`is_zh`] so unit tests can pin the language→
/// behaviour mapping without touching the global config.
#[must_use]
pub fn is_zh_for(language: &str) -> bool {
    language.starts_with("zh")
}

#[cfg(test)]
mod tests {
    use super::{is_zh, is_zh_for, DEFAULT_LANGUAGE};

    #[test]
    fn is_zh_for_zh_cn_returns_true() {
        assert!(is_zh_for("zh-CN"));
    }

    #[test]
    fn is_zh_for_zh_hant_returns_true() {
        assert!(is_zh_for("zh-Hant-TW"));
    }

    #[test]
    fn is_zh_for_en_us_returns_false() {
        assert!(!is_zh_for("en-US"));
    }

    #[test]
    fn is_zh_for_empty_string_returns_false() {
        assert!(!is_zh_for(""));
    }

    #[test]
    fn is_zh_uses_default_language() {
        assert_eq!(is_zh(), is_zh_for(DEFAULT_LANGUAGE));
    }
}
