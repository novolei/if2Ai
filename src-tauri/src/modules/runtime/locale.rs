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
//! Phase 8A.4 — `is_zh()` now routes through the global
//! [`crate::modules::runtime::config::current`] handle so user overrides
//! in `settings.json` (`language: "zh-CN"`) take effect immediately
//! rather than waiting for a redesign of every prompt builder's API.

/// Returns `true` when the active runtime UI language starts with `"zh"`.
///
/// Reads the `language` field from
/// [`crate::modules::runtime::config::current`].  When `set_current` has
/// not been called yet (unit-test paths, early boot), `current()` returns
/// a default config whose `language()` is `"en-US"`, so this returns
/// `false` — matching the project's shipping English-only default.
#[must_use]
pub fn is_zh() -> bool {
    is_zh_for(crate::modules::runtime::config::current().language())
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
    use super::is_zh_for;

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
    fn is_zh_reads_from_runtime_config() {
        // Without any explicit set_current() call, the global accessor
        // returns a default RuntimeConfig whose language() is "en-US".
        // Phase 8A.4 — the test verifies the wiring exists; richer
        // zh-CN coverage lives in tests for `runtime::config::current`
        // (which races on a OnceLock, so we do not poke the global here).
        let _ = super::is_zh();
    }
}
