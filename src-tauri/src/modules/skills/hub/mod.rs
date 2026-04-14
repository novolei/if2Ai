#![allow(unused)]

//! Skills Hub — multi-source skill marketplace adapters.
//!
//! Ported from Hermes `tools/skills_hub.py`.
//!
//! Provides:
//! - SkillSource trait for implementing hub adapters
//! - GitHubSource adapter for GitHub-hosted skills
//! - HubError and result types

pub mod github;
pub mod source;
pub mod types;

pub use github::{GitHubAuth, GitHubSource, DEFAULT_TAPS};
pub use source::{BoxedSkillSource, SkillSource};
pub use types::{HubError, HubResult, SkillBundle, SkillMeta};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_taps() {
        assert_eq!(DEFAULT_TAPS.len(), 4);
        assert!(DEFAULT_TAPS.contains(&"openai/skills"));
        assert!(DEFAULT_TAPS.contains(&"anthropics/skills"));
    }
}
